//! Winamp skins for the mini player: the ones in the skins folder, putting
//! a new one there, and the one being worn.
//!
//! A skin is a classic Winamp 2 `.wsz` file (or a folder it was unpacked
//! into). Reading one is [`crate::skin`]'s work; this is the app's side of
//! it. With no skin chosen the mini player is the app's own.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;

use crate::skin::modern::Modern;
use crate::skin::{MAX_SKIN_DEPTH, Sheet, Skin, SkinError};

/// What the settings call the skin the app comes with. No file can have
/// this name, so none can be mistaken for it.
pub const BUILT_IN: &str = ":classic";
/// Where skins are to be had.
pub const MUSEUM: &str = "https://skins.webamp.org";
/// The sizes a skin is drawn at: this many screen pixels to each of its own.
pub const SCALES: [u8; 4] = [1, 2, 3, 4];
/// `.wal` is a modern skin, for Winamp 3 and 5; the others are classic.
const ARCHIVE_EXTENSIONS: [&str; 3] = ["wsz", "zip", "wal"];
/// A skin is a few hundred kilobytes; a file larger than this is not one.
const MOST_BYTES: u64 = 32 << 20;
/// Nor does anyone browse more skins than this in a row of chips.
const MOST_SKINS: usize = 400;

/// A skin in the folder, by its file's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub file: String,
}

/// The name shown for a skin: its file's, without the archive's ending.
pub fn label(file: &str) -> &str {
    if file == BUILT_IN {
        return "Classic";
    }
    match file.rsplit_once('.') {
        Some((stem, extension)) if is_archive_extension(extension) => stem,
        _ => file,
    }
}

fn is_archive_extension(extension: &str) -> bool {
    ARCHIVE_EXTENSIONS
        .iter()
        .any(|known| extension.eq_ignore_ascii_case(known))
}

/// Whether a file could be a skin, by its name.
pub fn is_skin_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_archive_extension)
}

/// Every skin in `folder`, by name.
pub fn list(folder: &Path) -> Vec<Choice> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut skins: Vec<Choice> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_skin = if path.is_dir() {
                has_main_bitmap(&path)
            } else {
                is_skin_file(&path)
            };
            is_skin.then(|| Choice {
                file: entry.file_name().to_string_lossy().into_owned(),
            })
        })
        .take(MOST_SKINS)
        .collect();
    skins.sort_by_key(|skin| skin.file.to_lowercase());
    skins
}

/// Whether an unpacked skin has a main window bitmap, looked for the way
/// [`Skin::from_dir`] reads one: in nested folders too, without following
/// links.
fn has_main_bitmap(folder: &Path) -> bool {
    let mut folders = VecDeque::from([(folder.to_path_buf(), 0)]);
    while let Some((folder, depth)) = folders.pop_front() {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && depth < MAX_SKIN_DEPTH {
                folders.push_back((entry.path(), depth + 1));
            } else if kind.is_file() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name == "main.bmp" || name == "main.png" {
                    return true;
                }
            }
        }
    }
    false
}

/// Puts a skin file in the folder, once it has read as a skin, and gives
/// the name it is kept under. A file already there by that name is
/// replaced: installing a skin again is how it is updated.
pub fn install(file: &Path, folder: &Path) -> Result<String, String> {
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or("that is not a file")?;
    let said = |error: &dyn std::fmt::Display| format!("{}: {error}", label(&name));
    let size = std::fs::metadata(file).map_err(|error| said(&error))?.len();
    if file.is_file() && size > MOST_BYTES {
        return Err(said(&"too large to be a skin"));
    }
    if file.is_dir() {
        return Err(said(&"unpacked skins go in the skins folder by hand"));
    }
    read(file).map_err(|error| said(&error))?;
    let destination = folder.join(&name);
    if destination != file {
        std::fs::create_dir_all(folder).map_err(|error| said(&error))?;
        std::fs::copy(file, &destination).map_err(|error| said(&error))?;
    }
    Ok(name)
}

/// A skin as it was read: classic, or modern.
enum Read {
    Classic(Box<Skin>),
    Modern(Modern),
}

/// Reads a skin file of either kind. A `.wal` is a modern skin and is read
/// as one first: its pictures are named as it pleases, and some of those
/// names are a classic skin's too. Anything else is read as a classic
/// skin, and as a modern one if it says that is what it is.
fn read(file: &Path) -> Result<Read, SkinError> {
    let modern = || {
        let name = file.file_stem().unwrap_or_default().to_string_lossy();
        let bytes = std::fs::read(file)?;
        // A modern skin whose window cannot be laid out is still a modern
        // skin: the reason given is that, not "no bitmaps".
        let modern = Modern::from_archive(name, &bytes);
        modern.map(Read::Modern).map_err(|error| match error {
            SkinError::Empty => SkinError::ModernSkin,
            other => other,
        })
    };
    let named_modern = file
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wal"));
    if named_modern && file.is_file() {
        return modern().or_else(|error| match Skin::load(file) {
            Ok(skin) => Ok(Read::Classic(Box::new(skin))),
            Err(_) => Err(error),
        });
    }
    match Skin::load(file) {
        Ok(skin) => Ok(Read::Classic(Box::new(skin))),
        Err(SkinError::ModernSkin) => modern(),
        Err(error) => Err(error),
    }
}

