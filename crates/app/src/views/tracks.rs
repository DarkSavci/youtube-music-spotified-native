//! Track rows and the table that holds them.
//!
//! Only the rows in view are laid out, so a playlist of thousands costs the
//! same per frame as one of ten.

use std::borrow::Borrow;

use eframe::egui::{Align2, Id, Key, Modifiers, Rect, Response, Sense, Ui, pos2, vec2};
use spotified_client::models::Track;

use super::widgets::menu::{self, Menu};
use super::widgets::{self, ArtShape};
use super::{drag, format, menus};
use crate::actions::Action;
use crate::state::{Page, Select, State};
use crate::theme::{self, Icon};

const ROW_HEIGHT: f32 = 56.0;
pub const HEADER_HEIGHT: f32 = 34.0;
// The Electron app's grid: the number, the title, the album, the length
// and the row's two buttons, with a gap between each and room at the ends.
const EDGE: f32 = 16.0;
const GAP: f32 = 16.0;
const NUMBER_WIDTH: f32 = 32.0;
const COVER: f32 = 40.0;
/// A cover and the room between it and the title.
const COVER_CELL: f32 = COVER + 12.0;
const LAST_WIDTH: f32 = 96.0;
/// The heart and the menu button, at the end of every row.
const BUTTONS_WIDTH: f32 = 56.0;
const BUTTON: f32 = 26.0;
/// The album column gives way first when the table is narrow.
const ALBUM_COLUMN_FROM: f32 = 560.0;

#[derive(Clone, Copy)]
pub struct Columns {
    /// A cover beside each title: a playlist mixes albums, an album does not.
    pub cover: bool,
    pub album: bool,
}

