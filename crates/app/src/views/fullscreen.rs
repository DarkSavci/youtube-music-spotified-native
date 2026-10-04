//! Full-screen now playing.
//!
//! The cover bleeds to all four edges and everything else sits over it:
//! where the music is from at the top left, the song at the bottom left
//! beside a small cover, and the transport across the bottom. The cover is
//! the subject; a centred square with a blur behind it would make the blur
//! the subject and the cover a stamp on top of it.
//!
//! While a song plays and nothing is touched, the controls fade away and
//! leave the cover alone on the screen.

use eframe::egui::{self, Align2, Color32, Frame, Pos2, Rect, Sense, Ui, pos2, vec2};
use spotified_client::session::Repeat;

use super::widgets::{self, ArtShape};
use super::{format, player_bar, speed, video, volume};
use crate::actions::Action;
use crate::state::{Page, Playback, State};
use crate::theme::{self, Icon, Palette};

/// The room kept at the sides, and under the controls.
const SIDE: f32 = 32.0;
const TOP: f32 = 24.0;
const BOTTOM: f32 = 24.0;
const THUMB: f32 = 64.0;
const PLAY_DISC: f32 = 56.0;
const GAP: f32 = 20.0;
const VOLUME_WIDTH: f32 = 120.0;
/// How long nothing must be touched before the controls go, and how long
/// they take to.
const IDLE_SECONDS: f64 = 3.0;
const FADE_SECONDS: f32 = 0.18;

/// The controls' colours over a cover, whatever the theme: a cover is
/// darkened under them, so they are light.
fn over_art(palette: &Palette) -> Palette {
    Palette {
        text: Color32::WHITE,
        secondary: Color32::from_white_alpha(190),
        dim: Color32::from_white_alpha(100),
        surface_active: Color32::from_white_alpha(70),
        ..*palette
    }
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, playback: &Playback) {
    let Some(track) = playback.current() else {
        return;
    };
    let palette = over_art(&state.palette);
    egui::CentralPanel::default()
        .frame(Frame::new().fill(Color32::BLACK))
        .show(ui, |ui| {
            let area = ui.max_rect();
            if state.video.enabled {
                video::surface(state, ui, actions, area, false);
            } else {
                let shape = ArtShape::Rounded(0);
                widgets::artwork(ui, state, &track.artwork, area, shape, Icon::Music);
            }

            let foot = area.with_min_y(area.bottom() - (BOTTOM + PLAY_DISC + 16.0 * 4.0 + THUMB));
            let head = Rect::from_min_max(area.min, pos2(area.right(), area.top() + TOP + 40.0));
            let shown = controls_shown(ui, playback, &[foot, head]);
            if shown <= 0.0 {
                // Gone, and with nothing left to click.
                ui.ctx().set_cursor_icon(egui::CursorIcon::None);
            } else {
                ui.scope(|ui| {
                    ui.set_opacity(shown);
                    scrim(ui, area);
                    context(ui, actions, playback, &palette, video::notice(state), area);
                    self::foot(state, &palette, ui, actions, playback, area);
                });
            }
            let leaving = ui.input(|input| input.key_pressed(egui::Key::Escape));
            // A panel that is open takes the key first.
            if leaving && !egui::Popup::is_any_open(ui.ctx()) {
                actions.push(Action::SetFullscreenPlayer(false));
            }
        });
}

/// How strongly the controls show, from 0 to 1. They stay while the song
/// is not playing, while the pointer is on them, and for a few seconds
/// after anything is touched.
fn controls_shown(ui: &Ui, playback: &Playback, controls: &[Rect]) -> f32 {
    let id = egui::Id::new("fullscreen-touched");
    let (now, touched) = ui.input(|input| {
        let touched = input.pointer.is_moving()
            || input.pointer.any_down()
            || input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Key { .. }));
        (input.time, touched)
    });
    let last: f64 = ui.data(|data| data.get_temp(id)).unwrap_or(now);
    let over = controls.iter().any(|rect| ui.rect_contains_pointer(*rect));
    let busy = touched || over || !playback.is_playing() || egui::Popup::is_any_open(ui.ctx());
    if busy || ui.data(|data| data.get_temp::<f64>(id)).is_none() {
        ui.data_mut(|data| data.insert_temp(id, now));
    }
    let idle = if busy { 0.0 } else { now - last };
    let visible = idle < IDLE_SECONDS;
    if visible && playback.is_playing() {
        // Nothing else may happen in the meantime to prompt the frame in
        // which they go.
        let left = (IDLE_SECONDS - idle).max(0.0);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left + 0.02));
    }
    let time = if widgets::still(ui.ctx()) {
        0.0
    } else {
        FADE_SECONDS
    };
    ui.ctx()
        .animate_bool_with_time(id.with("shown"), visible, time)
}

