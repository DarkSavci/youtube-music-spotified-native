//! Listen Together: finding songs from inside a room, and the radio that
//! keeps a room's music going.

use std::time::Instant;

use spotified_client::ApiError;

use super::together::{hear, in_room, room};
use super::*;
use crate::together::protocol::{Entry, Member, Mode, Repeat, Room, RoomTrack};
use crate::together::{Ask, Event};

fn ask(state: &mut State, ask: Ask) -> Vec<Effect> {
    apply(state, Action::Room(ask))
}

fn heard(state: &mut State, room: Room) -> Vec<Effect> {
    let event = Event::Room {
        room: Box::new(room),
        offset_ms: 0.0,
    };
    hear(state, event)
}

fn answer(state: &mut State, response: Response) -> Vec<Effect> {
    apply(state, Action::Loaded(Box::new(response)))
}

fn song(id: &str) -> Track {
    Track {
        title: id.to_uppercase(),
        ..track(id)
    }
}

/// A queue of `songs` entries added long ago, the first of them playing.
fn long_room(revision: u64, songs: usize) -> Room {
    let mut room = room(revision);
    room.queue = (0..songs)
        .map(|index| Entry {
            id: format!("e{index}"),
            track: RoomTrack {
                id: format!("song{index:07}"),
                duration_ms: 200_000.0,
                ..RoomTrack::default()
            },
            ..Entry::default()
        })
        .collect();
    room.current = Some("e0".into());
    room
}

fn radio_asked(effects: &[Effect]) -> Option<&str> {
    effects.iter().find_map(|effect| match effect {
        Effect::Fetch(Request::Radio(seed)) => Some(seed.as_str()),
        _ => None,
    })
}

#[test]
fn typing_in_the_rooms_search_waits_for_a_pause_and_then_asks() {
    let mut state = in_room();
    assert_eq!(
        ask(&mut state, Ask::Search("daft".into())),
        [Effect::DebounceRoomSearch]
    );
    assert!(!state.together.search.searching);
    let serial = state.together.search.serial;
    assert_eq!(
        ask(&mut state, Ask::RunSearch),
        [Effect::Fetch(Request::RoomSearch {
            serial,
            query: "daft".into()
        })]
    );
    assert!(state.together.search.searching);
    let found = Response::RoomSearch {
        serial,
        result: Ok(vec![song("abcdefghijk")]),
    };
    answer(&mut state, found);
    assert!(!state.together.search.searching);
    assert_eq!(state.together.search.results[0].title, "ABCDEFGHIJK");
}

#[test]
fn an_answer_to_an_older_search_is_dropped_and_clearing_the_field_empties_it() {
    let mut state = in_room();
    ask(&mut state, Ask::Search("daft".into()));
    let older = state.together.search.serial;
    ask(&mut state, Ask::Search("daft punk".into()));
    let found = Response::RoomSearch {
        serial: older,
        result: Ok(vec![song("abcdefghijk")]),
    };
    answer(&mut state, found);
    assert!(state.together.search.results.is_empty());
    // Emptied: nothing is asked, and what was found goes.
    state.together.search.results = vec![song("abcdefghijk")];
    assert!(ask(&mut state, Ask::Search(String::new())).is_empty());
    assert!(state.together.search.results.is_empty());
    assert!(ask(&mut state, Ask::RunSearch).is_empty());
}

#[test]
fn a_search_that_fails_says_so_on_the_page() {
    let mut state = in_room();
    ask(&mut state, Ask::Search("daft".into()));
    let failed = Response::RoomSearch {
        serial: state.together.search.serial,
        result: Err(ApiError::Unreachable("offline".into())),
    };
    answer(&mut state, failed);
    assert!(state.together.error.is_some());
}

#[test]
fn radio_in_a_room_plays_the_song_at_once_and_adds_its_radio_after() {
    let mut state = in_room();
    heard(&mut state, long_room(1, 8));
    let effects = ask(&mut state, Ask::Radio(Box::new(song("abcdefghijk"))));
    assert!(matches!(
        &effects[..],
        [Effect::TogetherCommand("replace", fields), Effect::Fetch(Request::Radio(seed))]
            if fields["tracks"][0]["id"] == "abcdefghijk" && seed == "abcdefghijk"
    ));
    // The room now plays the song: its radio goes in after it, marked so.
    let mut playing = room(2);
    playing.queue[0].track.id = "abcdefghijk".into();
    heard(&mut state, playing);
    let found = vec![song("abcdefghijk"), song("lmnopqrstuv")];
    let effects = answer(&mut state, Response::Radio("abcdefghijk".into(), Ok(found)));
    let [Effect::TogetherCommand("enqueue", fields)] = &effects[..] else {
        panic!("the radio is added, not {effects:?}");
    };
    assert_eq!(fields["radio"], true);
    // The song itself is not added again.
    assert_eq!(fields["tracks"].as_array().map(Vec::len), Some(1));
    assert_eq!(fields["tracks"][0]["id"], "lmnopqrstuv");
}

#[test]
fn a_radio_that_arrives_before_the_room_has_the_song_waits_for_it() {
    let mut state = in_room();
    heard(&mut state, long_room(1, 8));
    ask(&mut state, Ask::Radio(Box::new(song("abcdefghijk"))));
    // The core answers faster than the relay says the song is playing.
    let found = vec![song("lmnopqrstuv")];
    let effects = answer(&mut state, Response::Radio("abcdefghijk".into(), Ok(found)));
    assert!(effects.is_empty());
    assert!(state.together.radio.busy);
    let mut playing = room(2);
    playing.queue[0].track.id = "abcdefghijk".into();
    let effects = heard(&mut state, playing);
    let added = |effect: &Effect| matches!(effect, Effect::TogetherCommand("enqueue", fields) if fields["radio"] == true);
    assert!(effects.iter().any(added));
    assert!(!state.together.radio.busy);
}

