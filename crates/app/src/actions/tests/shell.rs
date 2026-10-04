//! The sidebar's shape, Home's moods, and sharing.

use super::*;
use crate::settings::LibrarySort;
use crate::share;
use spotified_client::models::HomeChip;

#[test]
fn collapsing_the_sidebar_is_remembered_and_forgets_what_was_typed_in_it() {
    let mut state = state();
    state.library_query = "daft".into();
    let effects = apply(&mut state, Action::ToggleSidebar);
    assert!(state.settings.sidebar_collapsed);
    // The rail has no field to clear it from.
    assert!(state.library_query.is_empty());
    assert_eq!(effects, [Effect::SaveSettings]);

    apply(&mut state, Action::ToggleSidebar);
    assert!(!state.settings.sidebar_collapsed);
}

#[test]
fn an_order_chosen_for_the_library_is_remembered() {
    let mut state = state();
    for sort in [
        LibrarySort::Added,
        LibrarySort::Alphabetical,
        LibrarySort::Creator,
        LibrarySort::Recent,
    ] {
        let effects = apply(&mut state, Action::SetLibrarySort(sort));
        assert_eq!(state.settings.library_sort, sort);
        assert_eq!(effects, [Effect::SaveSettings]);
    }
    // Choosing the order already chosen changes nothing.
    assert!(apply(&mut state, Action::SetLibrarySort(LibrarySort::Recent)).is_empty());
}

#[test]
fn the_sidebar_and_the_wide_library_each_remember_rows_or_a_grid() {
    let mut state = state();
    // The sidebar starts as rows, the wide library as a grid.
    assert!(!state.settings.library_grid);
    assert!(state.settings.library_expanded_grid);

    let effects = apply(&mut state, Action::ToggleLibraryGrid);
    assert!(state.settings.library_grid);
    assert!(state.settings.library_expanded_grid);
    assert_eq!(effects, [Effect::SaveSettings]);

    apply(&mut state, Action::ToggleLibraryExpanded);
    apply(&mut state, Action::ToggleLibraryGrid);
    assert!(state.settings.library_grid);
    assert!(!state.settings.library_expanded_grid);
}

#[test]
fn the_wide_library_gives_the_room_back_when_a_page_is_opened() {
    let mut state = ready();
    assert!(apply(&mut state, Action::ToggleLibraryExpanded).is_empty());
    assert!(state.library_expanded);
    apply(&mut state, Action::Open(Page::Album("al".into())));
    assert!(!state.library_expanded);

    apply(&mut state, Action::ToggleLibraryExpanded);
    apply(&mut state, Action::SetSearchQuery("daft".into()));
    assert!(!state.library_expanded);
}

fn home_with_chips() -> BrowsePage {
    BrowsePage {
        chips: vec![
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
        ],
        ..BrowsePage::default()
    }
}

#[test]
fn a_mood_chip_reads_home_again_through_it() {
    let mut state = ready();
    let loaded = Response::Home(String::new(), Ok(home_with_chips()));
    apply(&mut state, Action::Loaded(Box::new(loaded)));
    assert_eq!(state.home_chips.len(), 2);

    let effects = apply(&mut state, Action::ChooseMood("en".into()));
    assert_eq!(state.home_mood, "en");
    assert_eq!(state.home, Loadable::Loading);
    assert_eq!(effects, [Effect::Fetch(Request::Home("en".into()))]);
    // The row of moods stays while the mood's page is on its way.
    assert_eq!(state.home_chips.len(), 2);
}

#[test]
fn choosing_the_chosen_mood_goes_back_to_plain_home() {
    let mut state = ready();
    apply(&mut state, Action::ChooseMood("en".into()));
    let effects = apply(&mut state, Action::ChooseMood("en".into()));
    assert!(state.home_mood.is_empty());
    assert_eq!(effects, [Effect::Fetch(Request::Home(String::new()))]);
}

#[test]
fn a_home_that_arrives_for_another_mood_is_dropped() {
    let mut state = ready();
    apply(&mut state, Action::ChooseMood("en".into()));
    // Plain Home, asked for before the chip was chosen, arrives late.
    let late = Response::Home(String::new(), Ok(home_with_chips()));
    apply(&mut state, Action::Loaded(Box::new(late)));
    assert_eq!(state.home, Loadable::Loading);

    let wanted = Response::Home("en".into(), Ok(BrowsePage::default()));
    apply(&mut state, Action::Loaded(Box::new(wanted)));
    assert!(matches!(state.home, Loadable::Loaded(_)));
}

#[test]
fn sharing_copies_the_youtube_music_link_and_says_so() {
    let mut state = state();
    let effects = apply(
        &mut state,
        Action::Share {
            kind: share::Kind::Album,
            id: "MPREb_1".into(),
        },
    );
    assert_eq!(
        effects,
        [Effect::CopyToClipboard(
            "https://music.youtube.com/browse/MPREb_1".into()
        )]
    );
    let said = state.toasts.last().map(|toast| toast.text.as_str());
    assert_eq!(said, Some("Album link copied to clipboard"));
}

#[test]
fn a_new_playlist_can_start_with_a_name_to_keep_or_change() {
    let mut state = ready();
    apply(
        &mut state,
        Action::NewPlaylist {
            name: "Discovery".into(),
            track_ids: vec!["a".into(), "b".into()],
        },
    );
    let effects = apply(&mut state, Action::ConfirmDialog);
    assert_eq!(
        effects,
        [Effect::Fetch(Request::CreatePlaylist {
            title: "Discovery".into(),
            track_ids: vec!["a".into(), "b".into()],
        })]
    );
}

#[test]
fn an_update_that_has_arrived_is_said_once_in_the_window_and_by_the_system() {
    let mut state = ready();
    let arrived = || crate::update::Status::Ready {
        version: "9.9.9".into(),
        installer: "setup.exe".into(),
    };
    assert_eq!(
        apply(&mut state, Action::UpdateChanged(arrived())),
        [Effect::NotifyUpdate("9.9.9".into())]
    );
    assert_eq!(state.toasts.len(), 1);
    assert!(state.toasts[0].text.contains("installs when you quit"));
    // Heard of again, as each check reports it: nothing more is said.
    assert!(apply(&mut state, Action::UpdateChanged(arrived())).is_empty());
    assert_eq!(state.toasts.len(), 1);
}
