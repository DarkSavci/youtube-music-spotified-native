//! The Google accounts kept signed in, and which of them is in use.
//!
//! Each account has a folder of its own for its credentials and the core's
//! database, named by an id made here: nothing an account calls itself ever
//! becomes a path. The first account, from before there could be several,
//! stays where it always was, in the profile's root. `accounts.json` lists
//! them, in the shape the Electron app keeps its own list.
//!
//! The core serves one account at a time, so changing which is in use is a
//! restart of it on another folder.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use spotified_client::models::Channel;

mod databases;

pub use databases::{database_of, local_id_of};

const FILE: &str = "accounts.json";
const VERSION: u32 = 1;
pub const CREDENTIALS: &str = "credentials.json";
/// The session as the core hands it to yt-dlp, beside the credentials. It
/// is the same sign-in, and goes when they do.
pub const RESOLVER_COOKIES: &str = "yt-dlp-cookies.txt";
pub const DATABASE: &str = "spotified.db";
/// The answers the core keeps from YouTube, beside its database.
const ANSWERS: &str = "responses.db";
/// What an account is called until it has said its name.
const UNNAMED: [&str; 2] = ["Saved account", "New account"];

/// A channel of an account, as the list remembers it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedChannel {
    pub id: String,
    pub name: String,
    pub handle: String,
    /// The address of the channel's picture; empty when it has none.
    pub avatar_url: String,
    /// What the folder of its database is called, made here: nothing a
    /// channel calls itself becomes a path. The Electron app's list has
    /// the same, under the same name.
    pub local_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedAccount {
    pub id: String,
    pub name: String,
    /// The account from before there could be several: it lives in the
    /// profile's root.
    pub legacy: bool,
    pub avatar_url: String,
    /// The channel in use; empty for the account's own.
    pub channel: String,
    pub channels: Vec<SavedChannel>,
    /// Each channel keeps a database of its own. An account in a list
    /// from before they did has one for all, until it has been moved.
    pub channel_databases: bool,
}

impl SavedAccount {
    /// What the channel in use is called, when it is one of those known.
    pub fn channel_name(&self) -> Option<&str> {
        self.channels
            .iter()
            .find(|channel| channel.id == self.channel)
            .map(|channel| channel.name.as_str())
    }
}

/// The list, as the views read it and as the file holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Accounts {
    version: u32,
    /// The id of the account in use; `None` when signed out.
    pub active: Option<String>,
    pub accounts: Vec<SavedAccount>,
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            version: VERSION,
            active: None,
            accounts: Vec::new(),
        }
    }
}

impl Accounts {
    /// The list for a profile that has none: empty, or holding the one
    /// account whose credentials are already in the profile's root.
    fn first(has_credentials: bool) -> Self {
        let mut list = Self::default();
        if has_credentials {
            let account = SavedAccount {
                legacy: true,
                // Its database was there before the list: it is looked at
                // before it is taken to be one channel's.
                channel_databases: false,
                ..new_account(UNNAMED[0])
            };
            list.active = Some(account.id.clone());
            list.accounts.push(account);
        }
        list
    }

    pub fn get(&self, id: &str) -> Option<&SavedAccount> {
        self.accounts.iter().find(|account| account.id == id)
    }

    /// The account in use.
    pub fn active(&self) -> Option<&SavedAccount> {
        self.get(self.active.as_deref()?)
    }

    fn active_mut(&mut self) -> Option<&mut SavedAccount> {
        let id = self.active.clone()?;
        self.accounts.iter_mut().find(|account| account.id == id)
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active.as_deref() == Some(id)
    }

    /// The accounts that are not the one in use.
    pub fn others(&self) -> impl Iterator<Item = &SavedAccount> {
        self.accounts
            .iter()
            .filter(|account| !self.is_active(&account.id))
    }

    /// Takes in a newly signed-in account, which is then the one in use.
    pub fn add(&mut self, account: SavedAccount) {
        self.active = Some(account.id.clone());
        self.accounts.push(account);
    }

    /// Takes in an account brought from the Electron app. Which account is
    /// in use does not change.
    pub fn keep(&mut self, account: SavedAccount) {
        if self.get(&account.id).is_none() {
            self.accounts.push(account);
        }
    }

    /// Whether one of the accounts is the one from before there could be
    /// several, whose files are in the profile's root.
    pub fn has_legacy(&self) -> bool {
        self.accounts.iter().any(|account| account.legacy)
    }

    /// Makes a saved account the one in use. `false` if there is none such.
    pub fn activate(&mut self, id: &str) -> bool {
        let known = self.get(id).is_some();
        if known {
            self.active = Some(id.to_owned());
        }
        known
    }

