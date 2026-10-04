//! Bringing across what was chosen. Runs on a thread of its own: it copies
//! files and waits on the core, neither of which the window may wait for.
//!
//! It writes only into this app's profile, and only what does not disturb
//! a running core: an account's folder that is not in the list yet, and,
//! through the core, the database and the song cache. The list of accounts
//! and the settings are the window thread's; what is to change in them is
//! handed back in the [`Outcome`].

use std::path::{Path, PathBuf};

use spotified_client::migrate::{ImportedSongs, Merged};

use super::{Found, Kinds, Line, OldAccount, Outcome, Progress, signin};
use crate::accounts::{self, Accounts, CREDENTIALS, DATABASE, RESOLVER_COOKIES, SavedAccount};

/// How many songs are asked of the core at a time, and about how much
/// audio: each request has to be answered within the client's time limit.
const SONGS_AT_ONCE: usize = 8;
/// What a line says of the account that was listened to signed out.
const SIGNED_OUT: &str = "Listening while signed out";

/// The core, as a run needs it.
pub trait Core {
    /// Merges the database at `from` into the one at `into`. `live` says
    /// the running core owns `into`, and must be the one to do it.
    fn merge(&self, from: &Path, into: &Path, live: bool) -> Result<Merged, String>;
    /// Copies these songs from the cache folder `dir` into this app's.
    fn songs(
        &self,
        dir: &Path,
        ids: &[String],
        limit_mb: Option<u32>,
    ) -> Result<ImportedSongs, String>;
    /// Whether a core is running to ask.
    fn running(&self) -> bool;
}

/// Everything a run needs to know, taken as it stood when it was asked for.
#[derive(Debug, Clone)]
pub struct Job {
    pub found: Found,
    pub kinds: Kinds,
    /// This app's profile, and the accounts in it.
    pub root: PathBuf,
    pub accounts: Accounts,
    /// The database the running core is on.
    pub live_database: PathBuf,
    /// The most the song cache may hold, in megabytes.
    pub cache_max_mb: u32,
}

pub fn run(job: &Job, core: &dyn Core, progress: &dyn Fn(Progress)) -> Outcome {
    let mut outcome = Outcome {
        done: job.kinds,
        ..Outcome::default()
    };
    let step = |step: &str| {
        progress(Progress {
            step: step.to_owned(),
            ..Progress::default()
        });
    };

    // Where each account's history goes: the account here that it is.
    let mut homes: Vec<(&OldAccount, Option<Home>)> = Vec::new();
    step("Looking at the accounts");
    for old in &job.found.accounts {
        let made = outcome.new_accounts.len();
        let home = if old.guest {
            guest_home(job)
        } else {
            account_home(job, old, &mut outcome)
        };
        // With nobody signed in here, whoever the Electron app was using
        // is the one to use; failing that, the first that was brought.
        if job.accounts.active.is_none()
            && let Some(new) = outcome.new_accounts.get(made)
            && (old.in_use || outcome.activate.is_none())
        {
            outcome.activate = Some(new.id.clone());
        }
        homes.push((old, home));
    }

    if job.kinds.history {
        for (old, home) in &homes {
            let name = if old.guest { SIGNED_OUT } else { &old.name };
            step(&format!("Bringing the listening history of {name}"));
            history(job, core, old, home.as_ref(), name, &mut outcome);
        }
    }
    if job.kinds.songs {
        songs(job, core, progress, &mut outcome);
    }
    if job.kinds.preferences {
        if let Some(reason) = &job.found.prefs.unread {
            outcome.lines.push(Line::skipped(format!(
                "The old app's settings could not be read ({reason})."
            )));
        }
        // Taken in on the window's thread, which the settings belong to.
        outcome.prefs = Some(job.found.prefs.clone());
    }
    outcome
}

/// Where an account's history goes: the folder of the account here that
/// it is, and that account, whose channels say where each of theirs goes.
/// Signed-out listening has a folder and no account.
struct Home {
    folder: PathBuf,
    account: Option<SavedAccount>,
}

/// Where signed-out listening goes: the profile's root, unless an account
/// here keeps its own history there.
fn guest_home(job: &Job) -> Option<Home> {
    (!job.accounts.has_legacy()).then(|| Home {
        folder: job.root.clone(),
        account: None,
    })
}