/// The skin the mini player wears, read and ready to draw.
#[derive(Clone)]
pub struct Worn {
    /// What the settings call it: a file in the folder, or [`BUILT_IN`].
    pub file: String,
    /// The classic skin; for a modern one, the built-in, whose colours
    /// its menus are drawn in.
    pub skin: Arc<Skin>,
    textures: HashMap<Sheet, egui::TextureHandle>,
    /// The modern skin, when that is what is worn.
    pub modern: Option<ModernWorn>,
}

/// A modern skin with its pictures as textures.
#[derive(Clone)]
pub struct ModernWorn {
    pub skin: Arc<Modern>,
    textures: Vec<egui::TextureHandle>,
}

impl ModernWorn {
    pub fn texture(&self, sheet: usize) -> Option<egui::TextureId> {
        self.textures.get(sheet).map(egui::TextureHandle::id)
    }
}

impl Worn {
    pub fn texture(&self, sheet: Sheet) -> Option<egui::TextureId> {
        self.textures.get(&sheet).map(egui::TextureHandle::id)
    }
}

/// Reads the skin the settings name and makes its sheets into textures:
/// nearest-neighbour ones, so its pixels stay pixels at any size.
pub fn wear(ctx: &egui::Context, file: &str, folder: &Path) -> Result<Worn, SkinError> {
    let skin = if file == BUILT_IN {
        Skin::builtin()
    } else {
        match read(&folder.join(file))? {
            Read::Classic(skin) => Arc::from(skin),
            Read::Modern(modern) => return Ok(wear_modern(ctx, file, modern)),
        }
    };
    log::info!("the mini player wears the skin {}", skin.name);
    let textures = Sheet::ALL
        .into_iter()
        .map(|sheet| {
            let bitmap = skin.sheet(sheet);
            let size = [bitmap.width as usize, bitmap.height as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &bitmap.rgba);
            let name = format!("skin-{}", sheet.file_stem());
            let handle = ctx.load_texture(name, image, egui::TextureOptions::NEAREST);
            (sheet, handle)
        })
        .collect();
    Ok(Worn {
        file: file.to_owned(),
        skin,
        textures,
        modern: None,
    })
}

/// A modern skin's pictures as textures: smooth ones, since these skins
/// are photographs and gradients as often as they are pixels.
fn wear_modern(ctx: &egui::Context, file: &str, modern: Modern) -> Worn {
    log::info!(
        "the mini player wears the modern skin {} ({} by {})",
        modern.name,
        modern.width,
        modern.height
    );
    let textures = modern
        .sheets
        .iter()
        .enumerate()
        .map(|(index, bitmap)| {
            let size = [bitmap.width as usize, bitmap.height as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &bitmap.rgba);
            ctx.load_texture(
                format!("modern-skin-{index}"),
                image,
                egui::TextureOptions::LINEAR,
            )
        })
        .collect();
    Worn {
        file: file.to_owned(),
        skin: Skin::builtin(),
        textures: HashMap::new(),
        modern: Some(ModernWorn {
            skin: Arc::new(modern),
            textures,
        }),
    }
}

/// What can be asked about skins.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Wear this skin of the folder's, or [`BUILT_IN`]; `None` for the
    /// app's own mini player.
    Wear(Option<String>),
    /// Put these files in the folder and wear the last that is a skin.
    Install(Vec<PathBuf>),
    /// Ask the person for skin files to install.
    Pick,
    /// The skin the settings name could not be read.
    Failed(String),
    OpenFolder,
    /// Show where skins are to be had, in the browser.
    OpenMuseum,
    /// Read the skins folder again.
    Reload,
    /// Draw the skin at this many screen pixels to each of its own.
    Scale(u8),
    /// Roll the main window up to its title bar, or down again.
    ToggleShade,
    ToggleEqualizer,
    ToggleEqualizerShade,
    TogglePlaylist,
    /// The playlist was stretched to this many skin pixels tall.
    PlaylistHeight(u32),
    /// Roll the playlist up to its title bar, or down again.
    TogglePlaylistShade,
    /// The display's next look: the spectrum, the wave, nothing.
    CycleAnalyser,
    /// Turn the sound this far to one side, from -1 (left) to 1. Heard at
    /// once; written down by `Keep`.
    Balance(f32),
    /// Write down where a drag has left things.
    Keep,
}