/// Darkens the cover where the text and the controls are, and hardly at
/// all in between, where the eye actually looks.
fn scrim(ui: &Ui, area: Rect) {
    let shade = |strength: f32| Color32::from_black_alpha((strength * 255.0) as u8);
    let at = |share: f32| area.top() + area.height() * share;
    let band = |from: f32, to: f32| {
        Rect::from_min_max(pos2(area.left(), at(from)), pos2(area.right(), at(to)))
    };
    widgets::vertical_gradient(ui, band(0.0, 0.22), shade(0.55), shade(0.15));
    ui.painter().rect_filled(band(0.22, 0.45), 0.0, shade(0.15));
    widgets::vertical_gradient(ui, band(0.45, 1.0), shade(0.15), shade(0.85));
}

/// Where the music is from, at the top left, and the way out at the right.
fn context(
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    palette: &Palette,
    why: Option<&str>,
    area: Rect,
) {
    let origin = playback.session.queue.origin.trim();
    let (label, name) = if origin.is_empty() {
        ("PLAYING", crate::APP_NAME)
    } else {
        ("PLAYING FROM", origin)
    };
    let left = area.left() + SIDE;
    let quiet = Color32::from_white_alpha(180);
    let room = (area.width() * 0.4, 1);
    let label = widgets::tracked(ui, label, theme::regular(11.0), quiet, 0.9, room);
    ui.painter()
        .galley(pos2(left, area.top() + TOP), label, quiet);
    let font = theme::semibold(12.0);
    let name = widgets::elided(ui, name, font, Color32::WHITE, room.0, 1);
    ui.painter()
        .galley(pos2(left, area.top() + TOP + 16.0), name, Color32::WHITE);
    if let Some(why) = why {
        let why = widgets::elided(ui, why, theme::regular(13.0), quiet, room.0, 2);
        ui.painter()
            .galley(pos2(left, area.top() + TOP + 40.0), why, quiet);
    }

    let close = widgets::IconButton {
        icon: Icon::X,
        size: 20.0,
        tooltip: "Exit full screen",
        active: false,
    };
    let at = pos2(area.right() - SIDE - 16.0, area.top() + TOP + 16.0);
    if close.show_at(ui, palette, at).clicked() {
        actions.push(Action::SetFullscreenPlayer(false));
    }
}

/// The song, the seek bar and the controls, up from the bottom edge.
fn foot(
    state: &State,
    palette: &Palette,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    area: Rect,
) {
    let (left, right) = (area.left() + SIDE, area.right() - SIDE);
    let controls_y = area.bottom() - BOTTOM - PLAY_DISC / 2.0;
    let progress_y = controls_y - PLAY_DISC / 2.0 - 16.0 - 8.0;
    let thumb_bottom = progress_y - 8.0 - 16.0;

    if let Some(track) = playback.current() {
        let thumb = Rect::from_min_size(pos2(left, thumb_bottom - THUMB), egui::Vec2::splat(THUMB));
        ui.painter().rect_filled(
            thumb.translate(vec2(0.0, 8.0)).expand(4.0),
            8.0,
            Color32::from_black_alpha(60),
        );
        let shape = ArtShape::Rounded(4);
        widgets::artwork(ui, state, &track.artwork, thumb, shape, Icon::Music);
        let text_left = thumb.right() + 16.0;
        let width = (right - text_left).max(40.0);
        let font = theme::bold(32.0);
        let title = widgets::elided(ui, &track.title, font, Color32::WHITE, width, 1);
        ui.painter()
            .galley(pos2(text_left, thumb.top() + 2.0), title, Color32::WHITE);
        credits(
            ui,
            actions,
            track,
            pos2(text_left, thumb.bottom() - 20.0),
            width,
        );
    }

    // The seek bar, with the time either side of it.
    let quiet = Color32::from_white_alpha(204);
    let bar = Rect::from_min_max(
        pos2(left + 52.0, progress_y - 8.0),
        pos2(right - 52.0, progress_y + 8.0),
    );
    let (shown, duration) = player_bar::seek_bar(ui, palette, actions, Some(playback), bar);
    widgets::text_at(
        ui,
        pos2(left + 20.0, progress_y),
        Align2::CENTER_CENTER,
        &format::duration(shown),
        theme::regular(11.0),
        quiet,
    );
    let end = player_bar::EndTime {
        at: pos2(right - 20.0, progress_y),
        shown,
        duration,
        color: quiet,
    };
    end.show(state, ui, actions);

    let share = widgets::IconButton {
        icon: Icon::Share,
        size: 18.0,
        tooltip: "Share",
        active: false,
    };
    if let Some(track) = playback.current()
        && share
            .show_at(ui, palette, pos2(left + 15.0, controls_y))
            .clicked()
    {
        actions.push(Action::Share {
            kind: crate::share::Kind::Track,
            id: track.id.clone(),
        });
    }
    transport(
        palette,
        ui,
        actions,
        playback,
        pos2(area.center().x, controls_y),
    );

    // From the right: the volume and its mute, then the speed.
    let slider = Rect::from_min_max(
        pos2(right - VOLUME_WIDTH, controls_y - 8.0),
        pos2(right, controls_y + 8.0),
    );
    let mute_at = pos2(slider.left() - 24.0, controls_y);
    let control = volume::Volume {
        bar: Some(slider),
        mute_at,
        icon: 18.0,
    };
    control.show(state, palette, ui, actions, Some(playback));
    // Leftwards from the volume: the video button, then the speed.
    video::switch(state, palette, ui, actions, mute_at - vec2(36.0, 0.0));
    speed::button(state, palette, ui, actions, mute_at - vec2(80.0, 0.0));
}

