//! The music video: the button that switches between a song and its
//! video, and the surface the picture is drawn on.
//!
//! One picture is drawn wherever a surface is on screen: above the page,
//! in the full-screen player, in the mini player. The sound is the audio
//! engine's throughout; the picture only follows it.

use eframe::egui::{
    self, Align, Align2, Color32, Layout, Pos2, Rect, Sense, Ui, UiBuilder, Vec2, pos2, vec2,
};

use super::widgets::{self, ArtShape};
use crate::actions::{Action, VideoAsk};
use crate::state::video::{self, Availability};
use crate::state::{Playback, State};
use crate::theme::{self, Icon, Palette};

/// The most of the window's height the picture above the page takes, and
/// the most it is in points: as the Electron app had it.
const MAIN_SHARE: f32 = 0.4;
const MAIN_MOST: f32 = 420.0;
/// How strongly a disabled video button shows.
const BLOCKED: f32 = 0.38;

/// How tall the picture above the page is, in a window this tall.
pub fn main_height(window: f32) -> f32 {
    (window * MAIN_SHARE).min(MAIN_MOST)
}

/// The song/video button: one icon, lit while the video shows.
pub fn switch(state: &State, palette: &Palette, ui: &mut Ui, actions: &mut Vec<Action>, at: Pos2) {
    let playback = state.playback.as_ref();
    let track = playback.and_then(Playback::current);
    let following = playback.is_some_and(|playback| playback.following_room);
    let together = &state.together;
    let controls = together
        .room
        .as_ref()
        .is_some_and(|room| room.may_control(&together.me));
    let control = video::control(&state.video, track, following, controls);
    let button = widgets::IconButton {
        icon: Icon::Video,
        size: 18.0,
        tooltip: control.label,
        active: state.video.enabled,
    };
    let response = ui
        .scope(|ui| {
            if control.blocked {
                ui.set_opacity(BLOCKED);
            }
            button.show_at(ui, palette, at)
        })
        .inner;
    // Whether the song has a video is asked when the pointer gets here,
    // not for every song that plays.
    let unknown = state.video.availability == Availability::Unknown;
    if response.hovered() && !state.video.enabled && unknown && track.is_some() {
        actions.push(Action::Video(VideoAsk::Check));
    }
    if response.clicked() && !control.blocked {
        actions.push(Action::Video(VideoAsk::Set(!state.video.enabled)));
    }
}

/// Why the video is not showing, when it was asked for and could not be.
pub fn notice(state: &State) -> Option<&str> {
    if state.video.enabled {
        return None;
    }
    state.video.error.as_deref()
}

/// The picture, fitted whole into `rect` on black, or the cover until
/// there is one. With `expand`, a button at the corner gives it the screen.
pub fn surface(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, rect: Rect, expand: bool) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
    // Said so the decoder runs: it stops when no surface has been drawn.
    state
        .video
        .watch((rect.height() * ui.pixels_per_point()).round() as u32);
    match state.video.picture {
        Some((texture, size)) => {
            let fitted = Rect::from_center_size(rect.center(), fit(size, rect.size()));
            egui::Image::from_texture(egui::load::SizedTexture::new(texture, fitted.size()))
                .paint_at(ui, fitted);
        }
        None => cover(state, ui, rect),
    }
    if let Some(error) = &state.video.error {
        failed(state, ui, actions, rect, error);
    } else if state.video.loading {
        veil(ui, rect);
        said(ui, rect.center(), "Loading video…", rect.width() - 48.0);
    }
    if expand {
        let at = pos2(rect.right() - 28.0, rect.bottom() - 28.0);
        ui.painter()
            .circle_filled(at, 17.0, Color32::from_black_alpha(170));
        let button = widgets::IconButton {
            icon: Icon::Expand,
            size: 18.0,
            tooltip: "Watch video full screen",
            active: false,
        };
        if button
            .show_at(ui, &on_picture(&state.palette), at)
            .clicked()
        {
            actions.push(Action::SetFullscreenPlayer(true));
        }
    }
}

/// The colours of a control that lies on the picture, whatever the theme.
fn on_picture(palette: &Palette) -> Palette {
    Palette {
        text: Color32::WHITE,
        secondary: Color32::from_white_alpha(210),
        ..*palette
    }
}

/// The size at which a picture of `size` fits whole inside `room`.
pub fn fit(size: Vec2, room: Vec2) -> Vec2 {
    if size.x <= 0.0 || size.y <= 0.0 {
        return room;
    }
    size * (room.x / size.x).min(room.y / size.y)
}

/// The song's cover, whole, where the picture will be.
fn cover(state: &State, ui: &Ui, rect: Rect) {
    let Some(track) = state.playback.as_ref().and_then(Playback::current) else {
        return;
    };
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));
    let shape = ArtShape::Rounded(0);
    widgets::artwork(ui, state, &track.artwork, square, shape, Icon::Music);
}

fn veil(ui: &Ui, rect: Rect) {
    ui.painter()
        .rect_filled(rect, 0.0, Color32::from_black_alpha(170));
}

fn said(ui: &Ui, centre: Pos2, text: &str, width: f32) {
    let font = theme::regular(13.0);
    let galley = widgets::elided(ui, text, font, Color32::WHITE, width.max(40.0), 3);
    let at = centre - galley.size() / 2.0;
    ui.painter().galley(at, galley, Color32::WHITE);
}

/// The picture could not be had: why, and the way to try again.
fn failed(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, rect: Rect, error: &str) {
    veil(ui, rect);
    let room = rect.shrink(24.0);
    if room.height() < 60.0 {
        said(ui, rect.center(), error, rect.width() - 16.0);
        return;
    }
    said(ui, room.center() - vec2(0.0, 18.0), error, room.width());
    let row = Rect::from_center_size(room.center() + vec2(0.0, 22.0), vec2(room.width(), 30.0));
    let layout = Layout::top_down(Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(row).layout(layout), |ui| {
        let chip = widgets::chip(ui, &on_picture(&state.palette), "Retry video", false);
        if chip.clicked() {
            actions.push(Action::Video(VideoAsk::Retry));
        }
    });
}

/// The line above the page that says why there is no video.
pub fn notice_line(state: &State, ui: &mut Ui) {
    let Some(text) = notice(state) else {
        return;
    };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
    widgets::text_at(
        ui,
        pos2(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        theme::regular(13.0),
        state.palette.secondary,
    );
}
