//! The views, driven as a person would drive them: find a control by its
//! name, click it, and see what was asked for.

mod browse;
mod controls;
mod desktop;
mod equalizer;
mod library;
mod menus;
mod migration;
mod pages;
mod player;
mod selection;
mod shell;
mod skinned;
mod together;
mod together_room;
mod video;

use std::time::Instant;

use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use spotified_client::models::{
    Album, Artist, ArtistRef, BrowsePage, Folder, Item, LibraryItem, LibraryKind, MoodChip,
    Playlist, Podcast, SearchFilter, SearchResults, Shelf, StatKind, Track,
};
use spotified_client::session::{PlayState, Queue, SessionState};

use crate::actions::Action;
use crate::settings::Settings;
use crate::sidecar::CoreStatus;
use crate::state::{Loadable, Page, Playback, RecentSearch, State, Surface};
use crate::theme;

pub(super) struct Fixture {
    state: State,
    actions: Vec<Action>,
    themed: bool,
}

pub(super) fn track(id: &str, title: &str) -> Track {
    Track {
        id: id.into(),
        title: title.into(),
        artists: vec![ArtistRef {
            id: "artist-1".into(),
            name: "The Artist".into(),
        }],
        duration_ms: 200_000,
        playable: true,
        ..Track::default()
    }
}

/// A signed-out app with a core that is up.
pub(super) fn state() -> State {
    let mut state = State::new(Settings::default());
    state.core = CoreStatus::Ready {
        origin: "http://127.0.0.1:1".into(),
    };
    // Nothing left loading: a spinner redraws for ever, and the harness
    // waits for the window to come to rest.
    state.home = Loadable::Loaded(BrowsePage::default());
    state.library = Loadable::Failed("Sign in to see this.".into());
    state
}

pub(super) fn on_playlist() -> State {
    let mut state = state();
    let playlist = Playlist {
        id: "pl".into(),
        title: "Road trip".into(),
        tracks: vec![
            track("a", "First song"),
            track("b", "Second song"),
            track("c", "Third song"),
        ],
        ..Playlist::default()
    };
    state
        .playlists
        .insert("pl".into(), Loadable::Loaded(playlist));
    state.nav.open(Page::Playlist("pl".into()));
    state
}

pub(super) fn playing(mut state: State) -> State {
    state.playback = Some(Playback {
        session: SessionState {
            state: PlayState::Playing,
            volume: 0.5,
            queue: Queue {
                items: vec![track("a", "First song")],
                ..Queue::default()
            },
            ..SessionState::default()
        },
        received: Instant::now(),
        offline: false,
        following_room: false,
        room_ended: None,
        room_length: None,
        speed: 1.0,
    });
    state
}

pub(super) fn harness(state: State) -> Harness<'static, Fixture> {
    let fixture = Fixture {
        state,
        actions: Vec::new(),
        themed: false,
    };
    let mut harness = Harness::builder()
        .with_size(vec2(1240.0, 800.0))
        // A frame at sixty a second. The default quarter-second is too slow
        // for two clicks a frame apart to count as a double click.
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(16)
        .build_ui_state(
            |ui, fixture: &mut Fixture| {
                // Fonts set on one frame are there on the next, so the
                // first frame only installs them.
                if !fixture.themed {
                    theme::install(ui.ctx(), &fixture.state.palette);
                    fixture.themed = true;
                    return;
                }
                super::show(&fixture.state, ui, &mut fixture.actions);
            },
            fixture,
        );
    harness.run();
    harness
}

/// The mini player alone, in a window of `size`.
pub(super) fn mini_harness(state: State, size: eframe::egui::Vec2) -> Harness<'static, Fixture> {
    let fixture = Fixture {
        state,
        actions: Vec::new(),
        themed: false,
    };
    let mut harness = Harness::builder()
        .with_size(size)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(16)
        .build_ui_state(
            |ui, fixture: &mut Fixture| {
                if !fixture.themed {
                    theme::install(ui.ctx(), &fixture.state.palette);
                    fixture.themed = true;
                    return;
                }
                super::mini::show(&fixture.state, ui, &mut fixture.actions);
            },
            fixture,
        );
    harness.run();
    harness
}

pub(super) fn asked(harness: &Harness<'_, Fixture>, wanted: impl Fn(&Action) -> bool) -> bool {
    harness.state().actions.iter().any(wanted)
}

