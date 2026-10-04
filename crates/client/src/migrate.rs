//! Bringing another profile's data in: the parts only the running core can
//! do, because the database and the song cache are its while it runs.

use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::{ApiError, Client};

/// What a merge of another profile's database added. All of it counts what
/// was new: merging the same database again answers with zeroes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Merged {
    pub plays: u64,
    /// Plays that were here already, from an earlier merge.
    pub plays_known: u64,
    pub folders: u64,
    pub pins: u64,
    /// Library items put in a folder they were not in here.
    pub filed: u64,
    /// The queue left in the other profile was taken.
    pub resume: bool,
}

/// What copying songs from another profile's cache came to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ImportedSongs {
    pub copied: u64,
    pub bytes: u64,
    /// Already here, with as much of the song or more.
    pub present: u64,
    /// Would have taken the cache past its limit.
    pub no_room: u64,
    pub failed: u64,
}

impl ImportedSongs {
    pub fn add(&mut self, other: ImportedSongs) {
        self.copied += other.copied;
        self.bytes += other.bytes;
        self.present += other.present;
        self.no_room += other.no_room;
        self.failed += other.failed;
    }
}

impl Client {
    /// Merges the plays, folders and pins of the database at `path` into
    /// the one the core is running on. The other database is only read.
    pub fn migrate_history(&self, path: &Path) -> Result<Merged, ApiError> {
        self.post("/v1/migrate/history", &json!({ "path": path }))
    }

    /// Copies these songs from the cache folder at `dir` into the core's
    /// own, as far as they fit under its limit; `limit_mb` sets that limit
    /// first, for when the settings that will set it are still on their way.
    pub fn migrate_songs(
        &self,
        dir: &Path,
        ids: &[String],
        limit_mb: Option<u32>,
    ) -> Result<ImportedSongs, ApiError> {
        let body = json!({ "dir": dir, "ids": ids, "maxMB": limit_mb.unwrap_or(0) });
        self.post("/v1/migrate/cache", &body)
    }
}
