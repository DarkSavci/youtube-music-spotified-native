//! The skin's playlist window, joined to the bottom of the others.
//!
//! `pledit.bmp` is its frame and `pledit.txt` its colours. It lists the
//! queue: a click selects a row (with Ctrl or Shift, several), a double
//! click plays from it, and the corner stretches the window a row of tiles
//! at a time. The buttons along the bottom open menus as Winamp's did;
//! Winamp's dealt in files, and these in what a queue has instead.

mod menus;

use std::collections::BTreeSet;

use eframe::egui::{self, Align2, Color32, Id, Sense};

use super::super::super::format;
use super::{View, play, stop, times};
use crate::actions::Action;
use crate::skin::font;
use crate::skin::layout::{self, Area};
use crate::skin::sprites;
use crate::skins::Ask;
use crate::state::{Playback, State};
use crate::theme;

/// What the list remembers between frames.
#[derive(Clone, Default)]
struct Kept {
    /// The row at the top of the list.
    top: usize,
    selected: BTreeSet<usize>,
    /// The row a Shift click reaches from: the last one clicked plainly.
    anchor: Option<usize>,
    /// The song that was playing when the list was last drawn: when it
    /// changes, the list follows it.
    playing: Option<usize>,
    /// How far the corner has been dragged since the height last changed,
    /// in skin pixels.
    stretched: f32,
}

/// The list's own area, between the frame's tiles.
fn list_area(height: u32) -> Area {
    Area::new(
        layout::PLAYLIST_LEFT_WIDTH,
        layout::PLAYLIST_TITLE_HEIGHT,
        layout::WINDOW_WIDTH - layout::PLAYLIST_LEFT_WIDTH - layout::PLAYLIST_RIGHT_WIDTH,
        height - layout::PLAYLIST_TITLE_HEIGHT - layout::PLAYLIST_BOTTOM_HEIGHT,
    )
}

fn rows_visible(height: u32) -> usize {
    (list_area(height).height / layout::PLAYLIST_TRACK_HEIGHT) as usize
}

/// The row that should be at the top, so that `playing` can be seen.
fn following(top: usize, playing: usize, visible: usize) -> usize {
    if playing < top {
        playing
    } else if playing >= top + visible {
        playing + 1 - visible.max(1)
    } else {
        top
    }
}

/// What a click on `row` leaves selected: that row alone, or with Ctrl the
/// selection with that row put in or taken out, or with Shift everything
/// from the anchor to it.
fn clicked(kept: &mut Kept, row: usize, modifiers: egui::Modifiers) {
    if modifiers.shift
        && let Some(anchor) = kept.anchor
    {
        kept.selected = (anchor.min(row)..=anchor.max(row)).collect();
        return;
    }
    if modifiers.command {
        if !kept.selected.remove(&row) {
            kept.selected.insert(row);
        }
    } else {
        kept.selected = BTreeSet::from([row]);
    }
    kept.anchor = Some(row);
}

/// A song as the list names it: its place, its artists, its title.
fn label(index: usize, artists: &str, title: &str) -> String {
    if artists.is_empty() {
        format!("{}. {title}", index + 1)
    } else {
        format!("{}. {artists} - {title}", index + 1)
    }
}

pub(super) fn show(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) {
    if state.settings.skin_playlist_shaded {
        return shade(view, actions, focused, playback);
    }
    let height = layout::playlist_height(state.settings.skin_playlist_height);
    frame(view, height, focused);
    let title = Area::new(0, 0, layout::WINDOW_WIDTH, layout::PLAYLIST_TITLE_HEIGHT);
    if view.title_bar(title, "Playlist title bar").double_clicked() {
        actions.push(Action::Skin(Ask::TogglePlaylistShade));
    }
    if view
        .lamp_button(
            layout::PLAYLIST_SHADE,
            sprites::PLAYLIST_SHADE_PRESSED,
            false,
            "Roll the playlist up",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::TogglePlaylistShade));
    }
    if view
        .lamp_button(
            layout::PLAYLIST_CLOSE,
            sprites::PLAYLIST_CLOSE_PRESSED,
            false,
            "Close the playlist",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::TogglePlaylist));
    }

    let id = Id::new("skin-playlist");
    let mut kept: Kept = view.ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let queue = playback.map(|playback| &playback.session.queue);
    let rows = queue.map_or(0, |queue| queue.items.len());
    let visible = rows_visible(height);
    let playing = queue.map(|queue| queue.index).filter(|_| rows > 0);
    if playing != kept.playing {
        kept.playing = playing;
        if let Some(playing) = playing {
            kept.top = following(kept.top, playing, visible);
        }
    }
    kept.selected.retain(|row| *row < rows);
    kept.top = kept.top.min(rows.saturating_sub(visible));

    list(view, actions, playback, &mut kept, height);
    scrollbar(view, &mut kept, rows, height);
    grip(view, actions, &mut kept, height);
    foot(view, actions, playback, &kept, height);
    menus::show(state, view, actions, playback, &mut kept, height);
    view.ui.data_mut(|data| data.insert_temp(id, kept));
}

