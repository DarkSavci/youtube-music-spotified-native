//! The Electron app's preferences, and what each becomes here.
//!
//! Its page kept them in the browser's Local Storage, and its shell kept
//! the mini player's window in a file beside. Only what this app has a
//! setting for is read; the rest is named in [`NOT_BROUGHT`], so that
//! nobody wonders where it went.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use super::leveldb;
use crate::actions::MAX_CROSSFADE_SECONDS;
use crate::settings::{CACHE_SIZES_MB, Settings, VolumeLevel, clamp_speed};
use crate::state::searches::{RECENT_KEPT, remember_after};
use crate::themes::Choice;
use crate::together::{Mode, SavedServer, protocol::checked_address};

/// The page of the installed app; a development build's is some port on
/// localhost, and is only looked at when this one has nothing.
const INSTALLED_ORIGIN: &str = "file://";
const SETTINGS_KEY: &str = "spotifier.settings";
const ROOMS_KEY: &str = "spotifier.rooms.v2";
const SEARCHES_KEY: &str = "spotifier.recentSearches";
const SIDEBAR_KEY: &str = "sidebar.width";
/// At or under this the Electron app's sidebar was a rail of covers.
const RAIL_AT_MOST: f32 = 170.0;
/// The centres of the Electron app's five equalizer bands, in hertz.
const OLD_BANDS: [f32; 5] = [60.0, 250.0, 1000.0, 4000.0, 12000.0];

/// What the Electron app has a setting for and this app does not.
pub const NOT_BROUGHT: &str = "Music videos, the playback engine, the quality badge, timed \
                               lyrics and the sidebar's width have no setting to take them \
                               here, so they were left.";

/// The settings object, as the page's store wrote it. Every field may be
/// missing: an older version of the app knew fewer.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OldSettings {
    crossfade_ms: Option<f64>,
    gapless: Option<bool>,
    normalization: Option<bool>,
    normalization_level: Option<String>,
    theme: Option<String>,
    reduce_motion: Option<bool>,
    resume_on_launch: Option<bool>,
    #[serde(rename = "continueFromYouTubeMusic")]
    continue_from_youtube_music: Option<bool>,
    close_to_tray: Option<bool>,
    #[serde(rename = "reportToYouTube")]
    report_to_youtube: Option<bool>,
    volume_boost: Option<bool>,
    #[serde(rename = "cacheMaxMB")]
    cache_max_mb: Option<f64>,
    autoplay: Option<bool>,
    remaining_time: Option<bool>,
    playback_speed: Option<f32>,
    eq: Option<Vec<f32>>,
}

/// Listen Together, as the page's store wrote it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OldRooms {
    servers: Vec<SavedServer>,
    selected: String,
    room_name: String,
    mode: String,
    name: String,
    /// The picture shown to a room; empty for none.
    avatar: String,
    notifications: bool,
}

/// The mini player's window, as the shell wrote it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OldMini {
    bounds: Option<Bounds>,
    always_on_top: Option<bool>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
#[serde(default)]
struct Bounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// What a store wraps its state in.
#[derive(Deserialize)]
struct Stored<T> {
    state: T,
}

/// Everything of the Electron app's preferences that has a place here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OldPrefs {
    pub settings: Option<OldSettings>,
    pub sidebar_width: Option<f32>,
    /// Recent searches, newest first, the account in use's before the rest.
    pub searches: Vec<String>,
    pub rooms: Option<OldRooms>,
    pub mini: Option<OldMini>,
    /// Why the page's storage could not be read, when it could not.
    pub unread: Option<String>,
}

impl OldPrefs {
    pub fn is_empty(&self) -> bool {
        self.settings.is_none()
            && self.sidebar_width.is_none()
            && self.searches.is_empty()
            && self.rooms.is_none()
            && self.mini.is_none()
    }

    /// The size the Electron app held its song cache to, as one of the
    /// sizes on offer here.
    pub fn cache_max_mb(&self) -> Option<u32> {
        let megabytes = self.settings.as_ref()?.cache_max_mb?;
        CACHE_SIZES_MB
            .into_iter()
            .min_by_key(|size| (f64::from(*size) - megabytes).abs() as u64)
    }

