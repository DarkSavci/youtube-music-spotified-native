//! Keeping this player with the room, and the room's controls with this
//! player's buttons. Both are plain functions of what the room says and
//! what the player is doing.

use std::time::Duration;

use serde_json::{Value, json};
use spotified_client::session::Command;

use super::protocol::{MOST_TRACKS, Mode, Room, tracks_json};

/// How far this player may be from the room before it is brought back.
const DRIFT_MS: u64 = 1000;
/// How long after one correction the next may come, so a slow player is
/// not seeked over and over.
const BETWEEN_CORRECTIONS: Duration = Duration::from_secs(4);
/// Past this far into a song, "previous" starts it again instead.
const RESTART_AFTER_MS: u64 = 3000;

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

const NOT_YOURS: &str = "Only the leader and DJs can steer this room.";

/// Turns a press of the player's controls into the room's command for it.
/// The room then tells everyone, this player among them.
pub fn route(command: &Command, room: &Room, me: &str, position_ms: u64) -> Routed {
    let entry_at = |index: usize| room.queue.get(index).map(|entry| entry.id.clone());
    let steers = room.may_control(me);
    let (kind, fields) = match command {
        // What the room itself asked of the player, and what is the
        // player's alone, go straight through.
        Command::FollowRoom { .. } | Command::LeaveRoom { .. } | Command::SetVolume(_) => {
            return Routed::Core;
        }
        Command::SetRepeat(_) | Command::SetShuffle(_) => {
            return Routed::Refused("Repeat and shuffle are the room's, not each listener's.");
        }
        // Adding is open to everyone where the room takes requests.
        Command::Enqueue { tracks, at } if steers || room.mode == Mode::Contributions => {
            let before = at.filter(|_| steers).and_then(entry_at);
            (
                "enqueue",
                json!({ "tracks": tracks_json(tracks), "before": before }),
            )
        }
        _ if !steers => return Routed::Refused(NOT_YOURS),
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
        Command::Enqueue { .. } => return Routed::Refused(NOT_YOURS),
    };
    Routed::Room(kind, fields)
}

#[cfg(test)]
mod tests {
    use super::super::protocol::{Entry, Member, RoomTrack};
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
            Routed::Refused(NOT_YOURS)
        );
        let add = Command::Enqueue {
            tracks: Vec::new(),
            at: None,
        };
        assert_eq!(route(&add, &room, "guest", 0), Routed::Refused(NOT_YOURS));
        room.mode = Mode::Contributions;
        assert!(matches!(
            route(&add, &room, "guest", 0),
            Routed::Room("enqueue", _)
        ));
    }
}