    /// Forgets an account. Nobody is signed in after the one in use goes:
    /// which of the others to use instead is the person's choice.
    pub fn remove(&mut self, id: &str) -> Option<SavedAccount> {
        let at = self.accounts.iter().position(|account| account.id == id)?;
        if self.is_active(id) {
            self.active = None;
        }
        Some(self.accounts.remove(at))
    }

    /// What the account in use turned out to be called. A name is only
    /// taken until it has one; the picture is the account's own, so it is
    /// left alone while a channel is acted as. Returns whether anything
    /// changed.
    pub fn set_name(&mut self, name: &str, avatar_url: &str) -> bool {
        let Some(account) = self.active_mut() else {
            return false;
        };
        let before = account.clone();
        if !name.is_empty() && UNNAMED.contains(&account.name.as_str()) {
            name.clone_into(&mut account.name);
        }
        if account.channel.is_empty() && !avatar_url.is_empty() {
            avatar_url.clone_into(&mut account.avatar_url);
        }
        *account != before
    }

    /// The channels the account in use can act as, as the core listed
    /// them. Returns whether anything changed.
    pub fn set_channels(&mut self, channels: &[Channel]) -> bool {
        let Some(account) = self.active_mut() else {
            return false;
        };
        // A channel known already keeps the folder it has.
        let local_id = |id: &str| {
            let known = account.channels.iter().find(|known| known.id == id);
            known
                .map(|known| known.local_id.clone())
                .filter(|local| valid_id(local))
                .unwrap_or_else(new_id)
        };
        let channels: Vec<SavedChannel> = channels
            .iter()
            .map(|channel| SavedChannel {
                id: channel.id.clone(),
                name: channel.name.clone(),
                handle: channel.handle.clone(),
                avatar_url: channel.avatar_url.clone(),
                local_id: local_id(&channel.id),
            })
            .collect();
        let changed = account.channels != channels;
        account.channels = channels;
        changed
    }

    /// Notes the channel the account in use acts as; empty for its own.
    pub fn select_channel(&mut self, id: &str) -> bool {
        let Some(account) = self.active_mut() else {
            return false;
        };
        let changed = account.channel != id;
        id.clone_into(&mut account.channel);
        name_channel(account) || changed
    }

    /// Takes in what a move from the Electron app learnt of a channel of
    /// `account`: the folder its history was put in, and what it is
    /// called, where that was not known here.
    pub fn adopt_channel(&mut self, account: &str, channel: SavedChannel) {
        let Some(account) = self.accounts.iter_mut().find(|known| known.id == account) else {
            return;
        };
        match account
            .channels
            .iter_mut()
            .find(|known| known.id == channel.id)
        {
            Some(known) if !valid_id(&known.local_id) => known.local_id = channel.local_id,
            Some(_) => {}
            None => account.channels.push(channel),
        }
    }
}

/// Makes sure the channel `account` acts as has a folder name for its
/// database, before one is needed: the core's list of channels comes
/// after the core has started, on a database that must be the right one.
/// Returns whether one had to be made.
fn name_channel(account: &mut SavedAccount) -> bool {
    if account.channel.is_empty() {
        return false;
    }
    let id = account.channel.clone();
    match account.channels.iter_mut().find(|known| known.id == id) {
        Some(known) if valid_id(&known.local_id) => false,
        Some(known) => {
            known.local_id = new_id();
            true
        }
        None => {
            account.channels.push(SavedChannel {
                id,
                local_id: new_id(),
                ..SavedChannel::default()
            });
            true
        }
    }
}

/// An account that is not in the list yet, with an id of its own.
pub fn new_account(name: &str) -> SavedAccount {
    SavedAccount {
        id: new_id(),
        name: name.to_owned(),
        channel_databases: true,
        ..SavedAccount::default()
    }
}

/// An id in the shape of a UUID: random enough to be unique, with no need
/// to be secret.
fn new_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    // The standard library seeds each hasher state from the system's
    // randomness, which is all the randomness this needs.
    let random = || {
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
    };
    let (high, low) = (random(), random());
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        high >> 32,
        (high >> 16) & 0xffff,
        high & 0xffff,
        low >> 48,
        low & 0xffff_ffff_ffff
    )
}

