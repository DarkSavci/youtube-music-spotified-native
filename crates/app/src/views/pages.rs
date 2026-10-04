//! What the page area shows for each [`Page`].

use eframe::egui::{self, Ui};
use spotified_client::models::{Item, SearchFilter, Shelf, Track};

use super::{browse, cards, changelog, collection, settings, stats, together, tracks, widgets};
use crate::actions::Action;
use crate::sidecar::CoreStatus;
use crate::state::{Loadable, Page, State};
use crate::theme::{self, Icon};

/// How many songs a search shows before the other kinds of result.
const SEARCH_SONGS: usize = 4;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    match state.nav.page() {
        Page::Home => home(state, ui, actions),
        Page::Search => search(state, ui, actions),
        Page::Settings => settings::show(state, ui, actions),
        Page::Stats => stats::show(state, ui, actions),
        Page::Browse(surface) => browse::surface(state, ui, actions, surface),
        Page::History => browse::history(state, ui, actions),
        Page::Changelog => changelog::show(state, ui),
        Page::Together => together::show(state, ui, actions),
        Page::Mix(id) => collection::mix(state, ui, actions, id),
        Page::Album(id) => collection::album(state, ui, actions, state.albums.get(id)),
        Page::Playlist(id) => collection::playlist(state, ui, actions, state.playlists.get(id)),
        Page::Artist(id) => collection::artist(state, ui, actions, state.artists.get(id)),
        Page::Podcast(id) => collection::podcast(state, ui, actions, state.podcasts.get(id)),
    }
}

pub fn title(ui: &mut Ui, text: &str, size: f32) {
    ui.label(egui::RichText::new(text).font(theme::bold(size)));
    ui.add_space(12.0);
}

/// Draws a page's loaded data, or says that it is loading or why it failed.
pub fn loaded<T>(state: &State, ui: &mut Ui, page: &Loadable<T>, draw: impl FnOnce(&mut Ui, &T)) {
    match page {
        Loadable::NotLoaded | Loadable::Loading => {
            widgets::loading(ui, &state.palette, "Loading…");
        }
        Loadable::Failed(message) => widgets::error(ui, &state.palette, message),
        Loadable::Loaded(data) => draw(ui, data),
    }
}

fn home(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    title(ui, greeting(local_hour()), 30.0);
    if let CoreStatus::Failed(message) = &state.core {
        widgets::error(ui, &state.palette, message);
        return;
    }
    // What the app has made from the listening done here comes first.
    if !state.mixes.is_empty() {
        cards::shelf(ui, "Made for you", "mixes", |ui| {
            for mix in &state.mixes {
                cards::mix(state, ui, actions, mix);
            }
        });
    }
    loaded(state, ui, &state.home, |ui, page| {
        browse::shelves(state, ui, actions, page, "home");
    });
}

fn search(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if state.search.query.trim().is_empty() {
        browse::search_start(state, ui, actions);
        return;
    }
    browse::suggestions(state, ui, actions);
    ui.horizontal(|ui| {
        for filter in SearchFilter::EVERY {
            let active = state.search.filter == filter;
            if widgets::chip(ui, &state.palette, filter.label(), active).clicked() && !active {
                actions.push(Action::SetSearchFilter(filter));
            }
        }
    });
    ui.add_space(8.0);
    loaded(state, ui, &state.search.results, |ui, results| {
        if results.top_result.is_none() && results.shelves.iter().all(|s| s.items.is_empty()) {
            widgets::empty_state(
                ui,
                &state.palette,
                Icon::Search,
                &format!("No results for “{}”", results.query),
                "Check the spelling, or try fewer words.",
            );
            return;
        }
        // Everything at a glance, or one kind in full.
        let narrowed = state.search.filter != SearchFilter::All;
        if let Some(top) = results.top_result.as_ref().filter(|_| !narrowed) {
            cards::shelf(ui, "Top result", "search-top", |ui| {
                cards::item(state, ui, actions, top);
            });
        }
        for (index, shelf) in results.shelves.iter().enumerate() {
            search_shelf(state, ui, actions, shelf, index, narrowed);
        }
    });
}

/// One kind of result. Songs read better as rows than as cards; the rest
/// are cards, in a row to scroll or, when it is the only kind shown, a grid.
fn search_shelf(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    shelf: &Shelf,
    index: usize,
    narrowed: bool,
) {
    if shelf.items.is_empty() {
        return;
    }
    let songs: Vec<&Track> = shelf
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Track(track) => Some(track),
            _ => None,
        })
        .take(if narrowed { usize::MAX } else { SEARCH_SONGS })
        .collect();
    let all_songs = shelf
        .items
        .iter()
        .all(|item| matches!(item, Item::Track(_)));
    if all_songs {
        cards::section_title(ui, &shelf.title);
        let list = tracks::List {
            tracks: &songs,
            origin: &shelf.title,
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: true,
            },
        };
        tracks::rows(state, ui, actions, list);
    } else if narrowed {
        cards::section_title(ui, &shelf.title);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(7.0, 14.0);
            for item in &shelf.items {
                cards::item(state, ui, actions, item);
            }
        });
    } else {
        cards::shelf(ui, &shelf.title, ("search", index), |ui| {
            for item in &shelf.items {
                cards::item(state, ui, actions, item);
            }
        });
    }
}

/// Morning from five, afternoon from noon, evening from six.
fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    }
}

/// The hour on the wall clock. Asked of the system directly: the standard
/// library only knows UTC, and a time-zone crate is a lot for one greeting.
#[cfg(windows)]
fn local_hour() -> u32 {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut time = SystemTime::default();
    // SAFETY: GetLocalTime fills the struct it is given and cannot fail.
    unsafe { GetLocalTime(&mut time) };
    u32::from(time.hour)
}

#[cfg(not(windows))]
fn local_hour() -> u32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    ((seconds / 3600) % 24) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_greeting_follows_the_hour() {
        assert_eq!(greeting(5), "Good morning");
        assert_eq!(greeting(11), "Good morning");
        assert_eq!(greeting(12), "Good afternoon");
        assert_eq!(greeting(17), "Good afternoon");
        assert_eq!(greeting(18), "Good evening");
        assert_eq!(greeting(4), "Good evening");
    }
}
