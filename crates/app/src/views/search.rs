//! The search page: before anything is typed, what was searched for lately
//! and what there is to browse; after, the results.

use eframe::egui::{self, Align2, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{Item, MoodChip, SearchFilter, SearchResults, Shelf, Track};

use super::pages::{self, Skeleton};
use super::{browse, cards, tracks, widgets};
use crate::actions::Action;
use crate::state::{State, Surface};
use crate::theme::{self, Icon};

/// The top result's card is this wide, with its songs beside it.
const TOP_RESULT_CARD: f32 = 280.0;
/// A page narrower than this puts the top result's songs under its card.
const TOP_RESULT_BESIDE_FROM: f32 = 640.0;
const RECENT_HEIGHT: f32 = 36.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if state.search.query.trim().is_empty() {
        start(state, ui, actions);
        return;
    }
    suggestions(state, ui, actions);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for filter in SearchFilter::EVERY {
            let active = state.search.filter == filter;
            if widgets::chip(ui, &state.palette, filter.label(), active).clicked() && !active {
                actions.push(Action::SetSearchFilter(filter));
            }
        }
    });
    ui.add_space(8.0);
    let results = &state.search.results;
    pages::loaded_or(state, ui, results, Skeleton::Shelves(1), |ui, results| {
        if results.top_result.is_none() && results.shelves.iter().all(|s| s.items.is_empty()) {
            widgets::empty_state(
                ui,
                &state.palette,
                Icon::Search,
                &format!("No results for “{}”", results.query),
                "Check the spelling, or try a different term.",
            );
            return;
        }
        let origin = format!("Search: {}", results.query);
        top_result(state, ui, actions, results, &origin);
        // Every song found, as one table: the arrangement that makes an
        // unfiltered search scannable.
        let songs: Vec<&Track> = results
            .shelves
            .iter()
            .flat_map(|shelf| songs_of(&shelf.items))
            .collect();
        if !songs.is_empty() {
            cards::section_title(ui, "Songs");
            song_table(state, ui, actions, &songs, &origin, true);
        }
        // Then each other kind.
        let others = results
            .shelves
            .iter()
            .filter(|shelf| !shelf.items.is_empty() && !all_songs(&shelf.items));
        for (index, shelf) in others.enumerate() {
            ui.push_id(("found", index), |ui| {
                cards::section_title(ui, &shelf.title);
                shelf_cards(state, ui, actions, shelf);
            });
        }
    });
}

/// The songs among `items`.
fn songs_of(items: &[Item]) -> Vec<&Track> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Track(track) => Some(track),
            _ => None,
        })
        .collect()
}

fn all_songs(items: &[Item]) -> bool {
    items.iter().all(|item| matches!(item, Item::Track(_)))
}

/// Songs a search found. One is picked out to be heard, not a list to be
/// played through: a row plays its song, then songs like it.
fn song_table(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    songs: &[&Track],
    origin: &str,
    album: bool,
) {
    let list = tracks::List {
        tracks: songs,
        origin,
        editable_playlist: None,
        columns: tracks::Columns { cover: true, album },
        mode: tracks::Mode::Radio,
    };
    tracks::table(state, ui, actions, list);
}

/// The confident match, as a large card, with what YouTube shows inside
/// it beside it: an artist's top songs, an album's songs, other versions
/// of a song. Whatever else it holds follows as a row of cards.
fn top_result(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    results: &SearchResults,
    origin: &str,
) {
    let Some(top) = &results.top_result else {
        return;
    };
    cards::section_title(ui, "Top result");
    let room = ui.available_width();
    let from = ui.cursor().min;
    // Each side is laid out in a place of its own, as tall as it needs.
    let side = |ui: &mut Ui, left: f32, top: f32, width: f32| {
        let rect = Rect::from_min_size(pos2(left, top), vec2(width, 4000.0));
        ui.new_child(egui::UiBuilder::new().max_rect(rect))
    };
    let mut card = side(ui, from.x, from.y, TOP_RESULT_CARD.min(room));
    cards::item(state, &mut card, actions, top);
    let mut bottom = card.min_rect().bottom();
    let songs = songs_of(&results.top_result_items);
    if !songs.is_empty() {
        let beside = room >= TOP_RESULT_BESIDE_FROM;
        let (left, top) = if beside {
            (from.x + TOP_RESULT_CARD + 24.0, from.y)
        } else {
            (from.x, bottom + 8.0)
        };
        let mut table = side(ui, left, top, from.x + room - left);
        table.spacing_mut().item_spacing.y = 0.0;
        song_table(state, &mut table, actions, &songs, origin, false);
        bottom = bottom.max(table.min_rect().bottom());
    }
    ui.allocate_space(vec2(room, bottom - from.y));
    let others: Vec<&Item> = results
        .top_result_items
        .iter()
        .filter(|item| !matches!(item, Item::Track(_)))
        .collect();
    if !others.is_empty() {
        ui.add_space(16.0);
        ui.push_id("top-others", |ui| {
            cards::row(ui, others.len(), |ui, index| {
                cards::item(state, ui, actions, others[index]);
            });
        });
    }
}

