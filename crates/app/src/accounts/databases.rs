//! Which database an account's listening is kept in.
//!
//! A Google account can act as any of its YouTube channels, and each has a
//! library and a history of its own. So each has a database of its own, as
//! in the Electron app: the account's in its folder, a channel's in
//! `channels/<id made here>/` under it.
//!
//! This app first kept one database for an account whatever channel it
//! acted as. An account from then is moved over once: its database becomes
//! that of the channel it is acting as, which is whose listening it most
//! likely holds, and from then on each channel keeps its own. The move is
//! done only while no core is running, by renaming, and is put back if any
//! part of it fails; an account whose move failed goes on with its one
//! database, and is tried again at the next start.

use std::io;
use std::path::{Path, PathBuf};

use super::{DATABASE, SavedAccount, valid_id};

/// Beside a database while it is open, and sometimes after: what was
/// written and not yet folded into it. They go wherever it goes.
const BESIDE: [&str; 2] = ["-wal", "-shm"];

/// The database of `account` in its `folder`, acting as the channel it
/// has in use.
pub fn database_of(folder: &Path, account: Option<&SavedAccount>) -> PathBuf {
    match account.and_then(channel_folder) {
        Some(local) => folder.join("channels").join(local).join(DATABASE),
        None => folder.join(DATABASE),
    }
}

/// The folder name of the channel in use, when the account keeps a
/// database for each and is acting as one that has a name to keep it by.
fn channel_folder(account: &SavedAccount) -> Option<&str> {
    if !account.channel_databases || account.channel.is_empty() {
        return None;
    }
    local_id_of(account, &account.channel)
}

/// The name of the folder a channel of `account` keeps its database in.
pub fn local_id_of<'a>(account: &'a SavedAccount, channel: &str) -> Option<&'a str> {
    account
        .channels
        .iter()
        .find(|known| known.id == channel)
        .map(|known| known.local_id.as_str())
        // The list can be edited by hand, and this is about to be a path.
        .filter(|local| valid_id(local))
}

/// Moves the one database of an account from before channels had their
/// own to the channel it is acting as. Returns whether the account keeps
/// a database for each channel from now on: `false` only when the move
/// was needed and failed, in which case everything is where it was.
pub fn settle(folder: &Path, account: &SavedAccount) -> bool {
    if account.channel_databases {
        return true;
    }
    let single = folder.join(DATABASE);
    let ahead = SavedAccount {
        channel_databases: true,
        ..account.clone()
    };
    let target = database_of(folder, Some(&ahead));
    // Its own channel keeps the database where it is; an account that has
    // not listened to anything has none to move.
    if target == single || !single.exists() {
        return true;
    }
    // A channel that somehow has a database already keeps it: two cannot
    // be made one here, and neither is thrown away.
    if target.exists() {
        log::warn!("a channel already has a database; the account's own is left where it is");
        return true;
    }
    match move_database(&single, &target) {
        Ok(()) => {
            log::info!("the account's listening is now kept by channel");
            true
        }
        Err(error) => {
            log::warn!("the account's database could not be moved to its channel: {error}");
            false
        }
    }
}