/// The playlist rolled up: a bar that names the song and says how far it
/// has got.
fn shade(
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) {
    use sprites::*;
    let (width, height) = (layout::WINDOW_WIDTH, layout::PLAYLIST_SHADE_HEIGHT);
    view.sprite_at(PLAYLIST_SHADE_LEFT, 0, 0);
    let run = Area::new(25, 0, width - 75, height);
    for x in (25..width - 50).step_by(25) {
        view.sprite_clipped(PLAYLIST_SHADE_TILE, x, 0, run);
    }
    let right = if focused {
        PLAYLIST_SHADE_RIGHT_ACTIVE
    } else {
        PLAYLIST_SHADE_RIGHT
    };
    view.sprite_at(right, width - 50, 0);
    let bar = Area::new(0, 0, width, height);
    if view.title_bar(bar, "Playlist title bar").double_clicked() {
        actions.push(Action::Skin(Ask::TogglePlaylistShade));
    }
    if let Some(playback) = playback
        && let Some(track) = playback.current()
    {
        let (position, _) = times(view, playback);
        let time = format::duration(position);
        let time_width = 5 * time.len() as u32;
        let time_x = width - 30 - time_width;
        view.text(&time, Area::new(time_x, 4, time_width, 6));
        let index = playback.session.queue.index;
        let name = label(index, &track.artist_names(), &track.title);
        let room = Area::new(5, 4, time_x - 10, 6);
        if name.chars().all(font::covered) {
            view.text(&name, room);
        } else {
            // What the skin's font cannot say is set in the app's type.
            let rect = view.rect(room);
            let [red, green, blue] = view.skin().playlist.normal;
            let color = Color32::from_rgb(red, green, blue);
            let face = theme::medium((room.height as f32 + 2.0) * view.unit);
            let clip = rect.expand2(egui::vec2(0.0, 2.0 * view.unit));
            let at = egui::pos2(rect.left(), rect.center().y);
            view.ui
                .painter()
                .with_clip_rect(clip.intersect(view.ui.clip_rect()))
                .text(at, Align2::LEFT_CENTER, name, face, color);
        }
    }
    if view
        .lamp_button(
            layout::PLAYLIST_SHADE,
            PLAYLIST_UNSHADE_PRESSED,
            false,
            "Roll the playlist down",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::TogglePlaylistShade));
    }
    if view
        .lamp_button(
            layout::PLAYLIST_CLOSE,
            PLAYLIST_CLOSE_PRESSED,
            false,
            "Close the playlist",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::TogglePlaylist));
    }
}

fn frame(view: &mut View<'_>, height: u32, focused: bool) {
    use sprites::*;
    let (top_left, title, top_tile, top_right) = if focused {
        (
            PLAYLIST_TOP_LEFT_ACTIVE,
            PLAYLIST_TITLE_ACTIVE,
            PLAYLIST_TOP_TILE_ACTIVE,
            PLAYLIST_TOP_RIGHT_ACTIVE,
        )
    } else {
        (
            PLAYLIST_TOP_LEFT,
            PLAYLIST_TITLE,
            PLAYLIST_TOP_TILE,
            PLAYLIST_TOP_RIGHT,
        )
    };
    // The title sits centred; the tiles either side of it run out to the
    // corners, the last on each side cut to fit, as Winamp cut them.
    let tile = layout::PLAYLIST_TILE_WIDTH;
    let width = layout::WINDOW_WIDTH;
    let inner = width - 2 * tile - 100;
    let (left, right) = (inner / 2, inner - inner / 2);
    view.sprite_at(top_left, 0, 0);
    for (from, run) in [(tile, left), (tile + left + 100, right)] {
        let clip = Area::new(from, 0, run, layout::PLAYLIST_TITLE_HEIGHT);
        for x in (from..from + run).step_by(tile as usize) {
            view.sprite_clipped(top_tile, x, 0, clip);
        }
    }
    view.sprite_at(title, tile + left, 0);
    view.sprite_at(top_right, width - tile, 0);

    let middle = Area::new(
        0,
        layout::PLAYLIST_TITLE_HEIGHT,
        width,
        height - layout::PLAYLIST_TITLE_HEIGHT - layout::PLAYLIST_BOTTOM_HEIGHT,
    );
    for y in (middle.y..middle.y + middle.height).step_by(layout::PLAYLIST_TILE_HEIGHT as usize) {
        view.sprite_clipped(PLAYLIST_LEFT_TILE, 0, y, middle);
        let x = width - layout::PLAYLIST_RIGHT_WIDTH;
        view.sprite_clipped(PLAYLIST_RIGHT_TILE, x, y, middle);
    }
    let bottom = height - layout::PLAYLIST_BOTTOM_HEIGHT;
    view.sprite_at(PLAYLIST_BOTTOM_LEFT, 0, bottom);
    view.sprite_at(PLAYLIST_BOTTOM_RIGHT, 125, bottom);
}