/// One kind of result that is not songs: a row of cards, as many as fit.
/// Narrowed to one kind it is laid out the same, as the Electron app laid
/// it out: the filter changes what is found, not how it is shown.
fn shelf_cards(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, shelf: &Shelf) {
    cards::row(ui, shelf.items.len(), |ui, index| {
        cards::item(state, ui, actions, &shelf.items[index]);
    });
}

/// The search page before anything is typed. Recent searches sit above
/// what there is to browse: someone opening search with nothing typed is
/// more often returning to a search than starting a new one.
fn start(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if !state.search.recent.is_empty() {
        if cards::title_with(state, ui, "Recent searches", Some("Clear")) {
            actions.push(Action::ClearSearches);
        }
        for recent in &state.search.recent {
            let from_account = !recent.token.is_empty();
            recent_row(state, ui, actions, &recent.query, from_account);
        }
    }
    cards::section_title(ui, "Browse all");
    let moods = state.surfaces.get(&Surface::moods().key());
    pages::loaded_or(state, ui, moods, Skeleton::Shelves(1), |ui, page| {
        let first = browse::first_tiles();
        let moods: Vec<&MoodChip> = first.iter().chain(&page.moods).collect();
        browse::mood_tiles(state, ui, actions, &moods);
    });
}

/// One earlier search: a click searches for it again, and the cross that
/// appears under the pointer takes it out of the list.
fn recent_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    query: &str,
    from_account: bool,
) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), RECENT_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, query);
    if !ui.is_rect_visible(rect) {
        return;
    }
    // The cross sits over the row, so the pointer being anywhere in the
    // row is what shows it.
    let inside = ui.rect_contains_pointer(rect);
    let lift = widgets::hover_of(ui, response.id, inside);
    if lift > 0.0 {
        let fill = palette.surface.gamma_multiply(lift);
        ui.painter().rect_filled(rect, theme::RADIUS_ROW, fill);
    }
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let glass = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(16.0, 16.0));
    widgets::paint_icon(ui, Icon::Search, glass, 16.0, color);
    let width = rect.width() - 40.0 - 44.0;
    let text = widgets::elided(ui, query, theme::regular(14.0), color, width, 1);
    let at = pos2(rect.left() + 40.0, rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, color);
    let mut removed = false;
    if inside {
        let cross =
            Rect::from_center_size(pos2(rect.right() - 22.0, rect.center().y), vec2(28.0, 28.0));
        let button = ui.interact(cross, response.id.with("remove"), Sense::click());
        widgets::name(ui, &button, &format!("Remove {query} from recent searches"));
        let over = widgets::hover(ui, &button);
        ui.painter()
            .circle_filled(cross.center(), 14.0, widgets::wash(ui, over));
        let glyph = crate::tint::blend(palette.secondary, palette.text, over);
        widgets::paint_icon(ui, Icon::X, cross, 14.0, glyph.gamma_multiply(lift));
        let tooltip = if from_account {
            "Remove from your YouTube Music history"
        } else {
            "Remove"
        };
        removed = button.on_hover_text(tooltip).clicked();
    }
    if removed {
        actions.push(Action::ForgetSearch(query.to_owned()));
    } else if response.clicked() {
        actions.push(Action::Search(query.to_owned()));
    }
}

/// Ways the query on screen might go on, to search for with a click.
fn suggestions(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let typed = state.search.query.trim();
    let mut others = state
        .search
        .suggestions
        .iter()
        .filter(|suggestion| !suggestion.eq_ignore_ascii_case(typed))
        .peekable();
    if others.peek().is_none() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        for suggestion in others {
            let font = theme::medium(13.0);
            let color = state.palette.secondary;
            let galley = ui.painter().layout_no_wrap(suggestion.clone(), font, color);
            let size = galley.size() + vec2(22.0, 8.0);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            widgets::name(ui, &response, suggestion);
            let lift = widgets::hover(ui, &response);
            let color = crate::tint::blend(color, state.palette.text, lift);
            let icon = egui::Rect::from_min_size(rect.left_top(), vec2(16.0, rect.height()));
            widgets::paint_icon(ui, Icon::Search, icon, 13.0, color);
            widgets::text_at(
                ui,
                rect.left_center() + vec2(20.0, 0.0),
                Align2::LEFT_CENTER,
                suggestion,
                theme::medium(13.0),
                color,
            );
            if response.clicked() {
                actions.push(Action::Search(suggestion.clone()));
            }
        }
    });
    ui.add_space(8.0);
}