/// Renames a database and what lies beside it. All of it moves or none
/// does: what has moved is put back if a later part will not.
fn move_database(from: &Path, to: &Path) -> io::Result<()> {
    let folder = to
        .parent()
        .ok_or_else(|| io::Error::other("a database has a folder"))?;
    std::fs::create_dir_all(folder)?;
    let beside = |path: &Path, suffix: &str| {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        PathBuf::from(name)
    };
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let parts = std::iter::once((from.to_path_buf(), to.to_path_buf())).chain(
        BESIDE
            .iter()
            .map(|suffix| (beside(from, suffix), beside(to, suffix))),
    );
    for (old, new) in parts {
        if !old.exists() {
            continue;
        }
        if let Err(error) = std::fs::rename(&old, &new) {
            for (old, new) in moved.into_iter().rev() {
                if let Err(back) = std::fs::rename(&new, &old) {
                    log::error!("{} could not be put back: {back}", old.display());
                }
            }
            return Err(error);
        }
        moved.push((old, new));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{SavedChannel, new_account};
    use super::*;

    const LOCAL: &str = "aaaaaaaa-0000-0000-0000-00000000000c";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("spotified-dbs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// An account from before channels had databases, acting as `channel`.
    fn from_before(channel: &str) -> SavedAccount {
        SavedAccount {
            channel: channel.to_owned(),
            channel_databases: false,
            channels: vec![
                SavedChannel::default(),
                SavedChannel {
                    id: "123".into(),
                    name: "Band".into(),
                    local_id: LOCAL.into(),
                    ..SavedChannel::default()
                },
            ],
            ..new_account("Ada")
        }
    }

    fn settled(account: &SavedAccount) -> SavedAccount {
        SavedAccount {
            channel_databases: true,
            ..account.clone()
        }
    }

    #[test]
    fn each_channel_of_an_account_has_a_database_of_its_own() {
        let folder = Path::new("profile");
        let own = settled(&from_before(""));
        assert_eq!(database_of(folder, Some(&own)), folder.join(DATABASE));
        let band = settled(&from_before("123"));
        let theirs = folder.join("channels").join(LOCAL).join(DATABASE);
        assert_eq!(database_of(folder, Some(&band)), theirs);
        // Signed out, the profile's own.
        assert_eq!(database_of(folder, None), folder.join(DATABASE));
    }

    #[test]
    fn a_channel_with_no_folder_name_or_one_that_is_a_path_stays_with_the_account() {
        let folder = Path::new("profile");
        let mut account = settled(&from_before("999"));
        assert_eq!(database_of(folder, Some(&account)), folder.join(DATABASE));
        account.channel = "123".into();
        account.channels[1].local_id = "..\\..\\elsewhere".into();
        assert_eq!(database_of(folder, Some(&account)), folder.join(DATABASE));
    }

    #[test]
    fn the_one_database_of_an_account_goes_to_the_channel_it_is_acting_as() {
        let folder = scratch("move");
        std::fs::write(folder.join(DATABASE), "plays").expect("write");
        std::fs::write(folder.join(format!("{DATABASE}-wal")), "more").expect("write");
        let account = from_before("123");
        assert!(settle(&folder, &account));
        let theirs = folder.join("channels").join(LOCAL);
        assert_eq!(
            std::fs::read_to_string(theirs.join(DATABASE)).expect("read"),
            "plays"
        );
        assert_eq!(
            std::fs::read_to_string(theirs.join(format!("{DATABASE}-wal"))).expect("read"),
            "more"
        );
        assert!(!folder.join(DATABASE).exists());
        // Done again, as it is at every start, it changes nothing.
        assert!(settle(&folder, &settled(&account)));
        assert!(theirs.join(DATABASE).exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn an_account_acting_as_itself_keeps_its_database_where_it_is() {
        let folder = scratch("own");
        std::fs::write(folder.join(DATABASE), "plays").expect("write");
        assert!(settle(&folder, &from_before("")));
        assert!(folder.join(DATABASE).exists());
        assert!(!folder.join("channels").exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_channel_that_has_a_database_already_is_not_written_over() {
        let folder = scratch("both");
        let theirs = folder.join("channels").join(LOCAL);
        std::fs::create_dir_all(&theirs).expect("a folder");
        std::fs::write(theirs.join(DATABASE), "theirs").expect("write");
        std::fs::write(folder.join(DATABASE), "the account's").expect("write");
        assert!(settle(&folder, &from_before("123")));
        let read = |path: PathBuf| std::fs::read_to_string(path).expect("read");
        assert_eq!(read(theirs.join(DATABASE)), "theirs");
        assert_eq!(read(folder.join(DATABASE)), "the account's");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_move_that_fails_part_of_the_way_is_put_back_whole() {
        let folder = scratch("back");
        std::fs::write(folder.join(DATABASE), "plays").expect("write");
        std::fs::write(folder.join(format!("{DATABASE}-wal")), "more").expect("write");
        // Something stands where the second file is to go, so that rename
        // fails after the first has succeeded.
        let theirs = folder.join("channels").join(LOCAL);
        std::fs::create_dir_all(theirs.join(format!("{DATABASE}-wal")).join("in the way"))
            .expect("a folder");
        let account = from_before("123");
        assert!(!settle(&folder, &account));
        // Everything is where it was, and the account goes on as before.
        let read = |name: String| std::fs::read_to_string(folder.join(name)).expect("read");
        assert_eq!(read(DATABASE.to_owned()), "plays");
        assert_eq!(read(format!("{DATABASE}-wal")), "more");
        assert!(!theirs.join(DATABASE).exists());
        assert_eq!(database_of(&folder, Some(&account)), folder.join(DATABASE));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
