//! A made-up profile of the Electron app to try the move against, and the
//! small rules of this module.

use std::path::{Path, PathBuf};

use super::leveldb::tests::{log_file, scratch};
use super::*;

/// The id of the made-up profile's second account.
pub const SECOND: &str = "0f0f0f0f-1111-2222-3333-444444444444";

pub const SETTINGS: &str = r#"{"state":{"crossfadeMs":3000,"gapless":false,"normalization":true,
    "normalizationLevel":"loud","showMusicVideos":true,"enginePreference":"native","theme":"system",
    "reduceMotion":true,"resumeOnLaunch":false,"continueFromYouTubeMusic":true,"closeToTray":false,
    "reportToYouTube":false,"volumeBoost":true,"cacheMaxMB":10240,"autoplay":false,
    "showQualityBadge":true,"timedLyrics":true,"remainingTime":true,"playbackSpeed":1.25,
    "eq":[-1,0,0,3,6]},"version":2}"#;

pub const ROOMS: &str = r#"{"state":{"servers":[{"id":"d9456a9c","name":"Ours",
    "url":"wss://listen.example.com/"},{"id":"bad","name":"Plain","url":"http://example.com"}],
    "selected":"d9456a9c","roomName":"Friday","mode":"contributions","name":"Ada",
    "avatar":"https://example.com/a.png","followVideo":false,"notifications":true},"version":0}"#;

/// A value as Chromium stores a string of plain Latin letters.
fn latin(text: &str) -> Vec<u8> {
    let mut stored = vec![1];
    stored.extend(text.bytes());
    stored
}

/// A value as Chromium stores a string with letters beyond Latin-1.
fn wide(text: &str) -> Vec<u8> {
    let mut stored = vec![0];
    stored.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    stored
}

fn key(origin: &str, name: &str) -> Vec<u8> {
    let mut key = format!("_{origin}\0").into_bytes();
    key.extend(latin(name));
    key
}

/// Writes a Local Storage holding these names and values for the
/// installed app's page, and a stray one for a development build's.
pub fn write_storage(root: &Path, values: &[(&str, Vec<u8>)]) {
    let folder = root.join("Local Storage").join("leveldb");
    std::fs::create_dir_all(&folder).expect("a folder");
    let keys: Vec<Vec<u8>> = values
        .iter()
        .map(|(name, _)| key("file://", name))
        .collect();
    let mut writes: Vec<(&[u8], Option<&[u8]>)> = keys
        .iter()
        .zip(values)
        .map(|(key, (_, value))| (key.as_slice(), Some(value.as_slice())))
        .collect();
    let stray = key("http://127.0.0.1:5219", "spotifier.lastSeenVersion");
    let stray_value = latin("0.1.9");
    writes.push((&stray, Some(&stray_value)));
    let meta: &[u8] = b"META:file://";
    writes.push((meta, Some(b"\x08\x01")));
    std::fs::write(folder.join("000003.log"), log_file(&[(1, writes)])).expect("write");
}

fn write(path: PathBuf, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("a folder");
    }
    std::fs::write(path, text).expect("write");
}

/// A song kept on disk: its record and `bytes` of audio.
pub fn write_song(folder: &Path, id: &str, bytes: usize, used_at: &str) {
    let record = format!(
        r#"{{"mimeType":"audio/webm","size":{bytes},"have":{bytes},"usedAt":"{used_at}"}}"#
    );
    write(folder.join(format!("{id}.json")), &record);
    std::fs::write(folder.join(format!("{id}.audio")), vec![7; bytes]).expect("write");
}