#[test]
pub(super) fn a_double_click_plays_the_playlist_from_that_row() {
    let mut harness = harness(on_playlist());
    // Two clicks, a frame apart, as a hand makes them.
    harness.get_by_label("Second song").click();
    harness.step();
    harness.get_by_label("Second song").click();
    harness.run();
    // The playlist is named, not its songs: all of it is played, though
    // only a part may have been read.
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayPlaylist { id, index: 1 } if id == "pl"
    )));
}

#[test]
pub(super) fn a_single_click_on_a_row_does_not_play() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Second song").click();
    harness.run();
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::Play { .. }
    )));
}

#[test]
pub(super) fn a_right_click_offers_the_queue() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Third song").click_secondary();
    harness.run();
    harness.get_by_label("Add to queue").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(tracks) if tracks[0].id == "c"
    )));
}

#[test]
pub(super) fn the_menu_leads_to_the_songs_artist() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("First song").click_secondary();
    harness.run();
    harness.get_by_label("Go to The Artist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Artist(id)) if id == "artist-1"
    )));
}

#[test]
pub(super) fn a_click_selects_a_row() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Second song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Select {
            row: 1,
            how: crate::state::Select::Only,
            ..
        }
    )));
}

#[test]
pub(super) fn the_menu_on_a_selection_acts_on_all_of_it() {
    let mut state = on_playlist();
    // The list's key is whatever the view derives; take it from a click.
    let mut probe = harness(state);
    probe.get_by_label("First song").click();
    probe.run();
    let list = probe
        .state()
        .actions
        .iter()
        .find_map(|action| match action {
            Action::Select { list, .. } => Some(*list),
            _ => None,
        })
        .expect("a selection");
    state = on_playlist();
    state.selection.select_all(list, 3);

    let mut harness = harness(state);
    harness.get_by_label("Second song").click_secondary();
    harness.run();
    harness.get_by_label("Add to queue").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(tracks) if tracks.len() == 3
    )));
}

#[test]
pub(super) fn the_artist_under_a_title_is_a_link() {
    let mut harness = harness(on_playlist());
    // Not named for screen readers on its own; found by where it is drawn,
    // just under the first row's title.
    let row = harness.get_by_label("First song").rect();
    let at = eframe::egui::pos2(row.left() + 120.0, row.center().y + 10.0);
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.drop_at(at);
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Artist(id)) if id == "artist-1"
    )));
}

#[test]
pub(super) fn the_big_button_plays_the_playlist_from_the_top() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Play Road trip").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayPlaylist { id, index: 0 } if id == "pl"
    )));
}

#[test]
pub(super) fn the_normalise_switch_asks_for_the_other_state() {
    let mut state = state();
    state.nav.open(Page::Settings);
    let mut harness = harness(state);
    // The row's text carries the same words; the switch is the checkbox.
    harness
        .get_by_role_and_label(eframe::egui::accesskit::Role::CheckBox, "Normalise volume")
        .click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetNormaliseVolume(false)
    )));
}

#[test]
pub(super) fn a_sound_device_is_chosen_from_the_list_of_those_there_are() {
    let mut state = state();
    state.nav.open(Page::Settings);
    state.output_devices = vec![crate::settings::OutputDevice {
        id: "wasapi:desk".into(),
        name: "Desk speakers".into(),
    }];
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(eframe::egui::accesskit::Role::Button, "Output device")
        .click();
    harness.run();
    // Opening the list asks for it again: devices come and go.
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ListOutputDevices
    )));
    harness
        .get_by_role_and_label(eframe::egui::accesskit::Role::Button, "Desk speakers")
        .click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetOutputDevice(Some(device)) if device.id == "wasapi:desk"
    )));
}

#[test]
pub(super) fn the_new_playlist_dialog_creates_once_it_has_a_name() {
    let mut state = state();
    state.dialog = Some(crate::state::Dialog::NewPlaylist {
        name: "Road trip".into(),
        track_ids: Vec::new(),
    });
    let mut harness = harness(state);
    harness.get_by_label("Create").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ConfirmDialog
    )));
}

#[test]
pub(super) fn a_song_in_your_own_playlist_can_be_removed_from_it() {
    let mut state = on_playlist();
    if let Some(playlist) = state.playlists.loaded_mut(&"pl".into()) {
        playlist.editable = true;
        playlist.tracks[1].playlist_item_id = "item-b".into();
    }
    let mut harness = harness(state);
    harness.get_by_label("Second song").click_secondary();
    harness.run();
    harness.get_by_label("Remove from this playlist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::RemoveFromPlaylist { playlist_id, items }
            if playlist_id == "pl" && items == &[("b".to_owned(), "item-b".to_owned())]
    )));
}

