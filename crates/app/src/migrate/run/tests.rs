//! A run against a made-up profile, with a core that only notes what it
//! was asked.

use std::cell::RefCell;

use super::*;
use crate::accounts::{SavedChannel, new_account};
use crate::migrate::discover::discover;
use crate::migrate::leveldb::tests::scratch;
use crate::migrate::tests::{SECOND, old_profile};
use crate::migrate::{History, Mark, Record};

/// A core that merges five plays from every database, and has room for
/// two songs.
#[derive(Default)]
struct Noted {
    stopped: bool,
    /// From, into, and whether the running core was asked.
    merges: RefCell<Vec<(PathBuf, PathBuf, bool)>>,
    songs: RefCell<Vec<String>>,
    limits: RefCell<Vec<Option<u32>>>,
}

impl Core for Noted {
    fn merge(&self, from: &Path, into: &Path, live: bool) -> Result<Merged, String> {
        if from.to_string_lossy().contains("channels") {
            return Err("the database is damaged".into());
        }
        self.merges
            .borrow_mut()
            .push((from.to_path_buf(), into.to_path_buf(), live));
        Ok(Merged {
            plays: 5,
            plays_known: 2,
            pins: 1,
            ..Merged::default()
        })
    }

    fn songs(
        &self,
        _: &Path,
        ids: &[String],
        limit_mb: Option<u32>,
    ) -> Result<ImportedSongs, String> {
        self.limits.borrow_mut().push(limit_mb);
        let mut songs = self.songs.borrow_mut();
        let room = 2usize.saturating_sub(songs.len()).min(ids.len());
        songs.extend(ids[..room].iter().cloned());
        Ok(ImportedSongs {
            copied: room as u64,
            bytes: 400 * room as u64,
            no_room: (ids.len() - room) as u64,
            ..ImportedSongs::default()
        })
    }

    fn running(&self) -> bool {
        !self.stopped
    }
}

fn history(_: &Path) -> Result<History, String> {
    Ok(History {
        plays: 7,
        ..History::default()
    })
}

/// A job to bring `kinds` from a made-up profile into an empty one of
/// this app's, in which `accounts` are already signed in.
fn job(name: &str, kinds: Kinds, accounts: Accounts) -> Job {
    let old = old_profile(name);
    let root = scratch(&format!("{name}-native"));
    let found = discover(&old, &accounts, &Record::default(), &history).expect("a profile");
    Job {
        found,
        kinds,
        live_database: root.join(DATABASE),
        root,
        accounts,
        cache_max_mb: 2048,
    }
}

const ALL: Kinds = Kinds {
    sign_in: true,
    history: true,
    songs: true,
    preferences: true,
};

fn said(outcome: &Outcome, mark: Mark, part: &str) -> bool {
    outcome
        .lines
        .iter()
        .any(|line| line.mark == mark && line.text.contains(part))
}

#[test]
fn a_first_run_signs_the_account_in_and_gives_it_its_history() {
    let job = job("run-first", ALL, Accounts::default());
    let core = Noted::default();
    let steps = RefCell::new(Vec::new());
    let outcome = run(&job, &core, &|progress| steps.borrow_mut().push(progress));

    // Ada is signed in; Grace has no session to bring.
    assert_eq!(outcome.new_accounts.len(), 1);
    let ada = &outcome.new_accounts[0];
    assert_eq!(ada.name, "Ada");
    assert_eq!(ada.avatar_url, "https://example.com/ada.png");
    assert!(!ada.legacy);
    let folder = accounts::folder_of(&job.root, Some(ada));
    assert_eq!(
        std::fs::read_to_string(folder.join(CREDENTIALS)).expect("credentials"),
        r#"{"cookie":"SID=made-up; SAPISID=made-up"}"#
    );
    assert!(folder.join(RESOLVER_COOKIES).exists());
    assert!(said(&outcome, Mark::Brought, "Signed in as Ada."));
    assert!(said(&outcome, Mark::Skipped, "Grace is signed out"));
    // Nobody was signed in here, so the account brought is the one to use.
    assert_eq!(outcome.activate.as_deref(), Some(ada.id.as_str()));
    assert_eq!(outcome.pairs.len(), 1);

    // Her history went into her own database, which no core is running
    // on; Grace has nowhere to keep hers.
    let merges = core.merges.borrow();
    assert_eq!(merges.len(), 1);
    assert_eq!(merges[0].1, folder.join(DATABASE));
    assert!(!merges[0].2);
    assert!(said(
        &outcome,
        Mark::Brought,
        "Ada: 5 plays brought. 2 plays were here already. 1 pin came with them."
    ));
    assert!(said(
        &outcome,
        Mark::Skipped,
        "Grace: the listening history was left"
    ));
    assert!(!outcome.history_changed);

    // The newest song of each folder first, until the cache was full.
    assert_eq!(*core.songs.borrow(), ["songBBBBBB2", "songAAAAAA1"]);
    assert_eq!(*core.limits.borrow(), [Some(2048), Some(2048)]);
    assert!(outcome.songs_changed);
    assert!(said(&outcome, Mark::Brought, "2 songs copied (0 MB)."));
    assert!(said(
        &outcome,
        Mark::Skipped,
        "1 song was left: the song cache is held to 2.0 GB"
    ));

    assert!(outcome.prefs.is_some());
    assert_eq!(outcome.done, ALL);
    let steps = steps.borrow();
    assert!(steps.iter().any(|p| p.total == 3 && p.done == 2));
}

