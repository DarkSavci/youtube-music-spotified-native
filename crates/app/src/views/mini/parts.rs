//! The pieces a mini player is made of, whatever its shape: the cover, what
//! is playing, the transport, the seek bar, and the window's own buttons.

use eframe::egui::{self, Align2, Color32, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::Track;
use spotified_client::session::Repeat;

use super::super::widgets::{self, ArtShape};
use super::super::{chrome, format, player_bar, speed, volume};
use crate::actions::Action;
use crate::state::{MiniPanel, Page, Playback, State};
use crate::theme::{self, Icon};

const WINDOW_BUTTON: f32 = 28.0;
const VOLUME_WIDTH: f32 = 88.0;

/// What every piece of the player needs.
pub(super) struct Mini<'a> {
    pub(super) state: &'a State,
    pub(super) playback: Option<&'a Playback>,
    pub(super) track: Option<&'a Track>,
}

/// Makes `rect` move the window when it is dragged. Registered before the
/// controls that sit in it, so they keep their own clicks.
pub(super) fn drag(ui: &Ui, rect: Rect) {
    let response = ui.interact(rect, ui.id().with("mini-drag"), Sense::drag());
    // A drag that only drags starts on the press itself, which at the
    // window's edge is a resize, not a move.
    if response.drag_started() && !chrome::at_resize_edge(ui) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

/// The cover's colour, strongest at one edge and gone before the other:
/// down from the top, or for a bar, in from the left.
pub(super) fn wash(ui: &Ui, area: Rect, art: Option<Color32>, sideways: bool) {
    let Some(art) = art else {
        return;
    };
    let clear = Color32::TRANSPARENT;
    if sideways {
        let strong = art.gamma_multiply(0.45);
        let right = area.left() + area.width() * 0.7;
        let reach = area.with_max_x(right);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(reach.left_top(), strong);
        mesh.colored_vertex(reach.right_top(), clear);
        mesh.colored_vertex(reach.left_bottom(), strong);
        mesh.colored_vertex(reach.right_bottom(), clear);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 2, 3);
        ui.painter().add(egui::Shape::mesh(mesh));
    } else {
        let reach = area.with_max_y(area.top() + area.height() * 0.85);
        widgets::vertical_gradient(ui, reach, art.gamma_multiply(0.6), clear);
    }
}

pub(super) fn cover(mini: &Mini<'_>, ui: &mut Ui, rect: Rect, radius: u8) {
    let palette = &mini.state.palette;
    match mini.track {
        Some(track) => {
            let shape = ArtShape::Rounded(radius);
            widgets::artwork(ui, mini.state, &track.artwork, rect, shape, Icon::Music);
        }
        None => {
            ui.painter()
                .rect_filled(rect, radius, palette.surface_hover);
            let size = (rect.width() * 0.3).clamp(18.0, 64.0);
            widgets::paint_icon(ui, Icon::Music, rect, size, palette.dim);
        }
    }
}

/// Title over artist. Clicking the title brings the main window forward.
/// `on_art` is for text that lies on the cover rather than on the panel.
pub(super) fn meta(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    rect: Rect,
    on_art: bool,
) {
    let palette = &mini.state.palette;
    let (text, quiet) = if on_art {
        (Color32::WHITE, Color32::WHITE.gamma_multiply(0.78))
    } else {
        (palette.text, palette.secondary)
    };
    let (first, second) = (rect.center().y - 18.0, rect.center().y + 2.0);
    let Some(track) = mini.track else {
        for (y, words, font, color) in [
            (first, "Nothing playing", theme::semibold(14.0), text),
            (
                second,
                "Pick something in the app",
                theme::regular(12.0),
                quiet,
            ),
        ] {
            let at = pos2(rect.left(), y);
            widgets::text_at(ui, at, Align2::LEFT_TOP, words, font, color);
        }
        return;
    };
    let link = widgets::Link {
        text: &track.title,
        font: theme::semibold(14.0),
        color: text,
        width: rect.width(),
    };
    let id = ui.id().with("mini-title");
    if link.show(ui, id, pos2(rect.left(), first), true) {
        actions.push(Action::ShowMainWindow);
    }
    let artists = widgets::Artists {
        artists: &track.artists,
        font: theme::regular(12.0),
        color: quiet,
        width: rect.width(),
    };
    let id = ui.id().with("mini-artists");
    if let (Some(artist), _) = artists.show(ui, id, pos2(rect.left(), second)) {
        // The page is in the main window, which comes forward with it.
        actions.push(Action::ShowMainWindow);
        actions.push(Action::Open(Page::Artist(artist)));
    }
}

