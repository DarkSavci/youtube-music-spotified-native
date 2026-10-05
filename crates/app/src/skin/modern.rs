//! Modern Winamp skins (`.wal`, for Winamp 3 and 5): as much of one as can
//! be drawn without running it.
//!
//! A modern skin is two things. Its XML lays out pictures, buttons,
//! sliders and text, and names what each control does; its scripts,
//! compiled to a bytecode of Winamp's own, move those about: drawers that
//! slide out, lights that blink, windows that change shape. Only the
//! first is read here. The main window's normal layout is flattened into
//! a list of things to draw, each where the XML puts it, and the standard
//! actions (play, seek, volume and the rest) are wired to the player.
//! What a script would have shown, hidden or moved stays as the XML first
//! has it, so a simple skin is whole and an elaborate one is its resting
//! state.

pub mod xml;

use std::collections::HashMap;

use super::zip::Archive;
use super::{Bitmap, SkinError};
use xml::Node;

/// How deep groups may nest before a skin is taken to include itself.
const MAX_DEPTH: usize = 24;
/// The most XML files one skin may pull in, for the same reason.
const MAX_FILES: usize = 400;
/// A window larger than this is not a player's.
const MAX_SIDE: i32 = 2400;
/// A pixel fainter than this is not part of the window's shape.
const SOLID: u8 = 48;

/// Part of one of the skin's pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image {
    /// Which of [`Modern::sheets`] it is cut from.
    pub sheet: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// What pressing a button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    /// Winamp opened a file; here the main window comes forward.
    Eject,
    Close,
    Minimize,
    /// The options: the skins, and the ways out.
    Menu,
    Shuffle,
    Repeat,
}

/// What dragging a slider changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slide {
    Seek,
    Volume,
    Balance,
    /// One band of the equalizer, counted from nought.
    Band(usize),
    Preamp,
}

/// What a line of text says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Words {
    /// The artists and the title.
    Song,
    Title,
    Artist,
    /// How far the song has got.
    Time,
    Length,
    /// The bitrate and the sample rate, as Winamp wrote them together.
    Info,
    Bitrate,
    SampleRate,
    /// Whatever the skin wrote there.
    Fixed(String),
}

/// A font made of a picture: a grid of characters in Winamp's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapFont {
    pub sheet: usize,
    pub char_width: u32,
    pub char_height: u32,
    /// Added to the width between one character and the next.
    pub spacing: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// A picture, stretched to its place if that is another size.
    Layer(Image),
    Button {
        normal: Option<Image>,
        hover: Option<Image>,
        down: Option<Image>,
        /// Shown while what the button switches is on.
        active: Option<Image>,
        act: Act,
        /// What the skin calls it, for a tooltip.
        hint: String,
    },
    Slider {
        slide: Slide,
        thumb: Option<Image>,
        down_thumb: Option<Image>,
        /// The track: its two ends, and the middle stretched between.
        bars: [Option<Image>; 3],
        vertical: bool,
    },
    Text {
        words: Words,
        font: Option<BitmapFont>,
        color: [u8; 3],
        /// The type size, for text set in the app's own face.
        size: f32,
        /// -1 for the left, 0 for the middle, 1 for the right.
        align: i8,
    },
    /// The spectrum analyser, in these colours from bottom to top.
    Vis { colors: Vec<[u8; 3]>, peak: [u8; 3] },
}

/// One thing to draw: where, in the window's own pixels, and what.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub kind: Kind,
}

/// A modern skin's main window, ready to draw.
#[derive(Debug)]
pub struct Modern {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// The pictures the items are cut from.
    pub sheets: Vec<Bitmap>,
    /// What to draw, bottom first.
    pub items: Vec<Item>,
    /// The window's shape, when it is not its whole rectangle: the runs of
    /// each row that show, as left, top, right and bottom.
    pub shape: Option<Vec<[u32; 4]>>,
}

