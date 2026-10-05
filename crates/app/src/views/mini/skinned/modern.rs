//! The mini player in a modern Winamp skin: the main window's layout as
//! its XML lays it out, with the controls that have a standard meaning
//! wired to the player.
//!
//! The skin's scripts are not run (see [`crate::skin::modern`]), so this
//! is the window at rest: nothing slides, blinks or changes shape. The
//! whole window moves it, and a right click anywhere brings the menu with
//! the skins and the ways out, since a skin need not have a button for
//! either.

use std::time::Instant;

use eframe::egui::{self, Color32, Id, Pos2, Rect, Sense, Ui, ViewportCommand, pos2, vec2};
use spotified_client::session::Repeat;

use super::super::super::{format, volume};
use super::{balance_of, menu, options_menu, play, stop};
use crate::actions::Action;
use crate::equalizer::Ask as Eq;
use crate::skin::font;
use crate::skin::modern::{Act, BitmapFont, Image, Item, Kind, Modern, Slide, Words};
use crate::skins::{Ask, ModernWorn, Worn};
use crate::state::{Playback, State};
use crate::theme;
use spotified_audio::eq::RANGE_DB;

/// The menus' type is sized as a classic skin's at this scale.
const MENU_UNIT: f32 = 2.4;
/// How many characters a second a line too long for its place moves.
const SCROLL_A_SECOND: f64 = 5.0;
/// What separates the end of a scrolling line from its start again.
const GAP: &str = "  ***  ";

/// The window's size in points: a point to each of the skin's pixels.
pub fn size(skin: &Modern) -> egui::Vec2 {
    vec2(skin.width as f32, skin.height as f32)
}

struct View<'a> {
    ui: &'a mut Ui,
    origin: Pos2,
    worn: &'a ModernWorn,
}

impl View<'_> {
    fn rect(&self, item: &Item) -> Rect {
        Rect::from_min_size(
            self.origin + vec2(item.x as f32, item.y as f32),
            vec2(item.width as f32, item.height as f32),
        )
    }

    /// Draws a picture into a place, stretched to it.
    fn paint(&self, painter: &egui::Painter, image: Image, into: Rect) {
        let (Some(texture), Some(sheet)) = (
            self.worn.texture(image.sheet),
            self.worn.skin.sheets.get(image.sheet),
        ) else {
            return;
        };
        let (wide, tall) = (sheet.width as f32, sheet.height as f32);
        let uv = Rect::from_min_max(
            pos2(image.x as f32 / wide, image.y as f32 / tall),
            pos2(
                (image.x + image.width) as f32 / wide,
                (image.y + image.height) as f32 / tall,
            ),
        );
        painter.image(texture, into, uv, Color32::WHITE);
    }

    fn image(&self, image: Image, into: Rect) {
        self.paint(self.ui.painter(), image, into);
    }
}

pub fn show(
    state: &State,
    worn: &Worn,
    modern: &ModernWorn,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
) {
    let ctx = ui.ctx().clone();
    let playback = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some());
    let mut view = View {
        origin: ui.max_rect().min,
        ui,
        worn: modern,
    };
    // Under everything: the window itself, which moves when dragged and
    // has the menu a skin may have no button for.
    let whole = Rect::from_min_size(view.origin, size(&modern.skin));
    let window = view
        .ui
        .interact(whole, Id::new("modern-skin"), Sense::click_and_drag());
    if window.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    let options = |ui: &mut Ui, actions: &mut Vec<Action>| {
        options_menu(state, ui, actions, MENU_UNIT);
    };
    menu(
        egui::Popup::context_menu(&window),
        &worn.skin,
        MENU_UNIT,
        |ui| options(ui, actions),
    );

    let mut moving = false;
    for (index, item) in modern.skin.items.iter().enumerate() {
        let rect = view.rect(item);
        match &item.kind {
            Kind::Layer(image) => view.image(*image, rect),
            Kind::Button { .. } => button(state, worn, &mut view, actions, playback, item, index),
            Kind::Slider { .. } => slider(state, &mut view, actions, playback, item, index),
            Kind::Text { .. } => text(state, &view, playback, item),
            Kind::Vis { colors, peak } => {
                moving |= vis(state, &view, playback, rect, colors, *peak);
            }
        }
    }
    if moving {
        ctx.request_repaint_after(super::display::ANALYSER_FRAME);
    } else if playback.is_some() {
        ctx.request_repaint_after(super::display::MARQUEE_STEP);
    }
}

/// What a button is called, by a screen reader as by the tests.
fn name(act: Act) -> &'static str {
    match act {
        Act::Play => "Play",
        Act::Pause => "Pause",
        Act::Stop => "Stop",
        Act::Next => "Next",
        Act::Previous => "Previous",
        Act::Eject => "Open app",
        Act::Close => "Close mini player",
        Act::Minimize => "Minimise",
        Act::Menu => "Menu",
        Act::Shuffle => "Shuffle",
        Act::Repeat => "Repeat",
    }
}

