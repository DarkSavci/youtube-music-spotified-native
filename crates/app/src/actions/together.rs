//! Listen Together, as the app's state sees it: entering and leaving a
//! room, hearing the room, and keeping the player with it.

mod radio;
mod room;
mod servers;

use std::time::{Duration, Instant};

use serde_json::json;
use spotified_client::session::Command;

use super::{Action, Effect};
use crate::state::State;
use crate::together::protocol::{MOST_TRACKS, checked_address, tracks_json};
use crate::together::sync::{self, Local, Standing};
use crate::together::{Enter, Event, Field, Options, Phase, Seed, ServerForm, rules};

pub(super) use radio::{answered, start_radio};
pub(super) use room::{asked, confirmed};

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
                Field::Name => state.settings.together_name = kept(50),
                Field::RoomName => state.settings.together_room_name = kept(80),
                Field::Pin => {
                    let digits = text.chars().filter(char::is_ascii_digit).take(8);
                    state.together.form.pin = digits.collect();
                }
            }
            Vec::new()
        }
        Action::TogetherMode(mode) => {
            state.settings.together_mode = mode;
            Vec::new()
        }
        Action::TogetherCreate => {
            let enter = Enter::Create {
                mode: state.settings.together_mode,
                room_name: state.settings.together_room_name.trim().to_owned(),
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
    // With no server chosen the way in is to save one: the form opens.
    let Some(server) = state.settings.together_server() else {
        state.together.manage = Some(ServerForm::default());
        return Vec::new();
    };
    if !state.core_ready() {
        return refuse(
            state,
            "Wait for the local music service to connect, then try again.",
        );
    }
    let url = match checked_address(&server.url) {
        Ok(url) => url,
        Err(why) => return refuse(state, why),
    };
    if matches!(&enter, Enter::Join { pin } if pin.len() != 8) {
        return refuse(state, "A room's PIN has eight digits.");
    }
    let account = state.account.as_ref();
    let typed = state.settings.together_name.trim();
    let name = if !typed.is_empty() {
        typed.to_owned()
    } else {
        account.map_or_else(|| "Listener".to_owned(), |account| account.name.clone())
    };
    let avatar = account
        .filter(|_| state.settings.together_share_picture)
        .map(|account| account.avatar_url.clone())
        .unwrap_or_default();
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
    state.together.manage = None;
    let options = Options {
        url,
        name,
        avatar,
        enter,
    };
    vec![Effect::SaveSettings, Effect::TogetherConnect(options)]
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
            let previous = state.together.room.replace(*room);
            state.together.offset_ms = offset_ms;
            state.together.phase = Phase::Joined;
            if let Some(previous) = previous {
                room_changed(state, &previous);
            }
            super::video::room_heard(state);
            let mut effects = keep_up(state);
            effects.extend(radio::add_radio(state));
            effects.extend(radio::top_up(state));
            effects
        }
        Event::Refused(why) => {
            // Whatever was refused, a radio on its way is not waited for.
            state.together.radio.busy = false;
            state.together.radio.found = None;
            state.toast_error(why);
            Vec::new()
        }
        Event::Ended(reason) => leave(state, Some(reason).filter(|reason| !reason.is_empty())),
        Event::Failed(why) => leave(state, Some(why)),
    }
}

/// The room has gone from `previous` to what is now held: say what this
/// listener should hear of it, and note what the room has played.
fn room_changed(state: &mut State, previous: &crate::together::Room) {
    let together = &mut state.together;
    let Some(room) = &together.room else {
        return;
    };
    rules::remember_heard(&mut together.heard, room);
    // Someone sought: follow at once, not as drift that may wait.
    if rules::jumped(previous, room) {
        together.corrected_at = None;
    }
    // The limit on screen is the room's again once the room has a new one.
    if previous.limit != room.limit {
        together.limit_text = None;
    }
    let mut news = rules::request_news(previous, room, &together.me, &mut together.withdrawn);
    let latest = room.activity.last();
    let happened = latest.map(|last| &last.id) != previous.activity.last().map(|last| &last.id);
    if news.is_empty() && happened && state.settings.together_notifications {
        news.extend(latest.map(|last| last.text.clone()));
    }
    for line in news {
        state.toast(line);
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
    let stuck = state.notice.is_some();
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
    // The room's song is the one in the player and it will not play:
    // asking for it again each second would not make it, so the room is
    // told instead, and "Retry playback" is how it is asked for again.
    let the_rooms = room.current().is_some_and(|(_, entry)| {
        applied == Some(entry.id.as_str()) && local.current == Some(entry.track.id.as_str())
    });
    let unplayable = playback.current().is_some_and(|track| !track.playable);
    if the_rooms && (stuck || unplayable) {
        together.standing = Standing::Unavailable;
        let status = Standing::Unavailable.wire(room.playing);
        return vec![Effect::TogetherStatus(status, room.current.clone())];
    }
    // The length as it was measured here, where the room's is missing or
    // a little out: the room ends a song by its length. Said once for
    // each song, by someone the room lets steer it.
    if let (Some((entry, measured)), Some((_, current))) = (&playback.room_length, room.current())
        && *entry == current.id
        && local.current == Some(current.track.id.as_str())
        && room.may_control(&together.me)
        && sync::length_worth_telling(current.track.duration_ms.max(0.0) as u64, *measured)
    {
        let told = format!("{entry}:{}", current.track.id);
        if together.length_told.as_deref() != Some(told.as_str()) {
            together.length_told = Some(told);
            let fields = json!({ "entry": entry, "durationMs": measured });
            effects.push(Effect::TogetherCommand("duration", fields));
        }
    }
    // The room moves on when a player says the song is over; it is said
    // again until it does, in case the first was lost.
    let ended_here = playback.room_ended.is_some() && playback.room_ended == room.current;
    let standing = Standing::of(room, server_now, &local, ended_here);
    if let Some(command) = sync::follow(room, server_now, &local, applied, since) {
        together.applied_entry.clone_from(&room.current);
        together.corrected_at = Some(now);
        effects.push(Effect::Command(command));
    }
    let due = together
        .ended_sent
        .is_none_or(|at| now.duration_since(at) >= ENDED_EVERY);
    if ended_here && room.playing && due {
        together.ended_sent = Some(now);
        let fields = json!({ "current": room.current });
        effects.push(Effect::TogetherCommand("ended", fields));
    }
    together.standing = standing;
    let status = standing.wire(room.playing);
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