/// A place in a parent: where from, and how big.
#[derive(Debug, Clone, Copy)]
struct Place {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

struct BitmapDef {
    file: String,
    /// The part of the file, when it is not all of it.
    part: [Option<i32>; 4],
}

struct FontDef {
    file: String,
    char_width: u32,
    char_height: u32,
    spacing: i32,
}

/// Everything a skin defines, by name, with the files it is read from.
struct Reader<'a> {
    archive: Archive<'a>,
    /// The archive's entries by their paths, in lower case with forward
    /// slashes.
    files: HashMap<String, usize>,
    bitmaps: HashMap<String, BitmapDef>,
    fonts: HashMap<String, FontDef>,
    colors: HashMap<String, [u8; 3]>,
    groups: HashMap<String, Node>,
    /// The groups that stand behind a tag of their own, by that tag.
    tags: HashMap<String, String>,
    containers: Vec<Node>,
    files_read: usize,
    sheets: Vec<Bitmap>,
    sheet_of: HashMap<String, Option<usize>>,
    items: Vec<Item>,
}

fn normal(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}

fn rgb(value: &str) -> Option<[u8; 3]> {
    let mut parts = value.split(',').map(|part| part.trim().parse::<u8>());
    let color = [
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ];
    Some(color)
}

impl<'a> Reader<'a> {
    fn new(archive: Archive<'a>) -> Self {
        let files = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| !entry.is_dir() && !entry.name.starts_with("__MACOSX"))
            .map(|(index, entry)| (normal(&entry.name), index))
            .collect();
        Self {
            archive,
            files,
            bitmaps: HashMap::new(),
            fonts: HashMap::new(),
            colors: HashMap::new(),
            groups: HashMap::new(),
            tags: HashMap::new(),
            containers: Vec::new(),
            files_read: 0,
            sheets: Vec::new(),
            sheet_of: HashMap::new(),
            items: Vec::new(),
        }
    }

    /// Finds a file a skin names: from the skin's root, from the folder of
    /// the file that names it, or, since skins were moved about and their
    /// paths not always with them, by its name alone.
    fn find(&self, path: &str, beside: &str) -> Option<usize> {
        let path = normal(path);
        if let Some(index) = self.files.get(&path) {
            return Some(*index);
        }
        let folder = beside.rsplit_once('/').map_or("", |(folder, _)| folder);
        let mut parts: Vec<&str> = folder.split('/').filter(|part| !part.is_empty()).collect();
        for part in path.split('/') {
            match part {
                ".." => {
                    parts.pop();
                }
                "." | "" => {}
                part => parts.push(part),
            }
        }
        if let Some(index) = self.files.get(&parts.join("/")) {
            return Some(*index);
        }
        let name = path.rsplit('/').next()?;
        let mut named = self
            .files
            .iter()
            .filter(|(file, _)| file.rsplit('/').next() == Some(name));
        let found = named.next().map(|(_, index)| *index);
        // Two files of that name: there is no telling which was meant.
        found.filter(|_| named.next().is_none())
    }

    fn read_text(&self, index: usize) -> Option<String> {
        let bytes = self.archive.read(&self.archive.entries()[index]).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        Some(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
    }

    /// Reads an XML file with everything it includes put where the
    /// `include` stood, and every file it names given the path the
    /// archive has it under: a name is relative to the file that says it.
    fn expand(&mut self, path: &str) -> Vec<Node> {
        if self.files_read >= MAX_FILES {
            return Vec::new();
        }
        self.files_read += 1;
        let text = self
            .files
            .get(path)
            .and_then(|index| self.read_text(*index));
        let nodes = text.map(|text| xml::parse(&text)).unwrap_or_default();
        self.expand_nodes(nodes, path)
    }

    fn expand_nodes(&mut self, nodes: Vec<Node>, path: &str) -> Vec<Node> {
        let mut expanded = Vec::with_capacity(nodes.len());
        for mut node in nodes {
            let file = node.attributes.iter().position(|(key, _)| key == "file");
            if node.name == "include" {
                let included = file
                    .and_then(|at| self.find(&node.attributes[at].1, path))
                    .map(|index| normal(&self.archive.entries()[index].name));
                if let Some(included) = included {
                    expanded.extend(self.expand(&included));
                }
                continue;
            }
            if let Some(at) = file {
                node.attributes[at].1 = self.resolve(&node.attributes[at].1, path);
            }
            node.children = self.expand_nodes(std::mem::take(&mut node.children), path);
            expanded.push(node);
        }
        expanded
    }

    /// Notes what a skin defines: its pictures, fonts and colours, its
    /// groups, and its windows.
    fn define(&mut self, nodes: &[Node]) {
        for node in nodes {
            let id = node.get("id").map(str::to_ascii_lowercase);
            match (node.name.as_str(), id) {
                ("bitmap", Some(id)) => {
                    if let Some(file) = node.get("file") {
                        let part = ["x", "y", "w", "h"].map(|side| node.number(side));
                        let file = file.to_owned();
                        self.bitmaps.insert(id, BitmapDef { file, part });
                    }
                }
                ("bitmapfont", Some(id)) => {
                    let side = |name: &str| node.number(name).unwrap_or(0).max(0) as u32;
                    if let Some(file) = node.get("file") {
                        let font = FontDef {
                            file: file.to_owned(),
                            char_width: side("charwidth"),
                            char_height: side("charheight"),
                            spacing: node.number("hspacing").unwrap_or(0),
                        };
                        self.fonts.insert(id, font);
                    }
                }
                ("color", Some(id)) => {
                    if let Some(color) = node.get("value").and_then(rgb) {
                        self.colors.insert(id, color);
                    }
                }
                ("groupdef", Some(id)) => {
                    if let Some(tag) = node.get("xuitag") {
                        self.tags.insert(tag.to_ascii_lowercase(), id.clone());
                    }
                    // A group may define more inside itself.
                    self.define(&node.children);
                    self.groups.insert(id, node.clone());
                }
                ("container", _) => {
                    self.define(&node.children);
                    self.containers.push(node.clone());
                }
                // What only wraps other things: the root, `elements`, a
                // layout, and anything else that is not a control.
                _ if !is_control(&node.name) => self.define(&node.children),
                _ => {}
            }
        }
    }

    /// A file's path as the archive has it, or as written if it is not
    /// there to be found.
    fn resolve(&self, file: &str, beside: &str) -> String {
        match self.find(file, beside) {
            Some(index) => normal(&self.archive.entries()[index].name),
            None => normal(file),
        }
    }

    /// The picture in a file, decoded the first time it is asked for.
    fn sheet(&mut self, file: &str) -> Option<usize> {
        if let Some(known) = self.sheet_of.get(file) {
            return *known;
        }
        let decoded = self
            .files
            .get(file)
            .and_then(|index| self.archive.read(&self.archive.entries()[*index]).ok())
            .and_then(|bytes| Bitmap::decode(&bytes));
        let index = decoded.map(|bitmap| {
            self.sheets.push(bitmap);
            self.sheets.len() - 1
        });
        self.sheet_of.insert(file.to_owned(), index);
        index
    }

    /// A named picture, cut to the part of its file the skin gave it and
    /// to what the file holds.
    fn image(&mut self, id: Option<&str>) -> Option<Image> {
        let def = self.bitmaps.get(&id?.to_ascii_lowercase())?;
        let (file, part) = (def.file.clone(), def.part);
        let sheet = self.sheet(&file)?;
        let (wide, tall) = (self.sheets[sheet].width, self.sheets[sheet].height);
        let x = (part[0].unwrap_or(0).max(0) as u32).min(wide);
        let y = (part[1].unwrap_or(0).max(0) as u32).min(tall);
        let width = part[2].map_or(wide, |width| width.max(0) as u32);
        let height = part[3].map_or(tall, |height| height.max(0) as u32);
        let (width, height) = (width.min(wide - x), height.min(tall - y));
        (width > 0 && height > 0).then_some(Image {
            sheet,
            x,
            y,
            width,
            height,
        })
    }

    fn font(&mut self, id: Option<&str>) -> Option<BitmapFont> {
        let def = self.fonts.get(&id?.to_ascii_lowercase())?;
        let (file, char_width, char_height, spacing) = (
            def.file.clone(),
            def.char_width,
            def.char_height,
            def.spacing,
        );
        let sheet = self.sheet(&file)?;
        (char_width > 0 && char_height > 0).then_some(BitmapFont {
            sheet,
            char_width,
            char_height,
            spacing,
        })
    }

    fn color(&self, value: Option<&str>) -> Option<[u8; 3]> {
        let value = value?;
        rgb(value).or_else(|| self.colors.get(&value.to_ascii_lowercase()).copied())
    }

    /// Flattens a layout's or a group's children into things to draw.
    fn lay_out(&mut self, nodes: &[Node], parent: Place, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        for node in nodes {
            // Hidden until a script shows it, which nothing here will; and
            // the twin of a control that shows only in an idle window.
            let hidden = node.is_off("visible") || node.is_off("alpha");
            if hidden || node.is_off("activealpha") {
                continue;
            }
            match node.name.as_str() {
                "group" => self.group(node, node.get("id"), parent, depth),
                "layer" | "animatedlayer" => self.layer(node, parent),
                "button" | "togglebutton" | "nstatesbutton" | "fadebutton" | "fadetogglebutton" => {
                    self.button(node, parent)
                }
                "slider" => self.slider(node, parent),
                "text" => self.text(node, parent),
                "vis" => self.vis(node, parent),
                name => {
                    // A tag of the skin's own making stands for a group.
                    let Some(group) = self.tags.get(name).cloned() else {
                        continue;
                    };
                    self.group(node, Some(&group), parent, depth);
                    // The frame around a window names what goes in it;
                    // a script would have put it there.
                    if let Some(content) = node.get("content") {
                        let whole = self.place(node, None, parent, None);
                        self.group_in(content, whole, depth);
                    }
                }
            }
        }
    }

    /// Where an element goes in its parent. A side it does not give is
    /// taken from `fallback` (its group's definition), then from the
    /// picture's own size, then from the parent.
    fn place(
        &self,
        node: &Node,
        fallback: Option<&Node>,
        parent: Place,
        natural: Option<(u32, u32)>,
    ) -> Place {
        let read = |side: &str| {
            node.number(side)
                .or_else(|| fallback.and_then(|other| other.number(side)))
        };
        let how = |relative: &str| {
            node.get(relative)
                .or_else(|| fallback.and_then(|other| other.get(relative)))
                .unwrap_or("0")
                .to_owned()
        };
        let along = |given: Option<i32>, how: &str, room: i32, least: i32| match (given, how) {
            (Some(given), "1") => room + given,
            (Some(given), "%") => room * given / 100,
            (Some(given), _) => given,
            (None, _) => least,
        };
        let x = along(read("x"), &how("relatx"), parent.width, 0);
        let y = along(read("y"), &how("relaty"), parent.height, 0);
        let (wide, tall) = match natural {
            Some((wide, tall)) => (wide as i32, tall as i32),
            None => (parent.width, parent.height),
        };
        let width = along(read("w"), &how("relatw"), parent.width, wide);
        let height = along(read("h"), &how("relath"), parent.height, tall);
        Place {
            x: parent.x + x,
            y: parent.y + y,
            width: width.clamp(0, MAX_SIDE),
            height: height.clamp(0, MAX_SIDE),
        }
    }

    fn push(&mut self, place: Place, kind: Kind) {
        if place.width > 0 && place.height > 0 {
            self.items.push(Item {
                x: place.x,
                y: place.y,
                width: place.width,
                height: place.height,
                kind,
            });
        }
    }

    fn group(&mut self, node: &Node, id: Option<&str>, parent: Place, depth: usize) {
        let Some(def) = id.and_then(|id| self.groups.get(&id.to_ascii_lowercase()).cloned()) else {
            return;
        };
        let background = self.image(def.get("background"));
        let natural = background.map(|image| (image.width, image.height));
        let place = self.place(node, Some(&def), parent, natural);
        if let Some(background) = background {
            self.push(place, Kind::Layer(background));
        }
        self.lay_out(&def.children, place, depth + 1);
    }

    /// A group by its name alone, filling a place.
    fn group_in(&mut self, id: &str, place: Place, depth: usize) {
        if let Some(def) = self.groups.get(&id.to_ascii_lowercase()).cloned() {
            self.lay_out(&def.children, place, depth + 1);
        }
    }

    fn layer(&mut self, node: &Node, parent: Place) {
        let Some(mut image) = self.image(node.get("image")) else {
            return;
        };
        // A strip of frames: the first is the one at rest.
        if node.name == "animatedlayer" {
            let frame = |side: &str, whole: u32| {
                node.number(side)
                    .filter(|size| *size > 0)
                    .map_or(whole, |size| (size as u32).min(whole))
            };
            image.width = frame("framewidth", frame("w", image.width));
            image.height = frame("frameheight", frame("h", image.height));
        }
        let place = self.place(node, None, parent, Some((image.width, image.height)));
        self.push(place, Kind::Layer(image));
    }

    fn button(&mut self, node: &Node, parent: Place) {
        let normal = self.image(node.get("image"));
        let Some(size) = normal.map(|image| (image.width, image.height)) else {
            return;
        };
        let place = self.place(node, None, parent, Some(size));
        let Some(act) = act(node) else {
            // It does something only a script knows: drawn, and inert.
            if let Some(normal) = normal {
                self.push(place, Kind::Layer(normal));
            }
            return;
        };
        let kind = Kind::Button {
            normal,
            hover: self.image(node.get("hoverimage")),
            down: self.image(node.get("downimage")),
            active: self.image(node.get("activeimage")),
            act,
            hint: node.get("tooltip").unwrap_or_default().to_owned(),
        };
        self.push(place, kind);
    }

    fn slider(&mut self, node: &Node, parent: Place) {
        let action = node.get("action").unwrap_or_default().to_ascii_lowercase();
        let param = node.get("param").unwrap_or_default().to_ascii_lowercase();
        let slide = match (action.as_str(), param.as_str()) {
            ("seek", _) => Slide::Seek,
            ("volume", _) => Slide::Volume,
            ("pan", _) => Slide::Balance,
            ("eq_band", "preamp") => Slide::Preamp,
            ("eq_band", band) => match band.parse::<usize>() {
                Ok(band @ 1..=10) => Slide::Band(band - 1),
                _ => return,
            },
            _ => return,
        };
        let thumb = self.image(node.get("thumb"));
        let bars = ["barleft", "barmiddle", "barright"].map(|bar| self.image(node.get(bar)));
        let orientation = node.get("orientation").unwrap_or_default();
        let kind = Kind::Slider {
            slide,
            thumb,
            down_thumb: self.image(node.get("downthumb")),
            bars,
            vertical: orientation.to_ascii_lowercase().starts_with('v'),
        };
        let natural = thumb.map(|thumb| (thumb.width, thumb.height));
        let place = self.place(node, None, parent, natural);
        self.push(place, kind);
    }

    fn text(&mut self, node: &Node, parent: Place) {
        let display = node.get("display").unwrap_or_default().to_ascii_lowercase();
        let words = match display.as_str() {
            "songname" => Words::Song,
            "songtitle" => Words::Title,
            "songartist" => Words::Artist,
            "time" => Words::Time,
            "songlength" => Words::Length,
            "songinfo" => Words::Info,
            "songbitrate" => Words::Bitrate,
            "songsamplerate" => Words::SampleRate,
            "" => match node.get("text").or(node.get("default")) {
                Some(fixed) if !fixed.trim().is_empty() => Words::Fixed(fixed.to_owned()),
                _ => return,
            },
            // The name of a window, the state of something scripted.
            _ => return,
        };
        let font = self.font(node.get("font"));
        let size = node.number("fontsize").unwrap_or(11).clamp(6, 48) as f32;
        let natural = font.map_or((80, size as u32 + 2), |font| (80, font.char_height));
        let place = self.place(node, None, parent, Some(natural));
        let align = match node.get("align").unwrap_or_default() {
            align if align.eq_ignore_ascii_case("center") => 0,
            align if align.eq_ignore_ascii_case("right") => 1,
            _ => -1,
        };
        let kind = Kind::Text {
            words,
            font,
            color: self.color(node.get("color")).unwrap_or([0, 255, 0]),
            size,
            align,
        };
        self.push(place, kind);
    }

    fn vis(&mut self, node: &Node, parent: Place) {
        let all = self.color(node.get("colorallbands"));
        let colors: Vec<[u8; 3]> = (1..=16)
            .filter_map(|band| self.color(node.get(&format!("colorband{band}"))).or(all))
            .collect();
        let colors = if colors.is_empty() {
            vec![[0, 255, 0]]
        } else {
            colors
        };
        let peak = self.color(node.get("colorbandpeak"));
        let kind = Kind::Vis {
            peak: peak.unwrap_or([255, 255, 255]),
            colors,
        };
        let place = self.place(node, None, parent, Some((76, 16)));
        self.push(place, kind);
    }
}

