//! Keeping this player with the room, and the room's controls with this
//! player's buttons. Both are plain functions of what the room says and
//! what the player is doing.

use std::time::Duration;

use serde_json::{Value, json};
use spotified_client::models::Track;
use spotified_client::session::Command;

use super::protocol::{MOST_TRACKS, Mode, Room, tracks_json};

/// How far this player may be from the room before it is brought back.
const DRIFT_MS: u64 = 1000;
/// How long after one correction the next may come, so a slow player is
/// not seeked over and over.
const BETWEEN_CORRECTIONS: Duration = Duration::from_secs(4);
/// Past this far into a song, "previous" starts it again instead.
pub const RESTART_AFTER_MS: u64 = 3000;

/// What this player is doing.
pub struct Local<'a> {
    /// Whether the core is following a room at all yet.
    pub following: bool,
    pub queue: Vec<&'a str>,
    pub current: Option<&'a str>,
    pub position_ms: u64,
    /// Playing, or meaning to.
    pub playing: bool,
    /// Neither loading nor stalled: its position can be trusted.
    pub settled: bool,
}

/// The command that brings the player to the room, if it is not there:
/// another song, another queue, the other of playing and paused, or too
/// far from where the room is in the song.
pub fn follow(
    room: &Room,
    server_now: f64,
    local: &Local<'_>,
    applied_entry: Option<&str>,
    since_correction: Option<Duration>,
) -> Option<Command> {
    let Some((index, entry)) = room.current() else {
        // An empty room still takes the player: its own queue is put by.
        return (!local.following).then(|| Command::FollowRoom {
            tracks: Vec::new(),
            index: 0,
            entry: String::new(),
            position_ms: 0,
            playing: false,
        });
    };
    let position_ms = room.position_at(server_now);
    let another_song =
        applied_entry != Some(entry.id.as_str()) || local.current != Some(entry.track.id.as_str());
    let another_queue = !local
        .queue
        .iter()
        .copied()
        .eq(room.queue.iter().map(|entry| entry.track.id.as_str()));
    let adrift = local.settled
        && local.position_ms.abs_diff(position_ms) > DRIFT_MS
        && since_correction.is_none_or(|since| since >= BETWEEN_CORRECTIONS);
    let wanted = !local.following
        || another_song
        || another_queue
        || local.playing != room.playing
        || adrift;
    wanted.then(|| Command::FollowRoom {
        tracks: room.tracks(),
        index,
        entry: entry.id.clone(),
        position_ms,
        playing: room.playing,
    })
}

/// How this player stands with the room: what the page says of it, and
/// what the room's other listeners are told.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Standing {
    #[default]
    Joining,
    /// This player reached the song's end before the room did.
    WaitingForNext,
    Buffering,
    CatchingUp,
    InSync,
    Paused,
    /// The room's song cannot be played here: YouTube refuses this
    /// device for now, or will not give this song at all.
    Unavailable,
}

impl Standing {
    pub fn of(room: &Room, server_now: f64, local: &Local<'_>, ended_here: bool) -> Self {
        let adrift = local.position_ms.abs_diff(room.position_at(server_now)) > DRIFT_MS;
        if ended_here && room.playing {
            Standing::WaitingForNext
        } else if !local.settled {
            Standing::Buffering
        } else if adrift && room.playing {
            Standing::CatchingUp
        } else if room.playing {
            Standing::InSync
        } else {
            Standing::Paused
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Standing::Joining => "Ready to join",
            Standing::WaitingForNext => "Waiting for the next song",
            Standing::Buffering => "Buffering",
            Standing::CatchingUp => "Catching up",
            Standing::InSync => "In sync",
            Standing::Paused => "Paused together",
            Standing::Unavailable => "Track unavailable",
        }
    }

    /// What the relay calls it.
    pub fn wire(self, room_playing: bool) -> &'static str {
        match self {
            Standing::Buffering => "buffering",
            Standing::CatchingUp => "catching up",
            // The relay moves the room on when everyone says so of the
            // same song.
            Standing::Unavailable => "unavailable",
            _ if room_playing => "listening",
            _ => "paused",
        }
    }
}

/// How far a measured length must be from the room's figure to be worth
/// telling, and how far it may be before it is not the same recording.
const LENGTH_OFF_MS: std::ops::RangeInclusive<u64> = 1001..=15_000;

/// Whether the room should be told this player's measure of a song the
/// room has down as `listed` long: when it has no length for it, or one
/// that is out by more than a second. A measure wildly unlike the room's
/// is another recording, and says nothing of the room's.
pub fn length_worth_telling(listed_ms: u64, measured_ms: u64) -> bool {
    measured_ms > 0 && (listed_ms == 0 || LENGTH_OFF_MS.contains(&measured_ms.abs_diff(listed_ms)))
}