pub(super) fn like(mini: &Mini<'_>, ui: &mut Ui, actions: &mut Vec<Action>, at: egui::Pos2) {
    let Some(track) = mini.track else {
        return;
    };
    let liked = mini.state.likes.is_liked(&track.id);
    let (icon, tooltip) = if liked {
        (Icon::HeartFilled, "Remove from Liked Songs")
    } else {
        (Icon::Heart, "Save to Liked Songs")
    };
    let heart = widgets::IconButton {
        icon,
        size: 17.0,
        tooltip,
        active: liked,
    };
    if heart.show_at(ui, &mini.state.palette, at).clicked() {
        actions.push(Action::ToggleLike(track.clone()));
    }
}

/// Previous, play and next around `centre`; with room, shuffle and repeat
/// outside them.
pub(super) fn transport(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    centre: egui::Pos2,
    full: bool,
) {
    let palette = &mini.state.palette;
    let session = mini.playback.map(|playback| &playback.session);
    let shuffle = session.is_some_and(|session| session.shuffle);
    let repeat = session.map_or(Repeat::Off, |session| session.repeat);
    let repeat_icon = match repeat {
        Repeat::One => Icon::Repeat1,
        Repeat::Off | Repeat::All => Icon::Repeat,
    };
    let around = [
        (
            Icon::SkipBackFilled,
            18.0,
            -42.0,
            "Previous",
            false,
            Action::Previous,
        ),
        (
            Icon::SkipForwardFilled,
            18.0,
            42.0,
            "Next",
            false,
            Action::Next,
        ),
    ];
    let outside = [
        (
            Icon::Shuffle,
            16.0,
            -80.0,
            "Shuffle",
            shuffle,
            Action::ToggleShuffle,
        ),
        (
            repeat_icon,
            16.0,
            80.0,
            "Repeat",
            repeat != Repeat::Off,
            Action::CycleRepeat,
        ),
    ];
    ui.add_enabled_ui(mini.playback.is_some(), |ui| {
        player_bar::play_disc(ui, palette, actions, mini.playback, centre);
        let buttons = around
            .into_iter()
            .chain(outside.into_iter().filter(|_| full));
        for (icon, size, offset, tooltip, active, action) in buttons {
            let button = widgets::IconButton {
                icon,
                size,
                tooltip,
                active,
            };
            let at = pos2(centre.x + offset, centre.y);
            if button.show_at(ui, palette, at).clicked() {
                actions.push(action);
            }
        }
    });
}

/// The seek bar across `row`, with the time at either end when asked.
pub(super) fn progress(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    row: Rect,
    times: bool,
) {
    let palette = &mini.state.palette;
    let inset = if times { 40.0 } else { 0.0 };
    let bar = Rect::from_center_size(row.center(), vec2(row.width() - inset * 2.0, 16.0));
    ui.add_enabled_ui(mini.playback.is_some(), |ui| {
        let (shown, duration) = player_bar::seek_bar(ui, palette, actions, mini.playback, bar);
        if !times {
            return;
        }
        let at = pos2(row.left(), row.center().y);
        let text = format::duration(shown);
        let font = theme::regular(11.0);
        widgets::text_at(ui, at, Align2::LEFT_CENTER, &text, font, palette.secondary);
        let end = player_bar::EndTime {
            at: pos2(row.right() - 16.0, row.center().y),
            shown,
            duration,
            color: palette.secondary,
        };
        end.show(mini.state, ui, actions);
    });
}