/// Whether a tag is something drawn, as against something that only holds
/// definitions.
fn is_control(name: &str) -> bool {
    matches!(
        name,
        "group" | "layer" | "button" | "togglebutton" | "slider" | "text" | "vis" | "script"
    )
}

/// What a button does, from its `action` and what it is tied to.
fn act(node: &Node) -> Option<Act> {
    let action = node.get("action").unwrap_or_default().to_ascii_lowercase();
    // Shuffle and repeat are not actions but settings a button is tied to.
    let tied = node
        .get("cfgattrib")
        .unwrap_or_default()
        .to_ascii_lowercase();
    Some(match action.as_str() {
        "play" => Act::Play,
        "pause" => Act::Pause,
        "stop" => Act::Stop,
        "next" => Act::Next,
        "prev" => Act::Previous,
        "eject" => Act::Eject,
        "close" => Act::Close,
        "minimize" => Act::Minimize,
        "sysmenu" | "controlmenu" | "menu" => Act::Menu,
        "toggle_shuffle" => Act::Shuffle,
        "toggle_repeat" => Act::Repeat,
        _ if tied.ends_with(";shuffle") => Act::Shuffle,
        _ if tied.ends_with(";repeat") => Act::Repeat,
        _ => return None,
    })
}

impl Modern {
    /// Reads a `.wal` archive: its `skin.xml` and all that includes, and
    /// the main window's normal layout out of it.
    pub fn from_archive(name: impl Into<String>, bytes: &[u8]) -> Result<Self, SkinError> {
        let archive = Archive::parse(bytes).map_err(|error| match error {
            super::zip::ZipError::NotAnArchive => SkinError::NotAnArchive,
            other => SkinError::Archive(other),
        })?;
        let mut reader = Reader::new(archive);
        // Usually at the root; sometimes in the folder a skin was zipped as.
        let root = reader
            .files
            .keys()
            .filter(|file| file.rsplit('/').next() == Some("skin.xml"))
            .min_by_key(|file| file.len())
            .cloned()
            .ok_or(SkinError::Empty)?;
        let whole = reader.expand(&root);
        reader.define(&whole);

        let named = |node: &&Node, name: &str| {
            node.get("id")
                .is_some_and(|id| id.eq_ignore_ascii_case(name))
        };
        let containers = std::mem::take(&mut reader.containers);
        let container = containers
            .iter()
            .find(|container| named(container, "main"))
            .or(containers.first())
            .ok_or(SkinError::Empty)?;
        let layouts = || {
            container
                .children
                .iter()
                .filter(|node| node.name == "layout")
        };
        let layout = layouts()
            .find(|layout| named(layout, "normal"))
            .or_else(|| layouts().next())
            .ok_or(SkinError::Empty)?;

        let background = reader.image(layout.get("background"));
        let side = |sides: [&str; 3], natural: Option<u32>| {
            let given = sides.iter().find_map(|side| layout.number(side));
            given
                .filter(|size| *size > 0)
                .or(natural.map(|size| size as i32))
                .unwrap_or(0)
                .clamp(0, MAX_SIDE)
        };
        let width = side(
            ["w", "default_w", "minimum_w"],
            background.map(|image| image.width),
        );
        let height = side(
            ["h", "default_h", "minimum_h"],
            background.map(|image| image.height),
        );
        if width < 16 || height < 16 {
            return Err(SkinError::Empty);
        }
        let whole = Place {
            x: 0,
            y: 0,
            width,
            height,
        };
        if let Some(background) = background {
            reader.push(whole, Kind::Layer(background));
        }
        reader.lay_out(&layout.children, whole, 0);
        if reader.items.is_empty() {
            return Err(SkinError::Empty);
        }
        let mut skin = Self {
            name: name.into(),
            width: width as u32,
            height: height as u32,
            sheets: reader.sheets,
            items: reader.items,
            shape: None,
        };
        skin.shape = skin.outline();
        Ok(skin)
    }

