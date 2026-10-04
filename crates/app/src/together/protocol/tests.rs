use super::*;

#[test]
fn a_state_is_read_with_its_queue_and_ignores_what_it_does_not_know() {
    let text = r#"{"type":"state","room":{
        "name":"Friday","id":"r","pin":"01234567","owner":"m1","mode":"listen",
        "members":[{"id":"m1","name":"Ada","connected":true,"status":"listening","role":"listener","statusEntry":null}],
        "queue":[{"id":"e1","track":{"id":"abcdefghijk","title":"Song","artists":[{"name":"Band"}],"durationMs":200000,"artwork":[]},
                  "addedBy":{"id":"m1","name":"Ada","avatar":""},"addedAt":1,"catalogueMs":200000}],
        "current":"e1","positionMs":5000,"at":1000,"playing":true,"revision":7,"expires":99,"somethingNew":true}}"#;
    let Ok(Incoming::State { room }) = serde_json::from_str::<Incoming>(text) else {
        panic!("a state");
    };
    assert_eq!(room.mode, Mode::Listen);
    assert_eq!(room.revision, 7);
    assert_eq!(room.current().map(|(index, _)| index), Some(0));
    assert_eq!(room.tracks()[0].artist_names(), "Band");
    assert_eq!(
        room.leader().map(|member| member.name.as_str()),
        Some("Ada")
    );
}

#[test]
fn the_position_runs_on_while_playing_and_stops_at_the_songs_end() {
    let mut room = Room {
        queue: vec![Entry {
            id: "e1".into(),
            track: RoomTrack {
                duration_ms: 10_000.0,
                ..RoomTrack::default()
            },
            ..Entry::default()
        }],
        current: Some("e1".into()),
        position_ms: 2000.0,
        at: 1000.0,
        playing: true,
        ..Room::default()
    };
    assert_eq!(room.position_at(4000.0), 5000);
    assert_eq!(room.position_at(60_000.0), 10_000);
    room.playing = false;
    assert_eq!(room.position_at(4000.0), 2000);
}

#[test]
fn who_may_steer_follows_the_rooms_mode() {
    let mut room = Room {
        owner: "leader".into(),
        mode: Mode::Listen,
        members: vec![Member {
            id: "dj".into(),
            role: "dj".into(),
            ..Member::default()
        }],
        ..Room::default()
    };
    assert!(room.may_control("leader"));
    assert!(room.may_control("dj"));
    assert!(!room.may_control("guest"));
    room.mode = Mode::Collaborative;
    assert!(room.may_control("guest"));
}

#[test]
fn messages_of_a_kind_this_app_does_not_know_are_passed_over() {
    let unknown = serde_json::from_str::<Incoming>(r#"{"type":"fireworks","loud":true}"#);
    assert_eq!(unknown.ok(), Some(Incoming::Unknown));
    let ack = serde_json::from_str::<Incoming>(r#"{"type":"ack","op":"x","revision":3}"#);
    assert_eq!(ack.ok(), Some(Incoming::Ack));
}

#[test]
fn a_remote_server_must_be_secure_and_a_local_one_need_not_be() {
    assert!(checked_address("wss://listen.example.com/rooms").is_ok());
    assert!(checked_address(" ws://localhost:8766 ").is_ok());
    assert!(checked_address("ws://listen.example.com").is_err());
    assert!(checked_address("https://listen.example.com").is_err());
    assert!(checked_address("wss://user:pw@example.com").is_err());
    assert!(checked_address("listen.example.com").is_err());
}

/// A state as the relay sends one from a room that takes requests, with a
/// ready check on, a request waiting and an edit that can be taken back.
const BUSY: &str = r#"{"type":"state","room":{
    "id":"r","pin":"01234567","name":"","owner":"m1","mode":"contributions","locked":true,
    "members":[
      {"id":"m1","name":"Ada","avatar":"https://yt3.ggpht.com/ada=s88","connected":true,"disconnectedAt":null,"status":"listening","role":"listener","ready":true},
      {"id":"m2","name":"Bob","avatar":"","connected":false,"disconnectedAt":5,"status":"reconnecting","role":"dj","statusEntry":"e1"}],
    "queue":[{"id":"e1","track":{"id":"abcdefghijk","title":"Song","artists":[{"name":"Band"},{"name":"Guest"}],"durationMs":200000,"artwork":[],"explicit":false,"playable":true,"isVideo":false},
              "catalogueMs":200000,"addedBy":{"id":"m2","name":"Bob","avatar":""},"addedAt":1700000000000,"radio":true,"request":"q0"}],
    "current":"e1","positionMs":0,"at":1,"playing":false,"revision":9,"expires":1700021600000,
    "activity":[{"id":"a1","text":"Ada started the room.","at":1700000000000}],
    "history":[{"id":"e0","track":{"id":"lmnopqrstuv","title":"Before","artists":[],"durationMs":1000,"artwork":[]},"addedBy":{"id":"m1","name":"Ada","avatar":""},"addedAt":1}],
    "repeat":"all","policy":"turns","duplicates":false,"limit":3,"lastControlledBy":{"id":"m1","name":"Ada"},
    "video":null,"votes":["m2"],"voteSkip":true,"undo":{"revision":9,"expires":1700000010000,"by":"m2"},
    "joinApproval":true,"pending":[{"id":"p1","name":"Cy","avatar":"","at":3}],
    "countdown":{"expires":1700000060000,"startAt":null},
    "requests":[{"id":"q1","track":{"id":"zyxwvutsrqp","title":"Wanted","artists":[{"name":"Band"}],"durationMs":0,"artwork":[]},"by":{"id":"m2","name":"Bob","avatar":""},"at":4}],
    "autoAccept":false,"finished":false,"unplayableSince":null}}"#;

