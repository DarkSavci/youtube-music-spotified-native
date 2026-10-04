//! Listen Together, where the room's song meets this player: a song that
//! will not play here, and a song whose length the room has wrong.

use serde_json::json;
use spotified_client::models::Track;
use spotified_client::session::{Command, PlayState, Queue, SessionState};

use super::together::{hear, in_room, room};
use super::*;
use crate::state::Notice;
use crate::together::protocol::Mode;
use crate::together::sync::{Standing, length_worth_telling};
use crate::together::{Ask, Event};

/// In the room, with its one song in the player and playing.
fn playing_the_rooms_song() -> State {
    let mut state = in_room();
    hear(
        &mut state,
        Event::Room {
            room: Box::new(room(3)),
            offset_ms: 0.0,
        },
    );
    let playback = state.playback.as_mut().expect("a player");
    playback.following_room = true;
    playback.session = SessionState {
        state: PlayState::Playing,
        queue: Queue {
            items: vec![Track {
                id: "abcdefghijk".into(),
                duration_ms: 200_000,
                playable: true,
                ..Track::default()
            }],
            ..Queue::default()
        },
        ..SessionState::default()
    };
    state
}

fn tick(state: &mut State) -> Vec<Effect> {
    apply(state, Action::TogetherTick)
}

fn follows(effects: &[Effect]) -> bool {
    let follow = |effect: &Effect| matches!(effect, Effect::Command(Command::FollowRoom { .. }));
    effects.iter().any(follow)
}

#[test]
fn a_song_youtube_will_not_give_this_device_is_said_to_the_room_and_not_asked_for_again() {
    let mut state = playing_the_rooms_song();
    state.notice = Some(Notice::RateLimited);
    let effects = tick(&mut state);
    assert_eq!(
        effects,
        [Effect::TogetherStatus("unavailable", Some("e1".into()))]
    );
    assert_eq!(state.together.standing, Standing::Unavailable);
    assert_eq!(state.together.standing.label(), "Track unavailable");
}

#[test]
fn a_song_that_cannot_be_played_here_is_unavailable_too() {
    let mut state = playing_the_rooms_song();
    let playback = state.playback.as_mut().expect("a player");
    playback.session.queue.items[0].playable = false;
    let effects = tick(&mut state);
    assert_eq!(
        effects,
        [Effect::TogetherStatus("unavailable", Some("e1".into()))]
    );
}

#[test]
fn retry_playback_asks_the_player_for_the_rooms_song_again() {
    let mut state = playing_the_rooms_song();
    state.notice = Some(Notice::RateLimited);
    assert!(!follows(&tick(&mut state)));
    let effects = apply(&mut state, Action::Room(Ask::RetryPlayback));
    assert!(follows(&effects));
    // Asked once: the next second says it is unavailable again, if it is.
    assert!(!follows(&tick(&mut state)));
}

#[test]
fn a_notice_about_some_other_song_does_not_stop_the_room_being_followed() {
    let mut state = playing_the_rooms_song();
    state.notice = Some(Notice::RateLimited);
    // The room has moved on to a song this player has not been given.
    state.together.applied_entry = Some("e0".into());
    assert!(follows(&tick(&mut state)));
}

#[test]
fn the_room_is_told_a_songs_measured_length_once_when_its_own_is_out() {
    let mut state = playing_the_rooms_song();
    let playback = state.playback.as_mut().expect("a player");
    playback.room_length = Some(("e1".into(), 203_000));
    let effects = tick(&mut state);
    let told = Effect::TogetherCommand("duration", json!({ "entry": "e1", "durationMs": 203_000 }));
    assert!(effects.contains(&told), "{effects:?}");
    // Once for a song, however many seconds pass.
    let again = tick(&mut state);
    assert!(
        !again
            .iter()
            .any(|effect| matches!(effect, Effect::TogetherCommand("duration", _)))
    );
}

#[test]
fn a_listener_who_may_not_steer_the_room_keeps_the_measure_to_themselves() {
    let mut state = playing_the_rooms_song();
    let listened = state.together.room.as_mut().expect("a room");
    listened.owner = "someone-else".into();
    listened.mode = Mode::Listen;
    let playback = state.playback.as_mut().expect("a player");
    playback.room_length = Some(("e1".into(), 203_000));
    let effects = tick(&mut state);
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::TogetherCommand("duration", _)))
    );
}

#[test]
fn a_length_is_worth_telling_when_the_room_has_none_or_one_a_little_out() {
    // None at all.
    assert!(length_worth_telling(0, 187_000));
    // Within a second: near enough.
    assert!(!length_worth_telling(200_000, 200_900));
    assert!(length_worth_telling(200_000, 203_000));
    assert!(length_worth_telling(200_000, 190_000));
    // Another recording altogether says nothing of this one.
    assert!(!length_worth_telling(200_000, 260_000));
    // Not measured yet.
    assert!(!length_worth_telling(200_000, 0));
}
