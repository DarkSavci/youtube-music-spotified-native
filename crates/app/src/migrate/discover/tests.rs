//! Looking at a made-up profile: which accounts, how much history, how
//! many songs, and which of it is here already.

use std::cell::RefCell;

use super::*;
use crate::accounts::new_account;
use crate::migrate::Kinds;
use crate::migrate::tests::{SECOND, old_profile};

/// A count for every database: ten plays each.
fn ten_plays(_: &Path) -> Result<History, String> {
    Ok(History {
        plays: 10,
        first_play: "2026-09-22T10:26:25Z".into(),
        last_play: "2026-10-04T07:32:15Z".into(),
        ..History::default()
    })
}

fn look(root: &Path, native: &Accounts, record: &Record) -> Found {
    discover(root, native, record, &ten_plays).expect("a profile")
}

#[test]
fn every_account_is_found_with_its_sign_in_and_its_databases() {
    let root = old_profile("discover-accounts");
    let asked = RefCell::new(Vec::new());
    let inspect = |database: &Path| {
        asked.borrow_mut().push(database.to_path_buf());
        ten_plays(database)
    };
    let found =
        discover(&root, &Accounts::default(), &Record::default(), &inspect).expect("a profile");

    // The account whose id was edited into a path is not followed.
    let names: Vec<&str> = found.accounts.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Ada", "Grace"]);
    let (ada, grace) = (&found.accounts[0], &found.accounts[1]);
    assert!(ada.signed_in && ada.in_use);
    assert_eq!(ada.credentials, root.join("credentials.json"));
    assert_eq!(ada.databases, [root.join("spotifier.db")]);
    assert_eq!(ada.avatar_url, "https://example.com/ada.png");
    assert_eq!(ada.channels[0].handle, "@ada");
    assert_eq!(ada.history.plays, 10);

    // Signed out, and with a database for the channel she acted as.
    let folder = root.join("accounts").join(SECOND);
    assert!(!grace.signed_in && !grace.in_use);
    assert_eq!(
        grace.databases,
        [
            folder.join("spotifier.db"),
            folder.join("channels").join("c1").join("spotifier.db")
        ]
    );
    assert_eq!(grace.history.plays, 20);
    assert_eq!(asked.borrow().len(), 3);
    assert_eq!(found.history().plays, 30);
    assert_eq!(found.history().first_play, "2026-09-22T10:26:25Z");
}

#[test]
fn the_songs_of_every_account_are_counted_newest_first() {
    let root = old_profile("discover-songs");
    let found = look(&root, &Accounts::default(), &Record::default());
    assert_eq!(found.songs.count, 3);
    assert_eq!(found.songs.bytes, 1000);
    let (folder, ids) = &found.songs.folders[0];
    assert_eq!(folder, &root.join("audio-cache"));
    assert_eq!(ids, &["songBBBBBB2", "songAAAAAA1"]);
    assert_eq!(found.songs.folders[1].1, ["songDDDDDD4"]);
    assert_eq!(
        found.available(),
        Kinds {
            sign_in: true,
            history: true,
            songs: true,
            preferences: true
        }
    );
}

#[test]
fn an_account_already_here_is_recognised() {
    let root = old_profile("discover-here");
    let mut native = Accounts::default();
    // The same person by name and picture, signed in here some other way.
    let mut ada = new_account("Ada");
    ada.avatar_url = "https://example.com/ada.png".into();
    // The same name alone is not enough to be the same person.
    let grace = new_account("Grace");
    native.keep(ada.clone());
    native.keep(grace.clone());
    let found = look(&root, &native, &Record::default());
    assert_eq!(found.accounts[0].here.as_deref(), Some(ada.id.as_str()));
    assert_eq!(found.accounts[1].here, None);

    // What an earlier run paired is paired still, whatever the names say.
    let mut record = Record::default();
    record.accounts.insert(SECOND.into(), grace.id.clone());
    record.accounts.insert("gone".into(), "nobody".into());
    let found = look(&root, &native, &record);
    assert_eq!(found.accounts[1].here.as_deref(), Some(grace.id.as_str()));
    // Ada is here, and Grace has no sign-in: nothing to tick for accounts.
    assert!(!found.suggested(Kinds::default()).sign_in);
}

#[test]
fn listening_while_signed_out_is_an_account_of_its_own() {
    let root = old_profile("discover-guest");
    let guest = root.join("guest");
    std::fs::create_dir_all(&guest).expect("a folder");
    std::fs::write(guest.join("spotifier.db"), "").expect("write");
    let found = look(&root, &Accounts::default(), &Record::default());
    let last = found.accounts.last().expect("accounts");
    assert!(last.guest && !last.signed_in);
    assert_eq!(found.people().count(), 2);
}

#[test]
fn a_database_that_cannot_be_read_is_said_and_the_rest_is_still_found() {
    let root = old_profile("discover-unread");
    let refuse = |_: &Path| Err("locked".to_owned());
    let found =
        discover(&root, &Accounts::default(), &Record::default(), &refuse).expect("a profile");
    assert_eq!(found.accounts[0].unread.as_deref(), Some("locked"));
    assert_eq!(found.history().plays, 0);
    assert_eq!(found.songs.count, 3);
}

#[test]
fn a_profile_from_before_there_were_accounts_has_the_one_in_its_root() {
    let root = old_profile("discover-first");
    std::fs::remove_file(root.join("accounts.json")).expect("remove");
    let found = look(&root, &Accounts::default(), &Record::default());
    assert_eq!(found.accounts.len(), 1);
    assert!(found.accounts[0].signed_in);
    assert_eq!(found.accounts[0].name, "Saved account");
}

#[test]
fn no_profile_or_an_empty_one_is_nothing_found() {
    let root = old_profile("discover-none");
    let none = discover(
        &root.join("nowhere"),
        &Accounts::default(),
        &Record::default(),
        &ten_plays,
    );
    assert_eq!(none, None);
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).expect("a folder");
    let none = discover(&empty, &Accounts::default(), &Record::default(), &ten_plays);
    assert_eq!(none, None);
}

#[test]
fn looking_changes_nothing_in_the_old_profile() {
    let root = old_profile("discover-untouched");
    let before = listing(&root);
    look(&root, &Accounts::default(), &Record::default());
    assert_eq!(listing(&root), before);
}

/// Every file under `folder` with its length and when it was written.
fn listing(folder: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(folder).expect("a folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(listing(&path));
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