/// What playing a row of a list plays.
#[derive(Clone, Copy)]
pub enum Mode<'a> {
    /// The whole list, from the row.
    List,
    /// The row's song, then songs like it: a search result is one song
    /// picked out, not a list to play through.
    Radio,
    /// The rows are a playlist's, and it is the playlist that is played:
    /// all of it, though only a part may be here yet.
    Playlist(&'a str),
    /// The rows are part of a longer list, and that is what is queued: the
    /// list, and where in it these rows begin.
    Within(&'a [Track], usize),
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
    pub mode: Mode<'a>,
}

/// Where each column sits for a table of a given width.
struct Layout {
    /// The middle of the number column.
    number: f32,
    /// Where the cover starts, or the title when there is no cover.
    main_left: f32,
    title_left: f32,
    title_width: f32,
    /// Left edge and width of the album column, when there is room for it.
    album: Option<(f32, f32)>,
    /// The right edge of the time listened, in a table that says it.
    listened_right: Option<f32>,
    /// The right edge of the last column: the length, or the plays.
    last_right: f32,
    /// The middles of the heart and of the menu button.
    heart: f32,
    more: f32,
}

impl Layout {
    fn new(row: Rect, columns: Columns, listened: bool) -> Self {
        let left = row.left() + EDGE;
        let main_left = left + NUMBER_WIDTH + GAP;
        let buttons_left = row.right() - EDGE - BUTTONS_WIDTH;
        let last_right = buttons_left - GAP;
        // The time listened has a column of the last one's width, before it.
        let listened_right = listened.then_some(last_right - LAST_WIDTH - GAP);
        let fixed_left = listened_right.unwrap_or(last_right) - LAST_WIDTH - GAP;
        // What the title and the album share: three parts to two.
        let flexible = fixed_left - main_left;
        let with_album = columns.album && row.width() > ALBUM_COLUMN_FROM;
        let (main_width, album) = if with_album {
            let shared = flexible - GAP;
            let main = shared * 3.0 / 5.0;
            (main, Some((main_left + main + GAP, shared - main)))
        } else {
            (flexible, None)
        };
        let cover = if columns.cover { COVER_CELL } else { 0.0 };
        Self {
            number: left + NUMBER_WIDTH / 2.0,
            main_left,
            title_left: main_left + cover,
            title_width: (main_width - cover).max(40.0),
            album,
            listened_right,
            last_right,
            heart: buttons_left + BUTTON / 2.0,
            more: buttons_left + BUTTONS_WIDTH - BUTTON / 2.0,
        }
    }
}

/// What a table's last columns hold.
#[derive(Clone, Copy)]
pub struct Last {
    /// The last column's name: lengths, or, for a list that has none, how
    /// often each song was played.
    name: &'static str,
    /// A column before it says how long each song was listened to: the
    /// listener's own figures have it, nothing else does.
    listened: bool,
}

pub fn last_column<T: Borrow<Track>>(tracks: &[T]) -> Last {
    let any = |has: fn(&Track) -> bool| tracks.iter().any(|track| has(track.borrow()));
    Last {
        name: if any(|track| track.duration_ms > 0) {
            "TIME"
        } else {
            "PLAYS"
        },
        listened: any(|track| track.listened_ms.is_some()),
    }
}

/// A header and every track, for an album or a playlist page. Returns
/// where the header was put, for a page that keeps it in sight.
pub fn table<T: Borrow<Track>>(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    list: List<'_, T>,
) -> Rect {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
    header(state, ui, rect, list.columns, last_column(list.tracks));
    rows(state, ui, actions, list);
    rect
}

/// The column names, painted into `rect`. `last` says what the last
/// columns are.
pub fn header(state: &State, ui: &Ui, rect: Rect, columns: Columns, last: Last) {
    let palette = &state.palette;
    let layout = Layout::new(rect, columns, last.listened);
    // Small capitals, set a little apart.
    let label = |x: f32, anchor: Align2, text: &str| {
        let font = theme::regular(12.0);
        let galley = widgets::tracked(ui, text, font, palette.secondary, 0.7, (f32::MAX, 1));
        let size = galley.size();
        let at = anchor.anchor_size(pos2(x, rect.center().y), size).min;
        ui.painter().galley(at, galley, palette.secondary);
    };
    label(layout.number, Align2::CENTER_CENTER, "#");
    label(layout.main_left, Align2::LEFT_CENTER, "TITLE");
    if let Some((left, _)) = layout.album {
        label(left, Align2::LEFT_CENTER, "ALBUM");
    }
    if let Some(right) = layout.listened_right {
        label(right, Align2::RIGHT_CENTER, "LISTENED");
    }
    label(layout.last_right, Align2::RIGHT_CENTER, last.name);
    ui.painter()
        .hline(rect.x_range(), rect.bottom(), (1.0, palette.outline));
}

/// The songs a row's menu acts on: the whole selection when the row is
/// part of one, and that row alone otherwise.
fn menu_for<T: Borrow<Track>>(
    state: &State,
    menu: &mut Menu<'_>,
    actions: &mut Vec<Action>,
    list: &List<'_, T>,
    (list_key, row): (u64, &Row<'_>),
) {
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
    menus::tracks(state, menu, actions, &chosen, list.editable_playlist);
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
    let listened = last_column(list.tracks).listened;
    let in_view = list.tracks.iter().enumerate().take(last).skip(first);
    for (index, track) in in_view {
        let row = Row {
            track: track.borrow(),
            index,
            id: list_id.with(index),
            rect: row_rect(index),
            columns: list.columns,
            listened,
            selected: state.selection.contains(list_key, index),
        };
        let (response, more) = row.show(state, ui, actions);
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
        // The same menu from the right button and from the row's own.
        let palette = &state.palette;
        menu::context(&response, palette, |menu| {
            menu_for(state, menu, actions, &list, (list_key, &row));
        });
        menu::popup(&more, palette, |menu| {
            menu_for(state, menu, actions, &list, (list_key, &row));
        });
        if row.track.playable && (response.double_clicked() || row.number_clicked(ui)) {
            // Whatever is queued is copied only now, on the click, never
            // per frame.
            let origin = list.origin.to_owned();
            actions.push(match list.mode {
                Mode::List => Action::Play {
                    tracks: list.tracks.iter().map(|t| t.borrow().clone()).collect(),
                    index,
                    origin,
                },
                Mode::Radio => Action::StartRadio(row.track.clone()),
                Mode::Playlist(id) => Action::PlayPlaylist {
                    id: id.to_owned(),
                    index,
                },
                Mode::Within(all, offset) => Action::Play {
                    tracks: all.to_vec(),
                    index: offset + index,
                    origin,
                },
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
    /// The table says how long each song was listened to.
    listened: bool,
    selected: bool,
}

impl Row<'_> {
    fn number_rect(&self) -> Rect {
        Rect::from_center_size(
            pos2(
                self.rect.left() + EDGE + NUMBER_WIDTH / 2.0,
                self.rect.center().y,
            ),
            vec2(NUMBER_WIDTH, ROW_HEIGHT),
        )
    }

    /// Whether the row's number, which shows a play button under the
    /// pointer, was clicked.
    fn number_clicked(&self, ui: &Ui) -> bool {
        ui.interact(self.number_rect(), self.id.with("number"), Sense::click())
            .clicked()
    }

    /// Draws the row and returns its response: clicks select, a double
    /// click plays, and the caller hangs the menu on it and on the row's
    /// menu button, which is returned with it.
    fn show(&self, state: &State, ui: &mut Ui, actions: &mut Vec<Action>) -> (Response, Response) {
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
        let layout = Layout::new(rect, self.columns, self.listened);
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
                pos2(layout.main_left + COVER / 2.0, middle),
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
            theme::medium(14.0),
            title_color,
            layout.title_width,
            1,
        );
        // A song with nobody named under it has the row to itself.
        let artists = track.artist_names();
        let title_top = if artists.is_empty() { -9.0 } else { -19.0 };
        let at = pos2(layout.title_left, middle + title_top);
        ui.painter().galley(at, title, title_color);
        let quiet = if hovered || self.selected {
            palette.text
        } else {
            palette.secondary
        };

        // Each artist and the album are links to their pages.
        let artists = widgets::Artists {
            artists: &track.artists,
            font: theme::regular(12.0),
            color: quiet,
            width: layout.title_width,
        };
        let at = pos2(layout.title_left, middle + 2.0);
        if let (Some(artist), _) = artists.show(ui, self.id.with("artist"), at) {
            actions.push(Action::Open(Page::Artist(artist)));
        }
        if let (Some((left, width)), Some(album)) = (layout.album, &track.album) {
            let link = widgets::Link {
                text: &album.name,
                font: theme::regular(14.0),
                color: quiet,
                width,
            };
            let has_page = !album.id.is_empty();
            if link.show(
                ui,
                self.id.with("album"),
                pos2(left, middle - 9.0),
                has_page,
            ) {
                actions.push(Action::Open(Page::Album(album.id.clone())));
            }
        }

        if let Some(right) = layout.listened_right {
            widgets::text_at(
                ui,
                pos2(right, middle),
                Align2::RIGHT_CENTER,
                &format::listened(track.listened_ms.unwrap_or(0)),
                theme::regular(14.0),
                palette.secondary,
            );
        }
        // The length, or for a list that has none, how often it was played.
        let length;
        let last = if track.duration_ms > 0 {
            length = format::duration(track.duration_ms);
            length.as_str()
        } else {
            track.play_count.as_str()
        };
        widgets::text_at(
            ui,
            pos2(layout.last_right, middle),
            Align2::RIGHT_CENTER,
            last,
            theme::regular(14.0),
            palette.secondary,
        );
        let more = self.buttons(state, ui, actions, &layout);
        (response, more)
    }

    /// The heart and the menu button, at the end of every row, always in
    /// sight. Returns the menu button, for the caller to hang the menu on.
    fn buttons(
        &self,
        state: &State,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
        layout: &Layout,
    ) -> Response {
        let palette = &state.palette;
        let track = self.track;
        let middle = self.rect.center().y;
        let cell = |x: f32| Rect::from_center_size(pos2(x, middle), vec2(BUTTON, BUTTON));
        let liked = state.likes.is_liked(&track.id);
        let (name, tooltip) = if liked {
            (
                format!("Remove {} from your library", track.title),
                "Remove from Liked Songs",
            )
        } else {
            (format!("Save {}", track.title), "Save to Liked Songs")
        };
        let heart = ui.interact(cell(layout.heart), self.id.with("heart"), Sense::click());
        widgets::name(ui, &heart, &name);
        let lift = widgets::hover(ui, &heart);
        ui.painter()
            .circle_filled(heart.rect.center(), BUTTON / 2.0, widgets::wash(ui, lift));
        let (icon, color) = if liked {
            (Icon::HeartFilled, palette.accent)
        } else {
            let color = crate::tint::blend(palette.secondary, palette.text, lift);
            (Icon::Heart, color)
        };
        widgets::paint_icon(ui, icon, heart.rect, 16.0, color);
        if heart.on_hover_text(tooltip).clicked() {
            actions.push(Action::ToggleLike(track.clone()));
        }

        let more = ui.interact(cell(layout.more), self.id.with("more"), Sense::click());
        widgets::name(ui, &more, &format!("More options for {}", track.title));
        let lift = widgets::hover(ui, &more);
        ui.painter()
            .circle_filled(more.rect.center(), BUTTON / 2.0, widgets::wash(ui, lift));
        let color = crate::tint::blend(palette.secondary, palette.text, lift);
        widgets::paint_icon(ui, Icon::Ellipsis, more.rect, 16.0, color);
        more.on_hover_text("More options")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_of_the_listeners_own_figures_says_how_long_each_was_listened_to() {
        let played = Track {
            listened_ms: Some(90_000),
            play_count: "3".into(),
            ..Track::default()
        };
        let last = last_column(&[played]);
        assert!(last.listened);
        assert_eq!(last.name, "PLAYS");
        // A list from the catalogue has lengths, and no such column.
        let song = Track {
            duration_ms: 200_000,
            ..Track::default()
        };
        let last = last_column(&[song]);
        assert!(!last.listened);
        assert_eq!(last.name, "TIME");
    }

    #[test]
    fn the_time_listened_takes_its_room_from_the_title_and_sits_before_the_last_column() {
        let row = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, ROW_HEIGHT));
        let columns = Columns {
            cover: true,
            album: false,
        };
        let without = Layout::new(row, columns, false);
        let with = Layout::new(row, columns, true);
        assert_eq!(without.listened_right, None);
        let listened = with.listened_right.expect("a column");
        assert_eq!(listened, with.last_right - LAST_WIDTH - GAP);
        assert_eq!(with.title_width, without.title_width - LAST_WIDTH - GAP);
        assert_eq!(with.last_right, without.last_right);
    }
}
