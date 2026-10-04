//! The bar along the bottom: what is playing, the transport, and volume.
//!
//! Laid out by hand in three zones, because the transport must stay centred
//! in the window whatever the side zones hold.

use std::time::Instant;

use eframe::egui::{self, Align2, Margin, Rect, Sense, Ui, pos2, vec2};
use spotified_client::session::Repeat;

use super::widgets::{self, ArtShape};
use super::{equalizer, format, speed, video, visualizer, volume};
use crate::actions::Action;
use crate::settings::RightPanel;
use crate::state::{Page, Playback, State};
use crate::theme::{self, Icon, Palette};

const PADDING: f32 = 16.0;
const GUTTERS: Margin = Margin {
    left: theme::GUTTER,
    right: theme::GUTTER,
    top: 0,
    bottom: theme::GUTTER,
};
const COVER: f32 = 56.0;
/// How much of the playing cover's colour the bar takes.
const BAR_TINT: f32 = 0.12;
const PLAY_DISC: f32 = 38.0;
const TRANSPORT_GAP: f32 = 12.0;
/// The buttons sit above the bar's midline and the progress row below it.
const TRANSPORT_RISE: f32 = 10.0;
const PROGRESS_DROP: f32 = 30.0;
const VOLUME_WIDTH: f32 = 96.0;
/// How far the transport's buttons reach either side of the bar's middle,
/// with a little room to spare.
const TRANSPORT_REACH: f32 = 150.0;
/// How far apart the middles of the buttons at the right are.
const EXTRA_STEP: f32 = 36.0;
/// The least the middle zone is, and the part of the bar it takes.
const CENTRE_LEAST: f32 = 320.0;
const CENTRE_SHARE: f32 = 0.38;
/// The room each of the two times has beside the seek bar.
const TIME_WIDTH: f32 = 40.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    egui::Panel::bottom("player-bar")
        .exact_size(theme::PLAYER_BAR_HEIGHT + f32::from(theme::GUTTER))
        .resizable(false)
        .show_separator_line(false)
        .frame(widgets::card(palette, GUTTERS).fill(bar_fill(state, ui)))
        .show(ui, |ui| {
            let bar = ui.max_rect();
            let color = bar_tint(state, ui).unwrap_or(palette.accent);
            if visualizer::show(state, ui, bar, color) {
                widgets::round_off(ui, bar, palette);
            }
            let inner = bar.shrink2(vec2(PADDING, 0.0));
            // The middle takes its share of the bar, and the two sides
            // what is left, evenly: the transport stays centred.
            let middle = (inner.width() * CENTRE_SHARE)
                .max(CENTRE_LEAST)
                .min(inner.width());
            let side = (inner.width() - middle) / 2.0 - PADDING;
            let left = Rect::from_min_size(inner.min, vec2(side.max(0.0), inner.height()));
            let right = Rect::from_min_max(pos2(inner.right() - side, inner.top()), inner.max);
            let centre = Rect::from_center_size(inner.center(), vec2(middle, inner.height()));
            let playback = state
                .playback
                .as_ref()
                .filter(|playback| playback.current().is_some());
            now_playing(state, ui, actions, playback, left);
            transport(state, ui, actions, playback, centre);
            extras(state, ui, actions, right);
        });
}

/// The colour the playing cover lends, once it has loaded.
fn bar_tint(state: &State, ui: &Ui) -> Option<egui::Color32> {
    state
        .playback
        .as_ref()
        .and_then(Playback::current)
        .and_then(|track| widgets::artwork_tint(ui, state, &track.artwork, COVER))
}

/// The bar's colour: the panel's, with a little of the playing cover's.
fn bar_fill(state: &State, ui: &Ui) -> egui::Color32 {
    match bar_tint(state, ui) {
        Some(tint) => crate::tint::blend(state.palette.panel, tint, BAR_TINT),
        None => state.palette.panel,
    }
}