fn button(
    state: &State,
    worn: &Worn,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    item: &Item,
    index: usize,
) {
    let Kind::Button {
        normal,
        hover,
        down,
        active,
        act,
        hint,
    } = &item.kind
    else {
        return;
    };
    let rect = view.rect(item);
    let id = Id::new(("modern-button", index));
    let response = view.ui.interact(rect, id, Sense::click());
    let label = name(*act);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let session = playback.map(|playback| &playback.session);
    let on = match act {
        Act::Shuffle => session.is_some_and(|session| session.shuffle),
        Act::Repeat => session.is_some_and(|session| session.repeat != Repeat::Off),
        _ => false,
    };
    let shown = if response.is_pointer_button_down_on() {
        down.or(*normal)
    } else if on {
        active.or(*down).or(*normal)
    } else if response.hovered() {
        hover.or(*normal)
    } else {
        *normal
    };
    if let Some(image) = shown {
        view.image(image, rect);
    }
    if *act == Act::Menu {
        menu(egui::Popup::menu(&response), &worn.skin, MENU_UNIT, |ui| {
            options_menu(state, ui, actions, MENU_UNIT);
        });
        return;
    }
    let response = if hint.is_empty() {
        response
    } else {
        response.on_hover_text(hint)
    };
    if !response.clicked() {
        return;
    }
    match act {
        Act::Play => play(actions, playback),
        Act::Pause => {
            if playback.is_some() {
                actions.push(Action::TogglePlay);
            }
        }
        Act::Stop => stop(actions, playback),
        Act::Next => actions.push(Action::Next),
        Act::Previous => actions.push(Action::Previous),
        Act::Eject => actions.push(Action::ShowMainWindow),
        Act::Close => actions.push(Action::ToggleMiniPlayer),
        Act::Minimize => {
            let minimise = ViewportCommand::Minimized(true);
            view.ui.ctx().send_viewport_cmd(minimise);
        }
        Act::Shuffle => actions.push(Action::ToggleShuffle),
        Act::Repeat => actions.push(Action::CycleRepeat),
        Act::Menu => {}
    }
}

/// Where a slider stands, from nought to one.
fn value(state: &State, playback: Option<&Playback>, slide: Slide) -> f32 {
    let settings = &state.settings;
    let decibels = |gain: f32| ((gain + RANGE_DB) / (2.0 * RANGE_DB)).clamp(0.0, 1.0);
    match slide {
        Slide::Seek => playback.map_or(0.0, |playback| {
            let length = playback.current().map_or(0, |track| track.duration_ms);
            let position = playback.position_ms(Instant::now());
            if length == 0 {
                0.0
            } else {
                (position as f32 / length as f32).clamp(0.0, 1.0)
            }
        }),
        Slide::Volume => {
            let volume = playback.map_or(0.0, |playback| playback.session.volume);
            (volume / settings.max_volume()).clamp(0.0, 1.0)
        }
        Slide::Balance => (settings.balance + 1.0) / 2.0,
        Slide::Band(band) => decibels(settings.equalizer.get(band).copied().unwrap_or(0.0)),
        Slide::Preamp => decibels(settings.equalizer_preamp),
    }
}

