//! Pages read a part at a time, and what playing asks of them.

use spotified_client::models::{Artist, Playlist, PlaylistPage, RemoteQueue, Shelf};
use spotified_client::session::Command;

use std::time::Instant;

use super::*;
use crate::backend::{ArtistQueue, ArtistSeed};
use crate::state::{ToastLink, Whole};

fn loaded(state: &mut State, response: Response) -> Vec<Effect> {
    apply(state, Action::Loaded(Box::new(response)))
}

fn home_page(continuation: &str) -> BrowsePage {
    BrowsePage {
        shelves: vec![Shelf {
            title: "Quick picks".into(),
            ..Shelf::default()
        }],
        continuation: continuation.into(),
        ..BrowsePage::default()
    }
}

/// Home, with its first page here and more behind the token `first`.
fn at_home() -> State {
    let mut state = ready();
    loaded(
        &mut state,
        Response::Home(String::new(), Ok(home_page("first"))),
    );
    state
}

#[test]
fn the_end_of_home_asks_for_its_next_shelves_once() {
    let mut state = at_home();
    let more = Action::MoreHome { retry: false };
    assert_eq!(
        apply(&mut state, more),
        [Effect::Fetch(Request::HomeMore(
            String::new(),
            "first".into()
        ))]
    );
    // Still in view on the next frame: the page is already on its way.
    assert!(apply(&mut state, Action::MoreHome { retry: false }).is_empty());
}

#[test]
fn more_of_home_is_added_below_and_read_on_from_its_own_token() {
    let mut state = at_home();
    apply(&mut state, Action::MoreHome { retry: false });
    let answer = Response::HomeMore(String::new(), "first".into(), Ok(home_page("second")));
    loaded(&mut state, answer);
    assert_eq!(state.home_more.shelves.len(), 1);
    assert_eq!(
        apply(&mut state, Action::MoreHome { retry: false }),
        [Effect::Fetch(Request::HomeMore(
            String::new(),
            "second".into()
        ))]
    );
    // A token seen before would page in a circle; Home ends there.
    let answer = Response::HomeMore(String::new(), "second".into(), Ok(home_page("first")));
    loaded(&mut state, answer);
    assert_eq!(state.home_more.shelves.len(), 2);
    assert!(apply(&mut state, Action::MoreHome { retry: false }).is_empty());
}

#[test]
fn more_of_home_that_failed_waits_to_be_asked_for_again() {
    let mut state = at_home();
    apply(&mut state, Action::MoreHome { retry: false });
    let answer = Response::HomeMore(String::new(), "first".into(), Err(ApiError::RateLimited));
    loaded(&mut state, answer);
    assert!(state.home_more.tail.failed.is_some());
    assert!(apply(&mut state, Action::MoreHome { retry: false }).is_empty());
    assert_eq!(apply(&mut state, Action::MoreHome { retry: true }).len(), 1);
}

#[test]
fn more_of_home_for_another_mood_is_dropped() {
    let mut state = at_home();
    apply(&mut state, Action::MoreHome { retry: false });
    apply(&mut state, Action::ChooseMood("relax".into()));
    let late = Response::HomeMore(String::new(), "first".into(), Ok(home_page("second")));
    loaded(&mut state, late);
    assert!(state.home_more.shelves.is_empty());
}

fn playlist_page(ids: &[&str], next: &str) -> PlaylistPage {
    PlaylistPage {
        playlist: Playlist {
            id: "pl".into(),
            title: "Road trip".into(),
            tracks: ids.iter().map(|id| track(id)).collect(),
            ..Playlist::default()
        },
        next: next.into(),
    }
}

/// A playlist whose first two songs are here, with more behind `next`.
fn on_long_playlist(next: &str) -> State {
    let mut state = ready();
    apply(&mut state, Action::Open(Page::Playlist("pl".into())));
    let first = Response::Playlist("pl".into(), Ok(playlist_page(&["a", "b"], next)));
    loaded(&mut state, first);
    state
}

