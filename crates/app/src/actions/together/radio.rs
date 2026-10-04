//! Finding songs from inside a room, and the radio that keeps a room's
//! music going: after a song someone starts, and when the queue runs low.

use std::time::{Duration, Instant};

use serde_json::json;
use spotified_client::models::Track;

use super::super::Effect;
use super::room::routed;
use crate::backend::{Request, Response};
use crate::state::State;
use crate::together::protocol::tracks_json;
use crate::together::rules::{self, RADIO_LOW};
use crate::together::sync::{self, LEADER_PLAYS, Routed};
use crate::together::{Ask, Mode, Phase, Radio};

/// How long a top-up that failed waits before it is tried again.
const RETRY_AFTER: Duration = Duration::from_secs(30);
/// Whoever just added the queue's last song may be adding its radio after
/// it: the leader's top-up leaves it this long, on the relay's clock.
const JUST_ADDED_MS: f64 = 8000.0;

pub(super) fn search(state: &mut State, ask: Ask) -> Vec<Effect> {
    let search = &mut state.together.search;
    match ask {
        Ask::Search(text) => {
            // An answer on its way is for a query no longer on screen.
            search.serial += 1;
            search.query = text.chars().take(200).collect();
            search.searching = false;
            if search.query.trim().is_empty() {
                search.results.clear();
                return Vec::new();
            }
            vec![Effect::DebounceRoomSearch]
        }
        Ask::RunSearch if !search.query.trim().is_empty() => {
            search.searching = true;
            vec![Effect::Fetch(Request::RoomSearch {
                serial: search.serial,
                query: search.query.trim().to_owned(),
            })]
        }
        _ => Vec::new(),
    }
}

/// Plays a song in the room the way YouTube Music plays one on its own:
/// the song at once, then its radio after it. A guest who may only add
/// songs asks for the one they picked instead.
pub(in crate::actions) fn start_radio(state: &mut State, track: Track) -> Vec<Effect> {
    let together = &mut state.together;
    let me = together.me.clone();
    let Some(room) = together
        .room
        .as_ref()
        .filter(|_| together.phase == Phase::Joined)
    else {
        state.toast("Wait for the room to reconnect.");
        return Vec::new();
    };
    if !room.may_control(&me) {
        let asked = match room.mode {
            Mode::Contributions => sync::adding(room, &me, std::slice::from_ref(&track), None),
            _ => Routed::Refused(LEADER_PLAYS),
        };
        return routed(state, asked);
    }
    // The leader's top-up must not fetch the same radio a second time.
    together.radio = Radio {
        seed: track.id.clone(),
        busy: true,
        since: room.revision,
        ..Radio::default()
    };
    let replace = json!({ "tracks": tracks_json(std::slice::from_ref(&track)) });
    vec![
        Effect::TogetherCommand("replace", replace),
        Effect::Fetch(Request::Radio(track.id)),
    ]
}

/// Keeps a room's music going the way autoplay keeps a queue going: when
/// only a few songs are left, the leader adds the last song's radio. Only
/// the leader does, so members do not each add a copy of their own.
pub(super) fn top_up(state: &mut State) -> Vec<Effect> {
    let server_now = state.together.server_now();
    let together = &mut state.together;
    let Some(room) = together
        .room
        .as_ref()
        .filter(|_| together.phase == Phase::Joined)
    else {
        return Vec::new();
    };
    let waiting = together
        .radio
        .retry_at
        .is_some_and(|at| Instant::now() < at);
    let wanted = room.leads(&together.me)
        && room.repeat == crate::together::protocol::Repeat::Off
        && state.settings.autoplay
        && !together.radio.busy
        && together.seed == crate::together::Seed::None
        && !waiting;
    let Some(last) = room.queue.last().filter(|_| wanted) else {
        return Vec::new();
    };
    if last.track.id == together.radio.seed
        || room.left() >= RADIO_LOW
        || server_now - last.added_at < JUST_ADDED_MS
    {
        return Vec::new();
    }
    let fetch = Effect::Fetch(Request::Radio(last.track.id.clone()));
    together.radio.busy = true;
    together.radio.found = None;
    together.radio.topping_up = Some(last.id.clone());
    vec![fetch]
}

pub(in crate::actions) fn answered(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::RoomSearch { serial, result } => {
            let search = &mut state.together.search;
            // An answer to a query that has since been replaced, or cleared.
            if serial != search.serial {
                return Vec::new();
            }
            search.searching = false;
            match result {
                Ok(found) => search.results = found,
                Err(error) => state.together.error = Some(error.to_string()),
            }
            Vec::new()
        }
        Response::Radio(seed, result) => radio_found(state, &seed, result.ok()),
        _ => Vec::new(),
    }
}

/// A radio arrived, or failed to. It is added only if the room is still
/// where it was when the radio was asked for: someone may have picked
/// another song, or another leader, in the meantime.
fn radio_found(state: &mut State, seed: &str, found: Option<Vec<Track>>) -> Vec<Effect> {
    let radio = &mut state.together.radio;
    let by_hand = radio.topping_up.is_none();
    if !radio.busy || (by_hand && radio.seed != seed) {
        return Vec::new();
    }
    let Some(found) = found else {
        radio.busy = false;
        match radio.topping_up.take() {
            Some(_) => radio.retry_at = Some(Instant::now() + RETRY_AFTER),
            None => state.toast_error("Couldn't load this song's radio."),
        }
        return Vec::new();
    };
    radio.found = Some(found);
    add_radio(state)
}

/// Adds the radio that is held, once the room is known to be where it was
/// asked for. A song started by hand has to be the room's first: its radio
/// can arrive before the room says the song is playing, and waits for that.
pub(super) fn add_radio(state: &mut State) -> Vec<Effect> {
    let together = &mut state.together;
    let Some(room) = together
        .room
        .as_ref()
        .filter(|_| together.phase == Phase::Joined)
    else {
        return Vec::new();
    };
    let radio = &mut together.radio;
    let by_hand = radio.topping_up.is_none();
    if radio.found.is_none() || (by_hand && room.revision <= radio.since) {
        return Vec::new();
    }
    let found = radio.found.take().unwrap_or_default();
    radio.busy = false;
    let still = match radio.topping_up.take() {
        Some(last) => {
            room.leads(&together.me)
                && state.settings.autoplay
                && room.queue.last().is_some_and(|entry| entry.id == last)
        }
        None => room
            .current()
            .is_some_and(|(_, entry)| entry.track.id == radio.seed),
    };
    if !still {
        return Vec::new();
    }
    // The leader's top-up must not fetch this radio a second time.
    if let Some(last) = room.queue.last().filter(|_| !by_hand) {
        radio.seed.clone_from(&last.track.id);
    }
    let fresh = rules::fresh_radio(found, &radio.seed, room, &together.heard);
    if fresh.is_empty() {
        return Vec::new();
    }
    // Marked as radio: what people add later goes ahead of it.
    let fields = json!({ "tracks": tracks_json(&fresh), "radio": true });
    vec![Effect::TogetherCommand("enqueue", fields)]
}