fn slider(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    item: &Item,
    index: usize,
) {
    let Kind::Slider {
        slide,
        thumb,
        down_thumb,
        bars,
        vertical,
    } = &item.kind
    else {
        return;
    };
    let rect = view.rect(item);
    let id = Id::new(("modern-slider", index));
    let response = view.ui.interact(rect, id, Sense::click_and_drag());
    let (thumb_wide, thumb_tall) = thumb.map_or((0.0, 0.0), |thumb| {
        (thumb.width as f32, thumb.height as f32)
    });
    // The thumb's travel, and where the pointer has it along that.
    let (travel, thumb_long) = if *vertical {
        (rect.height() - thumb_tall, thumb_tall)
    } else {
        (rect.width() - thumb_wide, thumb_wide)
    };
    let pointer = response.interact_pointer_pos().map(|pos| {
        let along = if *vertical {
            // The top of an upright slider is its most.
            rect.bottom() - pos.y
        } else {
            pos.x - rect.left()
        };
        ((along - thumb_long / 2.0) / travel.max(1.0)).clamp(0.0, 1.0)
    });
    let held = response.dragged() || response.is_pointer_button_down_on();
    let dragging: Option<f32> = view.ui.data(|data| data.get_temp(id));
    let moved = (response.dragged() || response.drag_started())
        .then_some(pointer)
        .flatten();
    if let Some(to) = moved {
        view.ui.data_mut(|data| data.insert_temp(id, to));
    }
    let released = if response.drag_stopped() {
        view.ui.data_mut(|data| data.remove::<f32>(id));
        dragging.or(pointer)
    } else if response.clicked() {
        pointer
    } else {
        None
    };
    let most = state.settings.max_volume();
    let to_decibels = |to: f32| to * 2.0 * RANGE_DB - RANGE_DB;
    match slide {
        // Sent once, when it is let go: a seek for every step of a drag
        // would restart the song over and over.
        Slide::Seek => {
            let length = playback
                .and_then(Playback::current)
                .map_or(0, |track| track.duration_ms);
            if let Some(to) = released.filter(|_| length > 0) {
                actions.push(Action::Seek((to * length as f32) as u64));
            }
        }
        Slide::Volume => {
            if let Some(to) = moved.or(released) {
                actions.push(Action::SetVolume(to * most));
            }
            if playback.is_some()
                && response.hovered()
                && let Some(step) = volume::wheel(view.ui)
            {
                actions.push(Action::VolumeBy(step));
            }
        }
        Slide::Balance => {
            if let Some(to) = moved.or(released) {
                actions.push(Action::Skin(Ask::Balance(balance_of(to))));
            }
            if released.is_some() {
                actions.push(Action::Skin(Ask::Keep));
            }
        }
        Slide::Band(band) => {
            if let Some(to) = moved.or(released) {
                actions.push(Action::Equalizer(Eq::Band(*band, to_decibels(to))));
            }
            if released.is_some() {
                actions.push(Action::Equalizer(Eq::Keep));
            }
        }
        Slide::Preamp => {
            if let Some(to) = moved.or(released) {
                actions.push(Action::Equalizer(Eq::Preamp(to_decibels(to))));
            }
            if released.is_some() {
                actions.push(Action::Equalizer(Eq::Keep));
            }
        }
    }

    // The track: its two ends, and the middle stretched between them.
    let [left, middle, right] = bars;
    let end = |image: &Option<Image>| image.map_or(0.0, |image| image.width as f32);
    if let Some(middle) = middle {
        let between = Rect::from_min_max(
            pos2(rect.left() + end(left), rect.top()),
            pos2(rect.right() - end(right), rect.bottom()),
        );
        view.image(*middle, between);
    }
    if let Some(left) = left {
        view.image(*left, rect.with_max_x(rect.left() + left.width as f32));
    }
    if let Some(right) = right {
        view.image(*right, rect.with_min_x(rect.right() - right.width as f32));
    }
    // While held the thumb is where the hand has it, not where the music is.
    let shown = dragging
        .filter(|_| held)
        .unwrap_or_else(|| value(state, playback, *slide));
    let Some(image) = down_thumb.filter(|_| held).or(*thumb) else {
        return;
    };
    let size = vec2(image.width as f32, image.height as f32);
    let at = if *vertical {
        pos2(
            rect.center().x - size.x / 2.0,
            rect.top() + (1.0 - shown) * travel,
        )
    } else {
        pos2(rect.left() + shown * travel, rect.center().y - size.y / 2.0)
    };
    view.image(image, Rect::from_min_size(at, size));
}

/// What a line of text says now.
fn words(state: &State, playback: Option<&Playback>, words: &Words) -> String {
    let track = playback.and_then(Playback::current);
    let tap = state.audio_tap.as_ref();
    let kilobits = tap.map_or(0, |tap| tap.bitrate());
    let kilohertz = tap.map_or(0, |tap| tap.sample_rate() / 1000);
    match words {
        Words::Fixed(fixed) => fixed.clone(),
        Words::Song => match track {
            Some(track) if track.artists.is_empty() => track.title.clone(),
            Some(track) => format!("{} - {}", track.artist_names(), track.title),
            None => "Youtube Music Spotified".to_owned(),
        },
        Words::Title => track.map(|track| track.title.clone()).unwrap_or_default(),
        Words::Artist => track.map(|track| track.artist_names()).unwrap_or_default(),
        Words::Time => playback.map_or_else(String::new, |playback| {
            format::duration(playback.position_ms(Instant::now()))
        }),
        Words::Length => {
            track.map_or_else(String::new, |track| format::duration(track.duration_ms))
        }
        Words::Bitrate if track.is_some() && kilobits > 0 => kilobits.to_string(),
        Words::SampleRate if track.is_some() => kilohertz.to_string(),
        Words::Info if track.is_some() && kilobits > 0 => {
            format!("{kilobits}kbps stereo {kilohertz}khz")
        }
        Words::Info if track.is_some() => format!("stereo {kilohertz}khz"),
        Words::Bitrate | Words::SampleRate | Words::Info => String::new(),
    }
}

