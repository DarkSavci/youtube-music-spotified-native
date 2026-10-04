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
    /// As YouTube Music gives it: most recently used first.
    #[default]
    Recent,
    Name,
}

impl LibrarySort {
    pub fn label(self) -> &'static str {
        match self {
            LibrarySort::Recent => "Recents",
            LibrarySort::Name => "A to Z",
        }
    }

    pub fn next(self) -> Self {
        match self {
            LibrarySort::Recent => LibrarySort::Name,
            LibrarySort::Name => LibrarySort::Recent,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub sidebar_visible: bool,
    pub sidebar_width: f32,
    /// The order the library is listed in.
    pub library_sort: LibrarySort,
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
    /// The Listen Together relay last used, and the name shown to others.
    pub together_server: String,
    pub together_name: String,
    /// The version that last ran here, to notice an update by.
    pub last_seen_version: String,
    /// The newest release whose notes have been opened.
    pub release_notes_read: String,
    /// Leave the window's frame to the system rather than drawing the
    /// title bar in the app.
    pub system_title_bar: bool,
    /// Even out loudness between tracks.
    pub normalise_volume: bool,
    /// Draw the music's spectrum behind the player bar.
    pub visualizer: bool,
    /// Seconds one song fades into the next; 0 for none.
    pub crossfade_seconds: u32,
    pub equalizer_on: bool,
    /// Decibels for each of the equalizer's ten bands.
    pub equalizer: [f32; 10],
    /// How the core knows this installation among its devices. Made up on
    /// the first run and kept.
    pub device_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            sidebar_width: theme::SIDEBAR_WIDTH,
            library_sort: LibrarySort::Recent,
            panel: RightPanel::Closed,
            close_to_tray: true,
            system_title_bar: false,
            theme: crate::themes::Choice::Dark,
            custom_theme: None,
            mini_on_top: true,
            mini_size: [360.0, 360.0],
            mini_position: None,
            together_server: String::new(),
            together_name: String::new(),
            last_seen_version: String::new(),
            release_notes_read: String::new(),
            normalise_volume: true,
            visualizer: false,
            crossfade_seconds: 0,
            equalizer_on: false,
            equalizer: [0.0; 10],
            device_id: String::new(),
        }
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
    match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(settings) => settings,
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
            sidebar_visible: false,
            sidebar_width: 320.0,
            library_sort: LibrarySort::Name,
            panel: RightPanel::Lyrics,
            close_to_tray: false,
            system_title_bar: true,
            theme: crate::themes::Choice::Light,
            custom_theme: Some("nord.json".into()),
            mini_on_top: false,
            mini_size: [640.0, 280.0],
            mini_position: Some([40.0, 60.0]),
            together_server: "wss://listen.example.com".into(),
            together_name: "Ada".into(),
            last_seen_version: "0.1.0".into(),
            release_notes_read: "0.1.0".into(),
            normalise_volume: false,
            visualizer: true,
            crossfade_seconds: 6,
            equalizer_on: true,
            equalizer: [3.0; 10],
            device_id: "native-1".into(),
        };
        save(&path, &settings)?;
        assert_eq!(load(&path), settings);
        Ok(())
    }

    #[test]
    fn a_missing_field_takes_its_default() -> io::Result<()> {
        let path = scratch("partial")?;
        std::fs::write(&path, "\u{feff}{\"sidebar_visible\": false, \"later\": 1}")?;
        let settings = load(&path);
        assert!(!settings.sidebar_visible);
        assert_eq!(settings.sidebar_width, Settings::default().sidebar_width);
        Ok(())
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
