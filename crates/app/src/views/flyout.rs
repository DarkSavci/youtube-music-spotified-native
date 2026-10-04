//! The tray flyout: controls, and only controls.
//!
//! What is playing, so you know what you are pressing, and the buttons to
//! change it. Anything to watch (progress, the queue, the lyrics) is the
//! mini player's; this is for reaching the transport without finding a
//! window. It opens against the taskbar by the tray icon and goes away
//! when something else is clicked, like the flyouts Windows draws.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Key, Rect, Sense, Ui, ViewportBuilder, ViewportId,
    pos2, vec2,
};
use spotified_client::models::Track;
use spotified_client::session::Repeat;

use super::player_bar;
use super::widgets::{self, ArtShape};
use crate::actions::Action;
use crate::state::{Flyout, Playback, State};
use crate::theme::{self, Icon};

/// The card's size in points. The same with something playing and with
/// nothing, so the window never has to be resized under the pointer.
pub const SIZE: [f32; 2] = [360.0, 136.0];
/// The window's title. Nobody sees it; the window is found by it.
pub const TITLE: &str = "Youtube Music Spotified flyout";
const PADDING: f32 = 16.0;
const ART: f32 = 48.0;
/// The play button's row, measured from the bottom edge to its middle.
const TRANSPORT_UP: f32 = PADDING + 22.0;

pub fn viewport() -> ViewportId {
    ViewportId::from_hash_of("tray-flyout")
}

pub fn builder(flyout: &Flyout) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title(TITLE)
        // Said outright: the window is made hidden, and only a change
        // that is spelt out is passed on to it.
        .with_visible(true)
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_window_level(egui::WindowLevel::AlwaysOnTop)
        .with_inner_size(SIZE)
        .with_position(flyout.position)
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let playback = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some());
    egui::CentralPanel::default()
        .frame(Frame::new().fill(palette.panel))
        .show(ui, |ui| {
            let area = ui.max_rect();
            match playback.and_then(|playback| Some((playback, playback.current()?))) {
                Some((playback, track)) => controls(state, ui, actions, area, playback, track),
                None => empty(state, ui, actions, area),
            }
        });
    dismiss(state, ui, actions);
}

