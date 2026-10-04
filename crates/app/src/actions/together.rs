//! Listen Together, as the app's state sees it: entering and leaving a
//! room, hearing the room, and keeping the player with it.

use std::time::{Duration, Instant};

use serde_json::json;
use spotified_client::session::Command;

use super::{Action, Effect};
use crate::state::State;
use crate::together::protocol::{MOST_TRACKS, checked_address, tracks_json};
use crate::together::sync::{self, Local};
use crate::together::{Enter, Event, Field, Options, Phase, Seed};

/// How often the room is told again that this player reached a song's end,
/// until the room moves on.
const ENDED_EVERY: Duration = Duration::from_secs(5);
/// A song this far in or less is not worth taking a new room to.
const SEED_SEEK_FROM_MS: u64 = 2000;

pub(super) fn together(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::TogetherField(field, text) => {
            let kept = |most: usize| text.chars().take(most).collect::<String>();
            match field {
                Field::Server => state.settings.together_server = kept(300),
                Field::Name => state.settings.together_name = kept(50),
                Field::RoomName => state.together.form.room_name = kept(80),
                Field::Pin => {
                    let digits = text.chars().filter(char::is_ascii_digit).take(8);
                    state.together.form.pin = digits.collect();
                }
            }
            Vec::new()
        }
        Action::TogetherMode(mode) => {
            state.together.form.mode = mode;
            Vec::new()
        }
        Action::TogetherCreate => {
            let enter = Enter::Create {
                mode: state.together.form.mode,
                room_name: state.together.form.room_name.trim().to_owned(),
            };
            enter_room(state, enter)
        }
        Action::TogetherJoin => {
            let pin = state.together.form.pin.clone();
            enter_room(state, Enter::Join { pin })
        }
        Action::TogetherLeave => leave(state, None),
        Action::TogetherEvent(event) => heard(state, *event),
        Action::TogetherTick => keep_up(state),
        _ => Vec::new(),
    }
}

fn refuse(state: &mut State, why: &str) -> Vec<Effect> {
    state.together.error = Some(why.to_owned());
    Vec::new()
}

fn enter_room(state: &mut State, enter: Enter) -> Vec<Effect> {
    if state.together.phase != Phase::Idle {
        return Vec::new();
    }
    if !state.core_ready() {
        return refuse(
            state,
            "Wait for the music service to start, then try again.",
        );
    }
    let url = match checked_address(&state.settings.together_server) {
        Ok(url) => url,
        Err(why) => return refuse(state, why),
    };
    if matches!(&enter, Enter::Join { pin } if pin.len() != 8) {
        return refuse(state, "A room's PIN has eight digits.");
    }
    let typed = state.settings.together_name.trim();
    let name = if !typed.is_empty() {
        typed.to_owned()
    } else {
        let account = state.account.as_ref().map(|account| account.name.clone());
        account.unwrap_or_else(|| "Listener".to_owned())
    };
    // A room made while music plays starts with that music.
    state.together.seed = match (&enter, &state.playback) {
        (Enter::Create { .. }, Some(playback)) if playback.current().is_some() => {
            let queue = &playback.session.queue;
            let from = queue.index.min(queue.items.len());
            let end = (from + MOST_TRACKS).min(queue.items.len());
            Seed::Wanted {
                tracks: queue.items[from..end].to_vec(),
                position_ms: playback.position_ms(Instant::now()),
                playing: playback.wants_to_play(),
            }
        }
        _ => Seed::None,
    };
    state.together.phase = Phase::Connecting;
    state.together.error = None;
    vec![
        Effect::SaveSettings,
        Effect::TogetherConnect(Options { url, name, enter }),
    ]
}

/// Leaves the room, or gives up entering one. The music carries on from
/// the room's queue, now this player's own.
fn leave(state: &mut State, error: Option<String>) -> Vec<Effect> {
    let was_in_room = state.together.in_room();
    state.together.reset(error);
    let mut effects = vec![Effect::TogetherDisconnect];
    if was_in_room {
        effects.push(Effect::Command(Command::LeaveRoom { keep_queue: true }));
    }
    effects
}