    /// What was found, in a few words, for the list of what can be brought.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.settings.is_some() {
            parts.push("playback and window settings".into());
        }
        if let Some(rooms) = &self.rooms {
            match rooms.servers.len() {
                0 if rooms.name.is_empty() => {}
                0 => parts.push("your Listen Together name".into()),
                1 => parts.push("1 Listen Together server".into()),
                count => parts.push(format!("{count} Listen Together servers")),
            }
        }
        match self.searches.len() {
            0 => {}
            1 => parts.push("1 recent search".into()),
            count => parts.push(format!("{count} recent searches")),
        }
        if self.mini.is_some() {
            parts.push("the mini player's place".into());
        }
        if parts.is_empty() && self.sidebar_width.is_some() {
            parts.push("whether the sidebar is a rail".into());
        }
        let mut said = parts.join(", ");
        if let Some(first) = said.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        said
    }
}

/// The page's Local Storage: each origin's keys and values.
#[derive(Debug, Default)]
struct LocalStorage {
    origins: BTreeMap<String, BTreeMap<String, String>>,
}

impl LocalStorage {
    /// Reads the storage Chromium keeps under `root`. Its keys are an
    /// underscore, the origin, a zero byte and the name; names and values
    /// each start with a byte saying whether they are UTF-16 or Latin-1.
    fn read(root: &Path) -> std::io::Result<Self> {
        let mut storage = Self::default();
        for (key, value) in leveldb::read(&root.join("Local Storage").join("leveldb"))? {
            let Some(rest) = key.strip_prefix(b"_") else {
                continue;
            };
            let Some(split) = rest.iter().position(|byte| *byte == 0) else {
                continue;
            };
            let origin = String::from_utf8_lossy(&rest[..split]).into_owned();
            let (Some(name), Some(value)) = (text(&rest[split + 1..]), text(&value)) else {
                continue;
            };
            storage
                .origins
                .entry(origin)
                .or_default()
                .insert(name, value);
        }
        Ok(storage)
    }

    /// Every key and value of the origin that holds the app's settings:
    /// the installed app's, or failing that whichever has them.
    fn app(&self) -> Option<&BTreeMap<String, String>> {
        self.origins.get(INSTALLED_ORIGIN).or_else(|| {
            self.origins
                .values()
                .find(|keys| keys.contains_key(SETTINGS_KEY))
        })
    }
}

/// A string as Chromium stores it: a byte for the encoding, then the text.
fn text(stored: &[u8]) -> Option<String> {
    let (encoding, rest) = stored.split_first()?;
    match encoding {
        0 => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes(*pair))
                .collect();
            Some(String::from_utf16_lossy(&units))
        }
        1 => Some(rest.iter().map(|byte| char::from(*byte)).collect()),
        _ => None,
    }
}

/// Reads the preferences of the profile at `root`. `scope` is how the
/// Electron app named the account in use, whose searches come first.
pub fn read(root: &Path, scope: &str) -> OldPrefs {
    let mut prefs = OldPrefs {
        mini: std::fs::read_to_string(root.join("miniplayer.json"))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok()),
        ..OldPrefs::default()
    };
    let storage = match LocalStorage::read(root) {
        Ok(storage) => storage,
        Err(error) => {
            prefs.unread = Some(error.to_string());
            return prefs;
        }
    };
    let Some(keys) = storage.app() else {
        return prefs;
    };
    let stored = |key: &str| keys.get(key).map(String::as_str);
    prefs.settings = stored(SETTINGS_KEY)
        .and_then(|json| serde_json::from_str::<Stored<OldSettings>>(json).ok())
        .map(|stored| stored.state);
    prefs.rooms = stored(ROOMS_KEY)
        .and_then(|json| serde_json::from_str::<Stored<OldRooms>>(json).ok())
        .map(|stored| stored.state);
    prefs.sidebar_width = stored(SIDEBAR_KEY)
        .and_then(|width| width.parse::<f32>().ok())
        .filter(|width| width.is_finite() && *width > 0.0);
    // One list for each account and channel, and one from before there
    // were accounts. The account in use's is the one most likely wanted.
    let first = format!("{SEARCHES_KEY}.{scope}");
    let lists = keys
        .iter()
        .filter(|(key, _)| key.starts_with(SEARCHES_KEY))
        .map(|(key, list)| (*key != first, list));
    let mut lists: Vec<(bool, &String)> = lists.collect();
    lists.sort_by_key(|(later, _)| *later);
    for (_, list) in lists {
        let Ok(queries) = serde_json::from_str::<Vec<String>>(list) else {
            continue;
        };
        for query in queries {
            remember_after(&mut prefs.searches, &query);
        }
    }
    // As many as this app keeps; more could never be brought.
    prefs.searches.truncate(RECENT_KEPT);
    prefs
}

