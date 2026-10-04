//! Listen Together: a queue shared with friends through a relay.
//!
//! The relay owns the room. Everyone plays the music through their own
//! account and their own player; what is shared is which song, and where in
//! it. This app sends its controls to the room as commands, hears the room
//! back, and has the core follow it.

pub mod client;
pub mod protocol;
pub mod sync;

use std::time::Instant;

use spotified_client::models::Track;

pub use client::{Connection, Enter, Event, Options};
pub use protocol::{Mode, Room};

/// How things stand with the room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    /// Not in a room, nor on the way into one.
    #[default]
    Idle,
    Connecting,
    /// The leader has yet to let this listener in.
    Waiting,
    Joined,
    Reconnecting,
}

/// What is typed on the page before a room is entered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Form {
    /// The PIN of a room to join.
    pub pin: String,
    /// The name of a room to make.
    pub room_name: String,
    pub mode: Mode,
}

/// A field of the page's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Server,
    Name,
    Pin,
    RoomName,
}

/// Bringing the music that was playing into a room just made: the songs go
/// in, then the room is taken to where the song was, then it plays. Each
/// step waits for the room to have heard the one before.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Seed {
    #[default]
    None,
    /// To be offered once the room exists.
    Wanted {
        tracks: Vec<Track>,
        position_ms: u64,
        playing: bool,
    },
    /// The songs are on their way in.
    Enqueued { position_ms: u64, playing: bool },
    /// The room has been taken to the place in the song.
    Sought { playing: bool },
}

#[derive(Default)]
pub struct Together {
    pub form: Form,
    pub phase: Phase,
    /// The room as last heard, while in one.
    pub room: Option<Room>,
    /// This listener's member id in the room.
    pub me: String,
    /// How far the relay's clock is ahead of this computer's.
    pub offset_ms: f64,
    /// Why the last attempt failed, or why the room ended.
    pub error: Option<String>,
    /// The entry the core was last told to follow.
    pub applied_entry: Option<String>,
    /// When the player was last brought back to the room.
    pub corrected_at: Option<Instant>,
    pub seed: Seed,
    /// When the room was last told this player reached the song's end.
    pub ended_sent: Option<Instant>,
}

impl Together {
    /// In a room: seated, even if the line has dropped for a moment.
    pub fn in_room(&self) -> bool {
        matches!(self.phase, Phase::Joined | Phase::Reconnecting) && self.room.is_some()
    }

    /// Whether this listener may steer the room they are in.
    pub fn may_control(&self) -> bool {
        self.room
            .as_ref()
            .is_some_and(|room| room.may_control(&self.me))
    }

    /// The relay's clock now.
    pub fn server_now(&self) -> f64 {
        client::now_ms() + self.offset_ms
    }

    /// Back to how things were before any room, keeping what was typed.
    pub fn reset(&mut self, error: Option<String>) {
        *self = Self {
            form: std::mem::take(&mut self.form),
            error,
            ..Self::default()
        };
    }
}
