//! Preferences that survive a restart.
//!
//! The file is read tolerantly: unknown fields are ignored and missing ones
//! take their defaults, so an older or newer build never loses the rest. A
//! file that cannot be parsed is set aside rather than overwritten, since
//! overwriting it with defaults would destroy whatever the person had.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::theme;

/// How the library is ordered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibrarySort {
    /// Most recently played first.
    #[default]
    Recent,
    /// Most recently saved first.
    Added,
    /// By title. Settings written before there were four orders call it
    /// `name`.
    #[serde(alias = "name")]
    Alphabetical,
    /// By whose it is, then by title.
    Creator,
}

impl LibrarySort {
    pub const EVERY: [LibrarySort; 4] = [
        LibrarySort::Recent,
        LibrarySort::Added,
        LibrarySort::Alphabetical,
        LibrarySort::Creator,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LibrarySort::Recent => "Recents",
            LibrarySort::Added => "Recently added",
            LibrarySort::Alphabetical => "Alphabetical",
            LibrarySort::Creator => "Creator",
        }
    }
}

/// The queue and the lyrics share the right edge; one is open at a time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RightPanel {
    #[default]
    Closed,
    Queue,
    Lyrics,
}

impl RightPanel {
    /// What the edge shows after `panel`'s button is pressed: that panel,
    /// or nothing if it was already open.
    pub fn toggled(self, panel: RightPanel) -> RightPanel {
        if self == panel {
            RightPanel::Closed
        } else {
            panel
        }
    }
}

/// What evening out the loudness aims for. Three levels and not a slider,
/// because the choice is between kinds of compromise: quiet leaves headroom
/// for dynamics, loud matches everything else and spends it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeLevel {
    Quiet,
    #[default]
    Normal,
    Loud,
}

impl VolumeLevel {
    pub const EVERY: [VolumeLevel; 3] =
        [VolumeLevel::Quiet, VolumeLevel::Normal, VolumeLevel::Loud];

    pub fn label(self) -> &'static str {
        match self {
            VolumeLevel::Quiet => "Quiet",
            VolumeLevel::Normal => "Normal",
            VolumeLevel::Loud => "Loud",
        }
    }

    /// The loudness tracks are brought to, in LUFS: the same three targets
    /// the old app, and Spotify, offer.
    pub fn lufs(self) -> f32 {
        match self {
            VolumeLevel::Quiet => -19.0,
            VolumeLevel::Normal => -14.0,
            VolumeLevel::Loud => -11.0,
        }
    }
}

/// The sizes the song cache can be held to, in megabytes.
pub const CACHE_SIZES_MB: [u32; 5] = [512, 1024, 2048, 5120, 10_240];

/// The slowest and fastest playback, and the step between speeds.
pub const SPEED_RANGE: (f32, f32) = (0.5, 3.0);
pub const SPEED_STEP: f32 = 0.05;

