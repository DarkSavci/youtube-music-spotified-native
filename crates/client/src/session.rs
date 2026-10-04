//! The playback session: what is playing, as the core tells it, and the
//! commands and engine reports sent back.
//!
//! One oddity of the wire is kept as it is: the core writes its `Target`,
//! and reads `Command` and `EngineEvent`, with Go's field names (`Kind`,
//! `VideoID`), while everything else is camelCase.

use std::io::{BufRead, BufReader};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::models::{MixSeed, Track};
use crate::{ApiError, Client};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayState {
    #[default]
    Idle,
    Loading,
    Playing,
    Paused,
    Stalled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Queue {
    #[serde(deserialize_with = "crate::models::null_as_default")]
    pub items: Vec<Track>,
    pub index: usize,
    /// Where the queue came from, to show: an album's or playlist's name.
    pub origin: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SessionState {
    /// Rises with every change; an older snapshot is out of date.
    pub version: u64,
    pub queue: Queue,
    pub state: PlayState,
    pub repeat: Repeat,
    pub shuffle: bool,
    /// 0 to 1, or up to 2 with boost.
    pub volume: f32,
    pub position_ms: u64,
}

impl SessionState {
    pub fn current(&self) -> Option<&Track> {
        self.queue.items.get(self.queue.index)
    }
}

/// What the engine should be doing.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Target {
    pub epoch: u64,
    #[serde(rename = "VideoID")]
    pub video_id: String,
    pub start_at_ms: u64,
    pub playing: bool,
    #[serde(rename = "PreloadVideoID")]
    pub preload_video_id: String,
    pub volume: f32,
    pub transition: Transition,
}

/// How the core wants one track to give way to the next at its end.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Transition {
    /// `cut`, `gapless` or `crossfade`.
    pub kind: String,
    /// The crossfade's length.
    pub ms: u64,
}

impl Transition {
    /// The fade's length, or 0 when the transition is not a fade.
    pub fn crossfade_ms(&self) -> u64 {
        if self.kind == "crossfade" { self.ms } else { 0 }
    }
}

/// The core's whole view of playback, sent on every change.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Projection {
    pub state: SessionState,
    pub target: Target,
    /// The core is mirroring a Listen Together room, not its own queue.
    #[serde(rename = "followingRoom")]
    pub following_room: bool,
    /// What this player knows of the room's current entry; absent outside
    /// a room.
    pub room: Option<RoomPlayback>,
    /// The core has lost its connection to YouTube.
    pub offline: bool,
}

/// What only this player can know about the room's current entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RoomPlayback {
    /// The room's id for the entry.
    pub entry: String,
    /// It has played to its end here.
    pub ended: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Replace the queue and start at `start_index`.
    Play {
        tracks: Vec<Track>,
        start_index: usize,
        origin: String,
    },
    Toggle,
    Next,
    Previous,
    Seek(u64),
    SetRepeat(Repeat),
    SetShuffle(bool),
    SetVolume(f32),
    /// Jump to this place in the queue.
    Jump(usize),
    /// Take the entry at this place out of the queue.
    Remove(usize),
    /// Move an entry; `to` is where it ends up once it has been taken out.
    Move {
        from: usize,
        to: usize,
    },
    /// Mirror a Listen Together room: its queue, the entry that is playing
    /// (by the room's id for it), where in it, and whether it plays. While
    /// following, the core takes no other command but volume.
    FollowRoom {
        tracks: Vec<Track>,
        index: usize,
        entry: String,
        position_ms: u64,
        playing: bool,
    },
    /// Stop following, carrying on with the room's queue or going back to
    /// the queue from before it.
    LeaveRoom {
        keep_queue: bool,
    },
    /// Add tracks to the queue: at this place, or at the end.
    Enqueue {
        tracks: Vec<Track>,
        at: Option<usize>,
    },
}

