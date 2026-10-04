//! What is said over the socket, in the relay's version 2.
//!
//! The relay owns the room: its queue, what is playing, and where in the
//! song everyone should be. Members send commands and hear the whole room
//! back after every change. Unknown fields are ignored, so a newer relay
//! does not break an older app.

use serde::Deserialize;
use serde_json::{Value, json};
use spotified_client::models::{ArtistRef, Artwork, Track};

/// The protocol this app speaks.
pub const VERSION: u64 = 2;
/// The most songs a single command may carry.
pub const MOST_TRACKS: usize = 100;

/// Who may steer a room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Everyone can play, pause, skip and shape the queue.
    #[default]
    Collaborative,
    /// Friends request songs; the leader decides what plays.
    Contributions,
    /// The leader handles the music; friends listen.
    Listen,
}

impl Mode {
    pub const EVERY: [Mode; 3] = [Mode::Collaborative, Mode::Contributions, Mode::Listen];

    pub fn wire(self) -> &'static str {
        match self {
            Mode::Collaborative => "collaborative",
            Mode::Contributions => "contributions",
            Mode::Listen => "listen",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Mode::Collaborative => "Everyone's the DJ",
            Mode::Contributions => "Take requests",
            Mode::Listen => "Just listen",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Mode::Collaborative => "Everyone can play, pause, skip and shape the queue.",
            Mode::Contributions => "Friends request songs. You approve what plays and when.",
            Mode::Listen => "You handle the music. Friends settle in and listen.",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Room {
    pub name: String,
    pub id: String,
    pub pin: String,
    pub members: Vec<Member>,
    /// The leader's member id.
    pub owner: String,
    pub mode: Mode,
    pub queue: Vec<Entry>,
    /// The id of the entry that is playing.
    pub current: Option<String>,
    /// Where the song was at `at`, on the relay's clock, in milliseconds.
    pub position_ms: f64,
    pub at: f64,
    pub playing: bool,
    /// Counts changes; a command names the one it was made against.
    pub revision: u64,
    /// When the room closes, on the relay's clock.
    pub expires: f64,
    /// The queue has run out.
    pub finished: bool,
}

impl Room {
    /// The entry that is playing and its place in the queue.
    pub fn current(&self) -> Option<(usize, &Entry)> {
        let id = self.current.as_deref()?;
        self.queue
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.id == id)
    }

    /// Where the song is at `server_now`, as the relay reckons it.
    pub fn position_at(&self, server_now: f64) -> u64 {
        let elapsed = if self.playing {
            (server_now - self.at).max(0.0)
        } else {
            0.0
        };
        let position = self.position_ms + elapsed;
        let length = self
            .current()
            .map(|(_, entry)| entry.track.duration_ms)
            .filter(|length| *length > 0.0)
            .unwrap_or(86_400_000.0);
        position.clamp(0.0, length) as u64
    }

    pub fn leader(&self) -> Option<&Member> {
        self.members.iter().find(|member| member.id == self.owner)
    }

    /// Whether `member` may steer: the leader, a DJ, or anyone in a room
    /// where everyone is.
    pub fn may_control(&self, member: &str) -> bool {
        self.mode == Mode::Collaborative
            || self.owner == member
            || self
                .members
                .iter()
                .any(|other| other.id == member && other.role == "dj")
    }

    /// The room's queue as tracks, for the player to mirror.
    pub fn tracks(&self) -> Vec<Track> {
        self.queue
            .iter()
            .map(|entry| entry.track.to_track())
            .collect()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// What their player is doing: `listening`, `paused`, `buffering`…
    pub status: String,
    /// `dj` or `listener`; the leader is named by the room, not by a role.
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub track: RoomTrack,
    pub added_by: AddedBy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AddedBy {
    pub name: String,
}

/// A song as the relay passes it on: only what it lets through.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RoomTrack {
    pub id: String,
    pub title: String,
    pub artists: Vec<RoomArtist>,
    pub duration_ms: f64,
    pub artwork: Vec<Artwork>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RoomArtist {
    pub name: String,
}

impl RoomTrack {
    pub fn to_track(&self) -> Track {
        Track {
            id: self.id.clone(),
            title: self.title.clone(),
            artists: self
                .artists
                .iter()
                .map(|artist| ArtistRef {
                    id: String::new(),
                    name: artist.name.clone(),
                })
                .collect(),
            duration_ms: self.duration_ms.max(0.0) as u64,
            artwork: self.artwork.clone(),
            playable: true,
            ..Track::default()
        }
    }
}

/// A song as the relay takes it. It keeps only these fields and refuses a
/// track whose id is not a video id.
pub fn track_json(track: &Track) -> Value {
    json!({
        "id": track.id,
        "title": track.title.chars().take(300).collect::<String>(),
        "durationMs": track.duration_ms,
        "isVideo": track.is_video,
        "explicit": track.explicit,
        "artists": track.artists.iter().take(10).map(|artist| json!({ "name": artist.name })).collect::<Vec<_>>(),
        "artwork": track.artwork.iter().take(3).map(|art| json!({ "url": art.url, "width": art.width, "height": art.height })).collect::<Vec<_>>(),
    })
}

pub fn tracks_json(tracks: &[Track]) -> Value {
    Value::Array(tracks.iter().take(MOST_TRACKS).map(track_json).collect())
}

/// What the relay says.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Incoming {
    Hello {
        #[serde(default)]
        version: u64,
    },
    Ready,
    /// The answer to a ping: when it was sent, and the relay's clock then.
    Pong {
        #[serde(default)]
        sent: f64,
        #[serde(default)]
        at: f64,
    },
    /// A seat in the room, and what brings it back after a dropped line.
    #[serde(rename_all = "camelCase")]
    Joined {
        member: String,
        token: String,
        room_id: String,
    },
    /// The leader has yet to let this listener in.
    Waiting,
    State {
        room: Box<Room>,
    },
    Ack,
    Error {
        #[serde(default)]
        message: String,
        /// The command this refuses, when it refuses one.
        #[serde(default)]
        op: Option<String>,
        #[serde(default)]
        fatal: bool,
    },
    /// The room is over for this listener.
    Ended {
        #[serde(default)]
        reason: String,
    },
    #[serde(other)]
    Unknown,
}

/// Checks a relay's address as the relay's own client does: `wss://`, or
/// `ws://` to this computer for testing, and nothing but host and path.
pub fn checked_address(input: &str) -> Result<String, &'static str> {
    let address = input.trim();
    let (scheme, rest) = address
        .split_once("://")
        .ok_or("Enter the server's address, starting with wss://")?;
    let host = rest.split(['/', ':']).next().unwrap_or_default();
    if host.is_empty() {
        return Err("Enter the server's address, starting with wss://");
    }
    let local = ["localhost", "127.0.0.1", "[::1]"].contains(&host);
    match scheme {
        "wss" => {}
        "ws" if local => {}
        _ => return Err("Use wss:// for a remote server, or ws://localhost for local testing."),
    }
    if rest.contains(['@', '?', '#']) {
        return Err("The address must not contain a password, a query or a fragment.");
    }
    Ok(address.to_owned())
}

#[cfg(test)]
mod tests {
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
}