pub(super) fn switch<'a>(
    harness: &'a Harness<'_, Fixture>,
    label: &'a str,
) -> egui_kittest::Node<'a> {
    harness.get_by_role_and_label(eframe::egui::accesskit::Role::CheckBox, label)
}

#[test]
pub(super) fn the_visualizer_switch_asks_for_the_other_state() {
    let mut state = state();
    state.nav.open(Page::Settings);
    let mut harness = harness(state);
    // It sits below the fold of the test window.
    switch(&harness, "Player bar visualizer").scroll_to_me();
    harness.run();
    switch(&harness, "Player bar visualizer").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetVisualizer(true)
    )));
}

#[test]
pub(super) fn the_menu_starts_a_radio_from_a_song() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Second song").click_secondary();
    harness.run();
    harness.get_by_label("Go to song radio").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartRadio(track) if track.id == "b"
    )));
}

pub(super) fn with_library() -> State {
    let mut state = state();
    state.library = Loadable::Loaded(vec![LibraryItem {
        id: "pl".into(),
        kind: LibraryKind::Playlist,
        title: "Road trip".into(),
        ..LibraryItem::default()
    }]);
    state.folders = vec![Folder {
        id: "f1".into(),
        name: "Sport".into(),
    }];
    state
}

pub(super) fn on_artist() -> State {
    let mut state = state();
    let artist = Artist {
        id: "ar".into(),
        name: "Bonobo".into(),
        shuffle_id: "RDAO1".into(),
        shuffle_seed: "v1".into(),
        songs_id: "OLAK".into(),
        top_tracks: vec![track("a", "Kerala")],
        ..Artist::default()
    };
    state.artists.insert("ar".into(), Loadable::Loaded(artist));
    state.nav.open(Page::Artist("ar".into()));
    state
}

/// The cursor the window would show with the pointer on the control named
/// `label`.
fn cursor_over(harness: &mut Harness<'_, Fixture>, label: &str) -> eframe::egui::CursorIcon {
    harness.get_by_label(label).hover();
    harness.run();
    harness.output().platform_output.cursor_icon
}

#[test]
fn the_pointer_is_a_hand_over_what_can_be_clicked() {
    let mut harness = harness(playing(on_playlist()));
    let hand = eframe::egui::CursorIcon::PointingHand;
    assert_eq!(cursor_over(&mut harness, "Search"), hand);
    assert_eq!(cursor_over(&mut harness, "Pause"), hand);
    assert_eq!(cursor_over(&mut harness, "Settings"), hand);
    assert_eq!(cursor_over(&mut harness, "Second song"), hand);
}

#[test]
fn the_pointer_is_a_text_cursor_over_the_search_field() {
    let mut harness = harness(state());
    harness.get_by_label("Search music").hover();
    harness.run();
    assert_eq!(
        harness.output().platform_output.cursor_icon,
        eframe::egui::CursorIcon::Text
    );
}

#[test]
fn the_search_field_leads_to_browse_all() {
    let mut harness = harness(state());
    harness.get_by_label("Browse all").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::BrowseAll
    )));
}

#[test]
fn a_narrow_window_narrows_the_sidebar_before_it_squeezes_the_page() {
    use super::Side;

    // Wide: everything fits.
    let wide = super::Room::share(1400.0, false, true, 280.0);
    assert_eq!(wide.sidebar, Side::Full);
    assert_eq!(wide.right_most, 1400.0 - 420.0 - 280.0);
    // Narrower, with a panel open, the sidebar is a rail of covers.
    let narrow = super::Room::share(900.0, false, true, 280.0);
    assert_eq!(narrow.sidebar, Side::Rail);
    assert_eq!(narrow.right_most, 900.0 - 420.0 - theme::SIDEBAR_RAIL_WIDTH);
    // At the window's least width there is no room even for that.
    let least = super::Room::share(760.0, false, true, 280.0);
    assert_eq!(least.sidebar, Side::Away);
    assert_eq!(least.right_most, 340.0);
    // With no panel open the same window keeps its sidebar.
    assert_eq!(
        super::Room::share(760.0, false, false, 280.0).sidebar,
        Side::Full
    );
    // And a sidebar that was collapsed on purpose stays a rail.
    assert_eq!(
        super::Room::share(1400.0, true, false, 280.0).sidebar,
        Side::Rail
    );
}