fn songs_of(state: &State) -> Vec<&str> {
    match state.playlists.get(&"pl".to_owned()) {
        Loadable::Loaded(playlist) => playlist.tracks.iter().map(|t| t.id.as_str()).collect(),
        _ => Vec::new(),
    }
}

#[test]
fn a_long_playlist_is_read_a_page_at_a_time() {
    let mut state = on_long_playlist("t1");
    let more = || Action::MorePlaylist {
        id: "pl".into(),
        retry: false,
    };
    assert_eq!(
        apply(&mut state, more()),
        [Effect::Fetch(Request::PlaylistMore {
            id: "pl".into(),
            token: "t1".into(),
        })]
    );
    assert!(apply(&mut state, more()).is_empty());
    let page = Response::PlaylistMore {
        id: "pl".into(),
        token: "t1".into(),
        result: Ok(playlist_page(&["c"], "")),
    };
    loaded(&mut state, page);
    assert_eq!(songs_of(&state), ["a", "b", "c"]);
    // Its last page has come: there is nothing more to ask for.
    assert!(state.playlist_tails.is_empty());
    assert!(apply(&mut state, more()).is_empty());
}

#[test]
fn a_playlist_all_here_plays_at_once() {
    let mut state = on_long_playlist("");
    let effects = apply(
        &mut state,
        Action::PlayPlaylist {
            id: "pl".into(),
            index: 1,
        },
    );
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, start_index: 1, origin })]
            if tracks.len() == 2 && origin == "Road trip"
    ));
}

#[test]
fn playing_a_playlist_part_read_reads_the_rest_first_and_plays_all_of_it() {
    let mut state = on_long_playlist("t1");
    let play = Action::PlayPlaylist {
        id: "pl".into(),
        index: 1,
    };
    assert_eq!(
        apply(&mut state, play),
        [Effect::Fetch(Request::PlaylistRest {
            id: "pl".into(),
            token: "t1".into(),
        })]
    );
    let rest = Response::PlaylistRest {
        id: "pl".into(),
        token: "t1".into(),
        result: Ok(vec![track("c"), track("d")]),
    };
    let effects = loaded(&mut state, rest);
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, start_index: 1, .. })] if tracks.len() == 4
    ));
    assert_eq!(songs_of(&state), ["a", "b", "c", "d"]);
    assert_eq!(state.preparing_playlist, None);
}