#[test]
fn a_radio_for_a_song_someone_has_since_replaced_is_dropped() {
    let mut state = in_room();
    heard(&mut state, long_room(1, 8));
    ask(&mut state, Ask::Radio(Box::new(song("abcdefghijk"))));
    let found = vec![song("lmnopqrstuv")];
    answer(&mut state, Response::Radio("abcdefghijk".into(), Ok(found)));
    // The room moved on, to something else than the song asked for.
    let effects = heard(&mut state, long_room(2, 8));
    let added = |effect: &Effect| matches!(effect, Effect::TogetherCommand("enqueue", _));
    assert!(!effects.iter().any(added));
    assert!(!state.together.radio.busy);
    assert!(state.together.radio.found.is_none());
}

#[test]
fn a_radio_that_cannot_be_had_is_said() {
    let mut state = in_room();
    heard(&mut state, long_room(1, 8));
    ask(&mut state, Ask::Radio(Box::new(song("abcdefghijk"))));
    let failed = Err(ApiError::Unreachable("offline".into()));
    answer(&mut state, Response::Radio("abcdefghijk".into(), failed));
    let said: Vec<&str> = state
        .toasts
        .iter()
        .map(|toast| toast.text.as_str())
        .collect();
    assert_eq!(said, ["Couldn't load this song's radio."]);
}

#[test]
fn starting_a_songs_radio_from_anywhere_in_the_app_goes_to_the_room() {
    let mut state = in_room();
    heard(&mut state, long_room(1, 8));
    let effects = apply(&mut state, Action::StartRadio(song("abcdefghijk")));
    assert!(matches!(
        &effects[..],
        [
            Effect::TogetherCommand("replace", _),
            Effect::Fetch(Request::Radio(_))
        ]
    ));
    // A guest where the leader approves asks for the song instead.
    let mut requests = long_room(2, 8);
    requests.mode = Mode::Contributions;
    requests.owner = "someone".into();
    heard(&mut state, requests);
    let effects = apply(&mut state, Action::StartRadio(song("lmnopqrstuv")));
    assert!(matches!(
        &effects[..],
        [Effect::TogetherCommand("request", _)]
    ));
}

#[test]
fn the_leader_tops_the_queue_up_with_radio_when_it_runs_low() {
    let mut state = in_room();
    // Plenty left: nothing is asked for.
    assert_eq!(radio_asked(&heard(&mut state, long_room(1, 8))), None);
    // Four left after the current song: the last one's radio is fetched.
    let effects = heard(&mut state, long_room(2, 5));
    assert_eq!(radio_asked(&effects), Some("song0000004"));
    // Not asked for twice while it is on its way.
    assert_eq!(radio_asked(&heard(&mut state, long_room(3, 5))), None);
    let found = vec![song("lmnopqrstuv"), song("song0000001")];
    let effects = answer(&mut state, Response::Radio("song0000004".into(), Ok(found)));
    let [Effect::TogetherCommand("enqueue", fields)] = &effects[..] else {
        panic!("the radio is added, not {effects:?}");
    };
    assert_eq!(fields["radio"], true);
    // What the room already has is left out.
    assert_eq!(fields["tracks"].as_array().map(Vec::len), Some(1));
    // The same end of the queue is not topped up again.
    assert_eq!(radio_asked(&heard(&mut state, long_room(4, 5))), None);
}

#[test]
fn only_the_leader_tops_up_and_only_with_autoplay_on_and_repeat_off() {
    let low = |revision, change: fn(&mut Room)| {
        let mut room = long_room(revision, 3);
        change(&mut room);
        room
    };
    let mut state = in_room();
    let guest = low(1, |room| {
        room.owner = "someone".into();
        room.members.push(Member {
            id: "someone".into(),
            ..Member::default()
        });
    });
    assert_eq!(radio_asked(&heard(&mut state, guest)), None);
    let repeating = low(2, |room| room.repeat = Repeat::All);
    assert_eq!(radio_asked(&heard(&mut state, repeating)), None);
    state.settings.autoplay = false;
    assert_eq!(radio_asked(&heard(&mut state, low(3, |_| {}))), None);
    state.settings.autoplay = true;
    assert!(radio_asked(&heard(&mut state, low(4, |_| {}))).is_some());
}

#[test]
fn a_song_just_added_is_left_a_moment_for_its_own_radio() {
    let mut state = in_room();
    let mut fresh = long_room(1, 3);
    let now = state.together.server_now();
    if let Some(last) = fresh.queue.last_mut() {
        last.added_at = now - 2000.0;
    }
    assert_eq!(radio_asked(&heard(&mut state, fresh)), None);
}

#[test]
fn a_top_up_that_fails_is_tried_again_only_after_a_while() {
    let mut state = in_room();
    assert!(radio_asked(&heard(&mut state, long_room(1, 3))).is_some());
    let failed = Err(ApiError::Unreachable("offline".into()));
    answer(&mut state, Response::Radio("song0000002".into(), failed));
    assert!(state.toasts.is_empty());
    assert!(
        state
            .together
            .radio
            .retry_at
            .is_some_and(|at| at > Instant::now())
    );
    assert_eq!(radio_asked(&heard(&mut state, long_room(2, 3))), None);
}