    /// The window's shape, from where its pictures are solid: `None` when
    /// that is everywhere, or nowhere that can be told.
    fn outline(&self) -> Option<Vec<[u32; 4]>> {
        let (wide, tall) = (self.width as usize, self.height as usize);
        let mut solid = vec![false; wide * tall];
        for item in &self.items {
            let image = match &item.kind {
                Kind::Layer(image) => Some(*image),
                Kind::Button { normal, .. } => *normal,
                Kind::Slider { bars, .. } => bars[1],
                Kind::Text { .. } | Kind::Vis { .. } => None,
            };
            let Some(image) = image else {
                continue;
            };
            let sheet = &self.sheets[image.sheet];
            for row in 0..item.height {
                let y = item.y + row;
                if y < 0 || y >= tall as i32 {
                    continue;
                }
                let from_y = image.y + (row as u32 * image.height / item.height as u32);
                for column in 0..item.width {
                    let x = item.x + column;
                    if x < 0 || x >= wide as i32 {
                        continue;
                    }
                    let from_x = image.x + (column as u32 * image.width / item.width as u32);
                    if sheet
                        .pixel(from_x, from_y)
                        .is_some_and(|pixel| pixel[3] >= SOLID)
                    {
                        solid[y as usize * wide + x as usize] = true;
                    }
                }
            }
        }
        let covered = solid.iter().filter(|solid| **solid).count();
        // All of it, or so little that the pictures cannot be the window.
        if covered == solid.len() || covered < solid.len() / 8 {
            return None;
        }
        let mut boxes = Vec::new();
        for (y, row) in solid.chunks(wide).enumerate() {
            let mut start = None;
            for (x, solid) in row.iter().chain(std::iter::once(&false)).enumerate() {
                match (solid, start) {
                    (true, None) => start = Some(x),
                    (false, Some(from)) => {
                        boxes.push([from as u32, y as u32, x as u32, y as u32 + 1]);
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        Some(boxes)
    }
}

#[cfg(test)]
pub(crate) mod tests;