#[test]
fn a_playlist_that_cannot_be_read_to_its_end_is_not_played_in_part() {
    let mut state = on_long_playlist("t1");
    let play = Action::PlayPlaylist {
        id: "pl".into(),
        index: 0,
    };
    apply(&mut state, play);
    let failed = Response::PlaylistRest {
        id: "pl".into(),
        token: "t1".into(),
        result: Err(ApiError::RateLimited),
    };
    assert!(loaded(&mut state, failed).is_empty());
    assert_eq!(state.preparing_playlist, None);
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
fn something_else_played_meanwhile_is_not_taken_over_by_the_playlist() {
    let mut state = on_long_playlist("t1");
    let play = Action::PlayPlaylist {
        id: "pl".into(),
        index: 0,
    };
    apply(&mut state, play);
    apply(&mut state, Action::StartRadio(track("z")));
    let rest = Response::PlaylistRest {
        id: "pl".into(),
        token: "t1".into(),
        result: Ok(vec![track("c")]),
    };
    assert!(loaded(&mut state, rest).is_empty());
    // What was read is kept all the same.
    assert_eq!(songs_of(&state), ["a", "b", "c"]);
}

#[test]
fn a_playlist_card_plays_the_whole_playlist_once_its_first_page_shows_more() {
    let mut state = ready();
    let page = Page::Playlist("pl".into());
    assert_eq!(
        apply(&mut state, Action::PlayCollection(page)),
        [Effect::Fetch(Request::Playlist("pl".into()))]
    );
    let first = Response::Playlist("pl".into(), Ok(playlist_page(&["a", "b"], "t1")));
    assert_eq!(
        loaded(&mut state, first),
        [Effect::Fetch(Request::PlaylistRest {
            id: "pl".into(),
            token: "t1".into(),
        })]
    );
}

fn artist() -> Artist {
    Artist {
        id: "ar".into(),
        name: "Bonobo".into(),
        songs_id: "OLAK".into(),
        top_tracks: vec![track("a")],
        ..Artist::default()
    }
}

#[test]
fn an_artist_played_from_a_card_is_read_on_the_way() {
    let mut state = ready();
    let effects = apply(
        &mut state,
        Action::PlayCollection(Page::Artist("ar".into())),
    );
    assert_eq!(
        effects,
        [Effect::Fetch(Request::PlayArtist {
            device_id: String::new(),
            artist_id: "ar".into(),
            known: None,
            shuffle: false,
        })]
    );
}

#[test]
fn an_artist_played_from_their_page_sends_what_the_page_gave() {
    let mut state = ready();
    let page = Response::Artist("ar".into(), Ok(Box::new(artist())));
    loaded(&mut state, page);
    let effects = apply(
        &mut state,
        Action::PlayArtist {
            artist_id: "ar".into(),
            shuffle: true,
        },
    );
    let known = Some(ArtistSeed::of(&artist()));
    assert!(matches!(
        &effects[..],
        [Effect::Fetch(Request::PlayArtist { known: seed, shuffle: true, .. })] if seed == &known
    ));
    // YouTube names no shuffle for them, so none is sent to try first.
    assert!(known.is_some_and(|seed| seed.shuffle.is_none() && seed.songs_id == "OLAK"));
}

#[test]
fn an_artists_songs_are_played_once_they_have_been_gathered() {
    let mut state = ready();
    let gathered = Response::ArtistQueue {
        shuffle: false,
        result: Ok(ArtistQueue::Songs {
            tracks: vec![track("a"), track("b")],
            origin: "Bonobo".into(),
        }),
    };
    let effects = loaded(&mut state, gathered);
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, start_index: 0, origin })]
            if tracks.len() == 2 && origin == "Bonobo"
    ));
    // A queue the core started itself needs nothing more.
    let started = Response::ArtistQueue {
        shuffle: true,
        result: Ok(ArtistQueue::Started),
    };
    assert!(loaded(&mut state, started).is_empty());
}

#[test]
fn an_artist_with_nothing_to_play_says_so() {
    let mut state = ready();
    let nothing = Response::ArtistQueue {
        shuffle: true,
        result: Ok(ArtistQueue::Songs {
            tracks: Vec::new(),
            origin: "Bonobo · Shuffle".into(),
        }),
    };
    assert!(loaded(&mut state, nothing).is_empty());
    let said = state.toasts.last().map(|toast| toast.text.as_str());
    assert_eq!(said, Some("Could not shuffle this artist."));
}

#[test]
fn an_artists_page_asks_for_your_history_with_them_on_every_visit() {
    let mut state = ready();
    let open = || Action::Open(Page::Artist("ar".into()));
    let affinity = Effect::Fetch(Request::Affinity("ar".into()));
    assert_eq!(
        apply(&mut state, open()),
        [
            affinity.clone(),
            Effect::Fetch(Request::Artist("ar".into()))
        ]
    );
    loaded(
        &mut state,
        Response::Artist("ar".into(), Ok(Box::new(artist()))),
    );
    apply(&mut state, Action::Open(Page::Home));
    assert_eq!(apply(&mut state, open()), [affinity]);
}

#[test]
fn the_song_already_playing_is_not_started_over_by_its_card() {
    let mut state = playing();
    assert!(apply(&mut state, Action::StartRadio(track("a"))).is_empty());
    let effects = apply(&mut state, Action::StartRadio(track("b")));
    assert!(matches!(
        &effects[..],
        [Effect::Fetch(Request::StartRadio { track, .. })] if track.id == "b"
    ));
}

