//! Track rows and the table that holds them.
//!
//! Only the rows in view are laid out, so a playlist of thousands costs the
//! same per frame as one of ten.

use std::borrow::Borrow;

use eframe::egui::{Align2, Id, Key, Modifiers, Rect, Response, Sense, Ui, pos2, vec2};
use spotified_client::models::Track;

use super::widgets::{self, ArtShape};
use super::{drag, format, menus};
use crate::actions::Action;
use crate::state::{Page, Select, State};
use crate::theme::{self, Icon};

const ROW_HEIGHT: f32 = 56.0;
const HEADER_HEIGHT: f32 = 34.0;
const NUMBER_WIDTH: f32 = 44.0;
const COVER_CELL: f32 = 52.0;
const COVER: f32 = 40.0;
const DURATION_WIDTH: f32 = 56.0;
const EDGE: f32 = 8.0;
/// Room for the heart, between the album and the length.
const HEART_CELL: f32 = 36.0;
/// The album column gives way first when the table is narrow.
const ALBUM_COLUMN_FROM: f32 = 560.0;

#[derive(Clone, Copy)]
pub struct Columns {
    /// A cover beside each title: a playlist mixes albums, an album does not.
    pub cover: bool,
    pub album: bool,
}

/// A list of tracks and what playing from it is called in the queue.
#[derive(Clone, Copy)]
pub struct List<'a, T> {
    pub tracks: &'a [T],
    /// The album's or playlist's name, shown as "Playing from".
    pub origin: &'a str,
    /// The playlist these rows can be removed from, when it is the
    /// person's own.
    pub editable_playlist: Option<&'a str>,
    pub columns: Columns,
}

/// Where each column sits for a table of a given width.
struct Layout {
    title_left: f32,
    title_width: f32,
    /// Left edge and width of the album column, when there is room for it.
    album: Option<(f32, f32)>,
}

impl Layout {
    fn new(row: Rect, columns: Columns) -> Self {
        let title_left =
            row.left() + EDGE + NUMBER_WIDTH + if columns.cover { COVER_CELL } else { 0.0 };
        let right = row.right() - EDGE - DURATION_WIDTH - HEART_CELL;
        let album_width = (row.width() * 0.28).clamp(140.0, 360.0);
        let album = (columns.album && row.width() > ALBUM_COLUMN_FROM)
            .then_some((right - album_width, album_width));
        let title_right = album.map_or(right, |(left, _)| left);
        Self {
            title_left,
            title_width: (title_right - title_left - 16.0).max(40.0),
            album,
        }
    }
}

/// A header and every track, for an album or a playlist page.
pub fn table(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, list: List<'_, Track>) {
    header(state, ui, list.columns);
    ui.add_space(6.0);
    rows(state, ui, actions, list);
}

fn header(state: &State, ui: &mut Ui, columns: Columns) {
    let palette = &state.palette;
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
    let layout = Layout::new(rect, columns);
    let font = theme::regular(12.0);
    let label = |x: f32, anchor: Align2, text: &str| {
        widgets::text_at(
            ui,
            pos2(x, rect.center().y),
            anchor,
            text,
            font.clone(),
            palette.secondary,
        );
    };
    label(rect.left() + EDGE + 16.0, Align2::CENTER_CENTER, "#");
    label(layout.title_left, Align2::LEFT_CENTER, "TITLE");
    if let Some((left, _)) = layout.album {
        label(left, Align2::LEFT_CENTER, "ALBUM");
    }
    label(rect.right() - EDGE, Align2::RIGHT_CENTER, "TIME");
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), (1.0, palette.outline));
}

