//! Moving in from the Electron app.
//!
//! Youtube Music Spotified was an Electron app before it was this one, and
//! its profile is still on the computer of anyone who used it: the accounts
//! it was signed in to, everything it recorded of what was listened to, the
//! songs it kept on disk and its preferences. This brings them across.
//!
//! That profile is only ever read. Nothing in it is written, locked, moved
//! or deleted, and the other app may be running throughout: its database is
//! copied aside before it is opened (by the core, see `control/merge.go`),
//! and its preferences are read as plain bytes ([`leveldb`]).
//!
//! [`discover`] says what is there, [`run`] brings what was chosen, and
//! [`record`] remembers what was brought, so that it can be done again
//! later for the plays made since.

pub mod discover;
mod leveldb;
pub mod prefs;
pub mod record;
pub mod run;
pub mod signin;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::accounts::{SavedAccount, SavedChannel};
pub use prefs::OldPrefs;
pub use record::Record;

/// The Electron app's directory under the roaming application data.
const OLD_DIR: &str = "Spotifier";

/// Where the Electron app's profile would be: the one named on the command
/// line, or the usual place. A demo shows no account, so it looks for none
/// unless told where.
pub fn old_profile(named: Option<&Path>, demo: bool) -> Option<PathBuf> {
    match named {
        Some(named) => Some(named.to_path_buf()),
        None if demo => None,
        None => Some(directories::BaseDirs::new()?.config_dir().join(OLD_DIR)),
    }
}

/// The kinds of thing that can be brought, each with a tick of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    SignIn,
    History,
    Songs,
    Preferences,
}

impl Kind {
    pub const EVERY: [Kind; 4] = [Kind::SignIn, Kind::History, Kind::Songs, Kind::Preferences];

    pub fn label(self) -> &'static str {
        match self {
            Kind::SignIn => "Accounts",
            Kind::History => "Listening history",
            Kind::Songs => "Downloaded songs",
            Kind::Preferences => "Preferences",
        }
    }
}

/// A yes or no for each kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Kinds {
    pub sign_in: bool,
    pub history: bool,
    pub songs: bool,
    pub preferences: bool,
}

impl Kinds {
    pub fn get(self, kind: Kind) -> bool {
        match kind {
            Kind::SignIn => self.sign_in,
            Kind::History => self.history,
            Kind::Songs => self.songs,
            Kind::Preferences => self.preferences,
        }
    }

    pub fn set(&mut self, kind: Kind, on: bool) {
        match kind {
            Kind::SignIn => self.sign_in = on,
            Kind::History => self.history = on,
            Kind::Songs => self.songs = on,
            Kind::Preferences => self.preferences = on,
        }
    }

    pub fn any(self) -> bool {
        Kind::EVERY.into_iter().any(|kind| self.get(kind))
    }

    /// Each kind that is in either.
    pub fn with(self, other: Kinds) -> Kinds {
        Kinds {
            sign_in: self.sign_in || other.sign_in,
            history: self.history || other.history,
            songs: self.songs || other.songs,
            preferences: self.preferences || other.preferences,
        }
    }
}

/// What a database of the Electron app holds, as the core counts it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct History {
    pub plays: u64,
    /// RFC 3339; empty when there are no plays.
    pub first_play: String,
    pub last_play: String,
    pub listened_ms: u64,
    pub pins: u64,
    pub folders: u64,
    /// A queue was left to come back to.
    pub resume: bool,
}

impl History {
    /// Takes in another database's figures: one account can have several,
    /// one for each of its channels.
    pub fn add(&mut self, other: &History) {
        self.plays += other.plays;
        self.listened_ms += other.listened_ms;
        self.pins += other.pins;
        self.folders += other.folders;
        self.resume |= other.resume;
        // The same shape and zone throughout, so the text sorts by time.
        if !other.first_play.is_empty()
            && (self.first_play.is_empty() || other.first_play < self.first_play)
        {
            self.first_play.clone_from(&other.first_play);
        }
        if other.last_play > self.last_play {
            self.last_play.clone_from(&other.last_play);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.plays == 0 && self.pins == 0 && self.folders == 0
    }
}

/// An account of the Electron app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OldAccount {
    pub id: String,
    pub name: String,
    pub avatar_url: String,
    /// The channel it acted as; empty for its own.
    pub channel: String,
    pub channels: Vec<SavedChannel>,
    /// The one the Electron app was using.
    pub in_use: bool,
    /// What was listened to with nobody signed in. It has no sign-in to
    /// bring, and its history belongs with this app's signed-out listening.
    pub guest: bool,
    /// Its credentials file, which holds a session when `signed_in`.
    pub credentials: PathBuf,
    pub signed_in: bool,
    /// The session as yt-dlp reads it, beside the credentials.
    pub resolver_cookies: PathBuf,
    /// Its databases: its own and one for each channel it has acted as.
    pub databases: Vec<PathBuf>,
    pub history: History,
    /// Why its history could not be counted, when it could not.
    pub unread: Option<String>,
    /// The account here that it is, when it is here already.
    pub here: Option<String>,
}