#[test]
fn a_queue_from_another_device_starts_where_that_device_was() {
    let mut unplayable = track("b");
    unplayable.playable = false;
    let queue = RemoteQueue {
        tracks: vec![track("a"), unplayable, track("c")],
        index: 1,
        title: " Liked Music ".into(),
    };
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::ContinueFromRemote),
        [Effect::Fetch(Request::RemoteQueue)]
    );
    // Asked again while it is being read: nothing more is sent.
    assert!(apply(&mut state, Action::ContinueFromRemote).is_empty());
    let effects = loaded(&mut state, Response::RemoteQueue(Ok(queue)));
    // The entry it was on cannot be played here: the next that can is.
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, start_index: 1, origin })]
            if tracks.len() == 2 && origin == "Liked Music"
    ));
    assert!(!state.reading_remote_queue);
}

#[test]
fn an_empty_queue_on_the_other_devices_is_said_not_played() {
    let mut state = ready();
    apply(&mut state, Action::ContinueFromRemote);
    let empty = Response::RemoteQueue(Ok(RemoteQueue::default()));
    assert!(loaded(&mut state, empty).is_empty());
    let said = state.toasts.last().map(|toast| toast.text.as_str());
    assert_eq!(said, Some("Nothing is queued on your other devices."));
}

#[test]
fn the_text_about_a_page_is_clipped_again_on_the_next_page() {
    let mut state = ready();
    apply(&mut state, Action::ToggleAbout);
    assert!(state.about_expanded);
    apply(&mut state, Action::Open(Page::Search));
    assert!(!state.about_expanded);
}

#[test]
fn the_release_notes_over_the_page_count_as_read() {
    let mut state = state();
    assert!(state.release_notes_unread());
    assert_eq!(
        apply(&mut state, Action::ShowWhatsNew),
        [Effect::SaveSettings]
    );
    assert_eq!(state.dialog, Some(Dialog::WhatsNew));
    assert!(!state.release_notes_unread());
}

#[test]
fn a_toast_can_carry_a_way_onward() {
    let mut state = state();
    state.toast_with_link("Updated to 9.9.9", "See what's new", Page::Changelog);
    let link = state.toasts.last().and_then(|toast| toast.link.clone());
    assert_eq!(
        link,
        Some(ToastLink {
            label: "See what's new",
            page: Page::Changelog,
        })
    );
}

#[test]
fn queueing_a_playlist_part_read_queues_all_of_it_once_it_is_read() {
    let mut state = on_long_playlist("t1");
    let queue = Action::WholePlaylist {
        id: "pl".into(),
        then: Whole::Queue { next: false },
    };
    assert_eq!(apply(&mut state, queue).len(), 1);
    // Something else played meanwhile does not call off what was queued.
    apply(&mut state, Action::StartRadio(track("z")));
    let rest = Response::PlaylistRest {
        id: "pl".into(),
        token: "t1".into(),
        result: Ok(vec![track("c")]),
    };
    let effects = loaded(&mut state, rest);
    // With nothing playing, what is queued is played.
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, .. })] if tracks.len() == 3
    ));
}

#[test]
fn a_new_playlist_from_a_playlist_part_read_holds_all_of_its_songs() {
    let mut state = on_long_playlist("t1");
    let copy = Action::WholePlaylist {
        id: "pl".into(),
        then: Whole::NewPlaylist,
    };
    apply(&mut state, copy);
    let rest = Response::PlaylistRest {
        id: "pl".into(),
        token: "t1".into(),
        result: Ok(vec![track("c")]),
    };
    assert!(loaded(&mut state, rest).is_empty());
    assert!(matches!(
        &state.dialog,
        Some(Dialog::NewPlaylist { name, track_ids }) if name == "Road trip" && track_ids.len() == 3
    ));
}

#[test]
fn a_toast_with_a_button_stays_longer_than_one_without() {
    use std::time::Duration;

    let mut state = state();
    state.toast("Link copied");
    state.toast_with_link("Updated to 9.9.9", "See what's new", Page::Changelog);
    let later = Instant::now() + Duration::from_secs(5);
    assert!(state.expire_toasts(later).is_some());
    assert_eq!(state.toasts.len(), 1);
    assert!(state.toasts[0].link.is_some());
    let much_later = Instant::now() + Duration::from_secs(12);
    assert_eq!(state.expire_toasts(much_later), None);
    assert!(state.toasts.is_empty());
}