/// A speed held to the range and rounded to a step, so that floating-point
/// drift never shows in the label.
pub fn clamp_speed(speed: f32) -> f32 {
    if !speed.is_finite() {
        return 1.0;
    }
    let stepped = (speed / SPEED_STEP).round() * SPEED_STEP;
    (stepped.clamp(SPEED_RANGE.0, SPEED_RANGE.1) * 100.0).round() / 100.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The sidebar is a narrow rail of covers.
    pub sidebar_collapsed: bool,
    pub sidebar_width: f32,
    /// The order the library is listed in.
    pub library_sort: LibrarySort,
    /// The sidebar shows the library as a grid of covers rather than rows.
    pub library_grid: bool,
    /// As `library_grid`, for the library given the page's room: there a
    /// grid is the usual way.
    pub library_expanded_grid: bool,
    /// What is open on the right.
    pub panel: RightPanel,
    /// Closing the window keeps the app, and the music, in the tray.
    pub close_to_tray: bool,
    /// Light, dark, or whichever the system is set to.
    pub theme: crate::themes::Choice,
    /// A theme file in the themes folder, worn instead when it is there.
    pub custom_theme: Option<String>,
    /// Keep the mini player above other windows.
    pub mini_on_top: bool,
    /// The mini player's size, and where it was, as it was left.
    pub mini_size: [f32; 2],
    pub mini_position: Option<[f32; 2]>,
    /// The one relay address a profile kept before servers were saved by
    /// name. Read so that it can be brought along; never written again.
    #[serde(skip_serializing)]
    pub together_server: String,
    /// The Listen Together relays saved here, and the id of the one chosen.
    pub together_servers: Vec<crate::together::SavedServer>,
    pub together_selected: String,
    /// The name shown to others in a room.
    pub together_name: String,
    /// Show the account's picture to a room as well.
    pub together_share_picture: bool,
    /// What the next room made here is called, and who steers it.
    pub together_room_name: String,
    pub together_mode: crate::together::Mode,
    /// Say what happens in a room as it happens.
    pub together_notifications: bool,
    /// Show or hide the video when someone who steers the room does.
    pub together_follow_video: bool,
    /// The version that last ran here, to notice an update by.
    pub last_seen_version: String,
    /// The newest release whose notes have been opened.
    pub release_notes_read: String,
    /// What was searched for here lately, newest first. A convenience for
    /// whoever is at this keyboard; the account keeps a history of its own.
    pub recent_searches: Vec<String>,
    /// The same for each account and channel that has searched here, by
    /// its scope: what one of them looked for is not shown to the next.
    /// `recent_searches` is what is left: the list of nobody signed in,
    /// and, until any account has one of its own, the list from before
    /// they were kept apart.
    pub recent_searches_by: std::collections::BTreeMap<String, Vec<String>>,
    /// Leave the window's frame to the system rather than drawing the
    /// title bar in the app.
    pub system_title_bar: bool,
    /// Even out loudness between tracks.
    pub normalise_volume: bool,
    /// How loud that evening out leaves them.
    pub volume_level: VolumeLevel,
    /// Let the volume go past 100%, up to 200%. Off by default: past full
    /// scale a limiter has to hold the peaks down, which flattens loud
    /// passages.
    pub volume_boost: bool,
    /// Start the next song the moment this one ends.
    pub gapless: bool,
    /// Keep playing similar songs once the queue runs out.
    pub autoplay: bool,
    /// Bring the last queue back, paused, when the app starts.
    pub resume_on_launch: bool,
    /// At launch, pick up the queue the account has on another device. Off
    /// by default: it replaces the queue kept here, and costs a request on
    /// every launch.
    pub continue_from_youtube_music: bool,
    /// Count plays towards the account's YouTube history. The one setting
    /// that writes to the account.
    pub report_to_youtube: bool,
    /// The most the kept songs may take on disk, in megabytes.
    pub cache_max_mb: u32,
    /// Do without animation.
    pub reduce_motion: bool,
    /// Show music videos among the songs of shelves and lists. Off, as in
    /// the Electron app: this is a player of songs first.
    pub show_music_videos: bool,
    /// Show the time left, not the length, at the end of the seek bar.
    pub remaining_time: bool,
    /// How fast playback runs, as a multiple of normal. Kept across songs
    /// and launches: it is chosen for a kind of listening, and resetting it
    /// on every song would undo that.
    pub playback_speed: f32,
    /// Draw the music's spectrum behind the player bar.
    pub visualizer: bool,
    /// Seconds one song fades into the next; 0 for none.
    pub crossfade_seconds: u32,
    pub equalizer_on: bool,
    /// Decibels for each of the equalizer's ten bands.
    pub equalizer: [f32; 10],
    /// Decibels the equalizer raises or lowers everything by.
    pub equalizer_preamp: f32,
    /// Take off the level what the curve adds to it, so nothing clips.
    pub equalizer_headroom: bool,
    /// Curves saved under names of the listener's own.
    pub equalizer_presets: Vec<crate::equalizer::Saved>,
    /// How the core knows this installation among its devices. Made up on
    /// the first run and kept.
    pub device_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sidebar_collapsed: false,
            sidebar_width: theme::SIDEBAR_WIDTH,
            library_sort: LibrarySort::Recent,
            library_grid: false,
            library_expanded_grid: true,
            panel: RightPanel::Closed,
            close_to_tray: true,
            system_title_bar: false,
            theme: crate::themes::Choice::Dark,
            custom_theme: None,
            mini_on_top: true,
            mini_size: [360.0, 360.0],
            mini_position: None,
            together_server: String::new(),
            together_servers: Vec::new(),
            together_selected: String::new(),
            together_name: String::new(),
            together_share_picture: false,
            together_room_name: String::new(),
            together_mode: crate::together::Mode::Collaborative,
            together_notifications: false,
            together_follow_video: false,
            last_seen_version: String::new(),
            release_notes_read: String::new(),
            recent_searches: Vec::new(),
            recent_searches_by: std::collections::BTreeMap::new(),
            normalise_volume: true,
            volume_level: VolumeLevel::Normal,
            volume_boost: false,
            gapless: true,
            autoplay: true,
            resume_on_launch: true,
            continue_from_youtube_music: false,
            report_to_youtube: true,
            cache_max_mb: 2048,
            reduce_motion: false,
            show_music_videos: false,
            remaining_time: false,
            playback_speed: 1.0,
            visualizer: false,
            // Six seconds: the length that reads as one song becoming
            // another, not as a dip between them.
            crossfade_seconds: DEFAULT_CROSSFADE_SECONDS,
            equalizer_on: false,
            equalizer: [0.0; 10],
            equalizer_preamp: 0.0,
            equalizer_headroom: true,
            equalizer_presets: Vec::new(),
            device_id: String::new(),
        }
    }
}