/// The rows: number, artists and title at the left, the length at the
/// right, in the playlist's own colours and the app's type.
fn list(
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    kept: &mut Kept,
    height: u32,
) {
    let area = list_area(height);
    let style = view.skin().playlist.clone();
    let rgb = |[red, green, blue]: [u8; 3]| Color32::from_rgb(red, green, blue);
    view.fill(area, rgb(style.normal_background));
    let Some(queue) = playback.map(|playback| &playback.session.queue) else {
        return;
    };
    let visible = rows_visible(height);
    // The wheel moves the list a few rows at a time.
    let whole = view.interact(area, "Playlist", Sense::hover());
    if whole.hovered() {
        let turned = view.ui.input(|input| input.smooth_scroll_delta.y);
        let most = queue.items.len().saturating_sub(visible);
        if turned > 0.0 {
            kept.top = kept.top.saturating_sub(3);
        } else if turned < 0.0 {
            kept.top = (kept.top + 3).min(most);
        }
    }
    let modifiers = view.ui.input(|input| input.modifiers);
    let face = theme::regular(8.0 * view.unit);
    let row_height = layout::PLAYLIST_TRACK_HEIGHT;
    let shown = queue.items.iter().enumerate().skip(kept.top).take(visible);
    for (line, (index, track)) in shown.enumerate() {
        let row = Area::new(
            area.x,
            area.y + line as u32 * row_height,
            area.width,
            row_height,
        );
        let name = format!("{}. {}", index + 1, track.title);
        let response = view.interact(row, &name, Sense::click());
        if response.clicked() {
            clicked(kept, index, modifiers);
        }
        if response.double_clicked() {
            actions.push(Action::JumpTo(index));
        }
        if kept.selected.contains(&index) {
            view.fill(row, rgb(style.selected_background));
        }
        let color = if index == queue.index {
            rgb(style.current)
        } else {
            rgb(style.normal)
        };
        let rect = view.rect(row);
        let length = format::duration(track.duration_ms);
        let right = egui::pos2(rect.right() - view.unit, rect.center().y);
        let painter = view.ui.painter();
        let taken = painter.text(right, Align2::RIGHT_CENTER, length, face.clone(), color);
        let words = label(index, &track.artist_names(), &track.title);
        // Cut where the length begins, as Winamp cut a long name.
        let room = rect.with_max_x(taken.left() - 3.0 * view.unit);
        let left = egui::pos2(rect.left() + view.unit, rect.center().y);
        painter
            .with_clip_rect(room.intersect(view.ui.clip_rect()))
            .text(left, Align2::LEFT_CENTER, words, face.clone(), color);
    }
}

fn scrollbar(view: &mut View<'_>, kept: &mut Kept, rows: usize, height: u32) {
    let most = rows.saturating_sub(rows_visible(height));
    let top = layout::PLAYLIST_TITLE_HEIGHT;
    let travel = list_area(height)
        .height
        .saturating_sub(layout::PLAYLIST_SCROLL_HANDLE_HEIGHT);
    let fraction = if most == 0 {
        0.0
    } else {
        kept.top as f32 / most as f32
    };
    let handle = Area::new(
        layout::PLAYLIST_SCROLL_X,
        top + (fraction * travel as f32).round() as u32,
        8,
        layout::PLAYLIST_SCROLL_HANDLE_HEIGHT,
    );
    let response = view.interact(handle, "Scroll the playlist", Sense::click_and_drag());
    if response.dragged()
        && most > 0
        && let Some(pos) = response.interact_pointer_pos()
    {
        let half = layout::PLAYLIST_SCROLL_HANDLE_HEIGHT as f32 / 2.0;
        let pointer = view.skin_pos(pos).y - top as f32 - half;
        let fraction = (pointer / travel.max(1) as f32).clamp(0.0, 1.0);
        kept.top = (fraction * most as f32).round() as usize;
    }
    let sprite = if response.dragged() || response.is_pointer_button_down_on() {
        sprites::PLAYLIST_SCROLL_HANDLE_PRESSED
    } else {
        sprites::PLAYLIST_SCROLL_HANDLE
    };
    view.sprite(sprite, handle);
}