/// A balance held to its range; one that is not a number is the middle.
pub fn held_balance(balance: f32) -> f32 {
    if balance.is_finite() {
        balance.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spotified-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    fn png(color: [u8; 3]) -> Vec<u8> {
        let image = image::RgbImage::from_pixel(275, 116, image::Rgb(color));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("a picture");
        bytes.into_inner()
    }

    fn skin_file(dir: &Path, name: &str) -> PathBuf {
        let archive = crate::skin::zip::write(&[("main.bmp", &png([1, 2, 3]), true)]);
        let path = dir.join(name);
        std::fs::write(&path, archive).expect("a skin");
        path
    }

    #[test]
    fn labels_drop_the_archive_ending_only() {
        assert_eq!(label("Base 2.91.wsz"), "Base 2.91");
        assert_eq!(label("Nucleo.ZIP"), "Nucleo");
        assert_eq!(label("Winamp Modern.wal"), "Winamp Modern");
        assert_eq!(label("Unpacked v1.2"), "Unpacked v1.2");
        assert_eq!(label(BUILT_IN), "Classic");
    }

    #[test]
    fn the_folder_lists_archives_and_unpacked_skins_by_name() {
        let dir = temp_dir("skins-list");
        skin_file(&dir, "zeta.wsz");
        skin_file(&dir, "Alpha.zip");
        std::fs::write(dir.join("notes.txt"), b"not a skin").expect("a file");
        std::fs::create_dir_all(dir.join("Middle/inner")).expect("a folder");
        std::fs::write(dir.join("Middle/inner/MAIN.BMP"), png([4, 5, 6])).expect("a bitmap");
        std::fs::create_dir_all(dir.join("Empty")).expect("a folder");
        let files: Vec<String> = list(&dir).into_iter().map(|skin| skin.file).collect();
        std::fs::remove_dir_all(&dir).expect("removed");
        assert_eq!(files, ["Alpha.zip", "Middle", "zeta.wsz"]);
        assert!(list(Path::new("/nonexistent/skins")).is_empty());
    }

    #[test]
    fn installing_copies_a_skin_in_and_refuses_what_is_not_one() {
        let dir = temp_dir("skins-install");
        let (from, folder) = (dir.join("downloads"), dir.join("skins"));
        std::fs::create_dir_all(&from).expect("a folder");
        let skin = skin_file(&from, "Fresh.wsz");
        assert_eq!(install(&skin, &folder), Ok("Fresh.wsz".to_owned()));
        assert!(folder.join("Fresh.wsz").is_file());
        // Again, from the folder itself: nothing to copy, and still a skin.
        assert!(install(&folder.join("Fresh.wsz"), &folder).is_ok());

        let text = from.join("Words.wsz");
        std::fs::write(&text, b"just some text").expect("a file");
        let refused = install(&text, &folder).expect_err("not a skin");
        assert!(refused.starts_with("Words: not a zip archive"), "{refused}");
        assert!(!folder.join("Words.wsz").exists());
        assert!(install(&from.join("gone.wsz"), &folder).is_err());
        std::fs::remove_dir_all(&dir).expect("removed");
    }

    #[test]
    fn a_modern_skin_is_installed_and_read_as_what_it_is() {
        let dir = temp_dir("skins-modern");
        let (from, folder) = (dir.join("downloads"), dir.join("skins"));
        std::fs::create_dir_all(&from).expect("a folder");
        let skin = from.join("Small.wal");
        std::fs::write(&skin, crate::skin::modern::tests::skin()).expect("a skin");
        assert_eq!(install(&skin, &folder), Ok("Small.wal".to_owned()));
        let listed: Vec<String> = list(&folder).into_iter().map(|skin| skin.file).collect();
        assert_eq!(listed, ["Small.wal"]);
        assert!(matches!(
            read(&folder.join("Small.wal")),
            Ok(Read::Modern(modern)) if (modern.width, modern.height) == (200, 80)
        ));
        // The same skin under a classic skin's ending is still found out.
        let renamed = from.join("Small.wsz");
        std::fs::copy(&skin, &renamed).expect("a copy");
        assert!(matches!(read(&renamed), Ok(Read::Modern(_))));

        // A modern skin with no window to draw says so, and stays out.
        let bare = from.join("Bare.wal");
        let archive = crate::skin::zip::write(&[("skin.xml", b"<WasabiXML/>", false)]);
        std::fs::write(&bare, archive).expect("a skin");
        let refused = install(&bare, &folder).expect_err("nothing to draw");
        assert!(refused.contains("modern Winamp skin"), "{refused}");
        std::fs::remove_dir_all(&dir).expect("removed");
    }
}