/// Sets `setting` to `value`, and notes its name if that changed it.
fn bring<T: PartialEq>(setting: &mut T, value: Option<T>, name: &str, changed: &mut Vec<String>) {
    if let Some(value) = value
        && *setting != value
    {
        *setting = value;
        changed.push(name.to_owned());
    }
}

/// Takes the Electron app's preferences into these settings, and returns
/// what changed, in words. The same preferences taken twice change nothing
/// the second time.
pub fn apply(settings: &mut Settings, old: &OldPrefs, scope: &str) -> Vec<String> {
    let mut changed = Vec::new();
    let c = &mut changed;
    if let Some(theirs) = &old.settings {
        let seconds = theirs
            .crossfade_ms
            .map(|ms| ((ms / 1000.0).round().max(0.0) as u32).min(MAX_CROSSFADE_SECONDS));
        bring(&mut settings.crossfade_seconds, seconds, "crossfade", c);
        bring(&mut settings.gapless, theirs.gapless, "gapless playback", c);
        let normalise = theirs.normalization;
        let name = "volume normalisation";
        bring(&mut settings.normalise_volume, normalise, name, c);
        let level = match theirs.normalization_level.as_deref() {
            Some("quiet") => Some(VolumeLevel::Quiet),
            Some("normal") => Some(VolumeLevel::Normal),
            Some("loud") => Some(VolumeLevel::Loud),
            _ => None,
        };
        bring(&mut settings.volume_level, level, "volume level", c);
        // The Electron app had no light theme: its two choices are these.
        let worn = match theirs.theme.as_deref() {
            Some("system") => Some(Choice::System),
            Some("dark") => Some(Choice::Dark),
            _ => None,
        };
        bring(&mut settings.theme, worn, "theme", c);
        let still = theirs.reduce_motion;
        bring(&mut settings.reduce_motion, still, "reduce motion", c);
        let resume = theirs.resume_on_launch;
        bring(
            &mut settings.resume_on_launch,
            resume,
            "resume on launch",
            c,
        );
        let carry_on = theirs.continue_from_youtube_music;
        let name = "continue from YouTube Music";
        bring(&mut settings.continue_from_youtube_music, carry_on, name, c);
        let tray = theirs.close_to_tray;
        bring(&mut settings.close_to_tray, tray, "close to tray", c);
        let report = theirs.report_to_youtube;
        bring(
            &mut settings.report_to_youtube,
            report,
            "YouTube history",
            c,
        );
        let boost = theirs.volume_boost;
        bring(&mut settings.volume_boost, boost, "volume boost", c);
        let cache = old.cache_max_mb();
        bring(&mut settings.cache_max_mb, cache, "song cache size", c);
        bring(&mut settings.autoplay, theirs.autoplay, "autoplay", c);
        let remaining = theirs.remaining_time;
        bring(&mut settings.remaining_time, remaining, "time remaining", c);
        let speed = theirs.playback_speed.map(clamp_speed);
        bring(&mut settings.playback_speed, speed, "playback speed", c);
        // A flat curve there is no equalizer; the one here is left alone.
        if let Some(gains) = theirs.eq.as_deref().and_then(ten_bands)
            && (settings.equalizer != gains || !settings.equalizer_on)
        {
            settings.equalizer = gains;
            settings.equalizer_on = true;
            c.push("equalizer".to_owned());
        }
    }
    // Only whether it is a rail of covers: how wide the full sidebar is
    // follows the panel on screen, which a setting cannot move.
    let rail = old.sidebar_width.map(|width| width <= RAIL_AT_MOST);
    bring(&mut settings.sidebar_collapsed, rail, "sidebar", c);
    if let Some(mini) = &old.mini {
        let before = (
            settings.mini_on_top,
            settings.mini_size,
            settings.mini_position,
        );
        if let Some(on_top) = mini.always_on_top {
            settings.mini_on_top = on_top;
        }
        if let Some(bounds) = mini.bounds.filter(|b| b.width > 0.0 && b.height > 0.0) {
            settings.mini_size = [bounds.width, bounds.height];
            settings.mini_position = Some([bounds.x, bounds.y]);
        }
        let after = (
            settings.mini_on_top,
            settings.mini_size,
            settings.mini_position,
        );
        if before != after {
            changed.push("mini player".to_owned());
        }
    }
    if let Some(rooms) = &old.rooms {
        apply_rooms(settings, rooms, &mut changed);
    }
    // Searches made here stay first: they are the newer ones. They join
    // the list of whoever is in use here, which is who brought them.
    let room = RECENT_KEPT.saturating_sub(settings.recent_searches(scope).len());
    if room > 0 && !old.searches.is_empty() {
        let ours = settings.recent_searches_mut(scope);
        let before = ours.len();
        for query in &old.searches {
            if ours.len() >= RECENT_KEPT {
                break;
            }
            remember_after(ours, query);
        }
        if ours.len() != before {
            changed.push("recent searches".to_owned());
        }
    }
    changed
}