impl Command {
    fn to_wire(&self) -> Value {
        match self {
            Command::Play {
                tracks,
                start_index,
                origin,
            } => json!({
                "Kind": "play",
                "Tracks": tracks,
                "StartIndex": start_index,
                "Origin": origin,
            }),
            Command::Toggle => json!({ "Kind": "toggle" }),
            Command::Next => json!({ "Kind": "next" }),
            Command::Previous => json!({ "Kind": "prev" }),
            Command::Seek(position_ms) => json!({ "Kind": "seek", "PositionMs": position_ms }),
            Command::SetRepeat(repeat) => json!({ "Kind": "set_repeat", "Repeat": repeat }),
            Command::SetShuffle(on) => json!({ "Kind": "set_shuffle", "Shuffle": on }),
            Command::SetVolume(volume) => json!({ "Kind": "set_volume", "Volume": volume }),
            Command::Jump(index) => json!({ "Kind": "jump", "At": index }),
            Command::Remove(index) => json!({ "Kind": "remove", "At": index }),
            Command::FollowRoom {
                tracks,
                index,
                entry,
                position_ms,
                playing,
            } => json!({
                "Kind": "follow_room",
                "Tracks": tracks,
                "StartIndex": index,
                "ExpectedID": entry,
                "PositionMs": position_ms,
                "Playing": playing,
            }),
            Command::LeaveRoom { keep_queue } => {
                json!({ "Kind": "leave_room", "KeepQueue": keep_queue })
            }
            Command::Move { from, to } => json!({ "Kind": "move", "From": from, "To": to }),
            // The core reads a place outside the queue as "at the end".
            Command::Enqueue { tracks, at } => json!({
                "Kind": "enqueue",
                "Insert": tracks,
                "At": at.map_or(-1, |at| at as i64),
            }),
        }
    }
}