fn heard(state: &mut State, event: Event) -> Vec<Effect> {
    match event {
        Event::Reconnecting => {
            state.together.phase = Phase::Reconnecting;
            Vec::new()
        }
        Event::Waiting => {
            state.together.phase = Phase::Waiting;
            Vec::new()
        }
        Event::Joined(member) => {
            state.together.me = member;
            state.together.phase = Phase::Joined;
            Vec::new()
        }
        Event::Room { room, offset_ms } => {
            let newest = state.together.room.as_ref().map_or(0, |room| room.revision);
            // Equal is let through: who is listening changes without the
            // room's revision changing.
            if room.revision < newest {
                return Vec::new();
            }
            state.together.room = Some(*room);
            state.together.offset_ms = offset_ms;
            state.together.phase = Phase::Joined;
            keep_up(state)
        }
        Event::Refused(why) => {
            state.toast_error(why);
            Vec::new()
        }
        Event::Ended(reason) => {
            if !reason.is_empty() {
                state.toast(reason);
            }
            leave(state, None)
        }
        Event::Failed(why) => leave(state, Some(why)),
    }
}

/// Brings the player to the room if it has strayed, and tells the room how
/// the player is doing. Run when the room changes and once a second.
fn keep_up(state: &mut State) -> Vec<Effect> {
    if !state.together.in_room() {
        return Vec::new();
    }
    if state.together.seed != Seed::None {
        return seed_step(state);
    }
    let server_now = state.together.server_now();
    let together = &mut state.together;
    let (Some(room), Some(playback)) = (&together.room, &state.playback) else {
        return Vec::new();
    };
    let now = Instant::now();
    let settled = !playback.wants_to_play() || playback.is_playing();
    let local = Local {
        following: playback.following_room,
        queue: playback
            .session
            .queue
            .items
            .iter()
            .map(|track| track.id.as_str())
            .collect(),
        current: playback.current().map(|track| track.id.as_str()),
        position_ms: playback.position_ms(now),
        playing: playback.wants_to_play(),
        settled,
    };
    let since = together.corrected_at.map(|at| now.duration_since(at));
    let applied = together.applied_entry.as_deref();
    let mut effects = Vec::new();
    if let Some(command) = sync::follow(room, server_now, &local, applied, since) {
        together.applied_entry.clone_from(&room.current);
        together.corrected_at = Some(now);
        effects.push(Effect::Command(command));
    }
    // The room moves on when a player says the song is over; it is said
    // again until it does, in case the first was lost.
    let ended_here = playback.room_ended.is_some() && playback.room_ended == room.current;
    let due = together
        .ended_sent
        .is_none_or(|at| now.duration_since(at) >= ENDED_EVERY);
    if ended_here && room.playing && due {
        together.ended_sent = Some(now);
        let fields = json!({ "current": room.current });
        effects.push(Effect::TogetherCommand("ended", fields));
    }
    let status = match (settled, room.playing) {
        (false, _) => "buffering",
        (true, true) => "listening",
        (true, false) => "paused",
    };
    effects.push(Effect::TogetherStatus(status, room.current.clone()));
    effects
}

/// The next step of bringing the music that was playing into a new room.
fn seed_step(state: &mut State) -> Vec<Effect> {
    let together = &mut state.together;
    let Some(room) = &together.room else {
        return Vec::new();
    };
    match std::mem::take(&mut together.seed) {
        Seed::None => Vec::new(),
        Seed::Wanted {
            tracks,
            position_ms,
            playing,
        } => {
            together.seed = Seed::Enqueued {
                position_ms,
                playing,
            };
            let fields = json!({ "tracks": tracks_json(&tracks) });
            vec![Effect::TogetherCommand("enqueue", fields)]
        }
        Seed::Enqueued {
            position_ms,
            playing,
        } => {
            // Not there yet: wait for the room to say it has the songs.
            let Some(first) = room.queue.first() else {
                together.seed = Seed::Enqueued {
                    position_ms,
                    playing,
                };
                return Vec::new();
            };
            together.seed = Seed::Sought { playing };
            if room.current.is_none() {
                let fields = json!({ "entry": first.id });
                vec![Effect::TogetherCommand("jump", fields)]
            } else if position_ms > SEED_SEEK_FROM_MS {
                let fields = json!({ "positionMs": position_ms });
                vec![Effect::TogetherCommand("seek", fields)]
            } else {
                Vec::new()
            }
        }
        Seed::Sought { playing } => {
            if playing && !room.playing {
                vec![Effect::TogetherCommand("play", json!({}))]
            } else if !playing && room.playing {
                vec![Effect::TogetherCommand("pause", json!({}))]
            } else {
                Vec::new()
            }
        }
    }
}