/// The songs the Electron app kept on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Songs {
    /// Each folder of them with the ids it holds, most recently played
    /// first.
    pub folders: Vec<(PathBuf, Vec<String>)>,
    pub count: usize,
    pub bytes: u64,
}

/// What the Electron app's profile holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Found {
    pub root: PathBuf,
    pub accounts: Vec<OldAccount>,
    pub songs: Songs,
    pub prefs: OldPrefs,
}

impl Found {
    /// The history of every account, as one.
    pub fn history(&self) -> History {
        let mut all = History::default();
        for account in &self.accounts {
            all.add(&account.history);
        }
        all
    }

    /// The accounts a person would call accounts: not the signed-out one.
    pub fn people(&self) -> impl Iterator<Item = &OldAccount> {
        self.accounts.iter().filter(|account| !account.guest)
    }

    /// The kinds there is something of.
    pub fn available(&self) -> Kinds {
        Kinds {
            sign_in: self.people().next().is_some(),
            history: !self.history().is_empty(),
            songs: self.songs.count > 0,
            preferences: !self.prefs.is_empty(),
        }
    }

    /// The kinds worth ticking to begin with: what there is, less what is
    /// here already. Preferences are brought once; after that they are
    /// this app's, and bringing them again would undo changes made here.
    pub fn suggested(&self, brought: Kinds) -> Kinds {
        let available = self.available();
        Kinds {
            sign_in: self
                .people()
                .any(|account| account.here.is_none() && account.signed_in),
            preferences: available.preferences && !brought.preferences,
            ..available
        }
    }
}

/// How far a run has got. A `total` of zero is a step that cannot be
/// counted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub step: String,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Brought,
    Skipped,
    Failed,
}

/// One line of what a run came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub mark: Mark,
    pub text: String,
}

impl Line {
    pub fn brought(text: impl Into<String>) -> Self {
        Self::new(Mark::Brought, text)
    }

    pub fn skipped(text: impl Into<String>) -> Self {
        Self::new(Mark::Skipped, text)
    }

    pub fn failed(text: impl Into<String>) -> Self {
        Self::new(Mark::Failed, text)
    }

    fn new(mark: Mark, text: impl Into<String>) -> Self {
        Self {
            mark,
            text: text.into(),
        }
    }
}

/// What a run came to, and what is left for the window's thread to do
/// about it: the list of accounts and the settings are its to change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub lines: Vec<Line>,
    /// Accounts whose files are in place and which are not in the list yet.
    pub new_accounts: Vec<SavedAccount>,
    /// Each account of the Electron app with the account here that it is.
    pub pairs: Vec<(String, String)>,
    /// The account to use, when the Electron app's was brought and nobody
    /// is signed in here.
    pub activate: Option<String>,
    /// The preferences to take in.
    pub prefs: Option<OldPrefs>,
    /// The kinds that were gone through, whatever came of them.
    pub done: Kinds,
    /// Channels whose history was put in a folder the account here did
    /// not have yet, with the account's id: the list is to remember the
    /// folder, or the channel would start another when it is next used.
    pub channels: Vec<(String, SavedChannel)>,
    /// Plays, pins or folders were added to the account in use.
    pub history_changed: bool,
    pub songs_changed: bool,
}

/// What the threads that look and bring tell the window's.
#[derive(Debug)]
pub enum Report {
    /// What is in the Electron app's profile; `None` when there is none.
    Found(Option<Box<Found>>),
    Progress(Progress),
    Done(Box<Outcome>),
}

/// A date as a person writes it, from the day an RFC 3339 time begins with:
/// `2026-09-22T10:26:25Z` is `22 Sep 2026`. Empty for anything else.
pub fn day(time: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = time.get(..10).unwrap_or_default().split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else {
        return String::new();
    };
    let month = month
        .parse::<usize>()
        .ok()
        .and_then(|month| MONTHS.get(month.checked_sub(1)?));
    match (month, day.parse::<u32>()) {
        (Some(month), Ok(day)) => format!("{day} {month} {year}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests;