fn text(state: &State, view: &View<'_>, playback: Option<&Playback>, item: &Item) {
    let Kind::Text {
        words: what,
        font: face,
        color,
        size,
        align,
    } = &item.kind
    else {
        return;
    };
    let said = words(state, playback, what);
    if said.is_empty() {
        return;
    }
    let rect = view.rect(item);
    let painter = view
        .ui
        .painter()
        .with_clip_rect(rect.intersect(view.ui.clip_rect()));
    let clock = view.ui.input(|input| input.time);
    // A picture of letters, when the skin has one that can say this.
    if let Some(face) = face.filter(|_| said.chars().all(font::covered)) {
        return bitmap_text(view, &painter, &said, face, rect, *align, clock);
    }
    let [red, green, blue] = *color;
    let color = Color32::from_rgb(red, green, blue);
    // No taller than its place: skins give text a line and no more.
    let face = theme::medium(size.min(rect.height().max(8.0)));
    let galley = painter.layout_no_wrap(said.clone(), face.clone(), color);
    let top = rect.center().y - galley.size().y / 2.0;
    if galley.size().x <= rect.width() {
        let left = match align {
            0 => rect.center().x - galley.size().x / 2.0,
            1 => rect.right() - galley.size().x,
            _ => rect.left(),
        };
        painter.galley(pos2(left, top), galley, color);
        return;
    }
    // Too long for its place: it goes round, as a marquee.
    let strip = painter.layout_no_wrap(format!("{said}{GAP}"), face, color);
    let width = strip.size().x;
    let along = (clock * SCROLL_A_SECOND * 6.0) as f32 % width;
    for copy in 0..2 {
        let left = rect.left() - along + copy as f32 * width;
        painter.galley(pos2(left, top), strip.clone(), color);
    }
}

/// A line set in a skin's own picture of letters, which is laid out as the
/// classic skins' `text.bmp` is.
fn bitmap_text(
    view: &View<'_>,
    painter: &egui::Painter,
    said: &str,
    face: BitmapFont,
    rect: Rect,
    align: i8,
    clock: f64,
) {
    let advance = (face.char_width as i32 + face.spacing).max(1) as f32;
    let mut letters: Vec<char> = said.chars().collect();
    let fits = (rect.width() / advance).floor().max(1.0) as usize;
    if letters.len() > fits {
        // Round and round, a letter at a time.
        letters.extend(GAP.chars());
        let offset = (clock * SCROLL_A_SECOND) as usize % letters.len();
        letters.rotate_left(offset);
        letters.truncate(fits);
    }
    let width = letters.len() as f32 * advance;
    let left = match align {
        0 => rect.center().x - width / 2.0,
        1 => rect.right() - width,
        _ => rect.left(),
    };
    let top = rect.center().y - face.char_height as f32 / 2.0;
    for (index, letter) in letters.into_iter().enumerate() {
        let (row, column) = font::cell(letter);
        let image = Image {
            sheet: face.sheet,
            x: column * face.char_width,
            y: row * face.char_height,
            width: face.char_width,
            height: face.char_height,
        };
        let at = pos2(left + index as f32 * advance, top);
        let size = vec2(face.char_width as f32, face.char_height as f32);
        view.paint(painter, image, Rect::from_min_size(at, size));
    }
}

/// The spectrum, as bars in the skin's colours. Returns whether it moves.
fn vis(
    state: &State,
    view: &View<'_>,
    playback: Option<&Playback>,
    rect: Rect,
    colors: &[[u8; 3]],
    peak: [u8; 3],
) -> bool {
    let sounding = playback.is_some_and(Playback::is_playing);
    let (true, true, Some(tap)) = (state.settings.skin_analyser, sounding, &state.audio_tap) else {
        return false;
    };
    // Bars three points wide with one between, as many as there is room for.
    let bars = ((rect.width() / 4.0) as usize).clamp(1, 64);
    let rgb = |[red, green, blue]: [u8; 3]| Color32::from_rgb(red, green, blue);
    let painter = view.ui.painter();
    for (index, level) in tap.spectrum(bars).into_iter().enumerate() {
        let left = rect.left() + index as f32 * 4.0;
        let tall = (level.clamp(0.0, 1.0) * rect.height()).round();
        if tall < 1.0 {
            continue;
        }
        // Lit from the bottom: the louder, the further up the colours.
        let rows = tall as usize;
        for row in 0..rows {
            let color = colors
                [(row * colors.len() / (rect.height() as usize).max(1)).min(colors.len() - 1)];
            let bottom = rect.bottom() - row as f32;
            let cell = Rect::from_min_max(pos2(left, bottom - 1.0), pos2(left + 3.0, bottom));
            painter.rect_filled(cell, 0.0, rgb(color));
        }
        let top = rect.bottom() - tall;
        let cap = Rect::from_min_max(pos2(left, top - 1.0), pos2(left + 3.0, top));
        painter.rect_filled(cap, 0.0, rgb(peak));
    }
    true
}