/// Closes the flyout when it is looked away from, or when it is told to.
fn dismiss(state: &State, ui: &Ui, actions: &mut Vec<Action>) {
    let (focused, closing, escape, space) = ui.input(|input| {
        let viewport = input.viewport();
        (
            viewport.focused,
            viewport.close_requested(),
            input.key_pressed(Key::Escape),
            input.key_pressed(Key::Space),
        )
    });
    // The window is not looked at until it has first been looked at: it
    // opens a moment before the system hands it the keyboard.
    let opened = state.flyout.map(|flyout| flyout.opened);
    let id = egui::Id::new(("flyout-looked-at", opened));
    let looked_at = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    match focused {
        Some(true) if !looked_at => {
            ui.data_mut(|data| data.insert_temp(id, true));
        }
        // Asked for once: a window made while another program has the
        // keyboard is not always given it.
        Some(false) if !looked_at => {
            let asked = egui::Id::new(("flyout-asked", opened));
            if !ui
                .data(|data| data.get_temp::<bool>(asked))
                .unwrap_or(false)
            {
                ui.data_mut(|data| data.insert_temp(asked, true));
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        _ => {}
    }
    if closing || escape || (looked_at && focused == Some(false)) {
        actions.push(Action::HideFlyout);
    }
    // Space on the card itself; a button with the keyboard takes its own.
    if space && ui.memory(|memory| memory.focused().is_none()) {
        actions.push(Action::TogglePlay);
    }
}

/// What is playing over the transport, on a wash of the cover's colour.
fn controls(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    area: Rect,
    playback: &Playback,
    track: &Track,
) {
    let palette = &state.palette;
    // The cover's colour fading down the card, as on every surface that
    // shows what is playing.
    if let Some(art) = widgets::artwork_tint(ui, state, &track.artwork, ART) {
        let clear = Color32::TRANSPARENT;
        widgets::vertical_gradient(ui, area, art.gamma_multiply(0.55), clear);
    }
    let inner = area.shrink(PADDING);
    let cover = Rect::from_min_size(inner.min, vec2(ART, ART));
    // A soft shadow lifts the cover off the wash.
    ui.painter().rect_filled(
        cover.translate(vec2(0.0, 3.0)).expand(1.0),
        CornerRadius::same(5),
        Color32::from_black_alpha(60),
    );
    let shape = ArtShape::Rounded(4);
    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);

    let can_like = state.account.is_some();
    let heart_room = if can_like { 40.0 } else { 0.0 };
    let left = cover.right() + 12.0;
    let width = inner.right() - heart_room - left;
    let middle = cover.center().y;
    let title = widgets::elided(
        ui,
        &track.title,
        theme::semibold(14.0),
        palette.text,
        width,
        1,
    );
    ui.painter()
        .galley(pos2(left, middle - 19.0), title, palette.text);
    let artists = track.artist_names();
    let font = theme::regular(12.5);
    let by = widgets::elided(ui, &artists, font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, middle + 2.0), by, palette.secondary);

    if can_like {
        let liked = state.likes.is_liked(&track.id);
        let (icon, tooltip) = if liked {
            (Icon::HeartFilled, "Remove from Liked Music")
        } else {
            (Icon::Heart, "Add to Liked Music")
        };
        let heart = widgets::IconButton {
            icon,
            size: 20.0,
            tooltip,
            active: liked,
        };
        if heart
            .show_at(ui, palette, pos2(inner.right() - 16.0, middle))
            .clicked()
        {
            actions.push(Action::ToggleLike(track.clone()));
        }
    }

    let centre = pos2(area.center().x, area.bottom() - TRANSPORT_UP);
    let session = &playback.session;
    let (repeat_icon, repeat_name) = match session.repeat {
        Repeat::Off => (Icon::Repeat, "Repeat: off"),
        Repeat::All => (Icon::Repeat, "Repeat: all"),
        Repeat::One => (Icon::Repeat1, "Repeat: one"),
    };
    let previous = (
        Icon::SkipBackFilled,
        22.0,
        -54.0,
        "Previous track",
        false,
        Action::Previous,
    );
    let next = (
        Icon::SkipForwardFilled,
        22.0,
        54.0,
        "Next track",
        false,
        Action::Next,
    );
    let shuffle = (
        Icon::Shuffle,
        18.0,
        -102.0,
        "Shuffle",
        session.shuffle,
        Action::ToggleShuffle,
    );
    let repeat = (
        repeat_icon,
        18.0,
        102.0,
        repeat_name,
        session.repeat != Repeat::Off,
        Action::CycleRepeat,
    );
    player_bar::play_disc(ui, palette, actions, Some(playback), centre);
    for (icon, size, offset, tooltip, active, action) in [shuffle, previous, next, repeat] {
        let button = widgets::IconButton {
            icon,
            size,
            tooltip,
            active,
        };
        if button
            .show_at(ui, palette, pos2(centre.x + offset, centre.y))
            .clicked()
        {
            actions.push(action);
        }
    }
}

/// Nothing in the player: say so, and offer the way to the app.
fn empty(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, area: Rect) {
    let palette = &state.palette;
    let centre = area.center().x;
    let top = area.top() + PADDING + 8.0;
    widgets::text_at(
        ui,
        pos2(centre, top),
        Align2::CENTER_TOP,
        "Nothing playing",
        theme::semibold(14.0),
        palette.text,
    );
    widgets::text_at(
        ui,
        pos2(centre, top + 21.0),
        Align2::CENTER_TOP,
        "Pick something in the app to start listening.",
        theme::regular(12.5),
        palette.secondary,
    );

    // A pill in the text's own colour, as the Electron app's: the one
    // thing here to press.
    let label = "Open app";
    let font = theme::semibold(12.5);
    let words = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, palette.panel);
    let size = vec2(16.0 + 16.0 + 6.0 + words.size().x + 16.0, 32.0);
    let rect = Rect::from_center_size(pos2(centre, area.bottom() - PADDING - 4.0 - 16.0), size);
    let response = ui.interact(rect, ui.id().with("open-app"), Sense::click());
    widgets::name(ui, &response, label);
    let lift = widgets::hover(ui, &response);
    let pressed = response.is_pointer_button_down_on();
    let grown = rect.expand(if pressed { -1.0 } else { lift * 1.5 });
    ui.painter()
        .rect_filled(grown, CornerRadius::same(u8::MAX), palette.text);
    let icon = Rect::from_center_size(pos2(rect.left() + 24.0, rect.center().y), vec2(16.0, 16.0));
    widgets::paint_icon(ui, Icon::ExternalLink, icon, 16.0, palette.panel);
    let at = pos2(icon.right() + 6.0, rect.center().y - words.size().y / 2.0);
    ui.painter().galley(at, words, palette.panel);
    if response.clicked() {
        actions.push(Action::ShowMainWindow);
    }
}