/// The crossfade a profile starts with.
pub const DEFAULT_CROSSFADE_SECONDS: u32 = 6;

impl Settings {
    /// What was searched for here lately by `scope`: an account and the
    /// channel it acts as, or nobody (the empty scope). The list from
    /// before searches were kept apart is shown to whoever is here, until
    /// the first of them searches and so takes it.
    pub fn recent_searches(&self, scope: &str) -> &[String] {
        match self.recent_searches_by.get(scope) {
            Some(theirs) => theirs,
            None if scope.is_empty() || self.recent_searches_by.is_empty() => &self.recent_searches,
            None => &[],
        }
    }

    /// The same, to change. The first account to change its list takes
    /// the one from before they were kept apart.
    pub fn recent_searches_mut(&mut self, scope: &str) -> &mut Vec<String> {
        if scope.is_empty() {
            return &mut self.recent_searches;
        }
        let from_before = if self.recent_searches_by.is_empty() {
            std::mem::take(&mut self.recent_searches)
        } else {
            Vec::new()
        };
        self.recent_searches_by
            .entry(scope.to_owned())
            .or_insert(from_before)
    }
    /// The saved Listen Together server that is chosen, if one is.
    pub fn together_server(&self) -> Option<&crate::together::SavedServer> {
        self.together_servers
            .iter()
            .find(|server| server.id == self.together_selected)
    }

    /// The most the volume can be set to: 2 with boost, otherwise 1.
    pub fn max_volume(&self) -> f32 {
        if self.volume_boost { 2.0 } else { 1.0 }
    }

    /// Puts everything the Settings page offers back as a new profile has
    /// it. What the page does not show stays: who this device is, where
    /// the windows were left, what has been searched for.
    pub fn reset_preferences(&mut self) {
        let fresh = Settings::default();
        *self = Settings {
            sidebar_collapsed: self.sidebar_collapsed,
            sidebar_width: self.sidebar_width,
            library_sort: self.library_sort,
            library_grid: self.library_grid,
            library_expanded_grid: self.library_expanded_grid,
            panel: self.panel,
            mini_on_top: self.mini_on_top,
            mini_size: self.mini_size,
            mini_position: self.mini_position,
            together_servers: std::mem::take(&mut self.together_servers),
            together_selected: std::mem::take(&mut self.together_selected),
            together_name: std::mem::take(&mut self.together_name),
            together_share_picture: self.together_share_picture,
            together_room_name: std::mem::take(&mut self.together_room_name),
            together_mode: self.together_mode,
            together_notifications: self.together_notifications,
            together_follow_video: self.together_follow_video,
            last_seen_version: std::mem::take(&mut self.last_seen_version),
            release_notes_read: std::mem::take(&mut self.release_notes_read),
            recent_searches: std::mem::take(&mut self.recent_searches),
            recent_searches_by: std::mem::take(&mut self.recent_searches_by),
            equalizer_presets: std::mem::take(&mut self.equalizer_presets),
            device_id: std::mem::take(&mut self.device_id),
            ..fresh
        };
    }
}

