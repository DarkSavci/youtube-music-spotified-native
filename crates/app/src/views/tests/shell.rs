//! The top bar, the sidebar's shapes, Home's moods, and the menus on a
//! page's row of buttons.

use super::*;
use crate::settings::LibrarySort;
use crate::share;
use spotified_client::models::{Account, HomeChip};

fn signed_in(mut state: State) -> State {
    state.account = Some(Account {
        name: "Ada".into(),
        handle: "@ada".into(),
        ..Account::default()
    });
    state
}

#[test]
fn the_top_bar_leads_home_and_to_what_is_new() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Home").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Home)
    )));
    harness.get_by_label("What's new").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ShowWhatsNew
    )));
}

#[test]
fn the_account_chip_opens_a_menu_of_its_pages() {
    let mut harness = harness(signed_in(state()));
    harness.get_by_label("Account").click();
    harness.run();
    harness.get_by_label("Recently played").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::History)
    )));
    harness.get_by_label("Account").click();
    harness.run();
    harness.get_by_label("Sign out").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::SignOut)));
}

#[test]
fn signed_out_the_top_bar_offers_to_sign_in() {
    let mut harness = harness(state());
    assert!(harness.query_by_label("Account").is_none());
    harness.get_by_label("Sign in to YouTube Music").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::SignIn)));
}

#[test]
fn the_library_header_collapses_the_sidebar_and_the_rail_widens_it() {
    let mut harness = harness(with_library());
    harness.get_by_label("Collapse Your Library").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleSidebar
    )));

    let mut collapsed = with_library();
    collapsed.settings.sidebar_collapsed = true;
    let mut harness = super::harness(collapsed);
    // The rail has the covers, each still named, and none of the tools.
    assert!(harness.query_by_label("Road trip").is_some());
    assert!(harness.query_by_label("Sort library").is_none());
    harness.get_by_label("Expand Your Library").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleSidebar
    )));
}

#[test]
fn the_sort_menu_offers_every_order() {
    let mut harness = harness(with_library());
    harness.get_by_label("Sort library").click();
    harness.run();
    for label in ["Recently added", "Alphabetical"] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    harness.get_by_label("Creator").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetLibrarySort(LibrarySort::Creator)
    )));
}

#[test]
fn the_library_can_be_shown_as_a_grid_and_keeps_what_its_things_do() {
    let mut harness = harness(with_library());
    harness.get_by_label("Show as grid").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleLibraryGrid
    )));

    let mut grid = with_library();
    grid.settings.library_grid = true;
    let mut harness = super::harness(grid);
    assert!(harness.query_by_label("Show as list").is_some());
    // Its name under the cover opens it; the cover itself, under the
    // pointer, is a play button, as in the list.
    let cell = harness.get_by_label("Road trip").rect();
    let name = cell.center_bottom() - vec2(0.0, 24.0);
    harness.hover_at(name);
    harness.run();
    harness.drag_at(name);
    harness.drop_at(name);
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Playlist(id)) if id == "pl"
    )));
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::PlayCollection(_)
    )));
    harness.hover_at(cell.center());
    harness.run();
    harness.get_by_label("Play Road trip").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayCollection(Page::Playlist(id)) if id == "pl"
    )));
    harness.get_by_label("Sport").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleFolder(id) if id == "f1"
    )));
}

#[test]
fn the_library_can_take_the_pages_room_and_give_it_back() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Expand library view").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleLibraryExpanded
    )));

    let mut wide = on_playlist();
    wide.library_expanded = true;
    let mut harness = super::harness(wide);
    // The page has made way.
    assert!(harness.query_by_label("First song").is_none());
    harness.get_by_label("Collapse library view").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleLibraryExpanded
    )));
}

#[test]
fn a_mood_chip_on_home_asks_for_that_mood_and_the_chosen_one_for_none() {
    let mut state = state();
    state.home_chips = vec![
        HomeChip {
            title: "Energize".into(),
            params: "en".into(),
            selected: false,
        },
        HomeChip {
            title: "Relax".into(),
            params: "re".into(),
            selected: false,
        },
    ];
    state.home_mood = "re".into();
    let mut harness = harness(state);
    harness.get_by_label("Energize").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ChooseMood(params) if params == "en"
    )));
    // Clicked again, the chosen chip asks for itself, which clears it.
    harness.get_by_label("Relax").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ChooseMood(params) if params == "re"
    )));
}

fn own_playlist() -> State {
    let mut state = on_playlist();
    if let Some(playlist) = state.playlists.loaded_mut(&"pl".to_owned()) {
        playlist.editable = true;
    }
    state.library = Loadable::Loaded(vec![
        LibraryItem {
            id: "pl".into(),
            kind: LibraryKind::Playlist,
            title: "Road trip".into(),
            ..LibraryItem::default()
        },
        LibraryItem {
            id: "other".into(),
            kind: LibraryKind::Playlist,
            title: "Evening".into(),
            ..LibraryItem::default()
        },
    ]);
    state
}

