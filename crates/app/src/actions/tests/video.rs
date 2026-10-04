//! A song and its music video: the switch between them, alone and in a
//! Listen Together room, and what the button says meanwhile.

use serde_json::json;
use spotified_client::session::Command;

use super::together::{hear, in_room, room};
use super::*;
use crate::actions::video::{
    CHECK_FAILED, LEADER_CHOOSES, LOAD_FAILED, NONE_FOR_SONG, NONE_IN_ROOM, SWITCH_FAILED, follow,
};
use crate::state::video::{Availability, control};
use crate::together::protocol::{Mode, RoomVideo};
use crate::together::{Ask, Event};

const SONG: &str = "song1234567";
const CLIP: &str = "clip1234567";

fn song() -> Track {
    track(SONG)
}

fn clip() -> Track {
    Track {
        is_video: true,
        ..track(CLIP)
    }
}

fn ask(state: &mut State, ask: VideoAsk) -> Vec<Effect> {
    apply(state, Action::Video(ask))
}

/// The session as the core reports it with `playing` in the queue.
fn now_playing(state: &mut State, playing: Track, following: bool) -> Vec<Effect> {
    let mut projection = Projection::default();
    projection.state.state = PlayState::Playing;
    projection.state.queue.items = vec![playing];
    projection.following_room = following;
    apply(state, Action::SessionChanged(Box::new(projection)))
}

fn playing_alone(playing: Track) -> State {
    let mut state = ready();
    now_playing(&mut state, playing, false);
    state
}

fn pair_found(state: &mut State, of: &str, pair: Result<Vec<Track>, ApiError>) -> Vec<Effect> {
    let response = Response::Versions {
        track_id: of.into(),
        result: pair,
    };
    apply(state, Action::Loaded(Box::new(response)))
}

/// Watching the video of a song that has one, up to the switch being sent.
fn switching_to_the_video() -> State {
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Set(true));
    pair_found(&mut state, SONG, Ok(vec![song(), clip()]));
    state
}

fn watching() -> State {
    let mut state = switching_to_the_video();
    now_playing(&mut state, clip(), false);
    state
}

#[test]
fn asking_for_the_video_of_a_song_asks_youtube_for_its_pair() {
    let mut state = playing_alone(song());
    let effects = ask(&mut state, VideoAsk::Set(true));
    assert_eq!(effects, [Effect::Fetch(Request::Versions(SONG.into()))]);
    assert!(state.video.busy);
    assert!(!state.video.enabled);
    // A second press while that is on its way does nothing.
    assert!(ask(&mut state, VideoAsk::Set(true)).is_empty());
}

#[test]
fn the_video_takes_the_songs_place_in_the_queue_and_shows_once_it_plays() {
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Set(true));
    let effects = pair_found(&mut state, SONG, Ok(vec![song(), clip()]));
    let switch = Command::SwitchVariant {
        expected: SONG.into(),
        track: Box::new(clip()),
    };
    assert_eq!(effects, [Effect::Command(switch)]);
    // Not shown until the player says the video is what plays.
    assert!(state.video.busy && !state.video.enabled);
    let before = state.video.revision;
    now_playing(&mut state, clip(), false);
    assert!(state.video.enabled && !state.video.busy);
    assert!(state.video.loading);
    assert!(state.video.revision > before);
    assert_eq!(
        state.video.key(Some(&clip())),
        Some((CLIP.into(), state.video.revision))
    );
}

#[test]
fn a_song_with_no_video_says_so_and_the_button_stops_answering() {
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Set(true));
    assert!(pair_found(&mut state, SONG, Ok(vec![song()])).is_empty());
    assert_eq!(state.video.error.as_deref(), Some(NONE_FOR_SONG));
    assert!(!state.video.enabled && !state.video.busy);
    assert_eq!(state.video.availability, Availability::Unavailable);
    let button = control(&state.video, Some(&song()), false, false);
    assert!(button.blocked);
    assert_eq!(button.label, NONE_FOR_SONG);
}