/// The account here that `old` is: the one it already is, or one made for
/// it now, signed in, when sign-ins are being brought.
fn account_home(job: &Job, old: &OldAccount, outcome: &mut Outcome) -> Option<Home> {
    let known = old.here.as_deref().and_then(|id| job.accounts.get(id));
    if let Some(ours) = known {
        outcome.pairs.push((old.id.clone(), ours.id.clone()));
        if job.kinds.sign_in {
            outcome.lines.push(Line::skipped(format!(
                "{} is already signed in here.",
                old.name
            )));
        }
        return Some(Home {
            folder: accounts::folder_of(&job.root, Some(ours)),
            account: Some(ours.clone()),
        });
    }
    if !job.kinds.sign_in {
        return None;
    }
    if !old.signed_in {
        outcome.lines.push(Line::skipped(format!(
            "{} is signed out in the old app, so there is no sign-in to bring.",
            old.name
        )));
        return None;
    }
    let mut ours = SavedAccount {
        avatar_url: old.avatar_url.clone(),
        channel: old.channel.clone(),
        channels: old.channels.clone(),
        ..accounts::new_account(&old.name)
    };
    let folder = accounts::folder_of(&job.root, Some(&ours));
    let copied = std::fs::create_dir_all(&folder)
        .map_err(signin::CopyError::Io)
        .and_then(|()| signin::copy(&old.credentials, &folder.join(CREDENTIALS)));
    if let Err(error) = copied {
        // The folder was made for this account and holds nothing else.
        let _ = std::fs::remove_dir_all(&folder);
        outcome.lines.push(Line::failed(format!(
            "{}: the sign-in could not be brought: {error}.",
            old.name
        )));
        return None;
    }
    // The same session in the form yt-dlp reads. The core writes it again
    // when it starts on the account, so a copy that fails costs nothing.
    let _ = std::fs::copy(&old.resolver_cookies, folder.join(RESOLVER_COOKIES));
    // The credentials say which channel they act as; the list follows them.
    ours.channel = crate::channel::active(&folder.join(CREDENTIALS));
    outcome
        .lines
        .push(Line::brought(format!("Signed in as {}.", old.name)));
    outcome.pairs.push((old.id.clone(), ours.id.clone()));
    outcome.new_accounts.push(ours.clone());
    Some(Home {
        folder,
        account: Some(ours),
    })
}

/// The database here that one of `old`'s goes into. The Electron app kept
/// one for the account and one for each channel it acted as, and so does
/// this app: each goes to its own, never several into one, or a channel
/// would be shown what another listened to.
///
/// Its own goes to the account's own. A channel's goes to that channel's
/// here; where the account here has no folder for the channel yet, the
/// Electron app's name for it is taken, and `outcome` says so for the list
/// to remember. What cannot be told to be any channel's (a folder the old
/// list does not name, or an account here that still keeps one database
/// for all) goes to the account's own, as nothing better is known.
fn database_for(old: &OldAccount, home: &Home, from: &Path, outcome: &mut Outcome) -> PathBuf {
    let own = home.folder.join(DATABASE);
    let Some(ours) = home.account.as_ref().filter(|ours| ours.channel_databases) else {
        return own;
    };
    // `…/channels/<its folder>/spotifier.db`.
    let folder = from.parent().filter(|folder| {
        let above = folder.parent().and_then(Path::file_name);
        above.is_some_and(|name| name == "channels")
    });
    let old_local = folder
        .and_then(Path::file_name)
        .and_then(|name| name.to_str());
    let channel = old_local.and_then(|local| {
        let mut theirs = old.channels.iter();
        theirs.find(|channel| channel.local_id == local && !channel.id.is_empty())
    });
    let Some(channel) = channel else {
        return own;
    };
    let local = match accounts::local_id_of(ours, &channel.id) {
        Some(local) => local.to_owned(),
        None if accounts::valid_id(&channel.local_id) => {
            let adopted = (ours.id.clone(), channel.clone());
            if !outcome.channels.contains(&adopted) {
                outcome.channels.push(adopted);
            }
            channel.local_id.clone()
        }
        None => return own,
    };
    home.folder.join("channels").join(local).join(DATABASE)
}

/// Merges every database of `old` into the database here that is its.
fn history(
    job: &Job,
    core: &dyn Core,
    old: &OldAccount,
    home: Option<&Home>,
    name: &str,
    outcome: &mut Outcome,
) {
    if old.databases.is_empty() || (old.history.is_empty() && old.unread.is_none()) {
        outcome.lines.push(Line::skipped(format!(
            "{name}: no listening history in the old app."
        )));
        return;
    }
    let Some(home) = home else {
        let why = if old.guest {
            "an account here keeps its own history in that place"
        } else if job.kinds.sign_in {
            "it has no account here to keep it"
        } else {
            "it has no account here to keep it; tick Accounts to add it"
        };
        outcome.lines.push(Line::skipped(format!(
            "{name}: the listening history was left, because {why}."
        )));
        return;
    };
    let mut total = Merged::default();
    let mut live_changed = false;
    for database in &old.databases {
        let into = database_for(old, home, database, outcome);
        let live = into == job.live_database && core.running();
        // A channel that has not listened to anything here has no folder.
        let made = into.parent().map_or(Ok(()), std::fs::create_dir_all);
        let merged = made
            .map_err(|error| error.to_string())
            .and_then(|()| core.merge(database, &into, live));
        match merged {
            Ok(merged) => {
                live_changed |= live && merged != Merged::default();
                total.plays += merged.plays;
                total.plays_known += merged.plays_known;
                total.folders += merged.folders;
                total.pins += merged.pins;
                total.filed += merged.filed;
                total.resume |= merged.resume;
            }
            Err(error) => {
                log::warn!("a database of the old app could not be merged: {error}");
                outcome.lines.push(Line::failed(format!(
                    "{name}: the listening history could not be brought: {error}"
                )));
                return;
            }
        }
    }
    outcome.history_changed |= live_changed;
    outcome.lines.push(history_line(name, total));
}