#[test]
fn a_busy_room_is_read_with_everything_the_page_shows() {
    let Ok(Incoming::State { room }) = serde_json::from_str::<Incoming>(BUSY) else {
        panic!("a state");
    };
    assert_eq!(room.mode, Mode::Contributions);
    assert_eq!((room.repeat, room.policy), (Repeat::All, Policy::Turns));
    assert!(room.locked && room.join_approval && room.vote_skip);
    assert!(!room.duplicates && !room.auto_accept);
    assert_eq!(room.limit, 3);
    assert_eq!(room.votes, ["m2"]);
    assert_eq!(room.title(), "Ada’s room");
    // Members: a picture where one is shared, and who is ready.
    assert_eq!(
        room.members[0].avatar[0].url,
        "https://yt3.ggpht.com/ada=s88"
    );
    assert!(room.members[0].ready && !room.members[1].ready);
    assert!(room.members[1].avatar.is_empty());
    // The queue: who added it, that it is radio, and the request it was.
    let entry = &room.queue[0];
    assert_eq!(entry.added_by.id, "m2");
    assert!(entry.radio);
    assert_eq!(entry.request.as_deref(), Some("q0"));
    assert_eq!(entry.track.artist_names(), "Band, Guest");
    assert_eq!(room.history[0].track.title, "Before");
    assert_eq!(room.activity[0].text, "Ada started the room.");
    assert_eq!(room.pending[0].name, "Cy");
    assert_eq!(room.requests[0].by.name, "Bob");
    assert_eq!(room.requests[0].track.id, "zyxwvutsrqp");
    let undo = room
        .undo
        .as_ref()
        .map(|undo| (undo.revision, undo.by.as_str()));
    assert_eq!(undo, Some((9, "m2")));
    let countdown = room.countdown.as_ref().map(|countdown| countdown.start_at);
    assert_eq!(countdown, Some(None));
    let steered = room.last_controlled_by.as_ref().map(|by| by.name.as_str());
    assert_eq!(steered, Some("Ada"));
}

#[test]
fn a_new_room_is_read_with_nothing_going_on_in_it() {
    let text = r#"{"type":"state","room":{"id":"r","pin":"01234567","name":"Friday","members":[],
        "owner":"","mode":"collaborative","locked":false,"queue":[],"current":null,"positionMs":0,
        "at":1,"playing":false,"revision":0,"expires":2,"activity":[],"history":[],"repeat":"off",
        "policy":"fifo","duplicates":true,"limit":50,"lastControlledBy":null,"video":null,
        "votes":[],"voteSkip":false,"undo":null,"joinApproval":false,"pending":[],
        "countdown":null,"requests":[],"autoAccept":false,"finished":false,"unplayableSince":null}}"#;
    let Ok(Incoming::State { room }) = serde_json::from_str::<Incoming>(text) else {
        panic!("a state");
    };
    assert_eq!(room.title(), "Friday");
    assert_eq!((room.repeat, room.policy), (Repeat::Off, Policy::Fifo));
    assert!(room.undo.is_none() && room.countdown.is_none());
    assert!(room.last_controlled_by.is_none());
    assert_eq!(room.limit, 50);
}

#[test]
fn a_started_count_says_when_the_room_begins() {
    let text = r#"{"expires":1700000060000,"startAt":1700000003000}"#;
    let countdown = serde_json::from_str::<Countdown>(text).ok();
    assert_eq!(
        countdown.and_then(|countdown| countdown.start_at),
        Some(1_700_000_003_000.0)
    );
}

#[test]
fn repeat_steps_as_the_players_button_does() {
    assert_eq!(Repeat::Off.next(), Repeat::All);
    assert_eq!(Repeat::All.next(), Repeat::One);
    assert_eq!(Repeat::One.next(), Repeat::Off);
    assert_eq!(Repeat::One.wire(), "one");
}