#[test]
fn youtube_not_answering_is_said_and_can_be_tried_again() {
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Set(true));
    pair_found(&mut state, SONG, Err(ApiError::RateLimited));
    assert_eq!(state.video.error.as_deref(), Some(CHECK_FAILED));
    assert!(!state.video.busy);
    // Nothing was remembered, so the next press asks again.
    let effects = ask(&mut state, VideoAsk::Set(true));
    assert_eq!(effects, [Effect::Fetch(Request::Versions(SONG.into()))]);
}

#[test]
fn a_track_that_is_the_video_already_shows_without_a_switch() {
    let mut state = playing_alone(clip());
    ask(&mut state, VideoAsk::Set(true));
    assert!(pair_found(&mut state, CLIP, Ok(vec![clip()])).is_empty());
    assert!(state.video.enabled && !state.video.busy);
    assert_eq!(state.video.error, None);
    // And needs no answer from YouTube to.
    let mut state = playing_alone(clip());
    ask(&mut state, VideoAsk::Set(true));
    pair_found(&mut state, CLIP, Err(ApiError::RateLimited));
    assert!(state.video.enabled);
}

#[test]
fn going_back_to_the_song_hides_the_picture_at_once() {
    let mut state = watching();
    let effects = ask(&mut state, VideoAsk::Set(false));
    assert!(!state.video.enabled);
    let switch = Command::SwitchVariant {
        expected: CLIP.into(),
        track: Box::new(song()),
    };
    // The pair is remembered from the way in: no second question.
    assert_eq!(effects, [Effect::Command(switch)]);
    now_playing(&mut state, song(), false);
    assert!(!state.video.enabled && !state.video.busy);
    assert_eq!(state.video.key(Some(&song())), None);
}

#[test]
fn a_video_with_no_song_behind_it_just_hides() {
    let mut state = playing_alone(clip());
    ask(&mut state, VideoAsk::Set(true));
    pair_found(&mut state, CLIP, Ok(vec![clip()]));
    assert!(ask(&mut state, VideoAsk::Set(false)).is_empty());
    assert!(!state.video.enabled && !state.video.busy);
}

#[test]
fn the_next_song_is_not_shown_as_a_video() {
    let mut state = watching();
    now_playing(&mut state, track("another1234"), false);
    assert!(!state.video.enabled);
    assert_eq!(state.video.availability, Availability::Unknown);
    // A video queued as one keeps the picture on.
    let mut state = watching();
    let next = Track {
        is_video: true,
        ..track("another1234")
    };
    now_playing(&mut state, next, false);
    assert!(state.video.enabled);
}

#[test]
fn a_switch_the_player_refuses_gives_the_button_back() {
    let mut state = switching_to_the_video();
    ask(&mut state, VideoAsk::Refused);
    assert_eq!(state.video.error.as_deref(), Some(SWITCH_FAILED));
    assert!(!state.video.busy && !state.video.enabled);
    // A refusal of something else, with no switch under way, is not ours.
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Refused);
    assert_eq!(state.video.error, None);
}

#[test]
fn a_press_about_a_song_that_has_gone_is_dropped() {
    let mut state = playing_alone(song());
    ask(&mut state, VideoAsk::Set(true));
    now_playing(&mut state, track("another1234"), false);
    assert!(!state.video.busy);
    assert!(pair_found(&mut state, SONG, Ok(vec![song(), clip()])).is_empty());
    assert!(!state.video.enabled && state.video.switching.is_none());
}

#[test]
fn reaching_the_button_asks_once_whether_the_song_has_a_video() {
    let mut state = playing_alone(song());
    assert_eq!(state.video.availability, Availability::Unknown);
    let asked = ask(&mut state, VideoAsk::Check);
    assert_eq!(asked, [Effect::Fetch(Request::Versions(SONG.into()))]);
    assert_eq!(state.video.availability, Availability::Checking);
    assert!(ask(&mut state, VideoAsk::Check).is_empty());
    pair_found(&mut state, SONG, Ok(vec![song(), clip()]));
    assert_eq!(state.video.availability, Availability::Available);
    // It was only a look: nothing switches.
    assert!(!state.video.busy && !state.video.enabled);
}

