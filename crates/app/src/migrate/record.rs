//! What has been brought from the Electron app so far, kept in the profile.
//!
//! It is what lets a second run find the account it made on the first,
//! rather than make another, and what tells a profile that was never asked
//! from one that said no.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::Kinds;

const FILE: &str = "migration.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    /// The offer was shown and closed: it is not made again by itself.
    pub seen: bool,
    /// Each account of the Electron app with the account here that it is.
    pub accounts: BTreeMap<String, String>,
    /// The kinds that have been brought at least once.
    pub brought: Kinds,
}

impl Record {
    /// Reads the record in the profile at `root`; an empty one if there is
    /// none, or none that can be read.
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(root.join(FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the record, whole and then moved into place.
    pub fn save(&self, root: &Path) {
        let written = serde_json::to_string_pretty(self)
            .map_err(io::Error::other)
            .and_then(|json| {
                let file = root.join(FILE);
                let partial = file.with_extension("json.part");
                std::fs::write(&partial, json)?;
                std::fs::rename(&partial, file)
            });
        if let Err(error) = written {
            log::warn!("what was brought from the old app could not be noted: {error}");
        }
    }

    /// Whether the offer to bring things over is still to be made: nothing
    /// has been brought, and nobody has said no.
    pub fn unasked(&self) -> bool {
        !self.seen && !self.brought.any()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::leveldb::tests::scratch;

    #[test]
    fn a_record_survives_being_saved_and_read_again() {
        let root = scratch("record");
        assert!(Record::load(&root).unasked());
        let mut record = Record {
            seen: true,
            ..Record::default()
        };
        record.accounts.insert("theirs".into(), "ours".into());
        record.brought.history = true;
        record.save(&root);
        assert_eq!(Record::load(&root), record);
        assert!(!record.unasked());
    }

    #[test]
    fn a_profile_that_said_no_is_not_asked_again() {
        let record = Record {
            seen: true,
            ..Record::default()
        };
        assert!(!record.unasked());
    }
}
