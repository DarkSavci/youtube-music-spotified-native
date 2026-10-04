use spotified_client::session::Repeat;

use super::super::protocol::{Entry, Member, Person, RoomTrack};
use super::*;

fn room() -> Room {
    let entry = |id: &str, track: &str| Entry {
        id: id.into(),
        track: RoomTrack {
            id: track.into(),
            duration_ms: 200_000.0,
            ..RoomTrack::default()
        },
        ..Entry::default()
    };
    Room {
        owner: "leader".into(),
        members: vec![Member {
            id: "leader".into(),
            ..Member::default()
        }],
        queue: vec![entry("e1", "one"), entry("e2", "two"), entry("e3", "three")],
        current: Some("e2".into()),
        position_ms: 30_000.0,
        at: 1000.0,
        playing: true,
        ..Room::default()
    }
}

fn with_the_room(position_ms: u64) -> Local<'static> {
    Local {
        following: true,
        queue: vec!["one", "two", "three"],
        current: Some("two"),
        position_ms,
        playing: true,
        settled: true,
    }
}

#[test]
fn a_player_with_the_room_is_left_alone() {
    let local = with_the_room(30_400);
    assert_eq!(follow(&room(), 1000.0, &local, Some("e2"), None), None);
}

#[test]
fn a_new_song_in_the_room_is_followed_from_where_the_room_is() {
    let local = with_the_room(30_000);
    let command = follow(&room(), 3000.0, &local, Some("e1"), None);
    assert_eq!(
        command,
        Some(Command::FollowRoom {
            tracks: room().tracks(),
            index: 1,
            entry: "e2".into(),
            position_ms: 32_000,
            playing: true,
        })
    );
}

#[test]
fn a_player_adrift_is_brought_back_but_not_twice_in_a_row() {
    let local = with_the_room(40_000);
    assert!(follow(&room(), 1000.0, &local, Some("e2"), None).is_some());
    let just_now = Some(Duration::from_secs(1));
    assert_eq!(follow(&room(), 1000.0, &local, Some("e2"), just_now), None);
    // Still loading: where it says it is cannot be trusted yet.
    let loading = Local {
        settled: false,
        ..with_the_room(0)
    };
    assert_eq!(follow(&room(), 1000.0, &loading, Some("e2"), None), None);
}

#[test]
fn a_pause_in_the_room_pauses_the_player() {
    let mut room = room();
    room.playing = false;
    let command = follow(&room, 9000.0, &with_the_room(30_000), Some("e2"), None);
    assert!(matches!(
        command,
        Some(Command::FollowRoom {
            playing: false,
            position_ms: 30_000,
            ..
        })
    ));
}

#[test]
fn an_empty_room_takes_the_player_once() {
    let empty = Room::default();
    let outside = Local {
        following: false,
        ..with_the_room(0)
    };
    assert!(follow(&empty, 0.0, &outside, None, None).is_some());
    assert_eq!(follow(&empty, 0.0, &with_the_room(0), None, None), None);
}

#[test]
fn the_players_buttons_become_the_rooms_commands() {
    let room = room();
    assert_eq!(
        route(&Command::Toggle, &room, "leader", 0),
        Routed::Room("pause", json!({}))
    );
    assert_eq!(
        route(&Command::Previous, &room, "leader", 5000),
        Routed::Room("previous", json!({ "restart": true }))
    );
    assert_eq!(
        route(&Command::Jump(2), &room, "leader", 0),
        Routed::Room("jump", json!({ "entry": "e3" }))
    );
    assert_eq!(
        route(&Command::SetVolume(0.5), &room, "leader", 0),
        Routed::Core
    );
}

#[test]
fn a_move_names_the_entry_that_will_follow() {
    let room = room();
    // The first to the end: nothing follows it.
    assert_eq!(
        route(&Command::Move { from: 0, to: 2 }, &room, "leader", 0),
        Routed::Room("move", json!({ "entry": "e1", "before": null }))
    );
    // The last to the front: the first follows it.
    assert_eq!(
        route(&Command::Move { from: 2, to: 0 }, &room, "leader", 0),
        Routed::Room("move", json!({ "entry": "e3", "before": "e1" }))
    );
}

#[test]
fn a_listener_may_only_do_what_the_room_allows() {
    let mut room = room();
    room.mode = Mode::Listen;
    assert_eq!(
        route(&Command::Next, &room, "guest", 0),
        Routed::Refused(LEADER_PLAYS)
    );
    let add = Command::Enqueue {
        tracks: Vec::new(),
        at: None,
    };
    assert_eq!(route(&add, &room, "guest", 0), Routed::Refused(LISTEN_ONLY));
    assert_eq!(
        route(&Command::Remove(2), &room, "guest", 0),
        Routed::Refused(LISTEN_ONLY)
    );
    room.mode = Mode::Contributions;
    room.auto_accept = true;
    assert!(matches!(
        route(&add, &room, "guest", 0),
        Routed::Room("enqueue", _)
    ));
    assert_eq!(
        route(&Command::Move { from: 0, to: 1 }, &room, "guest", 0),
        Routed::Refused(LEADER_ORDERS)
    );
}

fn song(id: &str) -> Track {
    Track {
        id: id.into(),
        ..Track::default()
    }
}

