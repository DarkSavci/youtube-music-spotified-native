//! Switching what plays between a song and its music video.
//!
//! Alone, the switch is the player's: the queue's entry becomes the other
//! edit and carries on from the same moment. In a Listen Together room the
//! edit is the room's, so everyone hears the same thing: those who steer
//! may change it, and anyone may hide the picture for themselves.

use std::time::Instant;

use serde_json::json;
use spotified_client::ApiError;
use spotified_client::models::Track;
use spotified_client::session::Command;

use super::Effect;
use super::types::VideoAsk;
use crate::backend::Request;
use crate::state::State;
use crate::state::video::{Availability, Switching, Wanting};
use crate::together::protocol::{self, RoomVideo};

pub const CHECK_FAILED: &str = "Could not check for a music video. Please try again.";
pub const NONE_FOR_SONG: &str = "No matching music video is available for this song.";
pub const NONE_IN_ROOM: &str = "No matching music video is available.";
pub const LEADER_CHOOSES: &str = "The leader chooses the media version in this room.";
pub const SWITCH_FAILED: &str = "Could not switch versions. Please try again.";
pub const LOAD_FAILED: &str = "The video could not be loaded. You can keep listening or retry.";

pub(super) fn asked(state: &mut State, ask: VideoAsk) -> Vec<Effect> {
    match ask {
        VideoAsk::Set(enabled) => set(state, enabled),
        VideoAsk::Check => check(state),
        VideoAsk::Retry => {
            state.video.error = None;
            state.video.revision += 1;
            state.video.loading = state.video.enabled;
            Vec::new()
        }
        VideoAsk::Loading(loading) => {
            state.video.loading = loading;
            Vec::new()
        }
        VideoAsk::Failed => {
            // Still on: the cover shows where the picture would, with the
            // way to try again.
            state.video.loading = false;
            state.video.error = Some(LOAD_FAILED.to_owned());
            Vec::new()
        }
        VideoAsk::Refused => {
            if state.video.switching.is_some() {
                fail(state, SWITCH_FAILED);
            }
            Vec::new()
        }
    }
}

fn current(state: &State) -> Option<(Track, bool)> {
    let playback = state.playback.as_ref()?;
    let track = playback.current()?.clone();
    Some((track, playback.following_room))
}

/// Whether this listener may steer the room they are in.
fn controls_room(state: &State) -> bool {
    let together = &state.together;
    together
        .room
        .as_ref()
        .is_some_and(|room| room.may_control(&together.me))
}

fn set(state: &mut State, enabled: bool) -> Vec<Effect> {
    let Some((track, following)) = current(state) else {
        return Vec::new();
    };
    let now = Instant::now();
    if state.video.busy && !state.video.switch_overdue(now) {
        return Vec::new();
    }
    state.video.switching = None;
    if following {
        // Hiding the picture, or showing the one the room already plays,
        // is this listener's own business; those who steer tell the room.
        if !enabled || track.is_video {
            shown(state, enabled);
            return if controls_room(state) {
                vec![Effect::TogetherCommand(
                    "display",
                    json!({ "shown": enabled }),
                )]
            } else {
                Vec::new()
            };
        }
        if !controls_room(state) {
            state.video.error = Some(LEADER_CHOOSES.to_owned());
            return Vec::new();
        }
    }
    state.video.busy = true;
    state.video.error = None;
    // Hiding the picture always works, whatever the switch back comes to.
    if !enabled {
        state.video.enabled = false;
    }
    match state.video.pair(&track.id).map(<[Track]>::to_vec) {
        Some(pair) => settle(state, &track, enabled, Ok(pair)),
        None => {
            state.video.wanting = Some(Wanting {
                track_id: track.id.clone(),
                enabled,
            });
            vec![Effect::Fetch(Request::Versions(track.id))]
        }
    }
}

/// Finds out whether the song playing has a video, unless that is known.
fn check(state: &mut State) -> Vec<Effect> {
    let Some((track, _)) = current(state) else {
        return Vec::new();
    };
    let video = &mut state.video;
    if video.about == track.id && video.availability != Availability::Unknown {
        return Vec::new();
    }
    video.about.clone_from(&track.id);
    if video.is_video(&track) {
        video.availability = Availability::Available;
        return Vec::new();
    }
    if let Some(pair) = video.pair(&track.id) {
        video.availability = availability(pair);
        return Vec::new();
    }
    video.availability = Availability::Checking;
    vec![Effect::Fetch(Request::Versions(track.id))]
}

