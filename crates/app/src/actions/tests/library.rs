//! The library: likes, playlists, pins and folders.

use super::*;

#[test]
fn a_ready_core_loads_the_open_page_and_the_library() {
    let state = &mut state();
    let effects = apply(
        state,
        Action::CoreChanged(CoreStatus::Ready {
            origin: "http://127.0.0.1:1".into(),
        }),
    );
    assert_eq!(
        effects,
        [
            Effect::Fetch(Request::Home(String::new())),
            Effect::Fetch(Request::Library),
            Effect::Fetch(Request::Liked),
            Effect::Fetch(Request::Mixes),
            Effect::Fetch(Request::Account),
            Effect::Fetch(Request::Folders)
        ]
    );
    assert_eq!(state.home, Loadable::Loading);
}

#[test]
fn a_change_of_account_starts_afresh() {
    let mut state = ready();
    apply(&mut state, Action::AccountChanged);
    assert_eq!(state.core, CoreStatus::Starting);
    assert_eq!(state.home, Loadable::NotLoaded);
    assert_eq!(state.library, Loadable::NotLoaded);
}

#[test]
fn a_like_shows_at_once_and_a_refusal_puts_it_back() {
    let mut state = ready();
    let track = Track {
        id: "a".into(),
        ..Track::default()
    };
    let effects = apply(&mut state, Action::ToggleLike(track));
    assert!(state.likes.is_liked("a"));
    assert_eq!(
        effects,
        [Effect::Fetch(Request::SetLiked {
            track_id: "a".into(),
            liked: true
        })]
    );
    let refused = Response::LikeSet {
        track_id: "a".into(),
        liked: true,
        result: Err(spotified_client::ApiError::RateLimited),
    };
    apply(&mut state, Action::Loaded(Box::new(refused)));
    assert!(!state.likes.is_liked("a"));
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
fn a_new_playlist_needs_a_name_before_it_is_created() {
    let mut state = ready();
    apply(
        &mut state,
        Action::NewPlaylist {
            name: String::new(),
            track_ids: vec!["a".into()],
        },
    );
    assert!(apply(&mut state, Action::ConfirmDialog).is_empty());
    assert!(state.dialog.is_some());

    apply(&mut state, Action::SetDialogText("  Road trip ".into()));
    let effects = apply(&mut state, Action::ConfirmDialog);
    assert_eq!(
        effects,
        [Effect::Fetch(Request::CreatePlaylist {
            title: "Road trip".into(),
            track_ids: vec!["a".into()],
        })]
    );
    assert!(state.dialog.is_none());
}

#[test]
fn deleting_the_open_playlist_leaves_its_page() {
    let mut state = ready();
    apply(&mut state, Action::Open(Page::Playlist("pl".into())));
    let deleted = Response::PlaylistDeleted {
        playlist_id: "pl".into(),
        title: "Road trip".into(),
        result: Ok(()),
    };
    let effects = apply(&mut state, Action::Loaded(Box::new(deleted)));
    assert_eq!(state.nav.page(), &Page::Home);
    assert_eq!(effects, [Effect::Fetch(Request::Library)]);
}

#[test]
fn a_refused_follow_is_put_back() {
    use spotified_client::models::Artist;

    let mut state = ready();
    state
        .artists
        .insert("ar".into(), Loadable::Loaded(Artist::default()));
    apply(&mut state, Action::ToggleFollow("ar".into()));
    assert!(
        state
            .artists
            .loaded_mut(&"ar".into())
            .is_some_and(|a| a.following)
    );
    let refused = Response::FollowingSet {
        artist_id: "ar".into(),
        follow: true,
        result: Err(spotified_client::ApiError::SignedOut),
    };
    apply(&mut state, Action::Loaded(Box::new(refused)));
    assert!(
        state
            .artists
            .loaded_mut(&"ar".into())
            .is_some_and(|a| !a.following)
    );
}

#[test]
fn a_library_chip_toggles() {
    let mut state = state();
    apply(&mut state, Action::FilterLibrary(LibraryKind::Album));
    assert_eq!(state.library_filter, Some(LibraryKind::Album));
    apply(&mut state, Action::FilterLibrary(LibraryKind::Album));
    assert_eq!(state.library_filter, None);
}

#[test]
fn dropping_songs_on_liked_music_likes_only_the_ones_not_liked() {
    let mut state = ready();
    let track = |id: &str| Track {
        id: id.into(),
        ..Track::default()
    };
    apply(&mut state, Action::ToggleLike(track("a")));
    let effects = apply(&mut state, Action::LikeAll(vec![track("a"), track("b")]));
    assert_eq!(
        effects,
        [Effect::Fetch(Request::SetLiked {
            track_id: "b".into(),
            liked: true
        })]
    );
    assert!(state.likes.is_liked("a") && state.likes.is_liked("b"));
}

#[test]
fn pinning_shows_at_once_and_is_told_to_the_core() {
    let mut state = with_library(vec![playlist_item("pl")]);
    let effects = apply(
        &mut state,
        Action::SetPinned {
            kind: LibraryKind::Playlist,
            item_id: "pl".into(),
            pinned: true,
        },
    );
    assert_eq!(
        effects,
        [Effect::Fetch(Request::Organise {
            kind: LibraryKind::Playlist,
            item_id: "pl".into(),
            pinned: Some(true),
            folder_id: None,
        })]
    );
    assert!(state.library.get().is_some_and(|library| library[0].pinned));
}

#[test]
fn filing_in_a_folder_opens_it_and_taking_out_leaves_it_be() {
    let mut state = with_library(vec![playlist_item("pl")]);
    let file = |folder: &str| Action::MoveToFolder {
        kind: LibraryKind::Playlist,
        item_id: "pl".into(),
        folder_id: folder.into(),
    };
    apply(&mut state, file("f1"));
    assert!(state.open_folders.contains("f1"));
    let effects = apply(&mut state, file(""));
    assert_eq!(
        effects,
        [Effect::Fetch(Request::Organise {
            kind: LibraryKind::Playlist,
            item_id: "pl".into(),
            pinned: None,
            folder_id: Some(String::new()),
        })]
    );
    assert!(
        state
            .library
            .get()
            .is_some_and(|library| library[0].folder_id.is_empty())
    );
}

#[test]
fn a_refused_change_fetches_the_library_as_it_really_is() {
    let mut state = with_library(vec![playlist_item("pl")]);
    let refused = Response::Organised(Err(ApiError::RateLimited));
    let effects = apply(&mut state, Action::Loaded(Box::new(refused)));
    assert_eq!(
        effects,
        [
            Effect::Fetch(Request::Folders),
            Effect::Fetch(Request::Library)
        ]
    );
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
fn a_new_folder_needs_a_name_before_it_is_made() {
    let mut state = ready();
    apply(&mut state, Action::NewFolder);
    assert!(apply(&mut state, Action::ConfirmDialog).is_empty());
    assert!(state.dialog.is_some());

    apply(&mut state, Action::SetDialogText(" Sport ".into()));
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        [Effect::Fetch(Request::CreateFolder("Sport".into()))]
    );
    assert!(state.dialog.is_none());
}

#[test]
fn a_folder_opens_and_shuts_and_goes_when_deleted() {
    let mut state = ready();
    state.folders = vec![Folder {
        id: "f1".into(),
        name: "Sport".into(),
    }];
    apply(&mut state, Action::ToggleFolder("f1".into()));
    assert!(state.open_folders.contains("f1"));
    apply(&mut state, Action::ToggleFolder("f1".into()));
    assert!(state.open_folders.is_empty());

    assert_eq!(
        apply(&mut state, Action::DeleteFolder("f1".into())),
        [Effect::Fetch(Request::DeleteFolder("f1".into()))]
    );
    assert!(state.folders.is_empty());
}

#[test]
fn an_artists_shuffle_is_started_for_this_device() {
    let mut state = ready();
    state.settings.device_id = "native-1".into();
    let seed = MixSeed {
        playlist_id: "RDAO1".into(),
        video_id: "v1".into(),
        params: String::new(),
    };
    let effects = apply(
        &mut state,
        Action::StartMix {
            seed: seed.clone(),
            origin: "Bonobo".into(),
        },
    );
    assert_eq!(
        effects,
        [Effect::Fetch(Request::StartMix {
            device_id: "native-1".into(),
            seed,
            origin: "Bonobo".into(),
        })]
    );
}