/// Where one of the player's own commands goes while it is in a room.
#[derive(Debug, PartialEq)]
pub enum Routed {
    /// To the room, as this command of its own.
    Room(&'static str, Value),
    /// To the core, as outside a room.
    Core,
    /// Nowhere, with a sentence saying why.
    Refused(&'static str),
}

pub const LEADER_PLAYS: &str = "The leader controls playback in this room.";
pub const LISTEN_ONLY: &str = "This room is listen only.";
const LEADER_SETS: &str = "Only the leader can change room settings.";
const NO_SHUFFLE: &str = "Choose First in, first out or Take turns in room settings.";
const NOT_AHEAD: &str = "Only playback controllers may insert ahead of others.";
const OWN_ONLY: &str = "You may only edit your upcoming contributions.";
const LEADER_ORDERS: &str = "The leader controls queue order.";
/// Said when a command held more songs than the room takes at once.
pub const FIRST_HUNDRED: &str = "Using the first 100 songs. Add more in batches from your library.";

/// Whether `command` holds more songs than go to the room in one go.
pub fn trimmed(command: &Command) -> bool {
    match command {
        Command::Enqueue { tracks, .. } => tracks.len() > MOST_TRACKS,
        Command::Play {
            tracks,
            start_index,
            ..
        } => tracks.len().saturating_sub(*start_index) > MOST_TRACKS,
        _ => false,
    }
}

/// The room's command that adds `tracks` for `me`: into the queue, before
/// the entry `before` when that is given, or as requests where the leader
/// approves what guests add. Which it became in the end is the relay's to
/// say: the leader may change the rules while this is on its way.
pub fn adding(room: &Room, me: &str, tracks: &[Track], before: Option<String>) -> Routed {
    if room.requesting(me) {
        // A request has no place yet; the leader chooses where it goes.
        return Routed::Room("request", json!({ "tracks": tracks_json(tracks) }));
    }
    if !room.may_add(me) {
        return Routed::Refused(LISTEN_ONLY);
    }
    if before.is_some() && !room.may_control(me) {
        return Routed::Refused(NOT_AHEAD);
    }
    Routed::Room(
        "enqueue",
        json!({ "tracks": tracks_json(tracks), "before": before }),
    )
}

/// Turns a press of the player's controls into the room's command for it.
/// The room then tells everyone, this player among them.
pub fn route(command: &Command, room: &Room, me: &str, position_ms: u64) -> Routed {
    let entry_at = |index: usize| room.queue.get(index).map(|entry| entry.id.clone());
    let steers = room.may_control(me);
    let guest_adds = !steers && room.mode == Mode::Contributions;
    let (kind, fields) = match command {
        // What the room itself asked of the player, and what is the
        // player's alone, go straight through.
        Command::FollowRoom { .. } | Command::LeaveRoom { .. } | Command::SetVolume(_) => {
            return Routed::Core;
        }
        Command::SetShuffle(_) => return Routed::Refused(NO_SHUFFLE),
        Command::SetRepeat(_) if !room.leads(me) => return Routed::Refused(LEADER_SETS),
        Command::SetRepeat(_) => ("settings", json!({ "repeat": room.repeat.next().wire() })),
        // "Next" with nothing after the current song is an ordinary add.
        Command::Enqueue { tracks, at } => {
            let before = at
                .and_then(entry_at)
                .filter(|_| steers || room.next_entry().is_some());
            return adding(room, me, tracks, before);
        }
        // A guest who may only add songs asks for the one they picked,
        // rather than being told they cannot start it.
        Command::Play {
            tracks,
            start_index,
            ..
        } if guest_adds => {
            return match tracks.get(*start_index) {
                Some(picked) => adding(room, me, std::slice::from_ref(picked), None),
                None => Routed::Refused(LEADER_PLAYS),
            };
        }
        Command::Remove(index) if !steers => {
            let own = room
                .queue
                .get(*index)
                .is_some_and(|entry| room.may_remove(me, entry));
            if !own {
                let why = if guest_adds { OWN_ONLY } else { LISTEN_ONLY };
                return Routed::Refused(why);
            }
            ("remove", json!({ "entry": entry_at(*index) }))
        }
        Command::Move { .. } if !steers => return Routed::Refused(LEADER_ORDERS),
        // A queue picked up from another device is put in place only when
        // nothing is going on here, which a room is.
        Command::Load { .. } => {
            return Routed::Refused("Leave the room to pick up a queue from another device.");
        }
        _ if !steers => return Routed::Refused(LEADER_PLAYS),
        Command::Toggle if room.playing => ("pause", json!({})),
        Command::Toggle => ("play", json!({})),
        Command::Next => ("next", json!({})),
        Command::Previous => (
            "previous",
            json!({ "restart": position_ms > RESTART_AFTER_MS }),
        ),
        Command::Seek(position_ms) => ("seek", json!({ "positionMs": position_ms })),
        Command::Jump(index) => ("jump", json!({ "entry": entry_at(*index) })),
        Command::Remove(index) => ("remove", json!({ "entry": entry_at(*index) })),
        Command::Move { from, to } => {
            // The room names a place by the entry that will follow it.
            let before = room
                .queue
                .iter()
                .enumerate()
                .filter(|(index, _)| index != from)
                .nth(*to)
                .map(|(_, entry)| entry.id.clone());
            (
                "move",
                json!({ "entry": entry_at(*from), "before": before }),
            )
        }
        Command::Play {
            tracks,
            start_index,
            ..
        } => {
            let from = (*start_index).min(tracks.len());
            let end = (from + MOST_TRACKS).min(tracks.len());
            (
                "replace",
                json!({ "tracks": tracks_json(&tracks[from..end]) }),
            )
        }
    };
    Routed::Room(kind, fields)
}

#[cfg(test)]
mod tests;