#[test]
fn an_account_already_here_keeps_its_sign_in_and_takes_the_history_live() {
    let mut accounts = Accounts::default();
    let mut ada = new_account("Ada");
    ada.legacy = true;
    ada.avatar_url = "https://example.com/ada.png".into();
    accounts.add(ada.clone());
    let job = job("run-again", ALL, accounts);
    let core = Noted::default();
    let outcome = run(&job, &core, &|_| {});

    assert!(outcome.new_accounts.is_empty());
    assert_eq!(outcome.activate, None);
    assert!(said(
        &outcome,
        Mark::Skipped,
        "Ada is already signed in here."
    ));
    assert!(!job.root.join(CREDENTIALS).exists());
    // It is the account in use: the running core does the merging.
    let merges = core.merges.borrow();
    assert_eq!(merges[0].1, job.live_database);
    assert!(merges[0].2);
    assert!(outcome.history_changed);
    assert_eq!(
        outcome.pairs,
        [("aaaaaaaa-0000-0000-0000-000000000001".to_owned(), ada.id)]
    );
}

#[test]
fn only_what_is_ticked_is_brought() {
    let kinds = Kinds {
        history: true,
        ..Kinds::default()
    };
    let job = job("run-history-only", kinds, Accounts::default());
    let core = Noted::default();
    let outcome = run(&job, &core, &|_| {});
    // No sign-in was asked for, so there is no account to keep it in.
    assert!(outcome.new_accounts.is_empty());
    assert!(core.merges.borrow().is_empty() && core.songs.borrow().is_empty());
    assert!(said(&outcome, Mark::Skipped, "tick Accounts to add it"));
    assert_eq!(outcome.prefs, None);
    assert!(!job.root.join("accounts").exists());
}

#[test]
fn a_database_that_fails_is_said_and_does_not_stop_the_rest() {
    // Grace is here, so her databases are merged: her own, then the one
    // for her channel, which fails.
    let mut accounts = Accounts::default();
    let grace = new_account("Grace");
    accounts.keep(grace.clone());
    let mut job = job("run-failed", ALL, accounts);
    job.found.accounts[1].here = Some(grace.id);
    let outcome = run(&job, &Noted::default(), &|_| {});
    assert!(said(
        &outcome,
        Mark::Failed,
        "Grace: the listening history could not be brought"
    ));
    assert!(said(&outcome, Mark::Brought, "Ada: 5 plays brought."));
    assert!(said(&outcome, Mark::Brought, "songs copied"));
    assert_eq!(job.found.accounts[1].id, SECOND);
}

#[test]
fn songs_wait_for_a_core_that_is_running() {
    let kinds = Kinds {
        songs: true,
        ..Kinds::default()
    };
    let job = job("run-stopped", kinds, Accounts::default());
    let core = Noted {
        stopped: true,
        ..Noted::default()
    };
    let outcome = run(&job, &core, &|_| {});
    assert!(core.songs.borrow().is_empty());
    assert!(said(
        &outcome,
        Mark::Skipped,
        "the playback service is not running"
    ));
    assert!(!outcome.songs_changed);
}

#[test]
fn signed_out_listening_goes_to_the_root_unless_an_account_lives_there() {
    let old = old_profile("run-guest");
    let guest = old.join("guest");
    std::fs::create_dir_all(&guest).expect("a folder");
    std::fs::write(guest.join("spotifier.db"), "").expect("write");
    let root = scratch("run-guest-native");
    let kinds = Kinds {
        history: true,
        ..Kinds::default()
    };
    let job_with = |accounts: Accounts| Job {
        found: discover(&old, &accounts, &Record::default(), &history).expect("a profile"),
        kinds,
        live_database: root.join(DATABASE),
        root: root.clone(),
        accounts,
        cache_max_mb: 2048,
    };

    let core = Noted::default();
    let outcome = run(&job_with(Accounts::default()), &core, &|_| {});
    assert_eq!(core.merges.borrow()[0].0, guest.join("spotifier.db"));
    assert_eq!(core.merges.borrow()[0].1, root.join(DATABASE));
    assert!(said(
        &outcome,
        Mark::Brought,
        "Listening while signed out: 5 plays brought."
    ));

    let mut accounts = Accounts::default();
    accounts.keep(SavedAccount {
        legacy: true,
        ..new_account("Someone")
    });
    let core = Noted::default();
    let outcome = run(&job_with(accounts), &core, &|_| {});
    assert!(core.merges.borrow().is_empty());
    assert!(said(
        &outcome,
        Mark::Skipped,
        "an account here keeps its own history in that place"
    ));
}