/// Who the song is by and the album it is on, each leading to its page.
fn credits(
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    track: &spotified_client::models::Track,
    at: Pos2,
    width: f32,
) {
    let color = Color32::from_white_alpha(199);
    let font = theme::regular(12.0);
    let album = track.album.as_ref().filter(|album| !album.name.is_empty());
    let artists = widgets::Artists {
        artists: &track.artists,
        font: font.clone(),
        color,
        width,
    };
    let (artist, drawn) = artists.show(ui, ui.id().with("fullscreen-artists"), at);
    if let Some(artist) = artist {
        // The page is under this view, which has to give way to it.
        actions.push(Action::SetFullscreenPlayer(false));
        actions.push(Action::Open(Page::Artist(artist)));
    }
    let mut left = at.x + drawn;
    let mut lead = |ui: &mut Ui, index: usize, text: &str, page: Option<Page>| {
        let link = widgets::Link {
            text,
            font: font.clone(),
            color,
            width: (at.x + width - left).max(0.0),
        };
        let id = ui.id().with(("fullscreen-credit", index));
        let (clicked, drawn) = link.show_measured(ui, id, pos2(left, at.y), page.is_some());
        left += drawn;
        if let (true, Some(page)) = (clicked, page) {
            actions.push(Action::SetFullscreenPlayer(false));
            actions.push(Action::Open(page));
        }
    };
    if let Some(album) = album {
        lead(ui, 1, " • ", None);
        let page = (!album.id.is_empty()).then(|| Page::Album(album.id.clone()));
        lead(ui, 2, &album.name, page);
    }
}

/// Shuffle, previous, play, next and repeat, centred on `centre`.
fn transport(
    palette: &Palette,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    centre: Pos2,
) {
    let wants_to_play = playback.wants_to_play();
    let disc = Rect::from_center_size(centre, egui::Vec2::splat(PLAY_DISC));
    let name = if wants_to_play { "Pause" } else { "Play" };
    let response = ui
        .interact(disc, ui.id().with("fullscreen-play"), Sense::click())
        .on_hover_text(name);
    widgets::name(ui, &response, name);
    let pressed = if response.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    ui.painter()
        .circle_filled(centre, PLAY_DISC / 2.0 * pressed, Color32::WHITE);
    let glyph = if wants_to_play {
        Icon::PauseFilled
    } else {
        Icon::PlayFilled
    };
    widgets::paint_icon(ui, glyph, disc, 22.0, Color32::BLACK);
    if response.clicked() {
        actions.push(Action::TogglePlay);
    }

    let repeat = playback.session.repeat;
    let repeat_icon = if repeat == Repeat::One {
        Icon::Repeat1
    } else {
        Icon::Repeat
    };
    // Outwards from the disc, mirrored left and right.
    let near = PLAY_DISC / 2.0 + GAP + 18.0;
    let far = near + 18.0 + GAP + 16.0;
    let buttons = [
        (
            Icon::SkipBackFilled,
            24.0,
            -near,
            "Previous",
            false,
            Action::Previous,
        ),
        (
            Icon::SkipForwardFilled,
            24.0,
            near,
            "Next",
            false,
            Action::Next,
        ),
        (
            Icon::Shuffle,
            20.0,
            -far,
            "Shuffle",
            playback.session.shuffle,
            Action::ToggleShuffle,
        ),
        (
            repeat_icon,
            20.0,
            far,
            "Repeat",
            repeat != Repeat::Off,
            Action::CycleRepeat,
        ),
    ];
    for (icon, size, offset, tooltip, active, action) in buttons {
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