/// A profile as the Electron app leaves one: Ada, the first account, in
/// the root and in use; Grace in a folder of her own, signed out; songs,
/// preferences and the mini player's place.
pub fn old_profile(name: &str) -> PathBuf {
    let root = scratch(name).join("Spotifier");
    let accounts = format!(
        r#"{{"version":1,"active":"aaaaaaaa-0000-0000-0000-000000000001","accounts":[
        {{"id":"aaaaaaaa-0000-0000-0000-000000000001","name":"Ada","legacy":true,"channel":"",
          "avatarUrl":"https://example.com/ada.png",
          "channels":[{{"id":"","name":"Ada","handle":"@ada","localId":"x","avatarUrl":"y"}}]}},
        {{"id":"{SECOND}","name":"Grace","legacy":false,"channel":"","channels":[]}},
        {{"id":"../../escape","name":"Mallory","legacy":false}}]}}"#
    );
    write(root.join("accounts.json"), &accounts);
    write(
        root.join("credentials.json"),
        r#"{"cookie":"SID=made-up; SAPISID=made-up"}"#,
    );
    write(root.join("yt-dlp-cookies.txt"), "# made up\n");
    write(root.join("spotifier.db"), "");
    let grace = root.join("accounts").join(SECOND);
    write(
        grace.join("credentials.json"),
        r#"{"cookie":"CONSENT=yes"}"#,
    );
    write(grace.join("spotifier.db"), "");
    write(grace.join("channels").join("c1").join("spotifier.db"), "");
    write_song(
        &root.join("audio-cache"),
        "songAAAAAA1",
        300,
        "2026-09-01T10:00:00Z",
    );
    write_song(
        &root.join("audio-cache"),
        "songBBBBBB2",
        500,
        "2026-10-01T10:00:00Z",
    );
    // A record whose audio never landed is not a song.
    write(root.join("audio-cache").join("songCCCCCC3.json"), "{}");
    write_song(
        &grace.join("audio-cache"),
        "songDDDDDD4",
        200,
        "2026-08-01T10:00:00Z",
    );
    write(
        root.join("miniplayer.json"),
        r#"{"bounds":{"x":2174,"y":34,"width":360,"height":267},"alwaysOnTop":false}"#,
    );
    write_storage(
        &root,
        &[
            ("spotifier.settings", latin(SETTINGS)),
            ("spotifier.rooms.v2", latin(ROOMS)),
            ("sidebar.width", latin("320")),
            ("spotifier.recentSearches", latin(r#"["older","duman"]"#)),
            (
                "spotifier.recentSearches.legacy:personal",
                wide(r#"["çökertme","Duman"]"#),
            ),
        ],
    );
    root
}

#[test]
fn a_time_is_shown_as_the_day_it_falls_on() {
    assert_eq!(day("2026-09-22T10:26:25Z"), "22 Sep 2026");
    assert_eq!(day("2026-10-04T07:32:15Z"), "4 Oct 2026");
    assert_eq!(day(""), "");
    assert_eq!(day("2026-00-04T00:00:00Z"), "");
    assert_eq!(day("not a time at all"), "");
}

#[test]
fn the_histories_of_several_databases_add_up_to_one() {
    let mut all = History::default();
    all.add(&History {
        plays: 10,
        first_play: "2026-09-22T10:00:00Z".into(),
        last_play: "2026-09-30T10:00:00Z".into(),
        pins: 1,
        ..History::default()
    });
    all.add(&History::default());
    all.add(&History {
        plays: 5,
        first_play: "2026-09-01T10:00:00Z".into(),
        last_play: "2026-09-02T10:00:00Z".into(),
        resume: true,
        ..History::default()
    });
    assert_eq!(all.plays, 15);
    assert_eq!(all.first_play, "2026-09-01T10:00:00Z");
    assert_eq!(all.last_play, "2026-09-30T10:00:00Z");
    assert!(all.resume && all.pins == 1);
}

#[test]
fn a_demo_looks_for_no_old_profile_unless_told_where() {
    assert_eq!(old_profile_at(None, true), None);
    let named = Path::new("copy");
    assert_eq!(old_profile_at(Some(named), true), Some(named.to_path_buf()));
    assert!(old_profile_at(None, false).is_some_and(|path| path.ends_with("Spotifier")));
}

fn old_profile_at(named: Option<&Path>, demo: bool) -> Option<PathBuf> {
    super::old_profile(named, demo)
}

#[test]
fn what_is_ticked_to_begin_with_is_what_is_not_here_yet() {
    let mut found = Found {
        accounts: vec![OldAccount {
            name: "Ada".into(),
            signed_in: true,
            history: History {
                plays: 3,
                ..History::default()
            },
            ..OldAccount::default()
        }],
        ..Found::default()
    };
    found.prefs.sidebar_width = Some(300.0);
    let all_but_songs = Kinds {
        sign_in: true,
        history: true,
        songs: false,
        preferences: true,
    };
    assert_eq!(found.available(), all_but_songs);
    assert_eq!(found.suggested(Kinds::default()), all_but_songs);

    // Brought once: the account is here, and the preferences are this
    // app's own now. The history is still worth another look.
    found.accounts[0].here = Some("ours".into());
    let before = Kinds {
        preferences: true,
        ..Kinds::default()
    };
    let again = found.suggested(before);
    assert_eq!(
        again,
        Kinds {
            history: true,
            ..Kinds::default()
        }
    );
}