/// What the engine reports. `reason` is one of the strings the core knows:
/// empty, `network`, or `stalled`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct EngineEvent {
    pub kind: EngineEventKind,
    pub epoch: u64,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineEventKind {
    Loaded,
    Position,
    Ended,
    Failed,
    Stalled,
    Blocked,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Answer {
    projection: Projection,
    /// Why the core refused the command; empty when it did not.
    rejected: String,
}

/// What this client tells the core it can do, which decides the transitions
/// the core asks for.
fn capabilities() -> Value {
    json!({
        "EQ": false,
        "Crossfade": "true",
        "Normalization": false,
        "PreciseSeek": true,
        "VolumeSteps": 0,
    })
}

impl Client {
    /// Announces this device. Needed before commands, and again after the
    /// event stream drops, since that unregisters it.
    pub fn register(&self, device_id: &str) -> Result<Projection, ApiError> {
        let body = json!({
            "deviceId": device_id,
            "name": "Youtube Music Spotified",
            "capabilities": capabilities(),
        });
        let answer: Answer = self.post("/v1/session/register", &body)?;
        Ok(answer.projection)
    }

    /// Sends a command and returns the session as it stands afterwards. A
    /// command the core refuses is an `Err` carrying its reason.
    pub fn command(&self, device_id: &str, command: &Command) -> Result<Projection, ApiError> {
        let body = json!({ "deviceId": device_id, "command": command.to_wire() });
        let answer: Answer = self.post("/v1/session/command", &body)?;
        if answer.rejected.is_empty() {
            Ok(answer.projection)
        } else {
            Err(ApiError::Status {
                code: 200,
                message: format!("The player refused that ({}).", answer.rejected),
            })
        }
    }

    /// Plays a song and has the core carry on from it with songs like it.
    pub fn start_radio(&self, device_id: &str, track: &Track) -> Result<(), ApiError> {
        self.radio(&json!({ "deviceId": device_id, "track": track }))
    }

    /// Plays one of YouTube's own queues: an artist's mix, or a shuffle of
    /// their songs. `origin` is what the queue is called.
    pub fn start_mix(&self, device_id: &str, seed: &MixSeed, origin: &str) -> Result<(), ApiError> {
        self.radio(&json!({
            "deviceId": device_id,
            "playlistId": seed.playlist_id,
            "videoId": seed.video_id,
            "params": seed.params,
            "origin": origin,
        }))
    }

    fn radio(&self, body: &Value) -> Result<(), ApiError> {
        let answer: Answer = self.post("/v1/session/radio", body)?;
        if answer.rejected.is_empty() {
            Ok(())
        } else {
            Err(ApiError::Status {
                code: 200,
                message: format!("The player refused that ({}).", answer.rejected),
            })
        }
    }

    /// Tells the core how tracks should follow one another. It asks the
    /// engine for a crossfade only when the length here is more than zero.
    pub fn set_transitions(&self, crossfade_ms: u64) -> Result<(), ApiError> {
        // Both fields are sent every time: the core reads a missing one as
        // zero or false, not as "unchanged".
        let body = json!({ "crossfadeMs": crossfade_ms, "gapless": true });
        self.post_empty("/v1/session/settings", &body)
    }

    pub fn engine_event(&self, device_id: &str, event: &EngineEvent) -> Result<(), ApiError> {
        let body = json!({ "deviceId": device_id, "event": event });
        self.post_empty("/v1/session/engine-event", &body)
    }

    /// Follows the session, calling `on_projection` for each snapshot.
    /// Returns when the stream ends, which it does when the core stops.
    pub fn events(
        &self,
        device_id: &str,
        on_projection: impl FnMut(Projection),
    ) -> Result<(), ApiError> {
        let path = format!("/v1/session/events?deviceId={}", crate::encode(device_id));
        let body = self.stream(&path)?;
        read_events(BufReader::new(body), on_projection);
        Ok(())
    }
}

/// Reads a server-sent event stream: `data:` lines, then a blank line.
/// Anything that is not a projection (comments kept as keep-alives, events
/// that do not parse) is passed over.
fn read_events(stream: impl BufRead, mut on_projection: impl FnMut(Projection)) {
    let mut data = String::new();
    for line in stream.lines().map_while(Result::ok) {
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
        } else if line.is_empty() && !data.is_empty() {
            if let Ok(projection) = serde_json::from_str(&data) {
                on_projection(projection);
            }
            data.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As the core sent it for a two-track queue, trimmed.
    const PROJECTION: &str = r#"{"followingRoom":false,"state":{"version":2,"epoch":1,
        "queue":{"items":[{"id":"abc","title":"T","artists":[{"name":"A"}],"durationMs":1000,
        "artwork":[],"explicit":false,"isVideo":false,"playable":true}],"index":0,"origin":"Test"},
        "state":"playing","repeat":"off","shuffle":false,"volume":0.5,"positionMs":0,
        "positionAt":"2026-10-03T21:34:52.4041335+03:00","ownerDeviceId":"probe"},
        "target":{"Epoch":1,"VideoID":"abc","StartAtMs":0,"Playing":true,"PreloadVideoID":"def",
        "Volume":0.5,"Transition":{"Kind":"gapless","Ms":0},"UserChange":true},
        "devices":[],"offline":false}"#;

    #[test]
    fn a_projection_reads_both_of_the_cores_spellings() {
        let projection: Projection = serde_json::from_str(PROJECTION).expect("a projection");
        assert_eq!(projection.state.version, 2);
        assert_eq!(projection.state.state, PlayState::Playing);
        assert_eq!(
            projection.state.current().map(|track| track.id.as_str()),
            Some("abc")
        );
        assert_eq!(projection.target.video_id, "abc");
        assert_eq!(projection.target.preload_video_id, "def");
        assert_eq!(projection.target.transition.kind, "gapless");
        assert_eq!(projection.target.transition.crossfade_ms(), 0);
        assert!(projection.target.playing);
    }

    #[test]
    fn commands_are_written_with_go_field_names() {
        assert_eq!(
            Command::Seek(61_000).to_wire(),
            json!({ "Kind": "seek", "PositionMs": 61_000 })
        );
        assert_eq!(
            Command::SetRepeat(Repeat::One).to_wire(),
            json!({ "Kind": "set_repeat", "Repeat": "one" })
        );
        let play = Command::Play {
            tracks: vec![Track {
                id: "abc".into(),
                duration_ms: 1000,
                ..Track::default()
            }],
            start_index: 0,
            origin: "Test".into(),
        }
        .to_wire();
        assert_eq!(play["Tracks"][0]["id"], "abc");
        let append = Command::Enqueue {
            tracks: vec![Track::default()],
            at: None,
        };
        assert_eq!(append.to_wire()["At"], -1);
        assert_eq!(play["Tracks"][0]["durationMs"], 1000);
    }

    #[test]
    fn an_engine_event_is_written_with_go_field_names() {
        let event = EngineEvent {
            kind: EngineEventKind::Position,
            epoch: 3,
            position_ms: 1500,
            duration_ms: 0,
            reason: String::new(),
        };
        assert_eq!(
            serde_json::to_value(&event).expect("json"),
            json!({"Kind":"position","Epoch":3,"PositionMs":1500,"DurationMs":0,"Reason":""})
        );
    }

    #[test]
    fn the_event_stream_yields_each_projection_and_skips_keep_alives() {
        let one_line = PROJECTION.replace('\n', "");
        let stream = format!(
            ": keep-alive\n\nevent: projection\ndata: {one_line}\n\n: keep-alive\n\n\
             event: projection\ndata: {{not json\n\nevent: projection\ndata: {one_line}\n\n"
        );
        let mut seen = 0;
        read_events(stream.as_bytes(), |projection| {
            assert_eq!(projection.target.video_id, "abc");
            seen += 1;
        });
        assert_eq!(seen, 2);
    }
}