#[test]
fn the_button_says_what_pressing_it_would_do() {
    let mut state = playing_alone(song());
    let label = |state: &State, track: &Track| control(&state.video, Some(track), false, false);
    assert_eq!(label(&state, &song()).label, "Watch music video");
    assert!(!label(&state, &song()).blocked);
    ask(&mut state, VideoAsk::Check);
    assert_eq!(label(&state, &song()).label, "Checking for a music video…");
    // Still checking, a press is taken: it waits for the answer.
    assert!(!label(&state, &song()).blocked);
    pair_found(&mut state, SONG, Err(ApiError::RateLimited));
    let failed = "Could not check video availability. Click to retry.";
    assert_eq!(label(&state, &song()).label, failed);
    ask(&mut state, VideoAsk::Set(true));
    assert_eq!(label(&state, &song()).label, "Switching playback format…");
    assert!(label(&state, &song()).blocked);
    let state = watching();
    assert_eq!(label(&state, &clip()).label, "Switch to song");
    let nothing = control(&State::new(Settings::default()).video, None, false, false);
    assert_eq!(nothing.label, "Play a song to watch its video.");
    assert!(nothing.blocked);
}

#[test]
fn a_picture_that_fails_keeps_the_way_to_try_again() {
    let mut state = watching();
    ask(&mut state, VideoAsk::Loading(false));
    assert!(!state.video.loading);
    ask(&mut state, VideoAsk::Failed);
    assert_eq!(state.video.error.as_deref(), Some(LOAD_FAILED));
    // Still on: the surface stays, with the cover and the way to retry.
    assert!(state.video.enabled);
    let before = state.video.key(Some(&clip()));
    ask(&mut state, VideoAsk::Retry);
    assert_eq!(state.video.error, None);
    assert!(state.video.loading);
    // A new key is what has the picture fetched afresh.
    assert_ne!(state.video.key(Some(&clip())), before);
}

#[test]
fn a_video_known_only_by_its_pair_counts_as_one() {
    // A queue saved by an older version has the video's id and no flag.
    let unflagged = track(CLIP);
    let mut state = playing_alone(unflagged.clone());
    assert!(!state.video.is_video(&unflagged));
    ask(&mut state, VideoAsk::Set(true));
    pair_found(&mut state, CLIP, Ok(vec![song(), clip()]));
    assert!(state.video.is_video(&unflagged));
    assert!(state.video.enabled);
    assert!(state.video.key(Some(&unflagged)).is_some());
}

/// In a room whose song is in the player, as its leader or as a guest who
/// may not steer.
fn in_a_room(steers: bool, playing: Track) -> State {
    let mut state = in_room();
    let mut room = room(3);
    if !steers {
        room.owner = "someone-else".into();
        room.mode = Mode::Listen;
    }
    hear(
        &mut state,
        Event::Room {
            room: Box::new(room),
            offset_ms: 0.0,
        },
    );
    now_playing(&mut state, playing, true);
    state
}

#[test]
fn the_rooms_edit_is_changed_by_those_who_steer() {
    let mut state = in_a_room(true, song());
    let effects = ask(&mut state, VideoAsk::Set(true));
    assert_eq!(effects, [Effect::Fetch(Request::Versions(SONG.into()))]);
    let effects = pair_found(&mut state, SONG, Ok(vec![song(), clip()]));
    let fields = json!({
        "track": crate::together::protocol::track_json(&clip()),
        "expectedID": SONG,
    });
    assert_eq!(effects, [Effect::TogetherCommand("variant", fields)]);
    // Shown once the room's song is the video here too.
    assert!(!state.video.enabled);
    now_playing(&mut state, clip(), true);
    assert!(state.video.enabled && !state.video.busy);
}

