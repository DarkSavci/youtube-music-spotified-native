//! The rules in `apply`, each stated as what a person would see.

mod account;
mod accounts;
mod desktop;
mod equalizer;
mod library;
mod listening;
mod migration;
mod pages;
mod playback;
mod preferences;
mod room;
mod room_playback;
mod room_radio;
mod room_servers;
mod search;
mod shell;
mod skins;
mod together;
mod video;

use spotified_client::ApiError;
use spotified_client::models::{
    BrowsePage, CacheUsage, Folder, LibraryItem, Podcast, SearchResults,
};
use spotified_client::session::PlayState;

use super::*;
use crate::backend::Response;
use crate::settings::Settings;
use crate::sidecar::CoreStatus;
use crate::state::Surface;
use crate::state::{MiniPanel, Select};
use spotified_client::models::{LibraryKind, MixSeed, SearchFilter, Track};
use spotified_client::session::Projection;

pub(super) fn state() -> State {
    State::new(Settings::default())
}

pub(super) fn ready() -> State {
    let mut state = state();
    apply(
        &mut state,
        Action::CoreChanged(CoreStatus::Ready {
            origin: "http://127.0.0.1:1".into(),
        }),
    );
    state
}

pub(super) fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        playable: true,
        ..Track::default()
    }
}

pub(super) fn results(query: &str) -> SearchResults {
    SearchResults {
        query: query.into(),
        ..SearchResults::default()
    }
}

#[test]
pub(super) fn a_sidebar_width_is_held_to_its_limits() {
    let mut state = state();
    apply(&mut state, Action::ResizeSidebar(5000.0));
    assert_eq!(state.settings.sidebar_width, theme::SIDEBAR_MAX_WIDTH);
}

#[test]
pub(super) fn an_unchanged_sidebar_width_is_not_saved_again() {
    let mut state = state();
    let width = state.settings.sidebar_width;
    assert!(apply(&mut state, Action::ResizeSidebar(width + 0.2)).is_empty());
}

#[test]
pub(super) fn nothing_is_fetched_before_the_core_is_ready() {
    let mut state = state();
    assert!(apply(&mut state, Action::Open(Page::Album("a".into()))).is_empty());
    assert_eq!(state.albums.get(&"a".to_owned()), &Loadable::NotLoaded);
}

#[test]
pub(super) fn a_page_is_fetched_once() {
    let mut state = ready();
    let open = || Action::Open(Page::Album("a".into()));
    assert_eq!(
        apply(&mut state, open()),
        [Effect::Fetch(Request::Album("a".into()))]
    );
    apply(&mut state, Action::Open(Page::Home));
    assert!(apply(&mut state, open()).is_empty());
}

pub(super) fn playing() -> State {
    let mut state = ready();
    let mut projection = Projection::default();
    projection.state.state = PlayState::Playing;
    projection.state.volume = 0.8;
    projection.state.queue.items = vec![Track {
        id: "a".into(),
        duration_ms: 200_000,
        ..Track::default()
    }];
    apply(&mut state, Action::SessionChanged(Box::new(projection)));
    state
}

pub(super) fn session(state: &State) -> &spotified_client::session::SessionState {
    &state.playback.as_ref().expect("a session").session
}

#[test]
pub(super) fn leaving_a_page_lets_go_of_its_selection() {
    let mut state = ready();
    apply(
        &mut state,
        Action::Select {
            list: 7,
            row: 2,
            how: Select::Only,
        },
    );
    assert!(state.selection.contains(7, 2));
    apply(&mut state, Action::Open(Page::Search));
    assert!(state.selection.is_empty());
}

#[test]
pub(super) fn a_collection_that_cannot_be_fetched_says_so_instead_of_playing() {
    let mut state = ready();
    apply(&mut state, Action::PlayCollection(Page::Album("al".into())));
    let failed = Response::Album("al".into(), Err(spotified_client::ApiError::RateLimited));
    let effects = apply(&mut state, Action::Loaded(Box::new(failed)));
    assert!(effects.is_empty());
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
pub(super) fn the_stats_page_counts_afresh_on_each_visit_and_for_each_period() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::Open(Page::Stats)),
        [Effect::Fetch(Request::Stats(30))]
    );
    assert_eq!(
        apply(&mut state, Action::SetStatsPeriod(7)),
        [Effect::Fetch(Request::Stats(7))]
    );
    // The month's answer arrives after the week was chosen: dropped.
    let late = Response::Stats(30, Ok(spotified_client::models::Stats::default()));
    apply(&mut state, Action::Loaded(Box::new(late)));
    assert_eq!(state.stats, Loadable::Loading);
}

#[test]
pub(super) fn turning_the_visualizer_on_is_kept_and_told_to_the_engine() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::SetVisualizer(true)),
        [Effect::SaveSettings, Effect::ApplyAudioSettings]
    );
    assert!(state.settings.visualizer);
}

#[test]
pub(super) fn a_surface_is_fetched_once_and_then_kept() {
    let mut state = ready();
    let explore = Page::Browse(Surface::explore());
    assert_eq!(
        apply(&mut state, Action::Open(explore.clone())),
        [Effect::Fetch(Request::Browse(
            "FEmusic_explore".into(),
            String::new()
        ))]
    );
    let page = BrowsePage::default();
    let answer = Response::Browse("FEmusic_explore".into(), String::new(), Ok(page));
    apply(&mut state, Action::Loaded(Box::new(answer)));
    apply(&mut state, Action::Open(Page::Home));
    assert!(apply(&mut state, Action::Open(explore)).is_empty());
}

#[test]
pub(super) fn deleting_the_kept_songs_shows_what_is_left() {
    let mut state = ready();
    state.cache_usage = Some(CacheUsage {
        bytes: 1 << 30,
        tracks: 200,
    });
    assert_eq!(
        apply(&mut state, Action::ClearCache),
        [Effect::Fetch(Request::ClearCache)]
    );
    let cleared = Response::CacheCleared(Ok(CacheUsage::default()));
    apply(&mut state, Action::Loaded(Box::new(cleared)));
    assert_eq!(state.cache_usage, Some(CacheUsage::default()));
}

#[test]
pub(super) fn recently_played_is_asked_for_on_every_visit() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::Open(Page::History)),
        [Effect::Fetch(Request::History)]
    );
    apply(
        &mut state,
        Action::Loaded(Box::new(Response::History(Ok(vec![track("a")])))),
    );
    apply(&mut state, Action::Open(Page::Home));
    assert_eq!(
        apply(&mut state, Action::Open(Page::History)),
        [Effect::Fetch(Request::History)]
    );
    // What was there stays on screen while the new answer is on its way.
    assert!(matches!(state.history, Loadable::Loaded(_)));
}

pub(super) fn with_library(items: Vec<LibraryItem>) -> State {
    let mut state = ready();
    state.library = Loadable::Loaded(items);
    state
}

pub(super) fn playlist_item(id: &str) -> LibraryItem {
    LibraryItem {
        id: id.into(),
        kind: LibraryKind::Playlist,
        ..LibraryItem::default()
    }
}