/// Track rows without a header, numbered from one. Takes tracks or
/// references to them, so a caller with a filtered view need not copy.
pub fn rows<T: Borrow<Track>>(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    list: List<'_, T>,
) {
    // Taken before the rows claim their space: unique to this list, so two
    // lists on one page do not share row ids or a selection.
    let list_id = ui.auto_id_with("tracks");
    let list_key = list_id.value();
    let height = list.tracks.len() as f32 * ROW_HEIGHT;
    let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let row_rect = |index: usize| {
        Rect::from_min_size(
            pos2(area.left(), area.top() + index as f32 * ROW_HEIGHT),
            vec2(area.width(), ROW_HEIGHT),
        )
    };
    if let Some(moved_to) = selection_keys(state, ui, actions, list_key, list.tracks.len()) {
        ui.scroll_to_rect(row_rect(moved_to), None);
    }

    let visible = ui.clip_rect();
    let first = ((visible.top() - area.top()) / ROW_HEIGHT).floor().max(0.0) as usize;
    let last = ((visible.bottom() - area.top()) / ROW_HEIGHT)
        .ceil()
        .max(0.0) as usize;
    let in_view = list.tracks.iter().enumerate().take(last).skip(first);
    for (index, track) in in_view {
        let row = Row {
            track: track.borrow(),
            index,
            id: list_id.with(index),
            rect: row_rect(index),
            columns: list.columns,
            selected: state.selection.contains(list_key, index),
        };
        let response = row.show(state, ui, actions);
        // A selected row carries the whole selection; any other, itself.
        drag::start(&response, || {
            if row.selected {
                state
                    .selection
                    .rows(list_key)
                    .filter_map(|row| list.tracks.get(row))
                    .map(|track| track.borrow().clone())
                    .collect()
            } else {
                vec![row.track.clone()]
            }
        });
        // The menu acts on the whole selection when the row under the
        // pointer is part of one, and on that row alone otherwise.
        response.context_menu(|ui| {
            let chosen: Vec<&Track> = if row.selected {
                state
                    .selection
                    .rows(list_key)
                    .filter_map(|row| list.tracks.get(row))
                    .map(Borrow::borrow)
                    .collect()
            } else {
                vec![row.track]
            };
            menus::tracks(state, ui, actions, &chosen, list.editable_playlist);
        });
        if row.track.playable && (response.double_clicked() || row.number_clicked(ui)) {
            // The whole list is queued, starting here. Copied only now, on
            // the click, never per frame.
            actions.push(Action::Play {
                tracks: list.tracks.iter().map(|t| t.borrow().clone()).collect(),
                index,
                origin: list.origin.to_owned(),
            });
        } else if response.clicked() {
            let how = ui.input(|input| match input.modifiers {
                modifiers if modifiers.shift => Select::Range,
                modifiers if modifiers.command => Select::Toggle,
                _ => Select::Only,
            });
            actions.push(Action::Select {
                list: list_key,
                row: index,
                how,
            });
        }
    }
}

/// The keys that act on this list's selection, while it has one: arrows
/// move it, Ctrl+A takes every row, Escape lets go. Returns the row an
/// arrow would move to, so it can be scrolled into sight.
fn selection_keys(
    state: &State,
    ui: &Ui,
    actions: &mut Vec<Action>,
    list: u64,
    len: usize,
) -> Option<usize> {
    if state.selection.rows(list).next().is_none() || ui.ctx().egui_wants_keyboard_input() {
        return None;
    }
    let mut moved_to = None;
    ui.input_mut(|input| {
        if input.consume_key(Modifiers::NONE, Key::Escape) {
            actions.push(Action::ClearSelection);
        }
        if input.consume_key(Modifiers::COMMAND, Key::A) {
            actions.push(Action::SelectAll { list, len });
        }
        for (key, step) in [(Key::ArrowUp, -1), (Key::ArrowDown, 1)] {
            for (modifiers, extend) in [(Modifiers::NONE, false), (Modifiers::SHIFT, true)] {
                if input.consume_key(modifiers, key) {
                    actions.push(Action::StepSelection {
                        list,
                        step,
                        len,
                        extend,
                    });
                    // Where the selection is now, plus the step: near
                    // enough to scroll to before the action is applied.
                    let from = if step < 0 {
                        state.selection.rows(list).next()
                    } else {
                        state.selection.rows(list).last()
                    };
                    moved_to = from.map(|row| row.saturating_add_signed(step).min(len - 1));
                }
            }
        }
    });
    moved_to
}

struct Row<'a> {
    track: &'a Track,
    index: usize,
    id: Id,
    rect: Rect,
    columns: Columns,
    selected: bool,
}