/// Listen Together: the saved servers join those saved here, told apart
/// by their addresses, and the rest is taken as it was.
fn apply_rooms(settings: &mut Settings, rooms: &OldRooms, changed: &mut Vec<String>) {
    let mut added = false;
    for server in &rooms.servers {
        // The same rule a server typed in here is held to.
        let Ok(url) = checked_address(&server.url) else {
            continue;
        };
        let same =
            |saved: &&SavedServer| saved.url.trim_end_matches('/') == url.trim_end_matches('/');
        let id = match settings.together_servers.iter().find(same) {
            Some(known) => known.id.clone(),
            None => {
                let taken = |id: &str| settings.together_servers.iter().any(|s| s.id == id);
                let id = if server.id.is_empty() || taken(&server.id) {
                    format!("old-{}", settings.together_servers.len() + 1)
                } else {
                    server.id.clone()
                };
                settings.together_servers.push(SavedServer {
                    id: id.clone(),
                    name: server.name.clone(),
                    url,
                });
                added = true;
                id
            }
        };
        // The one chosen there is chosen here when none is.
        if server.id == rooms.selected && settings.together_server().is_none() {
            settings.together_selected = id;
            added = true;
        }
    }
    if added {
        changed.push("Listen Together servers".to_owned());
    }
    let name = Some(rooms.name.trim().to_owned()).filter(|name| !name.is_empty());
    bring(
        &mut settings.together_name,
        name,
        "Listen Together name",
        changed,
    );
    let room = Some(rooms.room_name.trim().to_owned()).filter(|name| !name.is_empty());
    bring(&mut settings.together_room_name, room, "room name", changed);
    let mode = match rooms.mode.as_str() {
        "collaborative" => Some(Mode::Collaborative),
        "contributions" => Some(Mode::Contributions),
        "listen" => Some(Mode::Listen),
        _ => None,
    };
    bring(&mut settings.together_mode, mode, "room mode", changed);
    let picture = Some(!rooms.avatar.is_empty());
    let name = "Listen Together picture";
    bring(&mut settings.together_share_picture, picture, name, changed);
    let notify = Some(rooms.notifications);
    let name = "room notifications";
    bring(&mut settings.together_notifications, notify, name, changed);
}

/// The Electron app's five-band curve as this app's ten bands hear it: a
/// straight line between its bands on a scale of octaves, level beyond
/// the outermost, to the half decibel the sliders here move by. `None`
/// for a flat curve, or for one that is not five bands.
fn ten_bands(old: &[f32]) -> Option<[f32; 10]> {
    let old: [f32; 5] = old.try_into().ok()?;
    if old.iter().all(|gain| *gain == 0.0) || old.iter().any(|gain| !gain.is_finite()) {
        return None;
    }
    let range = spotified_audio::eq::RANGE_DB;
    let mut gains = [0.0; 10];
    for (gain, hz) in gains.iter_mut().zip(spotified_audio::eq::BANDS) {
        let above = OLD_BANDS.iter().position(|band| *band >= hz);
        let decibels = match above {
            Some(0) => old[0],
            None => old[4],
            Some(at) => {
                let (low, high) = (OLD_BANDS[at - 1], OLD_BANDS[at]);
                let along = (hz / low).log2() / (high / low).log2();
                old[at - 1] + (old[at] - old[at - 1]) * along
            }
        };
        *gain = ((decibels * 2.0).round() / 2.0).clamp(-range, range);
    }
    Some(gains)
}

#[cfg(test)]
pub(super) mod tests;