#[test]
fn a_run_leaves_the_old_profile_as_it_was() {
    let job = job("run-untouched", ALL, Accounts::default());
    let before = sizes(&job.found.root);
    run(&job, &Noted::default(), &|_| {});
    assert_eq!(sizes(&job.found.root), before);
}

/// Every file under `folder` with its length and when it was written.
fn sizes(folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder).expect("a folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(sizes(&path));
        } else {
            let meta = entry.metadata().expect("metadata");
            out.push(format!(
                "{} {} {:?}",
                path.display(),
                meta.len(),
                meta.modified().ok()
            ));
        }
    }
    out.sort();
    out
}

#[test]
fn nothing_new_is_said_as_nothing_new() {
    let line = history_line(
        "Ada",
        Merged {
            plays_known: 590,
            ..Merged::default()
        },
    );
    assert_eq!(line.mark, Mark::Skipped);
    assert_eq!(
        line.text,
        "Ada: nothing new. All 590 plays were here already."
    );
    let line = history_line(
        "Ada",
        Merged {
            plays: 1,
            folders: 2,
            resume: true,
            ..Merged::default()
        },
    );
    assert_eq!(
        line.text,
        "Ada: 1 play brought. 2 folders came with them. The queue you left is back too."
    );
}

/// A core that merges whatever it is asked, and notes where to.
#[derive(Default)]
struct Mapped {
    merges: RefCell<Vec<(PathBuf, PathBuf, bool)>>,
}

impl Core for Mapped {
    fn merge(&self, from: &Path, into: &Path, live: bool) -> Result<Merged, String> {
        let noted = (from.to_path_buf(), into.to_path_buf(), live);
        self.merges.borrow_mut().push(noted);
        Ok(Merged {
            plays: 5,
            ..Merged::default()
        })
    }

    fn songs(&self, _: &Path, _: &[String], _: Option<u32>) -> Result<ImportedSongs, String> {
        Ok(ImportedSongs::default())
    }

    fn running(&self) -> bool {
        true
    }
}

const BAND: &str = "bbbbbbbb-0000-0000-0000-00000000000b";

/// The made-up profile, with Ada having acted as her band's channel too:
/// the list names the channel and the folder its database is in.
fn old_profile_with_a_channel(name: &str) -> PathBuf {
    let old = old_profile(name);
    let list = format!(
        r#"{{"version":1,"active":"aaaaaaaa-0000-0000-0000-000000000001","accounts":[
        {{"id":"aaaaaaaa-0000-0000-0000-000000000001","name":"Ada","legacy":true,"channel":"UCband",
          "channels":[{{"id":"","name":"Ada","handle":"@ada","localId":"aaaaaaaa-0000-0000-0000-00000000000a"}},
                      {{"id":"UCband","name":"The Band","handle":"@band","localId":"{BAND}"}}]}}]}}"#
    );
    std::fs::write(old.join("accounts.json"), list).expect("write");
    let theirs = old.join("channels").join(BAND);
    std::fs::create_dir_all(&theirs).expect("a folder");
    std::fs::write(theirs.join("spotifier.db"), "").expect("write");
    // And a folder the list does not name: nobody's that can be told.
    let stray = old.join("channels").join("stray");
    std::fs::create_dir_all(&stray).expect("a folder");
    std::fs::write(stray.join("spotifier.db"), "").expect("write");
    old
}

fn history_job(old: &Path, name: &str, accounts: Accounts) -> Job {
    let root = scratch(&format!("{name}-native"));
    let kinds = Kinds {
        sign_in: true,
        history: true,
        ..Kinds::default()
    };
    Job {
        found: discover(old, &accounts, &Record::default(), &history).expect("a profile"),
        kinds,
        live_database: root.join(DATABASE),
        root,
        accounts,
        cache_max_mb: 2048,
    }
}

/// Where the database at `from` went.
fn went(core: &Mapped, from: &Path) -> PathBuf {
    let merges = core.merges.borrow();
    let merge = merges.iter().find(|(merged, _, _)| merged == from);
    merge.expect("a merge of that database").1.clone()
}

