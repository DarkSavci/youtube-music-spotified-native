//! Listen Together: entering a room, hearing it, and leaving.

use spotified_client::session::Command;

use super::*;
use crate::together::protocol::{Entry, Member, Room, RoomTrack};
use crate::together::{Enter, Event, Field, Phase, Seed};

fn with_server() -> State {
    let mut state = ready();
    state.settings.together_server = "wss://listen.example.com".into();
    state
}

fn hear(state: &mut State, event: Event) -> Vec<Effect> {
    apply(state, Action::TogetherEvent(Box::new(event)))
}

fn room(revision: u64) -> Room {
    Room {
        owner: "me".into(),
        members: vec![Member {
            id: "me".into(),
            name: "Ada".into(),
            connected: true,
            ..Member::default()
        }],
        queue: vec![Entry {
            id: "e1".into(),
            track: RoomTrack {
                id: "abcdefghijk".into(),
                duration_ms: 200_000.0,
                ..RoomTrack::default()
            },
            ..Entry::default()
        }],
        current: Some("e1".into()),
        playing: true,
        revision,
        ..Room::default()
    }
}

/// In a room that plays one song, with a player that has not caught up.
fn in_room() -> State {
    let mut state = with_server();
    apply(
        &mut state,
        Action::TogetherField(Field::Pin, "01234567".into()),
    );
    apply(&mut state, Action::TogetherJoin);
    hear(&mut state, Event::Joined("me".into()));
    state.playback = Some(Playback {
        session: Default::default(),
        received: Instant::now(),
        offline: false,
        following_room: false,
        room_ended: None,
    });
    state
}

#[test]
fn a_room_needs_a_secure_server_and_a_whole_pin() {
    let mut state = ready();
    assert!(apply(&mut state, Action::TogetherCreate).is_empty());
    assert!(state.together.error.is_some());

    let mut state = with_server();
    apply(
        &mut state,
        Action::TogetherField(Field::Pin, "12 34".into()),
    );
    assert_eq!(state.together.form.pin, "1234");
    assert!(apply(&mut state, Action::TogetherJoin).is_empty());
    assert_eq!(state.together.phase, Phase::Idle);
}

#[test]
fn joining_connects_under_the_typed_name_and_remembers_the_server() {
    let mut state = with_server();
    apply(
        &mut state,
        Action::TogetherField(Field::Name, " Ada ".into()),
    );
    apply(
        &mut state,
        Action::TogetherField(Field::Pin, "0123 4567".into()),
    );
    let effects = apply(&mut state, Action::TogetherJoin);
    assert_eq!(effects[0], Effect::SaveSettings);
    assert!(matches!(
        &effects[1],
        Effect::TogetherConnect(options)
            if options.name == "Ada" && options.enter == Enter::Join { pin: "01234567".into() }
    ));
    assert_eq!(state.together.phase, Phase::Connecting);
    // A second press while connecting does not open a second line.
    assert!(apply(&mut state, Action::TogetherJoin).is_empty());
}

#[test]
fn hearing_the_room_has_the_player_follow_it() {
    let mut state = in_room();
    let effects = hear(
        &mut state,
        Event::Room {
            room: Box::new(room(3)),
            offset_ms: 0.0,
        },
    );
    assert!(matches!(
        &effects[0],
        Effect::Command(Command::FollowRoom { entry, playing: true, .. }) if entry == "e1"
    ));
    assert!(matches!(effects.last(), Some(Effect::TogetherStatus(..))));
    assert_eq!(state.together.applied_entry.as_deref(), Some("e1"));
}

#[test]
fn an_older_room_than_the_one_held_is_dropped() {
    let mut state = in_room();
    let newer = Event::Room {
        room: Box::new(room(5)),
        offset_ms: 0.0,
    };
    hear(&mut state, newer);
    let older = Event::Room {
        room: Box::new(room(4)),
        offset_ms: 0.0,
    };
    assert!(hear(&mut state, older).is_empty());
    assert_eq!(
        state.together.room.as_ref().map(|room| room.revision),
        Some(5)
    );
}

#[test]
fn a_room_made_while_music_plays_is_given_that_music_first() {
    let mut state = with_server();
    let mut session = spotified_client::session::SessionState::default();
    session.queue.items = vec![track("abcdefghijk"), track("lmnopqrstuv")];
    session.position_ms = 42_000;
    state.playback = Some(Playback {
        session,
        received: Instant::now(),
        offline: false,
        following_room: false,
        room_ended: None,
    });
    apply(&mut state, Action::TogetherCreate);
    assert!(matches!(&state.together.seed, Seed::Wanted { tracks, .. } if tracks.len() == 2));
    hear(&mut state, Event::Joined("me".into()));
    let empty = Event::Room {
        room: Box::new(Room::default()),
        offset_ms: 0.0,
    };
    let effects = hear(&mut state, empty);
    assert!(matches!(
        &effects[..],
        [Effect::TogetherCommand("enqueue", _)]
    ));
    // The room has the songs: it is taken to where the song was.
    let filled = Event::Room {
        room: Box::new(Room {
            playing: false,
            ..room(1)
        }),
        offset_ms: 0.0,
    };
    let effects = hear(&mut state, filled);
    assert!(matches!(&effects[..], [Effect::TogetherCommand("seek", _)]));
}

#[test]
fn leaving_closes_the_line_and_keeps_the_rooms_queue_playing() {
    let mut state = in_room();
    hear(
        &mut state,
        Event::Room {
            room: Box::new(room(1)),
            offset_ms: 0.0,
        },
    );
    assert_eq!(
        apply(&mut state, Action::TogetherLeave),
        [
            Effect::TogetherDisconnect,
            Effect::Command(Command::LeaveRoom { keep_queue: true })
        ]
    );
    assert_eq!(state.together.phase, Phase::Idle);
    assert!(state.together.room.is_none());
}

#[test]
fn a_line_that_fails_says_why_on_the_page() {
    let mut state = in_room();
    hear(&mut state, Event::Failed("No such room.".into()));
    assert_eq!(state.together.phase, Phase::Idle);
    assert_eq!(state.together.error.as_deref(), Some("No such room."));
}