fn now_playing(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    zone: Rect,
) {
    let palette = &state.palette;
    // With nothing playing the bar says so, and shows no empty cover.
    let Some(track) = playback.and_then(Playback::current) else {
        widgets::text_at(
            ui,
            zone.left_center(),
            Align2::LEFT_CENTER,
            "Nothing playing",
            theme::regular(12.0),
            palette.secondary,
        );
        return;
    };
    let cover = Rect::from_min_size(
        pos2(zone.left(), zone.center().y - COVER / 2.0),
        egui::Vec2::splat(COVER),
    );
    let left = cover.right() + 12.0;
    let shape = ArtShape::Rounded(4);
    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
    // The text leaves room for the heart and the share button after it.
    let width = (zone.right() - left - 76.0).max(20.0);
    // The title leads to the album and the artists to the artist.
    let album_page = track
        .album
        .as_ref()
        .filter(|album| !album.id.is_empty())
        .map(|album| Page::Album(album.id.clone()));
    let title = widgets::Link {
        text: &track.title,
        font: theme::medium(14.0),
        color: palette.text,
        width,
    };
    let at = pos2(left, zone.center().y - 18.0);
    let id = ui.id().with(("now-playing", 0));
    let (clicked, title_width) = title.show_measured(ui, id, at, album_page.is_some());
    if let (true, Some(page)) = (clicked, album_page) {
        actions.push(Action::Open(page));
    }
    let artists = widgets::Artists {
        artists: &track.artists,
        font: theme::regular(12.0),
        color: palette.secondary,
        width,
    };
    let at = pos2(left, zone.center().y + 2.0);
    let (artist, artists_width) = artists.show(ui, ui.id().with(("now-playing", 1)), at);
    if let Some(artist) = artist {
        actions.push(Action::Open(Page::Artist(artist)));
    }
    let text_width = title_width.max(artists_width);

    let liked = state.likes.is_liked(&track.id);
    let heart = widgets::IconButton {
        icon: if liked {
            Icon::HeartFilled
        } else {
            Icon::Heart
        },
        size: 17.0,
        tooltip: if liked {
            "Remove from Liked Songs"
        } else {
            "Save to Liked Songs"
        },
        active: liked,
    };
    let at = pos2(left + text_width + 12.0 + 16.0, zone.center().y);
    if heart.show_at(ui, palette, at).clicked() {
        actions.push(Action::ToggleLike(track.clone()));
    }
    let share = widgets::IconButton {
        icon: Icon::Share,
        size: 17.0,
        tooltip: "Share",
        active: false,
    };
    if share.show_at(ui, palette, at + vec2(32.0, 0.0)).clicked() {
        actions.push(Action::Share {
            kind: crate::share::Kind::Track,
            id: track.id.clone(),
        });
    }
}

/// The transport, centred in `zone`: the buttons above, the seek bar and
/// its times below.
pub(super) fn transport(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    zone: Rect,
) {
    let palette = &state.palette;
    let row_y = zone.center().y - TRANSPORT_RISE;
    let centre = pos2(zone.center().x, row_y);
    // With nothing queued there is nothing to play, skip or seek in, so
    // those are drawn in their resting, disabled state. Shuffle and repeat
    // are the session's, and can be set before anything plays.
    let has_track = playback.is_some();
    ui.add_enabled_ui(has_track, |ui| {
        play_disc(ui, palette, actions, playback, centre);
    });

    let shuffle_on = playback.is_some_and(|playback| playback.session.shuffle);
    let repeat = playback.map_or(Repeat::Off, |playback| playback.session.repeat);
    let repeat_icon = if repeat == Repeat::One {
        Icon::Repeat1
    } else {
        Icon::Repeat
    };
    // Outwards from the disc, mirrored left and right.
    let near = PLAY_DISC / 2.0 + TRANSPORT_GAP + 16.0;
    let far = near + 32.0 + TRANSPORT_GAP;
    let session = state.playback.is_some();
    let buttons = [
        (
            Icon::SkipBackFilled,
            18.0,
            -near,
            "Previous",
            (false, has_track),
            Action::Previous,
        ),
        (
            Icon::SkipForwardFilled,
            18.0,
            near,
            "Next",
            (false, has_track),
            Action::Next,
        ),
        (
            Icon::Shuffle,
            18.0,
            -far,
            "Shuffle",
            (shuffle_on, session),
            Action::ToggleShuffle,
        ),
        (
            repeat_icon,
            18.0,
            far,
            "Repeat",
            (repeat != Repeat::Off, session),
            Action::CycleRepeat,
        ),
    ];
    for (icon, size, offset, tooltip, (active, enabled), action) in buttons {
        let at = pos2(centre.x + offset, row_y);
        let button = widgets::IconButton {
            icon,
            size,
            tooltip,
            active,
        };
        ui.add_enabled_ui(enabled, |ui| {
            if button.show_at(ui, palette, at).clicked() {
                actions.push(action);
            }
        });
    }

    progress(state, ui, actions, playback, zone, row_y + PROGRESS_DROP);
}