#[test]
fn each_channels_history_goes_to_that_channel_here_and_not_into_one() {
    let old = old_profile_with_a_channel("run-channels");
    let job = history_job(&old, "run-channels", Accounts::default());
    let core = Mapped::default();
    let outcome = run(&job, &core, &|_| {});

    let ada = &outcome.new_accounts[0];
    let folder = accounts::folder_of(&job.root, Some(ada));
    // Her own listening is her own.
    assert_eq!(
        went(&core, &old.join("spotifier.db")),
        folder.join(DATABASE)
    );
    // The band's is the band's, in a folder of the same name as it had.
    let bands = folder.join("channels").join(BAND).join(DATABASE);
    let from = old.join("channels").join(BAND).join("spotifier.db");
    assert_eq!(went(&core, &from), bands);
    assert!(bands.parent().is_some_and(Path::is_dir));
    // The account made here knows that folder as the channel's, so the
    // channel finds its history when it is next acted as.
    assert_eq!(accounts::local_id_of(ada, "UCband"), Some(BAND));
    assert!(ada.channel_databases);
    // What is nobody's that can be told stays with the account.
    let stray = old.join("channels").join("stray").join("spotifier.db");
    assert_eq!(went(&core, &stray), folder.join(DATABASE));
    // Said as one line for the account, as before.
    assert!(said(&outcome, Mark::Brought, "Ada: 15 plays brought."));
}

#[test]
fn an_account_here_keeps_its_own_folder_for_a_channel_it_knows() {
    const OURS: &str = "cccccccc-0000-0000-0000-00000000000c";
    let old = old_profile_with_a_channel("run-known-channel");
    let mut ada = new_account("Ada");
    ada.channels = vec![SavedChannel {
        id: "UCband".into(),
        name: "The Band".into(),
        local_id: OURS.into(),
        ..SavedChannel::default()
    }];
    let mut accounts = Accounts::default();
    accounts.keep(ada.clone());
    let mut job = history_job(&old, "run-known-channel", accounts);
    job.found.accounts[0].here = Some(ada.id.clone());
    let core = Mapped::default();
    let outcome = run(&job, &core, &|_| {});

    let folder = accounts::folder_of(&job.root, Some(&ada));
    let from = old.join("channels").join(BAND).join("spotifier.db");
    assert_eq!(
        went(&core, &from),
        folder.join("channels").join(OURS).join(DATABASE)
    );
    // Nothing for the list to learn: it knew the folder.
    assert!(outcome.channels.is_empty());
}

#[test]
fn a_channel_the_account_here_has_no_folder_for_takes_the_old_one_and_the_list_is_told() {
    let old = old_profile_with_a_channel("run-new-channel");
    let ada = new_account("Ada");
    let mut accounts = Accounts::default();
    accounts.keep(ada.clone());
    let mut job = history_job(&old, "run-new-channel", accounts);
    job.found.accounts[0].here = Some(ada.id.clone());
    let core = Mapped::default();
    let outcome = run(&job, &core, &|_| {});

    let folder = accounts::folder_of(&job.root, Some(&ada));
    let from = old.join("channels").join(BAND).join("spotifier.db");
    assert_eq!(
        went(&core, &from),
        folder.join("channels").join(BAND).join(DATABASE)
    );
    assert_eq!(outcome.channels.len(), 1);
    let (account, channel) = &outcome.channels[0];
    assert_eq!((account, channel.id.as_str()), (&ada.id, "UCband"));
    assert_eq!(channel.local_id, BAND);

    // Taken into the list, the channel answers to that folder.
    let mut list = job.accounts.clone();
    list.adopt_channel(account, channel.clone());
    let known = list.get(&ada.id).expect("the account");
    assert_eq!(accounts::local_id_of(known, "UCband"), Some(BAND));
}

#[test]
fn an_account_here_that_still_keeps_one_database_takes_all_of_it_there() {
    // Its move to a database for each channel failed: nothing is put
    // where it would not be looked for.
    let old = old_profile_with_a_channel("run-one-database");
    let ada = SavedAccount {
        channel_databases: false,
        ..new_account("Ada")
    };
    let mut accounts = Accounts::default();
    accounts.keep(ada.clone());
    let mut job = history_job(&old, "run-one-database", accounts);
    job.found.accounts[0].here = Some(ada.id.clone());
    let core = Mapped::default();
    run(&job, &core, &|_| {});
    let own = accounts::folder_of(&job.root, Some(&ada)).join(DATABASE);
    assert!(core.merges.borrow().iter().all(|(_, into, _)| *into == own));
}
