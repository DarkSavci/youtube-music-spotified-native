//! The Electron app's preferences, read from a made-up profile and taken
//! into this app's settings.

use super::*;
use crate::migrate::tests::old_profile;

fn theirs(name: &str) -> OldPrefs {
    read(&old_profile(name), "legacy:personal")
}

#[test]
fn the_preferences_are_read_from_the_page_and_the_shell() {
    let prefs = theirs("prefs-read");
    assert_eq!(prefs.unread, None);
    assert!(prefs.settings.is_some() && prefs.rooms.is_some() && prefs.mini.is_some());
    assert_eq!(prefs.sidebar_width, Some(320.0));
    // The account in use's searches first, then the rest, each once
    // whatever its case; letters beyond Latin-1 survive.
    assert_eq!(prefs.searches, ["çökertme", "Duman", "older"]);
    assert_eq!(prefs.cache_max_mb(), Some(10_240));
    assert_eq!(
        prefs.summary(),
        "Playback and window settings, 2 Listen Together servers, 3 recent searches, \
         the mini player's place"
    );
}

#[test]
fn each_setting_lands_on_its_counterpart_here() {
    let prefs = theirs("prefs-apply");
    let mut settings = Settings::default();
    let changed = apply(&mut settings, &prefs, "");

    assert_eq!(settings.crossfade_seconds, 3);
    assert!(!settings.gapless);
    assert!(settings.normalise_volume);
    assert_eq!(settings.volume_level, VolumeLevel::Loud);
    assert_eq!(settings.theme, Choice::System);
    assert!(settings.reduce_motion);
    assert!(!settings.resume_on_launch);
    assert!(settings.continue_from_youtube_music);
    assert!(!settings.close_to_tray);
    assert!(!settings.report_to_youtube);
    assert!(settings.volume_boost);
    assert_eq!(settings.cache_max_mb, 10_240);
    assert!(!settings.autoplay);
    assert!(settings.remaining_time);
    assert_eq!(settings.playback_speed, 1.25);
    assert!(settings.equalizer_on);
    assert!(!settings.sidebar_collapsed);
    assert!(!settings.mini_on_top);
    assert_eq!(settings.mini_size, [360.0, 267.0]);
    assert_eq!(settings.mini_position, Some([2174.0, 34.0]));
    assert_eq!(settings.recent_searches, ["çökertme", "Duman", "older"]);

    // Normalisation was on in both, so it is not among what changed.
    assert!(changed.contains(&"crossfade".to_owned()));
    assert!(changed.contains(&"equalizer".to_owned()));
    assert!(!changed.contains(&"volume normalisation".to_owned()));
}

#[test]
fn bringing_the_same_preferences_again_changes_nothing() {
    let prefs = theirs("prefs-twice");
    let mut settings = Settings::default();
    assert!(!apply(&mut settings, &prefs, "").is_empty());
    let once = settings.clone();
    assert_eq!(apply(&mut settings, &prefs, ""), Vec::<String>::new());
    assert_eq!(settings, once);
}

#[test]
fn listen_together_servers_join_those_saved_here() {
    let prefs = theirs("prefs-rooms");
    let mut settings = Settings::default();
    settings.together_servers.push(SavedServer {
        id: "server-1".into(),
        name: "Mine".into(),
        url: "wss://mine.example.com".into(),
    });
    settings.together_selected = "server-1".into();
    apply(&mut settings, &prefs, "");

    // The address no room could be reached at is left behind.
    let names: Vec<&str> = settings
        .together_servers
        .iter()
        .map(|server| server.name.as_str())
        .collect();
    assert_eq!(names, ["Mine", "Ours"]);
    // The server chosen here stays chosen.
    assert_eq!(settings.together_selected, "server-1");
    assert_eq!(settings.together_name, "Ada");
    assert_eq!(settings.together_room_name, "Friday");
    assert_eq!(settings.together_mode, Mode::Contributions);
    assert!(settings.together_share_picture && settings.together_notifications);

    // With none chosen here, the one chosen there is.
    let mut fresh = Settings::default();
    apply(&mut fresh, &prefs, "");
    assert_eq!(fresh.together_selected, "d9456a9c");
    assert_eq!(
        fresh.together_server().map(|s| s.name.as_str()),
        Some("Ours")
    );
}

#[test]
fn searches_made_here_stay_ahead_of_those_brought() {
    let prefs = theirs("prefs-searches");
    let mut settings = Settings {
        recent_searches: (1..=7).map(|n| format!("mine {n}")).collect(),
        ..Settings::default()
    };
    apply(&mut settings, &prefs, "");
    assert_eq!(settings.recent_searches.len(), RECENT_KEPT);
    assert_eq!(settings.recent_searches[0], "mine 1");
    assert_eq!(settings.recent_searches[7], "çökertme");
}

#[test]
fn five_bands_become_ten_along_the_same_curve() {
    let gains = ten_bands(&[-1.0, 0.0, 0.0, 3.0, 6.0]).expect("a curve");
    assert_eq!(gains, [-1.0, -1.0, -0.5, 0.0, 0.0, 0.0, 1.5, 3.0, 5.0, 6.0]);
    // A flat curve is no equalizer, and is not taken for one.
    assert_eq!(ten_bands(&[0.0; 5]), None);
    assert_eq!(ten_bands(&[1.0, 2.0]), None);
    let loud = ten_bands(&[40.0, 0.0, 0.0, 0.0, 0.0]).expect("a curve");
    assert_eq!(loud[0], spotified_audio::eq::RANGE_DB);
}

#[test]
fn a_flat_curve_leaves_the_equalizer_here_alone() {
    let mut prefs = theirs("prefs-flat");
    if let Some(settings) = &mut prefs.settings {
        settings.eq = Some(vec![0.0; 5]);
    }
    let mut settings = Settings::default();
    settings.equalizer[0] = 4.0;
    apply(&mut settings, &prefs, "");
    assert_eq!(settings.equalizer[0], 4.0);
    assert!(!settings.equalizer_on);
}

#[test]
fn a_narrow_sidebar_there_is_the_rail_here() {
    let prefs = OldPrefs {
        sidebar_width: Some(72.0),
        ..OldPrefs::default()
    };
    let mut settings = Settings::default();
    let width = settings.sidebar_width;
    assert_eq!(apply(&mut settings, &prefs, ""), ["sidebar"]);
    assert!(settings.sidebar_collapsed);
    assert_eq!(settings.sidebar_width, width);
}

#[test]
fn a_profile_with_no_storage_still_gives_the_mini_player_and_says_so() {
    let root = old_profile("prefs-unread");
    std::fs::remove_dir_all(root.join("Local Storage")).expect("remove");
    let prefs = read(&root, "legacy:personal");
    assert!(prefs.unread.is_some());
    assert!(prefs.mini.is_some() && prefs.settings.is_none());
    assert!(!prefs.is_empty());
}

#[test]
fn settings_kept_by_a_development_build_are_used_when_no_others_are() {
    let root = old_profile("prefs-origin");
    let folder = root.join("Local Storage").join("leveldb");
    let key = b"_http://127.0.0.1:5219\0\x01spotifier.settings".to_vec();
    let mut value = vec![1];
    value.extend(br#"{"state":{"crossfadeMs":9000},"version":2}"#);
    let log = crate::migrate::leveldb::tests::log_file(&[(1, vec![(&key, Some(&value))])]);
    std::fs::write(folder.join("000003.log"), log).expect("write");
    let prefs = read(&root, "guest");
    let mut settings = Settings::default();
    apply(&mut settings, &prefs, "");
    assert_eq!(settings.crossfade_seconds, 9);
}