impl Row<'_> {
    fn number_rect(&self) -> Rect {
        Rect::from_center_size(
            pos2(self.rect.left() + EDGE + 16.0, self.rect.center().y),
            vec2(NUMBER_WIDTH - 8.0, ROW_HEIGHT),
        )
    }

    /// Whether the row's number, which shows a play button under the
    /// pointer, was clicked.
    fn number_clicked(&self, ui: &Ui) -> bool {
        ui.interact(self.number_rect(), self.id.with("number"), Sense::click())
            .clicked()
    }

    /// Draws the row and returns its response: clicks select, a double
    /// click plays, and the caller hangs the menu on it.
    fn show(&self, state: &State, ui: &mut Ui, actions: &mut Vec<Action>) -> Response {
        let palette = &state.palette;
        let (track, rect) = (self.track, self.rect);
        // Dragged as well as clicked: a row can be carried to a playlist.
        let response = ui.interact(rect, self.id, Sense::click_and_drag());
        widgets::name(ui, &response, &track.title);
        let hovered = response.hovered();
        if self.selected {
            let fill = palette.secondary.gamma_multiply(0.2);
            ui.painter().rect_filled(rect, theme::RADIUS_ROW, fill);
        }
        widgets::row_hover(ui, &response, rect);
        let layout = Layout::new(rect, self.columns);
        let middle = rect.center().y;
        let playing = state
            .playback
            .as_ref()
            .and_then(|playback| playback.current())
            .is_some_and(|current| current.id == track.id);

        let number = self.number_rect();
        if hovered && track.playable {
            widgets::paint_icon(ui, Icon::PlayFilled, number, 14.0, palette.text);
        } else if playing {
            widgets::paint_icon(ui, Icon::AudioLines, number, 16.0, palette.accent);
        } else {
            widgets::text_at(
                ui,
                number.center(),
                Align2::CENTER_CENTER,
                &(self.index + 1).to_string(),
                theme::regular(14.0),
                palette.secondary,
            );
        }
        if self.columns.cover {
            let cover = Rect::from_center_size(
                pos2(rect.left() + EDGE + NUMBER_WIDTH + COVER / 2.0, middle),
                vec2(COVER, COVER),
            );
            let shape = ArtShape::Rounded(4);
            widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
        }

        // A track that cannot be played is shown, but dimmed.
        let title_color = match (playing, track.playable) {
            (true, _) => palette.accent,
            (false, true) => palette.text,
            (false, false) => palette.dim,
        };
        let title = widgets::elided(
            ui,
            &track.title,
            theme::medium(14.5),
            title_color,
            layout.title_width,
            1,
        );
        ui.painter()
            .galley(pos2(layout.title_left, middle - 19.0), title, title_color);
        let quiet = if hovered || self.selected {
            palette.text
        } else {
            palette.secondary
        };

        // The artists and the album are links to their pages.
        let artist_page = track
            .artists
            .iter()
            .find(|artist| !artist.id.is_empty())
            .map(|artist| Page::Artist(artist.id.clone()));
        let artists = widgets::Link {
            text: &track.artist_names(),
            font: theme::regular(12.5),
            color: quiet,
            width: layout.title_width,
        };
        let at = pos2(layout.title_left, middle + 2.0);
        if artists.show(ui, self.id.with("artist"), at, artist_page.is_some())
            && let Some(page) = artist_page
        {
            actions.push(Action::Open(page));
        }
        if let (Some((left, width)), Some(album)) = (layout.album, &track.album) {
            let link = widgets::Link {
                text: &album.name,
                font: theme::regular(13.0),
                color: quiet,
                width: width - 16.0,
            };
            let has_page = !album.id.is_empty();
            if link.show(
                ui,
                self.id.with("album"),
                pos2(left, middle - 8.0),
                has_page,
            ) {
                actions.push(Action::Open(Page::Album(album.id.clone())));
            }
        }

        // The heart shows on a liked song always, and on any song under the
        // pointer, so a list is not a column of empty hearts.
        let liked = state.likes.is_liked(&track.id);
        if liked || hovered {
            let cell = Rect::from_center_size(
                pos2(
                    rect.right() - EDGE - DURATION_WIDTH - HEART_CELL / 2.0,
                    middle,
                ),
                vec2(28.0, 28.0),
            );
            let heart = ui
                .interact(cell, self.id.with("heart"), Sense::click())
                .on_hover_text(if liked {
                    "Remove from Liked Songs"
                } else {
                    "Save to Liked Songs"
                });
            let (icon, color) = match (liked, heart.hovered()) {
                (true, _) => (Icon::HeartFilled, palette.accent),
                (false, true) => (Icon::Heart, palette.text),
                (false, false) => (Icon::Heart, palette.secondary),
            };
            widgets::paint_icon(ui, icon, cell, 16.0, color);
            if heart.clicked() {
                actions.push(Action::ToggleLike(track.clone()));
            }
        }
        // Some lists come without lengths; an empty cell beats "0:00".
        if track.duration_ms > 0 {
            widgets::text_at(
                ui,
                pos2(rect.right() - EDGE, middle),
                Align2::RIGHT_CENTER,
                &format::duration(track.duration_ms),
                theme::regular(13.0),
                palette.secondary,
            );
        }
        response
    }
}
