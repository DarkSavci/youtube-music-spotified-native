//! Listen Together: a queue shared with friends through a relay.
//!
//! The relay owns the room. Everyone plays the music through their own
//! account and their own player; what is shared is which song, and where in
//! it. This app sends its controls to the room as commands, hears the room
//! back, and has the core follow it.

pub mod ask;
pub mod client;
pub mod protocol;
pub mod rules;
pub mod servers;
pub mod sync;

use std::time::Instant;

use spotified_client::models::Track;

pub use ask::{Ask, Setting, Transport};
pub use client::{Connection, Enter, Event, Options};
pub use protocol::{Mode, Room};
pub use servers::{SavedServer, ServerForm};

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
}

/// What the room's page shows under what is playing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Queue,
    History,
    Activity,
}

impl Tab {
    pub const EVERY: [Tab; 3] = [Tab::Queue, Tab::History, Tab::Activity];

    /// What the tab is called, the queue's with how much of it is left.
    pub fn label(self, room: &Room) -> String {
        match self {
            Tab::Queue => format!("Queue · {}", room.upcoming().count()),
            Tab::History => "History".to_owned(),
            Tab::Activity => "Activity".to_owned(),
        }
    }
}

/// Looking for a song from inside the room.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Search {
    pub query: String,
    /// Counts searches, so an answer to an older one can be told apart.
    pub serial: u64,
    pub results: Vec<Track>,
    pub searching: bool,
}

impl Search {
    /// Whether the field holds something to look for.
    pub fn active(&self) -> bool {
        !self.query.trim().is_empty()
    }
}

/// The radio that keeps a room's music going.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Radio {
    /// The song radio was last fetched for.
    pub seed: String,
    pub busy: bool,
    /// The entry the queue ended with when the leader's top-up was asked
    /// for; none for radio someone started by hand.
    pub topping_up: Option<String>,
    /// When a top-up that failed may be tried again.
    pub retry_at: Option<Instant>,
    /// The room's revision when a song was started by hand: its radio is
    /// for the room after that.
    pub since: u64,
    /// The radio that has arrived and waits for the room to be heard.
    pub found: Option<Vec<Track>>,
}

/// A field of the page's form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
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
    /// The entry and song whose measured length the room has been told.
    pub length_told: Option<String>,
    /// The form that adds or edits a saved server, while it is open.
    pub manage: Option<ServerForm>,
    pub tab: Tab,
    /// The leader's settings are open.
    pub settings_open: bool,
    /// The leave panel is open, with the next leader chosen in it; empty
    /// for one picked at random.
    pub leaving: Option<String>,
    pub search: Search,
    /// What is typed in "Songs per guest" and not yet sent.
    pub limit_text: Option<String>,
    /// How this player stands with the room, as the page says it.
    pub standing: sync::Standing,
    /// Requests this listener withdrew, whose going is not the leader's no.
    pub withdrawn: Vec<String>,
    /// Songs the room has had while this listener was in it.
    pub heard: Vec<String>,
    pub radio: Radio,
    /// A playlist being made of the room's history, by its name.
    pub saving_history: Option<String>,
    /// How far this computer's clock is from Greenwich, in minutes, for the
    /// times the page shows.
    pub zone_minutes: i32,
}

impl Together {
    /// In a room: seated, even if the line has dropped for a moment.
    pub fn in_room(&self) -> bool {
        matches!(self.phase, Phase::Joined | Phase::Reconnecting) && self.room.is_some()
    }

    /// The relay's clock now.
    pub fn server_now(&self) -> f64 {
        client::now_ms() + self.offset_ms
    }

    /// Back to how things were before any room, keeping what was typed.
    pub fn reset(&mut self, error: Option<String>) {
        *self = Self {
            form: std::mem::take(&mut self.form),
            manage: self.manage.take(),
            zone_minutes: self.zone_minutes,
            error,
            ..Self::default()
        };
    }
}