#[test]
fn where_the_leader_approves_a_guests_song_goes_as_a_request() {
    let mut room = room();
    room.mode = Mode::Contributions;
    let add = Command::Enqueue {
        tracks: vec![song("abcdefghijk")],
        at: Some(2),
    };
    let Routed::Room("request", fields) = route(&add, &room, "guest", 0) else {
        panic!("a request");
    };
    assert_eq!(fields["tracks"][0]["id"], "abcdefghijk");
    // A request has no place: the leader chooses where it goes.
    assert!(fields.get("before").is_none());
    // Pressing play on a song asks for that one song.
    let play = Command::Play {
        tracks: vec![song("aaaaaaaaaaa"), song("bbbbbbbbbbb")],
        start_index: 1,
        origin: String::new(),
    };
    let Routed::Room("request", fields) = route(&play, &room, "guest", 0) else {
        panic!("a request");
    };
    assert_eq!(fields["tracks"].as_array().map(Vec::len), Some(1));
    assert_eq!(fields["tracks"][0]["id"], "bbbbbbbbbbb");
}

#[test]
fn a_guest_may_not_put_a_song_ahead_of_others_but_may_add_to_the_end() {
    let mut room = room();
    room.mode = Mode::Contributions;
    room.auto_accept = true;
    let next = Command::Enqueue {
        tracks: vec![song("abcdefghijk")],
        at: Some(2),
    };
    assert_eq!(route(&next, &room, "guest", 0), Routed::Refused(NOT_AHEAD));
    // With nothing after the current song, "next" is the end.
    room.current = Some("e3".into());
    let last = Command::Enqueue {
        tracks: vec![song("abcdefghijk")],
        at: Some(3),
    };
    let Routed::Room("enqueue", fields) = route(&last, &room, "guest", 0) else {
        panic!("an addition");
    };
    assert!(fields["before"].is_null());
    // The leader's "next" names the entry it goes before.
    room.current = Some("e2".into());
    let Routed::Room("enqueue", fields) = route(&next, &room, "leader", 0) else {
        panic!("an addition");
    };
    assert_eq!(fields["before"], "e3");
}

#[test]
fn a_guest_removes_only_their_own_upcoming_songs() {
    let mut room = room();
    room.mode = Mode::Contributions;
    room.queue[2].added_by = Person {
        id: "guest".into(),
        ..Person::default()
    };
    assert_eq!(
        route(&Command::Remove(2), &room, "guest", 0),
        Routed::Room("remove", json!({ "entry": "e3" }))
    );
    assert_eq!(
        route(&Command::Remove(0), &room, "guest", 0),
        Routed::Refused(OWN_ONLY)
    );
}

#[test]
fn repeat_is_the_rooms_and_the_leaders_to_change_and_shuffle_is_its_order() {
    let room = room();
    assert_eq!(
        route(&Command::SetRepeat(Repeat::All), &room, "leader", 0),
        Routed::Room("settings", json!({ "repeat": "all" }))
    );
    let mut once = room.clone();
    once.repeat = super::super::protocol::Repeat::All;
    assert_eq!(
        route(&Command::SetRepeat(Repeat::One), &once, "leader", 0),
        Routed::Room("settings", json!({ "repeat": "one" }))
    );
    assert_eq!(
        route(&Command::SetRepeat(Repeat::All), &room, "guest", 0),
        Routed::Refused(LEADER_SETS)
    );
    assert_eq!(
        route(&Command::SetShuffle(true), &room, "leader", 0),
        Routed::Refused(NO_SHUFFLE)
    );
}

#[test]
fn more_songs_than_the_room_takes_at_once_are_said_to_have_been_cut() {
    let many: Vec<Track> = (0..101).map(|_| song("abcdefghijk")).collect();
    let add = Command::Enqueue {
        tracks: many.clone(),
        at: None,
    };
    assert!(trimmed(&add));
    let play = Command::Play {
        tracks: many,
        start_index: 1,
        origin: String::new(),
    };
    assert!(!trimmed(&play));
    assert!(!trimmed(&Command::Next));
}

#[test]
fn how_a_player_stands_with_the_room_is_said_in_the_old_words() {
    let room = room();
    let standing = |local: &Local<'_>, ended| Standing::of(&room, 1000.0, local, ended);
    assert_eq!(standing(&with_the_room(30_200), false), Standing::InSync);
    assert_eq!(
        standing(&with_the_room(50_000), false),
        Standing::CatchingUp
    );
    let loading = Local {
        settled: false,
        ..with_the_room(0)
    };
    assert_eq!(standing(&loading, false), Standing::Buffering);
    assert_eq!(
        standing(&with_the_room(30_000), true),
        Standing::WaitingForNext
    );
    let mut paused = room.clone();
    paused.playing = false;
    let local = with_the_room(30_000);
    assert_eq!(
        Standing::of(&paused, 1000.0, &local, false),
        Standing::Paused
    );
    assert_eq!(Standing::CatchingUp.wire(true), "catching up");
    assert_eq!(Standing::InSync.wire(true), "listening");
    assert_eq!(Standing::Paused.wire(false), "paused");
    assert_eq!(Standing::Paused.label(), "Paused together");
}
