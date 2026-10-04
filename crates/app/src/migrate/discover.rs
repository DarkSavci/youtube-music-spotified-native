//! Looking at the Electron app's profile to say what is in it. Nothing is
//! brought here, and nothing in that profile is changed by looking.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{Found, History, OldAccount, Record, Songs, prefs, signin};
use crate::accounts::valid_id;
use crate::accounts::{Accounts, CREDENTIALS, RESOLVER_COOKIES, SavedAccount, SavedChannel};

/// The Electron app's name for the core's database.
const DATABASE: &str = "spotifier.db";
const SONGS: &str = "audio-cache";
/// Where it kept what was listened to with nobody signed in.
const GUEST: &str = "guest";
/// What it called an account that had not said its name yet.
const UNNAMED: [&str; 2] = ["Saved account", "New account"];

/// The Electron app's `accounts.json`.
#[derive(Deserialize, Default)]
#[serde(default)]
struct OldList {
    active: Option<String>,
    accounts: Vec<Listed>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Listed {
    id: String,
    name: String,
    /// The first account, from before there were several, lives in the root.
    legacy: bool,
    avatar_url: String,
    channel: String,
    channels: Vec<SavedChannel>,
}

/// Counts what a database holds, or says why it could not be.
pub type Inspect<'a> = &'a dyn Fn(&Path) -> Result<History, String>;

/// What the profile at `root` holds, or `None` if there is no profile
/// there, or nothing in it to bring. `native` and `record` are this app's
/// accounts and what was brought before, to tell which are here already.
pub fn discover(
    root: &Path,
    native: &Accounts,
    record: &Record,
    inspect: Inspect<'_>,
) -> Option<Found> {
    if !root.is_dir() {
        return None;
    }
    let list = read_list(root);
    let mut accounts: Vec<OldAccount> = Vec::new();
    for listed in &list.accounts {
        let Some(folder) = folder_of(root, listed) else {
            continue;
        };
        let mut account = account_in(&folder, inspect);
        account.id.clone_from(&listed.id);
        account.name.clone_from(&listed.name);
        account.avatar_url.clone_from(&listed.avatar_url);
        account.channel.clone_from(&listed.channel);
        account.channels.clone_from(&listed.channels);
        account.in_use = list.active.as_ref() == Some(&listed.id);
        account.here = here(&account, native, record);
        accounts.push(account);
    }
    let guest = root.join(GUEST);
    if guest.join(DATABASE).exists() {
        let account = OldAccount {
            id: GUEST.to_owned(),
            name: "Signed out".to_owned(),
            guest: true,
            signed_in: false,
            ..account_in(&guest, inspect)
        };
        // Worth a line only if something was listened to.
        if !account.history.is_empty() || account.unread.is_some() {
            accounts.push(account);
        }
    }

    // How the Electron app named the account in use to its page.
    let active = list
        .accounts
        .iter()
        .find(|listed| Some(&listed.id) == list.active.as_ref());
    let scope = active.map_or_else(
        || GUEST.to_owned(),
        |listed| {
            let who = if listed.legacy { "legacy" } else { &listed.id };
            let channel = match listed.channel.as_str() {
                "" => "personal",
                channel => channel,
            };
            format!("{who}:{channel}")
        },
    );
    let mut folders = vec![root.to_path_buf(), guest];
    folders.extend(list.accounts.iter().filter_map(|l| folder_of(root, l)));
    let found = Found {
        root: root.to_path_buf(),
        accounts,
        songs: songs_in(&folders),
        prefs: prefs::read(root, &scope),
    };
    let nothing = found.accounts.is_empty() && found.songs.count == 0 && found.prefs.is_empty();
    (!nothing).then_some(found)
}

/// The list of accounts; for a profile from before there was one, the one
/// account whose credentials are in the root.
fn read_list(root: &Path) -> OldList {
    let read = std::fs::read_to_string(root.join("accounts.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<OldList>(&text).ok());
    match read {
        Some(list) => list,
        None if root.join(CREDENTIALS).exists() => {
            let first = Listed {
                id: "legacy".to_owned(),
                name: UNNAMED[0].to_owned(),
                legacy: true,
                ..Listed::default()
            };
            OldList {
                active: Some(first.id.clone()),
                accounts: vec![first],
            }
        }
        None => OldList::default(),
    }
}

/// Where an account keeps its files. An id that could not have been made
/// by the Electron app is not followed: it is about to become a path.
fn folder_of(root: &Path, listed: &Listed) -> Option<PathBuf> {
    if listed.legacy {
        Some(root.to_path_buf())
    } else if valid_id(&listed.id) {
        Some(root.join("accounts").join(&listed.id))
    } else {
        None
    }
}

/// What the files in an account's folder say of it.
fn account_in(folder: &Path, inspect: Inspect<'_>) -> OldAccount {
    let credentials = folder.join(CREDENTIALS);
    let mut account = OldAccount {
        signed_in: signin::holds_session(&credentials),
        credentials,
        resolver_cookies: folder.join(RESOLVER_COOKIES),
        ..OldAccount::default()
    };
    // Its own database, and one for each channel it has acted as.
    let mut databases = vec![folder.join(DATABASE)];
    if let Ok(channels) = std::fs::read_dir(folder.join("channels")) {
        let mut theirs: Vec<PathBuf> = channels
            .flatten()
            .map(|channel| channel.path().join(DATABASE))
            .collect();
        theirs.sort();
        databases.extend(theirs);
    }
    for database in databases.into_iter().filter(|database| database.exists()) {
        match inspect(&database) {
            Ok(history) => account.history.add(&history),
            Err(error) => account.unread = Some(error),
        }
        account.databases.push(database);
    }
    account
}

/// The account here that an account of the Electron app is: the one a run
/// before this paired it with, or failing that one that is plainly the
/// same person, by name and by picture or channel.
fn here(old: &OldAccount, native: &Accounts, record: &Record) -> Option<String> {
    if let Some(id) = record.accounts.get(&old.id)
        && native.get(id).is_some()
    {
        return Some(id.clone());
    }
    let named = |name: &str| !name.is_empty() && !UNNAMED.contains(&name);
    let same = |ours: &&SavedAccount| {
        if !named(&old.name) || ours.name != old.name {
            return false;
        }
        let picture = !old.avatar_url.is_empty() && ours.avatar_url == old.avatar_url;
        let handle = old.channels.iter().any(|theirs| {
            !theirs.handle.is_empty()
                && ours
                    .channels
                    .iter()
                    .any(|channel| channel.handle == theirs.handle)
        });
        picture || handle
    };
    native
        .accounts
        .iter()
        .find(same)
        .map(|ours| ours.id.clone())
}

/// When a kept song was last played, as its record says.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct SongRecord {
    used_at: String,
}

/// The songs kept in the `audio-cache` of each of these folders: a record
/// and the audio it describes, under the song's id.
fn songs_in(folders: &[PathBuf]) -> Songs {
    let mut songs = Songs::default();
    let mut seen: Vec<PathBuf> = Vec::new();
    for folder in folders {
        let folder = folder.join(SONGS);
        if seen.contains(&folder) {
            continue;
        }
        seen.push(folder.clone());
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut kept: Vec<(String, String)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let id = path.file_stem().and_then(|stem| stem.to_str());
            let (Some(id), Some("json")) = (id, path.extension().and_then(|e| e.to_str())) else {
                continue;
            };
            let Ok(audio) = std::fs::metadata(path.with_extension("audio")) else {
                continue;
            };
            let record: SongRecord = std::fs::read_to_string(&path)
                .ok()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            songs.bytes += audio.len();
            kept.push((record.used_at, id.to_owned()));
        }
        // Most recently played first: should not all of them fit, these
        // are the ones worth having.
        kept.sort_by(|a, b| b.cmp(a));
        songs.count += kept.len();
        if !kept.is_empty() {
            let ids = kept.into_iter().map(|(_, id)| id).collect();
            songs.folders.push((folder, ids));
        }
    }
    songs
}

#[cfg(test)]
pub(super) mod tests;