/// The corner that stretches the list, a row of tiles at a time.
fn grip(view: &mut View<'_>, actions: &mut Vec<Action>, kept: &mut Kept, height: u32) {
    let side = layout::PLAYLIST_GRIP;
    let corner = Area::new(layout::WINDOW_WIDTH - side, height - side, side, side);
    let response = view
        .interact(corner, "Stretch the playlist", Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if response.drag_stopped() {
        kept.stretched = 0.0;
        actions.push(Action::Skin(Ask::Keep));
    }
    if !response.dragged() {
        return;
    }
    kept.stretched += response.drag_delta().y / view.unit;
    let step = layout::PLAYLIST_RESIZE_STEP as f32;
    let steps = (kept.stretched / step).trunc();
    if steps == 0.0 {
        return;
    }
    kept.stretched -= steps * step;
    let wanted = (height as f32 + steps * step).max(0.0) as u32;
    let wanted = layout::playlist_height(wanted);
    if wanted != height {
        actions.push(Action::Skin(Ask::PlaylistHeight(wanted)));
    }
}

/// The bar along the bottom: the times and the little transport.
fn foot(
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    kept: &Kept,
    height: u32,
) {
    let bottom = height - layout::PLAYLIST_BOTTOM_HEIGHT;
    let queue = playback.map(|playback| &playback.session.queue);
    let small = |view: &View<'_>, text: &str, (x, dy): (u32, u32)| {
        view.text(text, Area::new(x, bottom + dy, 5 * text.len() as u32, 6));
    };
    // The selected songs' length over the whole queue's, as Winamp's list
    // had them, and how far the song playing has got.
    if let Some(queue) = queue {
        let total: u64 = queue.items.iter().map(|track| track.duration_ms).sum();
        let selected: u64 = kept
            .selected
            .iter()
            .filter_map(|row| queue.items.get(*row))
            .map(|track| track.duration_ms)
            .sum();
        let running = format!("{}/{}", format::duration(selected), format::duration(total));
        small(view, &running, layout::PLAYLIST_RUNNING_TIME);
    }
    if let Some(playback) = playback {
        let (position, _) = times(view, playback);
        let elapsed = format::duration(position);
        small(view, &elapsed, layout::PLAYLIST_TRACK_TIME);
    }

    // The little transport is painted into the skin: places to click.
    type Press = fn(&mut Vec<Action>, Option<&Playback>);
    let little: [(&str, Press); 6] = [
        ("Previous, on the playlist", |actions, _| {
            actions.push(Action::Previous);
        }),
        ("Play, on the playlist", play),
        ("Pause, on the playlist", |actions, playback| {
            if playback.is_some() {
                actions.push(Action::TogglePlay);
            }
        }),
        ("Stop, on the playlist", stop),
        ("Next, on the playlist", |actions, _| {
            actions.push(Action::Next);
        }),
        ("Open app, on the playlist", |actions, _| {
            actions.push(Action::ShowMainWindow);
        }),
    ];
    let (x, dy) = layout::PLAYLIST_MINI_TRANSPORT;
    for (index, (name, press)) in little.into_iter().enumerate() {
        let cell = Area::new(x + 10 * index as u32, bottom + dy, 10, 10);
        if view.interact(cell, name, Sense::click()).clicked() {
            press(actions, playback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_follows_the_song_that_plays() {
        // In view already: nothing moves.
        assert_eq!(following(2, 4, 6), 2);
        // Above the top: it becomes the top.
        assert_eq!(following(5, 1, 6), 1);
        // Below the bottom: it becomes the last row shown.
        assert_eq!(following(0, 9, 6), 4);
        assert_eq!(following(0, 3, 0), 3);
    }

    #[test]
    fn the_smallest_playlist_shows_four_rows() {
        assert_eq!(rows_visible(layout::PLAYLIST_MIN_HEIGHT), 4);
        assert_eq!(list_area(layout::PLAYLIST_MIN_HEIGHT).width, 243);
    }

    #[test]
    fn clicks_select_one_row_several_or_a_run() {
        let mut kept = Kept::default();
        let (plain, ctrl, shift) = (
            egui::Modifiers::NONE,
            egui::Modifiers::COMMAND,
            egui::Modifiers::SHIFT,
        );
        clicked(&mut kept, 2, plain);
        assert_eq!(kept.selected, BTreeSet::from([2]));
        clicked(&mut kept, 5, ctrl);
        assert_eq!(kept.selected, BTreeSet::from([2, 5]));
        clicked(&mut kept, 2, ctrl);
        assert_eq!(kept.selected, BTreeSet::from([5]));
        // From the row last clicked on its own, in either direction.
        clicked(&mut kept, 4, plain);
        clicked(&mut kept, 1, shift);
        assert_eq!(kept.selected, BTreeSet::from([1, 2, 3, 4]));
        clicked(&mut kept, 6, shift);
        assert_eq!(kept.selected, BTreeSet::from([4, 5, 6]));
    }
}