/// Opens the menu on the page's row of buttons and clicks `entry`.
fn choose(harness: &mut Harness<'_, Fixture>, entry: &str) {
    harness.get_by_label("More options for Road trip").click();
    harness.run();
    harness.get_by_label(entry).click();
    harness.run();
}

#[test]
fn a_pages_menu_queues_the_whole_of_it() {
    let mut harness = harness(own_playlist());
    choose(&mut harness, "Add to queue");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(tracks) if tracks.len() == 3
    )));
    choose(&mut harness, "Play next");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayNext(tracks) if tracks.len() == 3
    )));
}

#[test]
fn a_pages_menu_puts_all_of_it_in_a_new_playlist_named_after_it() {
    let mut harness = harness(own_playlist());
    choose(&mut harness, "Add all to a new playlist…");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::NewPlaylist { name, track_ids } if name == "Road trip" && track_ids.len() == 3
    )));
}

#[test]
fn a_pages_menu_adds_all_of_it_to_another_of_your_playlists() {
    let mut harness = harness(own_playlist());
    harness.get_by_label("More options for Road trip").click();
    harness.run();
    harness.get_by_label_contains("Add all to…").click();
    harness.run();
    // The page's own playlist is not offered.
    assert!(harness.query_by_label("Add all to Road trip").is_none());
    harness.get_by_label("Add all to Evening").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToPlaylist { playlist_id, track_ids, .. }
            if playlist_id == "other" && track_ids.len() == 3
    )));
}

#[test]
fn a_pages_menu_shares_it_and_deletes_only_a_playlist_of_your_own() {
    let mut harness = harness(own_playlist());
    choose(&mut harness, "Share");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Share { kind: share::Kind::Playlist, id } if id == "pl"
    )));
    choose(&mut harness, "Delete playlist");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AskDeletePlaylist { playlist_id, .. } if playlist_id == "pl"
    )));

    // Somebody else's playlist can be shared, not deleted.
    let mut harness = super::harness(on_playlist());
    harness.get_by_label("More options for Road trip").click();
    harness.run();
    assert!(harness.query_by_label("Delete playlist").is_none());
    assert!(harness.query_by_label("Share").is_some());
}

#[test]
fn every_row_shows_its_heart_and_its_menu_button_without_the_pointer() {
    let mut harness = harness(on_playlist());
    harness.get_by_label("Save Second song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleLike(track) if track.id == "b"
    )));
    harness.get_by_label("More options for Third song").click();
    harness.run();
    harness.get_by_label("Go to song radio").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartRadio(track) if track.id == "c"
    )));
}

#[test]
fn an_artists_page_follows_from_a_chip_and_shares_from_its_menu() {
    let mut harness = harness(on_artist());
    harness.get_by_label("Follow").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleFollow(id) if id == "ar"
    )));
    harness.get_by_label("More options for Bonobo").click();
    harness.run();
    harness.get_by_label("Share").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Share { kind: share::Kind::Artist, id } if id == "ar"
    )));
}

#[test]
fn with_nothing_playing_the_bar_keeps_the_buttons_that_need_no_song() {
    let mut harness = harness(state());
    for label in ["Lyrics", "Queue", "Mute", "Mini player"] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    harness.get_by_label("Mini player").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleMiniPlayer
    )));
    // Nothing is playing, so nothing can be skipped.
    harness.get_by_label("Next").click();
    harness.run();
    assert!(!asked(&harness, |action| matches!(action, Action::Next)));
}

#[test]
fn the_playing_song_can_be_shared_from_the_bar() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Share").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Share {
            kind: share::Kind::Track,
            ..
        }
    )));
}

#[test]
fn the_play_button_stays_in_sight_when_a_long_playlist_is_scrolled() {
    use eframe::egui::{Event, Modifiers, MouseWheelUnit, TouchPhase};

    let mut state = on_playlist();
    if let Some(playlist) = state.playlists.loaded_mut(&"pl".to_owned()) {
        playlist.tracks = (0..40)
            .map(|index| track(&format!("t{index}"), &format!("Song {index}")))
            .collect();
    }
    let mut harness = harness(state);
    let before = harness.get_by_label("Play Road trip").rect().top();
    harness.get_by_label("Song 3").hover();
    harness.run();
    for _ in 0..4 {
        harness.event(Event::MouseWheel {
            unit: MouseWheelUnit::Point,
            delta: vec2(0.0, -400.0),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        // A scroll glides to rest, and what the pointer is left over may
        // have a tooltip to show: a few frames, not until all is still.
        harness.run_steps(12);
    }
    // The first songs have gone, and the button has not gone with them: it
    // is pinned at the top of the page, above where it began.
    assert!(harness.query_by_label("Song 3").is_none());
    let pinned = harness.get_by_label("Play Road trip").rect().top();
    assert!(pinned < before && pinned > 0.0, "{pinned} against {before}");
    harness.get_by_label("Play Road trip").click();
    harness.run_steps(4);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayPlaylist { id, index: 0 } if id == "pl"
    )));
}
