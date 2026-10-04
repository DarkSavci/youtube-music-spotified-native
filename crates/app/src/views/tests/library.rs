//! The sidebar: the library, its folders, and what is dropped on it.

use super::*;

#[test]
fn the_magnifier_leads_to_search() {
    let mut harness = harness(state());
    harness.get_by_label("Search").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Search)
    )));
}

#[test]
fn the_plus_in_the_sidebar_asks_for_a_new_playlist() {
    let mut harness = harness(state());
    harness.get_by_label("Create playlist or folder").click();
    harness.run();
    harness.get_by_label("Create a playlist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::NewPlaylist { name, track_ids } if name == "My playlist" && track_ids.is_empty()
    )));
}

#[test]
fn the_sidebar_offers_to_sign_in_when_signed_out() {
    let mut harness = harness(state());
    harness.get_by_label("Sign in").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::SignIn)));
}

#[test]
fn a_song_dragged_onto_a_sidebar_playlist_is_added_to_it() {
    use spotified_client::models::{LibraryItem, LibraryKind};

    let mut state = on_playlist();
    state.library = Loadable::Loaded(vec![LibraryItem {
        id: "mine".into(),
        kind: LibraryKind::Playlist,
        title: "My mix".into(),
        ..LibraryItem::default()
    }]);
    let mut harness = harness(state);
    let from = harness.get_by_label("Second song").rect().center();
    let to = harness.get_by_label("My mix").rect().center();
    harness.drag_at(from);
    harness.run();
    // A drag begins once the pointer has moved, and lands where it is let go.
    harness.hover_at(from + eframe::egui::vec2(20.0, 20.0));
    harness.run();
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToPlaylist { playlist_id, track_ids, .. }
            if playlist_id == "mine" && track_ids == &["b".to_owned()]
    )));
}

#[test]
fn a_library_item_can_be_pinned_from_its_menu() {
    let mut harness = harness(with_library());
    harness.get_by_label("Road trip").click_secondary();
    harness.run();
    harness.get_by_label("Pin to top").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetPinned { item_id, pinned: true, .. } if item_id == "pl"
    )));
}

#[test]
fn a_folder_opens_with_a_click() {
    let mut harness = harness(with_library());
    harness.get_by_label("Sport").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleFolder(id) if id == "f1"
    )));
}

#[test]
fn the_plus_in_the_sidebar_also_makes_folders() {
    let mut harness = harness(state());
    harness.get_by_label("Create playlist or folder").click();
    harness.run();
    harness.get_by_label("Create a folder").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::NewFolder
    )));
}