/// Gives the room's song its artists' pages and its album back.
///
/// A relay passes a song on as a title and the names of its artists: none
/// of them leads anywhere, and the album is not said at all. What YouTube
/// says of the song has both, so in a room it is asked about the song that
/// plays (once: the core keeps the answer) and the song is filled in from
/// it, here and on every later report of the session.
pub(super) fn room_credits(state: &mut State) -> Vec<Effect> {
    let Some(playback) = &mut state.playback else {
        return Vec::new();
    };
    let index = playback.session.queue.index;
    let Some(track) = playback.session.queue.items.get_mut(index) else {
        return Vec::new();
    };
    let bare = track.album.is_none() || track.artists.iter().any(|artist| artist.id.is_empty());
    if !playback.following_room || !bare {
        return Vec::new();
    }
    match state.video.pair(&track.id) {
        Some(pair) => {
            if let Some(known) = pair.iter().find(|edit| edit.id == track.id) {
                if !known.artists.is_empty() {
                    track.artists.clone_from(&known.artists);
                }
                if track.album.is_none() {
                    track.album.clone_from(&known.album);
                }
            }
            Vec::new()
        }
        // Asked once for each song, whatever comes of it: a song YouTube
        // will not speak of stays as the relay gave it.
        None if state.video.credits_asked != track.id => {
            state.video.credits_asked.clone_from(&track.id);
            vec![Effect::Fetch(Request::Versions(track.id.clone()))]
        }
        None => Vec::new(),
    }
}

fn availability(pair: &[Track]) -> Availability {
    if pair.iter().any(|edit| edit.is_video && edit.playable) {
        Availability::Available
    } else {
        Availability::Unavailable
    }
}

/// YouTube has said what the pair of `track_id` is, or could not.
pub(super) fn answered(
    state: &mut State,
    track_id: String,
    result: Result<Vec<Track>, ApiError>,
) -> Vec<Effect> {
    if let Ok(pair) = &result {
        state.video.remember(&track_id, pair.clone());
    }
    let playing = current(state);
    let video = &mut state.video;
    if video.about == track_id && video.availability == Availability::Checking {
        video.availability = match &result {
            Ok(pair) => availability(pair),
            Err(_) => Availability::Error,
        };
    }
    let waited = video
        .wanting
        .take_if(|wanting| wanting.track_id == track_id);
    let Some(wanting) = waited else {
        // Asked for the room's song, to say whose it is and what it is on.
        return room_credits(state);
    };
    match playing {
        Some((track, _)) if track.id == track_id => settle(state, &track, wanting.enabled, result),
        // Another song plays by now: the press was about the last one.
        _ => {
            state.video.busy = false;
            Vec::new()
        }
    }
}

/// Carries out a press of the button, now the song's pair is in hand.
fn settle(
    state: &mut State,
    track: &Track,
    enabled: bool,
    pair: Result<Vec<Track>, ApiError>,
) -> Vec<Effect> {
    let following = state
        .playback
        .as_ref()
        .is_some_and(|playback| playback.following_room);
    let is_video = state.video.is_video(track);
    let pair = match pair {
        Ok(pair) => pair,
        // Hiding the picture needs no answer from YouTube, and nor does
        // showing a track that is the video already.
        Err(_) if !following && (is_video || !enabled) => vec![track.clone()],
        Err(_) => return fail(state, CHECK_FAILED),
    };
    state.video.about.clone_from(&track.id);
    state.video.availability = if is_video {
        Availability::Available
    } else {
        availability(&pair)
    };
    let other = pair
        .iter()
        .find(|edit| edit.playable && edit.is_video == enabled)
        .filter(|edit| edit.id != track.id);
    let switching = |to: &Track| Switching {
        to: to.id.clone(),
        enabled,
        since: Instant::now(),
    };
    if following {
        let Some(other) = other else {
            return fail(state, NONE_IN_ROOM);
        };
        state.video.switching = Some(switching(other));
        let fields = json!({ "track": protocol::track_json(other), "expectedID": track.id });
        return vec![Effect::TogetherCommand("variant", fields)];
    }
    match other {
        Some(other) => {
            state.video.switching = Some(switching(other));
            vec![Effect::Command(Command::SwitchVariant {
                expected: track.id.clone(),
                track: Box::new(other.clone()),
            })]
        }
        None if enabled && !is_video => fail(state, NONE_FOR_SONG),
        None => {
            state.video.busy = false;
            shown(state, enabled);
            Vec::new()
        }
    }
}