/// Whether `id` is one made by [`new_id`]. The file can be edited by hand,
/// and an id from it is about to become part of a path.
pub fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id
            .bytes()
            .all(|byte| byte == b'-' || byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The list and the profile it is kept in.
pub struct AccountStore {
    root: PathBuf,
    pub list: Accounts,
}

impl AccountStore {
    /// Reads the list in `root`, or makes the first one for a profile that
    /// has none.
    pub fn load(root: &Path) -> Self {
        let file = root.join(FILE);
        let read = std::fs::read_to_string(&file).ok().and_then(|text| {
            serde_json::from_str::<Accounts>(&text)
                .inspect_err(|error| log::warn!("the list of accounts could not be read: {error}"))
                .ok()
                .filter(|list| list.version == VERSION)
        });
        let fresh = read.is_none();
        let list = read.unwrap_or_else(|| Accounts::first(root.join(CREDENTIALS).exists()));
        let mut store = Self {
            root: root.to_path_buf(),
            list,
        };
        // The credentials are what the core goes by; the list follows them.
        let channel = crate::channel::active(&store.credentials());
        let moved = store.list.select_channel(&channel);
        if fresh || moved {
            store.save();
        }
        store
    }

    /// Writes the list. Whole and then moved into place, so a crash cannot
    /// leave half of one.
    pub fn save(&self) {
        let written = serde_json::to_string_pretty(&self.list)
            .map_err(io::Error::other)
            .and_then(|json| {
                let file = self.root.join(FILE);
                let partial = file.with_extension("json.part");
                std::fs::write(&partial, json)?;
                std::fs::rename(&partial, file)
            });
        if let Err(error) = written {
            log::warn!("the list of accounts could not be saved: {error}");
        }
    }

    /// Where an account keeps its files; the profile's root for nobody, so
    /// that what is listened to signed out stays in one place.
    pub fn directory(&self, account: Option<&SavedAccount>) -> PathBuf {
        folder_of(&self.root, account)
    }

    fn active_directory(&self) -> PathBuf {
        self.directory(self.list.active())
    }

    /// Makes ready for a core to start: every account keeps a database
    /// for each of its channels, those from before they did being moved
    /// over, and the folder of the database to be used is there, which
    /// the core cannot make itself. Only called while no core is running.
    pub fn prepare(&mut self) {
        let mut changed = false;
        for account in &mut self.list.accounts {
            if account.channel_databases {
                continue;
            }
            changed |= name_channel(account);
            let folder = folder_of(&self.root, Some(account));
            if databases::settle(&folder, account) {
                account.channel_databases = true;
                changed = true;
            }
        }
        if changed {
            self.save();
        }
        let database = self.database();
        if let Some(folder) = database.parent()
            && let Err(error) = std::fs::create_dir_all(folder)
        {
            log::warn!("{} could not be made: {error}", folder.display());
        }
    }

    /// The credentials the core is to run on.
    pub fn credentials(&self) -> PathBuf {
        self.active_directory().join(CREDENTIALS)
    }

    /// The database the core is to run on: that of the account in use,
    /// and of the channel it acts as.
    pub fn database(&self) -> PathBuf {
        database_of(&self.active_directory(), self.list.active())
    }

    /// Where a new account's credentials are to be written, its folder made.
    pub fn credentials_for(&self, account: &SavedAccount) -> io::Result<PathBuf> {
        let folder = self.directory(Some(account));
        std::fs::create_dir_all(&folder)?;
        Ok(folder.join(CREDENTIALS))
    }

    /// Signs an account out: its credentials go, with the copy of them
    /// made for yt-dlp and the answers YouTube gave it. What it listened to
    /// stays on this computer. Only done while no core runs on its folder,
    /// since that holds the files open.
    pub fn remove(&mut self, id: &str) {
        let Some(account) = self.list.remove(id) else {
            return;
        };
        let folder = self.directory(Some(&account));
        for file in [CREDENTIALS, RESOLVER_COOKIES] {
            delete(&folder.join(file));
        }
        forget_answers(&folder);
        self.save();
    }

    /// Deletes the answers the core kept from YouTube for the account in
    /// use. They belong to the channel that asked; another must not be
    /// shown them. Only done while no core is running.
    pub fn forget_answers(&self) {
        forget_answers(&self.active_directory());
    }
}

/// Where an account keeps its files in the profile at `root`: a folder of
/// its own, or the root for the first account and for nobody.
pub fn folder_of(root: &Path, account: Option<&SavedAccount>) -> PathBuf {
    match account {
        Some(account) if !account.legacy && valid_id(&account.id) => {
            root.join("accounts").join(&account.id)
        }
        _ => root.to_path_buf(),
    }
}

/// Deletes the answers kept in an account's folder and in the folder of
/// each of its channels: they sit beside each database it uses.
fn forget_answers(folder: &Path) {
    let mut folders = vec![folder.to_path_buf()];
    if let Ok(channels) = std::fs::read_dir(folder.join("channels")) {
        folders.extend(channels.flatten().map(|channel| channel.path()));
    }
    for folder in folders {
        for suffix in ["", "-wal", "-shm"] {
            delete(&folder.join(format!("{ANSWERS}{suffix}")));
        }
    }
}

fn delete(file: &Path) {
    if let Err(error) = std::fs::remove_file(file)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::warn!("{} could not be deleted: {error}", file.display());
    }
}

#[cfg(test)]
mod tests;