/// The round play and pause button, centred on `centre`.
pub(super) fn play_disc(
    ui: &mut Ui,
    palette: &Palette,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    centre: egui::Pos2,
) {
    let wants_to_play = playback.is_some_and(Playback::wants_to_play);
    let disc = Rect::from_center_size(centre, egui::Vec2::splat(PLAY_DISC));
    let disc_response = ui
        .interact(disc, ui.id().with("play"), Sense::click())
        .on_hover_text(if wants_to_play { "Pause" } else { "Play" });
    widgets::name(
        ui,
        &disc_response,
        if wants_to_play { "Pause" } else { "Play" },
    );
    let disc_fill = if playback.is_some() {
        palette.text
    } else {
        palette.dim
    };
    let pressed = if disc_response.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    ui.painter()
        .circle_filled(centre, PLAY_DISC / 2.0 * pressed, disc_fill);
    let glyph = if wants_to_play {
        Icon::PauseFilled
    } else {
        Icon::PlayFilled
    };
    widgets::paint_icon(ui, glyph, disc, 16.0, palette.window);
    if disc_response.clicked() {
        actions.push(Action::TogglePlay);
    }
}

fn progress(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    zone: Rect,
    y: f32,
) {
    let palette = &state.palette;
    // A time either side of the bar, each in a place of its own width.
    let beside = TIME_WIDTH + 12.0;
    let width = (zone.width() - beside * 2.0).max(80.0);
    let bar = Rect::from_center_size(pos2(zone.center().x, y), vec2(width, 16.0));
    // With nothing playing there is nothing to seek in.
    let (shown, duration) = ui
        .add_enabled_ui(playback.is_some(), |ui| {
            seek_bar(ui, palette, actions, playback, bar)
        })
        .inner;
    let offset = 12.0 + TIME_WIDTH / 2.0;
    // With nothing playing there is no time to show.
    let Some(_) = playback else {
        for x in [bar.left() - offset, bar.right() + offset] {
            widgets::text_at(
                ui,
                pos2(x, y),
                Align2::CENTER_CENTER,
                format::NO_TIME,
                theme::regular(11.0),
                palette.secondary,
            );
        }
        return;
    };
    widgets::text_at(
        ui,
        pos2(bar.left() - offset, y),
        Align2::CENTER_CENTER,
        &format::duration(shown),
        theme::regular(11.0),
        palette.secondary,
    );
    let end = EndTime {
        at: pos2(bar.right() + offset, y),
        shown,
        duration,
        color: palette.secondary,
    };
    end.show(state, ui, actions);
}

/// The time at the end of a seek bar: the song's length, or, once clicked,
/// how much of it is left. Clicked again it is the length once more.
pub(super) struct EndTime {
    /// Where its middle goes.
    pub at: egui::Pos2,
    /// The position the seek bar shows, and the song's length.
    pub shown: u64,
    pub duration: u64,
    pub color: egui::Color32,
}

impl EndTime {
    pub(super) fn show(&self, state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
        let remaining = state.settings.remaining_time;
        let text = format::end_time(remaining, self.shown, self.duration);
        let font = theme::regular(11.0);
        let galley = ui.painter().layout_no_wrap(text, font, self.color);
        let rect = Rect::from_center_size(self.at, galley.size());
        let id = ui.id().with("end-time");
        let response = ui.interact(rect.expand2(vec2(4.0, 4.0)), id, Sense::click());
        let name = if remaining {
            "Show total duration"
        } else {
            "Show remaining time"
        };
        widgets::name(ui, &response, name);
        ui.painter().galley(rect.min, galley, self.color);
        if response.on_hover_text(name).clicked() {
            actions.push(Action::ToggleRemainingTime);
        }
    }
}