/// A new device id: random enough to be unique, with no need to be secret.
pub fn new_device_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    // The standard library seeds each hasher state from the system's
    // randomness, which is all the randomness this needs.
    let random = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    format!("native-{random:016x}")
}

pub fn load(path: &Path) -> Settings {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Settings::default(),
        Err(error) => {
            log::warn!("settings could not be read, using defaults: {error}");
            return Settings::default();
        }
    };
    // Some editors save a byte-order mark, which is not JSON.
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<Settings>(text) {
        Ok(mut settings) => {
            // A profile from before there was a crossfade has had none, and
            // keeps none: the six seconds are for a new profile only.
            if !text.contains("\"crossfade_seconds\"") {
                settings.crossfade_seconds = 0;
            }
            settings.playback_speed = clamp_speed(settings.playback_speed);
            crate::equalizer::hold(&mut settings, text);
            crate::together::servers::adopt(
                &mut settings.together_servers,
                &mut settings.together_selected,
                &mut settings.together_server,
            );
            settings
        }
        Err(error) => {
            log::warn!("settings are not valid, kept aside as settings.json.bad: {error}");
            let _ = std::fs::rename(path, path.with_extension("json.bad"));
            Settings::default()
        }
    }
}

/// Writes to a temporary file and renames it into place, so a crash mid-write
/// leaves the old settings rather than half a file.
pub fn save(path: &Path, settings: &Settings) -> io::Result<()> {
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> io::Result<std::path::PathBuf> {
        let dir = std::env::temp_dir().join(format!("spotified-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("settings.json"))
    }

    #[test]
    fn saved_settings_come_back() -> io::Result<()> {
        let path = scratch("roundtrip")?;
        let settings = Settings {
            sidebar_collapsed: true,
            sidebar_width: 320.0,
            library_sort: LibrarySort::Creator,
            library_grid: true,
            library_expanded_grid: false,
            panel: RightPanel::Lyrics,
            close_to_tray: false,
            system_title_bar: true,
            theme: crate::themes::Choice::Light,
            custom_theme: Some("nord.json".into()),
            mini_on_top: false,
            mini_size: [640.0, 280.0],
            mini_position: Some([40.0, 60.0]),
            together_server: String::new(),
            together_servers: vec![crate::together::SavedServer {
                id: "server-1".into(),
                name: "Ours".into(),
                url: "wss://listen.example.com".into(),
            }],
            together_selected: "server-1".into(),
            together_name: "Ada".into(),
            together_share_picture: true,
            together_room_name: "Friday night".into(),
            together_mode: crate::together::Mode::Listen,
            together_notifications: true,
            together_follow_video: true,
            last_seen_version: "0.1.0".into(),
            release_notes_read: "0.1.0".into(),
            recent_searches: vec!["bonobo".into(), "air".into()],
            recent_searches_by: [("a:personal".to_owned(), vec!["moby".to_owned()])].into(),
            normalise_volume: false,
            volume_level: VolumeLevel::Loud,
            volume_boost: true,
            gapless: false,
            autoplay: false,
            resume_on_launch: false,
            continue_from_youtube_music: true,
            report_to_youtube: false,
            cache_max_mb: 5120,
            reduce_motion: true,
            show_music_videos: true,
            remaining_time: true,
            playback_speed: 1.25,
            visualizer: true,
            crossfade_seconds: 3,
            equalizer_on: true,
            equalizer: [3.0; 10],
            equalizer_preamp: -2.0,
            equalizer_headroom: false,
            equalizer_presets: Vec::new(),
            device_id: "native-1".into(),
        };
        save(&path, &settings)?;
        assert_eq!(load(&path), settings);
        Ok(())
    }

    #[test]
    fn the_relay_an_older_profile_kept_is_saved_under_its_host() -> io::Result<()> {
        let path = scratch("relay")?;
        std::fs::write(&path, "{\"together_server\": \"ws://localhost:8791\"}")?;
        let settings = load(&path);
        assert!(settings.together_server.is_empty());
        let chosen = settings.together_server().map(|server| server.url.as_str());
        assert_eq!(chosen, Some("ws://localhost:8791"));
        assert_eq!(settings.together_servers[0].name, "localhost:8791");
        Ok(())
    }

    #[test]
    fn a_missing_field_takes_its_default() -> io::Result<()> {
        let path = scratch("partial")?;
        std::fs::write(
            &path,
            "\u{feff}{\"sidebar_collapsed\": true, \"library_sort\": \"name\", \"later\": 1}",
        )?;
        let settings = load(&path);
        assert!(settings.sidebar_collapsed);
        // What an older build called the alphabetical order still reads.
        assert_eq!(settings.library_sort, LibrarySort::Alphabetical);
        assert_eq!(settings.sidebar_width, Settings::default().sidebar_width);
        Ok(())
    }

    #[test]
    fn a_new_profile_crossfades_and_an_old_one_is_left_as_it_was() -> io::Result<()> {
        assert_eq!(Settings::default().crossfade_seconds, 6);
        // Written before there was a crossfade: it has had none.
        let path = scratch("crossfade")?;
        std::fs::write(&path, "{\"sidebar_collapsed\": true}")?;
        assert_eq!(load(&path).crossfade_seconds, 0);
        // One that chose a length keeps it.
        std::fs::write(&path, "{\"crossfade_seconds\": 9}")?;
        assert_eq!(load(&path).crossfade_seconds, 9);
        Ok(())
    }

    #[test]
    fn a_speed_is_held_to_the_range_and_to_a_step() {
        assert_eq!(clamp_speed(1.0), 1.0);
        assert_eq!(clamp_speed(1.26), 1.25);
        assert_eq!(clamp_speed(0.1), 0.5);
        assert_eq!(clamp_speed(9.0), 3.0);
        assert_eq!(clamp_speed(f32::NAN), 1.0);
        // Twenty steps up from the bottom is one and a half, not 1.4999.
        let mut speed = 0.5;
        for _ in 0..20 {
            speed = clamp_speed(speed + SPEED_STEP);
        }
        assert_eq!(speed, 1.5);
    }

    #[test]
    fn a_reset_puts_the_pages_settings_back_and_keeps_the_rest() {
        let mut settings = Settings {
            crossfade_seconds: 0,
            playback_speed: 2.0,
            reduce_motion: true,
            show_music_videos: true,
            together_follow_video: true,
            report_to_youtube: false,
            equalizer_on: true,
            sidebar_width: 320.0,
            device_id: "native-1".into(),
            recent_searches: vec!["air".into()],
            ..Settings::default()
        };
        settings.reset_preferences();
        let fresh = Settings::default();
        assert_eq!(settings.crossfade_seconds, fresh.crossfade_seconds);
        assert_eq!(settings.playback_speed, 1.0);
        assert!(!settings.reduce_motion);
        assert!(!settings.show_music_videos);
        // A room's own choices are made in the room, not on the page.
        assert!(settings.together_follow_video);
        assert!(settings.report_to_youtube);
        assert!(!settings.equalizer_on);
        // Not on the page, so not the page's to reset.
        assert_eq!(settings.sidebar_width, 320.0);
        assert_eq!(settings.device_id, "native-1");
        assert_eq!(settings.recent_searches, ["air"]);
    }

    #[test]
    fn the_volume_levels_aim_where_the_old_app_did() {
        let aims = VolumeLevel::EVERY.map(VolumeLevel::lufs);
        assert_eq!(aims, [-19.0, -14.0, -11.0]);
    }

    #[test]
    fn a_broken_file_is_kept_aside_not_overwritten() -> io::Result<()> {
        let path = scratch("broken")?;
        std::fs::write(&path, "{ not json")?;
        assert_eq!(load(&path), Settings::default());
        assert!(path.with_extension("json.bad").exists());
        Ok(())
    }
}