/// A hairline along the bottom edge that shows how far the song has got,
/// where there is no seek bar to.
pub(super) fn thin_progress(mini: &Mini<'_>, ui: &Ui, area: Rect) {
    let Some(playback) = mini.playback else {
        return;
    };
    let duration = playback.current().map_or(0, |track| track.duration_ms);
    if duration == 0 {
        return;
    }
    let done = playback.position_ms(std::time::Instant::now()) as f32 / duration as f32;
    let line = area.with_min_y(area.bottom() - 2.0);
    let unfilled = mini.state.palette.surface_active;
    ui.painter().rect_filled(line, 0.0, unfilled);
    let filled = line.with_max_x(line.left() + line.width() * done.clamp(0.0, 1.0));
    ui.painter()
        .rect_filled(filled, 0.0, mini.state.palette.text);
}

/// Queue and lyrics toggles at the left of `row`, volume at its right.
pub(super) fn extras(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    row: Rect,
    with_slider: bool,
) {
    let palette = &mini.state.palette;
    let y = row.center().y;
    let showing = mini.state.mini_panel;
    let toggles = [
        (Icon::ListMusic, "Queue", MiniPanel::Queue),
        (Icon::MicVocal, "Lyrics", MiniPanel::Lyrics),
    ];
    for (index, (icon, tooltip, panel)) in toggles.into_iter().enumerate() {
        let button = widgets::IconButton {
            icon,
            size: 17.0,
            tooltip,
            active: showing == panel,
        };
        let at = pos2(row.left() + 14.0 + index as f32 * 34.0, y);
        if button.show_at(ui, palette, at).clicked() {
            // Pressing the one that is showing goes back to the cover.
            let next = if showing == panel {
                MiniPanel::Art
            } else {
                panel
            };
            actions.push(Action::SetMiniPanel(next));
        }
    }
    let slider_width = if with_slider { VOLUME_WIDTH } else { 0.0 };
    let bar = Rect::from_min_max(
        pos2(row.right() - slider_width, y - 8.0),
        pos2(row.right(), y + 8.0),
    );
    let control = volume::Volume {
        bar: with_slider.then_some(bar),
        mute_at: pos2(bar.left() - 18.0, y),
        icon: 17.0,
    };
    control.show(mini.state, palette, ui, actions, mini.playback);
}

/// The playback speed, leftwards from `right_centre`: set once and left,
/// so it lives out of the way beside the window's own buttons. Returns the
/// width it took.
pub(super) fn speed_button(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    right_centre: egui::Pos2,
) -> f32 {
    const WIDTH: f32 = 40.0;
    let at = pos2(right_centre.x - WIDTH / 2.0, right_centre.y);
    let state = mini.state;
    speed::button(state, &state.palette, ui, actions, at);
    WIDTH + 2.0
}

/// Keep on top, open the app, and close, laid out leftwards from
/// `right_centre`. Returns the width they took.
pub(super) fn window_buttons(
    mini: &Mini<'_>,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    right_centre: egui::Pos2,
    open_app: bool,
) -> f32 {
    let on_top = mini.state.settings.mini_on_top;
    let pin = if on_top {
        "Keeping on top: click to stop"
    } else {
        "Not on top: click to keep on top"
    };
    let close = (
        Icon::X,
        "Close mini player",
        false,
        Action::ToggleMiniPlayer,
    );
    let open = (
        Icon::ExternalLink,
        "Open app",
        false,
        Action::ShowMainWindow,
    );
    let keep = (Icon::Pin, pin, on_top, Action::SetMiniOnTop(!on_top));
    let buttons = [Some(close), open_app.then_some(open), Some(keep)];
    let mut x = right_centre.x - WINDOW_BUTTON / 2.0;
    for (icon, tooltip, active, action) in buttons.into_iter().flatten() {
        let button = widgets::IconButton {
            icon,
            size: 15.0,
            tooltip,
            active,
        };
        let at = pos2(x, right_centre.y);
        if button.show_at(ui, &mini.state.palette, at).clicked() {
            actions.push(action);
        }
        x -= WINDOW_BUTTON + 2.0;
    }
    right_centre.x - x - WINDOW_BUTTON / 2.0
}
