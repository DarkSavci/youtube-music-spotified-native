//! The bar along the bottom: what is playing, the transport, and volume.
//!
//! Laid out by hand in three zones, because the transport must stay centred
//! in the window whatever the side zones hold.

use std::time::Instant;

use eframe::egui::{self, Align2, Margin, Rect, Sense, Ui, pos2, vec2};
use spotified_client::session::Repeat;

use super::widgets::{self, ArtShape};
use super::{format, visualizer};
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
const PLAY_DISC: f32 = 36.0;
const TRANSPORT_GAP: f32 = 10.0;
/// The buttons sit above the bar's midline and the progress row below it.
const TRANSPORT_RISE: f32 = 9.0;
const PROGRESS_DROP: f32 = 29.0;
const VOLUME_WIDTH: f32 = 92.0;
/// The width of the right-hand zone at which the mini player's button fits.
const MINI_BUTTON_NEEDS: f32 = 244.0;

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
            let side = (inner.width() * 0.30).clamp(200.0, 420.0);
            let left = Rect::from_min_size(inner.min, vec2(side, inner.height()));
            let right = Rect::from_min_max(pos2(inner.right() - side, inner.top()), inner.max);
            let centre = Rect::from_min_max(left.right_top(), right.left_bottom());
            // With nothing queued there is nothing for the controls to act
            // on, so they are drawn in their resting, disabled state.
            let playback = state
                .playback
                .as_ref()
                .filter(|playback| playback.current().is_some());
            ui.add_enabled_ui(playback.is_some(), |ui| {
                now_playing(state, ui, actions, playback, left);
                transport(state, ui, actions, playback, centre);
                extras(state, ui, actions, right);
            });
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
    let cover = Rect::from_min_size(
        pos2(zone.left() + 4.0, zone.center().y - COVER / 2.0),
        egui::Vec2::splat(COVER),
    );
    let left = cover.right() + 12.0;
    let Some(track) = playback.and_then(Playback::current) else {
        ui.painter()
            .rect_filled(cover, theme::RADIUS_ROW, palette.surface_hover);
        widgets::paint_icon(ui, Icon::Music, cover, 24.0, palette.dim);
        widgets::text_at(
            ui,
            pos2(left, zone.center().y),
            Align2::LEFT_CENTER,
            "Nothing playing",
            theme::medium(14.0),
            palette.dim,
        );
        return;
    };
    let shape = ArtShape::Rounded(theme::RADIUS_ROW);
    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
    // The text leaves room for the heart that follows it.
    let width = (zone.right() - left - 44.0).max(20.0);
    // The title leads to the album and the artists to the artist.
    let album_page = track
        .album
        .as_ref()
        .filter(|album| !album.id.is_empty())
        .map(|album| Page::Album(album.id.clone()));
    let artist_page = track
        .artists
        .iter()
        .find(|artist| !artist.id.is_empty())
        .map(|artist| Page::Artist(artist.id.clone()));
    let lines = [
        (
            &track.title,
            theme::medium(14.0),
            palette.text,
            -18.0,
            album_page,
        ),
        (
            &track.artist_names(),
            theme::regular(12.0),
            palette.secondary,
            2.0,
            artist_page,
        ),
    ];
    let mut text_width = 0.0f32;
    for (index, (text, font, color, offset, page)) in lines.into_iter().enumerate() {
        let link = widgets::Link {
            text,
            font,
            color,
            width,
        };
        let at = pos2(left, zone.center().y + offset);
        let id = ui.id().with(("now-playing", index));
        let (clicked, drawn) = link.show_measured(ui, id, at, page.is_some());
        text_width = text_width.max(drawn);
        if let (true, Some(page)) = (clicked, page) {
            actions.push(Action::Open(page));
        }
    }

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
    let at = pos2(left + text_width + 21.0, zone.center().y);
    if heart.show_at(ui, palette, at).clicked() {
        actions.push(Action::ToggleLike(track.clone()));
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
    play_disc(ui, palette, actions, playback, centre);

    let shuffle_on = playback.is_some_and(|playback| playback.session.shuffle);
    let repeat = playback.map_or(Repeat::Off, |playback| playback.session.repeat);
    let repeat_icon = if repeat == Repeat::One {
        Icon::Repeat1
    } else {
        Icon::Repeat
    };
    // Outwards from the disc, mirrored left and right.
    let near = PLAY_DISC / 2.0 + TRANSPORT_GAP + 15.0;
    let far = near + 18.0 + 12.0 + TRANSPORT_GAP;
    let buttons = [
        (
            Icon::SkipBackFilled,
            18.0,
            -near,
            "Previous",
            false,
            Action::Previous,
        ),
        (
            Icon::SkipForwardFilled,
            18.0,
            near,
            "Next",
            false,
            Action::Next,
        ),
        (
            Icon::Shuffle,
            17.0,
            -far,
            "Shuffle",
            shuffle_on,
            Action::ToggleShuffle,
        ),
        (
            repeat_icon,
            17.0,
            far,
            "Repeat",
            repeat != Repeat::Off,
            Action::CycleRepeat,
        ),
    ];
    for (icon, size, offset, tooltip, active, action) in buttons {
        let at = pos2(centre.x + offset, row_y);
        let button = widgets::IconButton {
            icon,
            size,
            tooltip,
            active,
        };
        if button.show_at(ui, palette, at).clicked() {
            actions.push(action);
        }
    }

    progress(ui, palette, actions, playback, zone, row_y + PROGRESS_DROP);
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
    ui: &mut Ui,
    palette: &Palette,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    zone: Rect,
    y: f32,
) {
    let width = (zone.width() - 120.0).clamp(120.0, 620.0);
    let bar = Rect::from_center_size(pos2(zone.center().x, y), vec2(width, 16.0));
    let (shown, duration) = seek_bar(ui, palette, actions, playback, bar);
    for (x, anchor, ms) in [
        (bar.left() - 10.0, Align2::RIGHT_CENTER, shown),
        (bar.right() + 10.0, Align2::LEFT_CENTER, duration),
    ] {
        widgets::text_at(
            ui,
            pos2(x, y),
            anchor,
            &format::duration(ms),
            theme::regular(11.5),
            palette.secondary,
        );
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
    let volume = state
        .playback
        .as_ref()
        .map_or(0.0, |playback| playback.session.volume);
    let bar = Rect::from_min_max(
        pos2(zone.right() - VOLUME_WIDTH, y - 8.0),
        pos2(zone.right(), y + 8.0),
    );
    // Volume follows the drag as it happens: it is heard, not just seen.
    let slider = widgets::slider(ui, palette, bar, volume.min(1.0), "volume");
    if let Some(level) = slider.dragging.or(slider.released)
        && (level - volume).abs() >= 0.01
    {
        actions.push(Action::SetVolume(level));
    }
    let icon = match volume {
        v if v <= 0.0 => Icon::VolumeX,
        v if v < 0.5 => Icon::Volume1,
        _ => Icon::Volume2,
    };
    let mute = widgets::IconButton {
        icon,
        size: 18.0,
        tooltip: "Mute",
        active: false,
    };
    if mute
        .show_at(ui, palette, pos2(bar.left() - 21.0, y))
        .clicked()
    {
        actions.push(Action::ToggleMute);
    }
    let queue = widgets::IconButton {
        icon: Icon::ListMusic,
        size: 18.0,
        tooltip: "Queue",
        active: state.settings.panel == RightPanel::Queue,
    };
    if queue
        .show_at(ui, palette, pos2(bar.left() - 57.0, y))
        .clicked()
    {
        actions.push(Action::ToggleQueue);
    }
    let lyrics = widgets::IconButton {
        icon: Icon::MicVocal,
        size: 18.0,
        tooltip: "Lyrics",
        active: state.settings.panel == RightPanel::Lyrics,
    };
    if lyrics
        .show_at(ui, palette, pos2(bar.left() - 93.0, y))
        .clicked()
    {
        actions.push(Action::ToggleLyrics);
    }
    // The narrowest bar has no room for a fourth button; the shortcut and
    // the tray still lead there.
    if zone.width() < MINI_BUTTON_NEEDS {
        return;
    }
    let mini = widgets::IconButton {
        icon: Icon::Shrink,
        size: 17.0,
        tooltip: "Mini player",
        active: false,
    };
    if mini
        .show_at(ui, palette, pos2(bar.left() - 129.0, y))
        .clicked()
    {
        actions.push(Action::ToggleMiniPlayer);
    }
}