fn history_line(name: &str, merged: Merged) -> Line {
    let nothing_new =
        merged.plays == 0 && merged.folders == 0 && merged.pins == 0 && !merged.resume;
    if nothing_new {
        return Line::skipped(match merged.plays_known {
            0 => format!("{name}: nothing to bring."),
            known => format!(
                "{name}: nothing new. All {} were here already.",
                counted(known, "play")
            ),
        });
    }
    let mut said = format!("{name}: {} brought.", counted(merged.plays, "play"));
    if merged.plays_known > 0 {
        said += &format!(
            " {} were here already.",
            counted(merged.plays_known, "play")
        );
    }
    let mut also = Vec::new();
    if merged.pins > 0 {
        also.push(counted(merged.pins, "pin"));
    }
    if merged.folders > 0 {
        also.push(counted(merged.folders, "folder"));
    }
    if !also.is_empty() {
        said += &format!(" {} came with them.", also.join(" and "));
    }
    if merged.resume {
        said += " The queue you left is back too.";
    }
    Line::brought(said)
}

/// Copies the kept songs, a few at a time, most recently played first.
fn songs(job: &Job, core: &dyn Core, progress: &dyn Fn(Progress), outcome: &mut Outcome) {
    let found = &job.found.songs;
    if found.count == 0 {
        outcome
            .lines
            .push(Line::skipped("No downloaded songs in the old app."));
        return;
    }
    if !core.running() {
        outcome.lines.push(Line::skipped(
            "Downloaded songs were left: the playback service is not running to take them.",
        ));
        return;
    }
    // A larger cache is on its way with the preferences: the songs should
    // not be turned away for want of room they are about to be given.
    let limit = Some(job.cache_max_mb);
    let total = u32::try_from(found.count).unwrap_or(u32::MAX);
    let mut done = 0;
    let mut all = ImportedSongs::default();
    for (folder, ids) in &found.folders {
        for batch in ids.chunks(SONGS_AT_ONCE) {
            progress(Progress {
                step: "Copying downloaded songs".to_owned(),
                done,
                total,
            });
            match core.songs(folder, batch, limit) {
                Ok(imported) => all.add(imported),
                Err(error) => {
                    outcome.lines.push(Line::failed(format!(
                        "Downloaded songs stopped part of the way: {error}"
                    )));
                    outcome.songs_changed = all.copied > 0;
                    return;
                }
            }
            done += u32::try_from(batch.len()).unwrap_or(0);
        }
    }
    outcome.songs_changed = all.copied > 0;
    if all.copied > 0 {
        outcome.lines.push(Line::brought(format!(
            "{} copied ({}).",
            counted(all.copied, "song"),
            crate::views::format::bytes(all.bytes)
        )));
    }
    if all.present > 0 {
        outcome.lines.push(Line::skipped(format!(
            "{} here already.",
            were(all.present, "song")
        )));
    }
    if all.no_room > 0 {
        outcome.lines.push(Line::skipped(format!(
            "{} left: the song cache is held to {}. Raise it under Storage and \
             bring them again.",
            were(all.no_room, "song"),
            crate::views::format::bytes(u64::from(job.cache_max_mb) << 20)
        )));
    }
    if all.failed > 0 {
        outcome.lines.push(Line::failed(format!(
            "{} could not be read.",
            counted(all.failed, "song")
        )));
    }
}

/// `1 play`, `2 plays`.
fn counted(count: u64, what: &str) -> String {
    match count {
        1 => format!("1 {what}"),
        count => format!("{count} {what}s"),
    }
}

/// `1 song was`, `2 songs were`.
fn were(count: u64, what: &str) -> String {
    match count {
        1 => format!("1 {what} was"),
        count => format!("{count} {what}s were"),
    }
}

#[cfg(test)]
mod tests;