#[test]
fn a_listener_who_does_not_steer_cannot_change_the_rooms_edit() {
    let mut state = in_a_room(false, song());
    assert!(ask(&mut state, VideoAsk::Set(true)).is_empty());
    assert_eq!(state.video.error.as_deref(), Some(LEADER_CHOOSES));
    assert!(!state.video.busy && !state.video.enabled);
    let button = control(&state.video, Some(&song()), true, false);
    assert!(button.blocked);
    assert_eq!(button.label, "The room leader chooses the media version.");
}

#[test]
fn anyone_in_a_room_may_show_or_hide_the_video_it_plays() {
    // A guest: for themselves alone, and the room is not told.
    let mut state = in_a_room(false, clip());
    assert!(ask(&mut state, VideoAsk::Set(true)).is_empty());
    assert!(state.video.enabled);
    assert_eq!(
        control(&state.video, Some(&clip()), true, false).label,
        "Hide music video"
    );
    assert!(ask(&mut state, VideoAsk::Set(false)).is_empty());
    assert!(!state.video.enabled);
    // One who steers tells the room, for those who follow.
    let mut state = in_a_room(true, clip());
    let effects = ask(&mut state, VideoAsk::Set(true));
    let shown = json!({ "shown": true });
    assert_eq!(effects, [Effect::TogetherCommand("display", shown)]);
}

#[test]
fn a_room_with_no_video_for_its_song_says_so() {
    let mut state = in_a_room(true, song());
    ask(&mut state, VideoAsk::Set(true));
    assert!(pair_found(&mut state, SONG, Ok(vec![song()])).is_empty());
    assert_eq!(state.video.error.as_deref(), Some(NONE_IN_ROOM));
}

#[test]
fn a_rooms_display_change_is_followed_only_by_those_who_chose_to() {
    let change = RoomVideo {
        shown: true,
        by: "someone-else".into(),
        revision: 7,
    };
    // Not following: seen, and left alone.
    assert_eq!(follow(Some(&change), 0, "me", false), (7, None));
    assert_eq!(follow(Some(&change), 0, "me", true), (7, Some(true)));
    // Seen once: the same change is not acted on again.
    assert_eq!(follow(Some(&change), 7, "me", true), (7, None));
    // A listener's own change is not played back at them.
    assert_eq!(follow(Some(&change), 0, "someone-else", true), (7, None));
    assert_eq!(follow(None, 3, "me", true), (3, None));
}

#[test]
fn following_the_room_shows_the_video_when_its_leader_does() {
    let mut state = in_a_room(false, clip());
    let shown_by_leader = |revision| {
        let mut room = room(revision);
        room.owner = "someone-else".into();
        room.mode = Mode::Listen;
        room.video = Some(RoomVideo {
            shown: true,
            by: "someone-else".into(),
            revision,
        });
        Event::Room {
            room: Box::new(room),
            offset_ms: 0.0,
        }
    };
    hear(&mut state, shown_by_leader(4));
    assert!(!state.video.enabled);
    let saved = apply(&mut state, Action::Room(Ask::FollowVideo(true)));
    assert_eq!(saved, [Effect::SaveSettings]);
    assert!(state.settings.together_follow_video);
    hear(&mut state, shown_by_leader(5));
    assert!(state.video.enabled);
}

#[test]
fn leaving_a_room_puts_the_video_back_as_it_was_before() {
    let mut state = watching();
    // Into a room that plays the same video, and the picture hidden there.
    now_playing(&mut state, clip(), true);
    assert!(state.video.before_room);
    state.video.enabled = false;
    now_playing(&mut state, clip(), false);
    assert!(state.video.enabled);
    // Not back on over a song, which has no picture to show.
    now_playing(&mut state, clip(), true);
    now_playing(&mut state, song(), false);
    assert!(!state.video.enabled);
}

#[test]
fn music_videos_among_songs_are_shown_only_when_asked_for() {
    let mut state = ready();
    assert!(!state.shows(&clip()));
    assert!(state.shows(&song()));
    let effects = apply(&mut state, Action::SetShowMusicVideos(true));
    assert_eq!(effects, [Effect::SaveSettings]);
    assert!(state.shows(&clip()));
}