/// The bar that shows how far the track has got, and seeks when dragged.
/// Returns the position it shows and the track's length, in milliseconds.
pub(super) fn seek_bar(
    ui: &mut Ui,
    palette: &Palette,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    bar: Rect,
) -> (u64, u64) {
    let duration = playback
        .and_then(Playback::current)
        .map_or(0, |track| track.duration_ms);
    let position = playback.map_or(0, |playback| playback.position_ms(Instant::now()));
    let fraction = if duration > 0 {
        position as f32 / duration as f32
    } else {
        0.0
    };
    // While the knob is held it shows where the drag is, not where the
    // music is; the seek is sent when it is let go.
    let slider = widgets::slider(ui, palette, bar, fraction, "seek");
    let shown = slider
        .dragging
        .map_or(position, |fraction| (fraction * duration as f32) as u64);
    if let Some(fraction) = slider.released {
        actions.push(Action::Seek((fraction * duration as f32) as u64));
    }
    (shown, duration)
}

fn extras(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, zone: Rect) {
    let palette = &state.palette;
    let y = zone.center().y;
    // From the right: the mini player, the volume and its mute, the
    // video button, the playback speed, the queue, the lyrics and the
    // equalizer.
    let mini_at = pos2(zone.right() - 16.0, y);
    let bar_right = zone.right() - 36.0;
    let bar = Rect::from_min_max(
        pos2(bar_right - VOLUME_WIDTH, y - 8.0),
        pos2(bar_right, y + 8.0),
    );
    let mute_at = pos2(bar.left() - 24.0, y);
    let video_at = mute_at - vec2(EXTRA_STEP, 0.0);
    let speed_at = video_at - vec2(EXTRA_STEP, 0.0);
    let queue_at = speed_at - vec2(EXTRA_STEP, 0.0);
    let lyrics_at = queue_at - vec2(EXTRA_STEP, 0.0);
    let equalizer_at = lyrics_at - vec2(EXTRA_STEP, 0.0);
    // The last in, and the one that goes where a narrow window would put
    // it on the transport: the queue's heading has the same button.
    let transport_ends = ui.max_rect().center().x + TRANSPORT_REACH;
    if equalizer_at.x - EXTRA_STEP / 2.0 >= transport_ends {
        equalizer::button_at(state, ui, actions, equalizer_at);
    }
    let control = volume::Volume {
        bar: Some(bar),
        mute_at,
        icon: 18.0,
    };
    control.show(state, palette, ui, actions, state.playback.as_ref());
    speed::button(state, palette, ui, actions, speed_at);
    // Only with something playing, as the Electron app had it.
    if state
        .playback
        .as_ref()
        .and_then(Playback::current)
        .is_some()
    {
        video::switch(state, palette, ui, actions, video_at);
    }
    let queue = widgets::IconButton {
        icon: Icon::ListMusic,
        size: 18.0,
        tooltip: "Queue",
        active: state.settings.panel == RightPanel::Queue,
    };
    if queue.show_at(ui, palette, queue_at).clicked() {
        actions.push(Action::ToggleQueue);
    }
    let lyrics = widgets::IconButton {
        icon: Icon::MicVocal,
        size: 18.0,
        tooltip: "Lyrics",
        active: state.settings.panel == RightPanel::Lyrics,
    };
    if lyrics.show_at(ui, palette, lyrics_at).clicked() {
        actions.push(Action::ToggleLyrics);
    }
    let mini = widgets::IconButton {
        icon: Icon::PictureInPicture,
        size: 18.0,
        tooltip: "Mini player",
        active: state.mini_player,
    };
    if mini.show_at(ui, palette, mini_at).clicked() {
        actions.push(Action::ToggleMiniPlayer);
    }
}
