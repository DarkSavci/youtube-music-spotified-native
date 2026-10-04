//! What is said over the socket, in the relay's version 2.
//!
//! The relay owns the room: its queue, what is playing, and where in the
//! song everyone should be. Members send commands and hear the whole room
//! back after every change. Unknown fields are ignored, so a newer relay
//! does not break an older app.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use spotified_client::models::{ArtistRef, Artwork, Track};

/// The protocol this app speaks.
pub const VERSION: u64 = 2;
/// The most songs a single command may carry.
pub const MOST_TRACKS: usize = 100;

/// Who may steer a room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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
            Mode::Collaborative => "Everyone’s the DJ",
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
    /// Songs that have played, oldest first.
    pub history: Vec<Entry>,
    /// What has happened in the room lately, oldest first.
    pub activity: Vec<Happening>,
    pub repeat: Repeat,
    pub policy: Policy,
    /// Whether a song may wait in the queue twice.
    pub duplicates: bool,
    /// How many songs a guest may have waiting.
    pub limit: u32,
    /// No one new may join.
    pub locked: bool,
    /// Listeners may vote the song away.
    pub vote_skip: bool,
    /// The members who have voted to skip this song.
    pub votes: Vec<String>,
    /// The leader lets each new listener in.
    pub join_approval: bool,
    /// Those waiting to be let in.
    pub pending: Vec<Knock>,
    /// A ready check, or the count to a shared start after one.
    pub countdown: Option<Countdown>,
    /// Songs guests asked for, waiting for the leader or a DJ.
    pub requests: Vec<SongRequest>,
    /// Requests go straight into the queue.
    pub auto_accept: bool,
    /// The last edit of the queue, while it can still be taken back.
    pub undo: Option<Undo>,
    pub last_controlled_by: Option<Person>,
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

    pub fn member(&self, id: &str) -> Option<&Member> {
        self.members.iter().find(|member| member.id == id)
    }

    /// What the room is called: its name, or whose it is.
    pub fn title(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let leader = self.leader().map_or("Your friends", |leader| &leader.name);
        format!("{leader}’s room")
    }

    /// The room's queue as tracks, for the player to mirror.
    pub fn tracks(&self) -> Vec<Track> {
        self.queue
            .iter()
            .map(|entry| entry.track.to_track())
            .collect()
    }
}

/// How a room's queue repeats.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

impl Repeat {
    pub fn wire(self) -> &'static str {
        match self {
            Repeat::Off => "off",
            Repeat::One => "one",
            Repeat::All => "all",
        }
    }

    /// The mode after this one, as the player's button steps: off, all, one.
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// The order additions take in the queue.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    /// As they came.
    #[default]
    Fifo,
    /// One from each person in turn.
    Turns,
}

impl Policy {
    pub const EVERY: [Policy; 2] = [Policy::Fifo, Policy::Turns];

    pub fn wire(self) -> &'static str {
        match self {
            Policy::Fifo => "fifo",
            Policy::Turns => "turns",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Policy::Fifo => "First in, first out",
            Policy::Turns => "Take turns",
        }
    }
}

/// A picture's address as the relay passes it on, as the artwork it is
/// drawn from; nothing for someone who shares none.
fn picture<'de, D: Deserializer<'de>>(from: D) -> Result<Vec<Artwork>, D::Error> {
    let url = Option::<String>::deserialize(from)?.unwrap_or_default();
    Ok(picture_of(&url))
}

pub fn picture_of(url: &str) -> Vec<Artwork> {
    if url.is_empty() {
        return Vec::new();
    }
    vec![Artwork {
        url: url.to_owned(),
        width: 96,
        height: 96,
    }]
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Member {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "picture")]
    pub avatar: Vec<Artwork>,
    /// Has answered the ready check that is on.
    pub ready: bool,
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
    pub added_by: Person,
    /// When it was added, on the relay's clock.
    pub added_at: f64,
    /// Added by the room's radio, not chosen by anyone.
    pub radio: bool,
    /// The request this came in as, if it was asked for.
    pub request: Option<String>,
}

/// Someone in the room, as the relay names them beside what they did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Person {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "picture")]
    pub avatar: Vec<Artwork>,
}

/// A line of the room's activity.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Happening {
    pub id: String,
    pub text: String,
    /// When, on the relay's clock.
    pub at: f64,
}

/// Someone waiting for the leader to let them in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Knock {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "picture")]
    pub avatar: Vec<Artwork>,
}

/// A ready check. Once everyone has answered, or the leader says so, it
/// has a moment at which the room starts.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Countdown {
    pub expires: f64,
    pub start_at: Option<f64>,
}

/// A song a guest asked for.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SongRequest {
    pub id: String,
    pub track: RoomTrack,
    pub by: Person,
}

/// An edit of the queue that can be taken back: by whom, until when, and
/// only while the room is still at the revision the edit left it at.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Undo {
    pub revision: u64,
    pub expires: f64,
    pub by: String,
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
    /// Whose song it is, as a row writes it.
    pub fn artist_names(&self) -> String {
        let names: Vec<&str> = self
            .artists
            .iter()
            .map(|artist| artist.name.as_str())
            .collect();
        names.join(", ")
    }

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
mod tests;