/// Shows the picture or the cover from here on, afresh.
fn shown(state: &mut State, enabled: bool) {
    let video = &mut state.video;
    video.enabled = enabled;
    video.loading = enabled;
    video.error = None;
    video.revision += 1;
}

fn fail(state: &mut State, why: &str) -> Vec<Effect> {
    let video = &mut state.video;
    video.enabled = false;
    video.busy = false;
    video.loading = false;
    video.switching = None;
    video.wanting = None;
    video.error = Some(why.to_owned());
    Vec::new()
}

/// The session has changed: a switch may have happened, a room may have
/// been entered or left, and another track may be playing.
pub(super) fn session_changed(state: &mut State) -> Vec<Effect> {
    let playing = state
        .playback
        .as_ref()
        .and_then(|playback| playback.current().cloned());
    let following = state
        .playback
        .as_ref()
        .is_some_and(|playback| playback.following_room);
    let id = playing.as_ref().map(|track| track.id.clone());
    if let Some(switching) = state.video.switching.clone() {
        if id.as_deref() == Some(switching.to.as_str()) {
            state.video.switching = None;
            state.video.busy = false;
            shown(state, switching.enabled);
        } else if state.video.switch_overdue(Instant::now()) {
            fail(state, SWITCH_FAILED);
        }
    }
    if following != state.video.in_room {
        state.video.in_room = following;
        if following {
            state.video.before_room = state.video.enabled;
            state.video.room_seen = 0;
        } else {
            // Back to how it was before the room, if what plays can be.
            let video = playing.as_ref().is_some_and(|track| track.is_video);
            state.video.enabled = state.video.before_room && video;
        }
    }
    if id == state.video.playing {
        return Vec::new();
    }
    state.video.playing.clone_from(&id);
    let video = &mut state.video;
    // A press that waited on the last track is over with it.
    if video
        .wanting
        .take_if(|wanting| Some(&wanting.track_id) != id.as_ref())
        .is_some()
        && video.switching.is_none()
    {
        video.busy = false;
    }
    let Some(track) = playing else {
        video.enabled = false;
        video.loading = false;
        video.error = None;
        video.about.clear();
        video.availability = Availability::Unknown;
        return Vec::new();
    };
    // Going on to another song is not a wish to watch its video: only an
    // explicit switch, or a video queued as one, shows a picture.
    if !track.is_video {
        video.enabled = false;
    }
    video.loading = video.enabled;
    video.about.clone_from(&track.id);
    video.availability = if track.is_video {
        Availability::Available
    } else {
        Availability::Unknown
    };
    Vec::new()
}

/// What a room's display change means here: the newest change seen, and
/// whether to show or hide the picture for it. A listener's own change is
/// not echoed back at them, and nobody follows unless they chose to.
pub fn follow(
    change: Option<&RoomVideo>,
    seen: u64,
    me: &str,
    follows: bool,
) -> (u64, Option<bool>) {
    match change {
        Some(change) if change.revision > seen => {
            let show = (change.by != me && follows).then_some(change.shown);
            (change.revision, show)
        }
        _ => (seen, None),
    }
}

/// The room has been heard from: someone may have shown or hidden the video.
pub(super) fn room_heard(state: &mut State) {
    let together = &state.together;
    let Some(room) = &together.room else {
        return;
    };
    let follows = state.settings.together_follow_video;
    let (seen, show) = follow(
        room.video.as_ref(),
        state.video.room_seen,
        &together.me,
        follows,
    );
    state.video.room_seen = seen;
    if let Some(show) = show {
        shown(state, show);
    }
}
