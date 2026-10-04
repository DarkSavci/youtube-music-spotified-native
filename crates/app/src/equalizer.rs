//! The equalizer as the app knows it: the curves to start from, the ones a
//! person has saved, and what the panel can ask for. The sound itself is
//! `spotified_audio::eq`.

use serde::{Deserialize, Serialize};
use spotified_audio::eq;

use crate::settings::Settings;

/// Decibels for the ten bands, low to high.
pub type Curve = [f32; 10];

/// A slider moves in steps of this many decibels: finer than can be heard,
/// coarse enough that a value is a round number.
pub const STEP_DB: f32 = 0.5;
/// The longest a saved curve's name may be.
pub const NAME_LENGTH: usize = 28;
/// The most curves that can be saved. Nobody has sixty, and the panel
/// would not hold them.
pub const MOST_SAVED: usize = 60;
/// Curves closer than this in every band are the same curve.
const SAME_DB: f32 = 0.05;

/// The curves the app comes with. Bands are 31, 62, 125, 250, 500 Hz and
/// 1, 2, 4, 8, 16 kHz. They are tuned to be listened through for an hour,
/// not to impress for a minute: nothing past 6 dB, and no band set against
/// its neighbour. Bass boost, Vocal and Treble boost are the Electron
/// app's three, drawn through its five bands' values.
pub const PRESETS: [(&str, Curve); 16] = [
    ("Flat", [0.0; 10]),
    (
        "Bass boost",
        [6.0, 5.5, 4.5, 3.0, 1.5, 0.0, 0.0, 0.0, 0.5, 1.0],
    ),
    (
        "Bass reducer",
        [-6.0, -5.0, -4.0, -2.5, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "Treble boost",
        [-1.0, -1.0, -0.5, 0.0, 0.0, 0.0, 1.5, 3.0, 4.5, 6.0],
    ),
    (
        "Vocal",
        [-2.0, -2.0, -1.0, 0.0, 2.0, 4.0, 3.5, 3.0, 1.5, 0.0],
    ),
    ("Rock", [4.5, 3.5, 2.5, 0.5, -1.0, -0.5, 1.0, 2.5, 3.5, 4.0]),
    ("Pop", [-1.0, 0.0, 1.5, 2.5, 3.0, 2.5, 1.0, 0.0, -0.5, -1.0]),
    ("Jazz", [3.0, 2.5, 1.0, 1.5, -1.0, -1.0, 0.0, 1.0, 2.5, 3.0]),
    (
        "Classical",
        [3.5, 3.0, 2.0, 1.5, -1.0, -1.0, 0.0, 1.5, 2.5, 3.0],
    ),
    (
        "Electronic",
        [4.5, 4.0, 1.5, 0.0, -1.5, 1.5, 0.5, 1.0, 3.5, 4.5],
    ),
    (
        "Hip-hop",
        [5.0, 4.5, 1.5, 2.5, -1.0, -1.0, 1.0, -0.5, 2.0, 2.5],
    ),
    (
        "Acoustic",
        [4.0, 4.0, 3.0, 1.0, 1.5, 1.5, 2.5, 3.0, 2.5, 2.0],
    ),
    (
        "Spoken word",
        [-4.0, -2.0, 0.0, 0.5, 2.5, 3.5, 3.5, 3.0, 1.5, 0.0],
    ),
    (
        "Loudness",
        [5.0, 4.0, 2.0, 0.0, -1.0, -1.0, 0.0, 1.5, 3.0, 4.0],
    ),
    // Where a small speaker has something to give, and nothing under it:
    // a laptop cannot play 31 Hz however far it is raised.
    (
        "Small speakers",
        [0.0, 1.5, 4.0, 3.5, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0],
    ),
    (
        "Headphones",
        [3.5, 3.0, 2.0, 0.5, -0.5, -0.5, 0.5, 1.5, 2.0, 1.5],
    ),
];

/// A curve someone saved under a name of their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub name: String,
    pub gains: Curve,
}

/// A name being typed in the panel: for the curve as it stands, or for a
/// saved one that is being renamed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Naming {
    /// The saved curve this renames; `None` when it names a new one.
    pub renaming: Option<String>,
    pub name: String,
}

/// What the panel can ask for.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    On(bool),
    /// One band, in decibels. Heard at once; written down by `Keep`.
    Band(usize, f32),
    /// The level of the whole, in decibels. As `Band`.
    Preamp(f32),
    /// Write down where a drag has left the sliders.
    Keep,
    /// Keep the level under what the curve raises it by.
    Headroom(bool),
    /// Every band at once, as choosing a preset does.
    Curve(Curve),
    /// Every band and the preamp back to nought.
    Reset,
    /// Begin naming the curve as it stands.
    Name,
    /// Begin renaming a saved curve.
    Rename(String),
    /// What has been typed of the name so far.
    Typed(String),
    /// Save under the name typed.
    Save,
    /// Put the name away unsaved.
    Cancel,
    /// Forget a saved curve.
    Delete(String),
}

/// What the sliders are set to, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named<'a> {
    BuiltIn(&'static str),
    Saved(&'a str),
    /// No curve anyone has named.
    Custom,
}

impl Named<'_> {
    pub fn label(&self) -> &str {
        match self {
            Named::BuiltIn(name) => name,
            Named::Saved(name) => name,
            Named::Custom => "Custom",
        }
    }
}

fn same(a: &Curve, b: &Curve) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < SAME_DB)
}

/// The name of the curve the sliders are set to. It is worked out, not
/// remembered, so it cannot be wrong: moving a band off a preset makes it
/// Custom, and moving it back makes it the preset again. A saved curve
/// that matches a built-in one is called by its own name: whoever saved it
/// named it.
pub fn named(settings: &Settings) -> Named<'_> {
    let curve = &settings.equalizer;
    let saved = settings.equalizer_presets.iter();
    if let Some(saved) = saved.into_iter().find(|saved| same(&saved.gains, curve)) {
        return Named::Saved(&saved.name);
    }
    PRESETS
        .iter()
        .find(|(_, gains)| same(gains, curve))
        .map_or(Named::Custom, |(name, _)| Named::BuiltIn(name))
}

/// A decibel value held to the range and to a slider's step.
pub fn stepped(decibels: f32) -> f32 {
    if !decibels.is_finite() {
        return 0.0;
    }
    let stepped = (decibels / STEP_DB).round() * STEP_DB;
    stepped.clamp(-eq::RANGE_DB, eq::RANGE_DB)
}

/// Holds what a settings file says to what the sliders can show: a file
/// may have been written by hand. And a profile from before there was
/// headroom to keep took no level off, and goes on sounding as it did:
/// keeping it is for a new profile, as the crossfade is.
pub fn hold(settings: &mut Settings, file: &str) {
    if !file.contains("\"equalizer_headroom\"") {
        settings.equalizer_headroom = false;
    }
    let range = eq::RANGE_DB;
    let held = |db: f32| {
        if db.is_finite() {
            db.clamp(-range, range)
        } else {
            0.0
        }
    };
    settings.equalizer = settings.equalizer.map(held);
    settings.equalizer_preamp = held(settings.equalizer_preamp);
    for saved in &mut settings.equalizer_presets {
        saved.gains = saved.gains.map(held);
    }
    settings.equalizer_presets.truncate(MOST_SAVED);
}

/// A name as it is kept: without the room around it, and no longer than
/// the panel has room to show.
pub fn tidy_name(name: &str) -> String {
    name.trim().chars().take(NAME_LENGTH).collect()
}

/// Whether a built-in curve has this name already.
pub fn is_built_in(name: &str) -> bool {
    PRESETS
        .iter()
        .any(|(built_in, _)| built_in.eq_ignore_ascii_case(name))
}

/// What the engine is told.
pub fn for_engine(settings: &Settings) -> eq::Settings {
    eq::Settings {
        enabled: settings.equalizer_on,
        gains: settings.equalizer,
        preamp: settings.equalizer_preamp,
        headroom: settings.equalizer_headroom,
    }
}

/// "+4.5 dB", "0 dB", "−12 dB": a level as the panel writes it, with a
/// real minus sign, which is as wide as the plus.
pub fn decibels(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    let sign = if rounded > 0.0 {
        "+"
    } else if rounded < 0.0 {
        "−"
    } else {
        ""
    };
    let size = rounded.abs();
    if size.fract() == 0.0 {
        format!("{sign}{size:.0} dB")
    } else {
        format!("{sign}{size:.1} dB")
    }
}

/// "31", "1k", "16k": a band by its frequency.
pub fn band_label(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{}k", hz / 1000.0)
    } else {
        format!("{hz}")
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::settings;

    fn scratch(name: &str) -> io::Result<std::path::PathBuf> {
        let dir = std::env::temp_dir().join(format!("spotified-eq-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join("settings.json"))
    }

    #[test]
    fn every_built_in_curve_has_a_name_of_its_own_and_stays_musical() {
        for (index, (name, gains)) in PRESETS.iter().enumerate() {
            assert!(PRESETS[index + 1..].iter().all(|(other, _)| other != name));
            assert!(
                PRESETS[index + 1..]
                    .iter()
                    .all(|(_, other)| !same(other, gains)),
                "{name} is another curve over again"
            );
            // On the sliders' steps, nothing past 6 dB, and no band more
            // than 3.5 dB from the one beside it.
            assert!(
                gains
                    .iter()
                    .all(|db| stepped(*db) == *db && db.abs() <= 6.0)
            );
            let jumps = gains.windows(2).map(|pair| (pair[1] - pair[0]).abs());
            assert!(jumps.fold(0.0, f32::max) <= 3.5, "{name} is jagged");
        }
        assert_eq!(PRESETS[0], ("Flat", [0.0; 10]));
    }

    #[test]
    fn the_curve_is_called_by_its_preset_until_a_band_moves() {
        let mut settings = Settings::default();
        assert_eq!(named(&settings), Named::BuiltIn("Flat"));
        settings.equalizer = PRESETS[5].1;
        assert_eq!(named(&settings).label(), "Rock");
        settings.equalizer[3] += 0.5;
        assert_eq!(named(&settings), Named::Custom);
        // Saved, it has the name it was given, and keeps it over a
        // built-in curve that happens to be the same.
        settings.equalizer_presets.push(Saved {
            name: "Car".into(),
            gains: settings.equalizer,
        });
        assert_eq!(named(&settings), Named::Saved("Car"));
        settings.equalizer_presets[0].gains = PRESETS[5].1;
        settings.equalizer = PRESETS[5].1;
        assert_eq!(named(&settings), Named::Saved("Car"));
    }

    #[test]
    fn a_level_is_written_with_its_sign_and_no_needless_nought() {
        assert_eq!(decibels(4.5), "+4.5 dB");
        assert_eq!(decibels(-12.0), "−12 dB");
        assert_eq!(decibels(0.0), "0 dB");
        assert_eq!(decibels(-0.01), "0 dB");
        let labels = eq::BANDS.map(band_label);
        let wanted = [
            "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
        ];
        assert_eq!(labels, wanted);
    }

    #[test]
    fn a_value_is_held_to_the_range_and_to_a_step() {
        assert_eq!(stepped(4.3), 4.5);
        assert_eq!(stepped(40.0), 12.0);
        assert_eq!(stepped(-40.0), -12.0);
        assert_eq!(stepped(f32::NAN), 0.0);
        assert_eq!(tidy_name("  My  car  "), "My  car");
        assert_eq!(tidy_name(&"x".repeat(80)).len(), NAME_LENGTH);
        assert!(is_built_in("bass BOOST"));
        assert!(!is_built_in("Car"));
    }

    #[test]
    fn the_equalizer_and_its_saved_curves_come_back() -> io::Result<()> {
        let path = scratch("roundtrip")?;
        let settings = Settings {
            equalizer_on: true,
            equalizer: PRESETS[9].1,
            equalizer_preamp: -3.5,
            equalizer_headroom: false,
            equalizer_presets: vec![
                Saved {
                    name: "Car".into(),
                    gains: [1.0, 2.0, 3.0, 4.0, 5.0, -1.0, -2.0, -3.0, -4.0, -5.0],
                },
                Saved {
                    name: "Küche".into(),
                    gains: [0.5; 10],
                },
            ],
            ..Settings::default()
        };
        settings::save(&path, &settings)?;
        let back = settings::load(&path);
        assert_eq!(back, settings);
        assert_eq!(for_engine(&back).preamp, -3.5);
        Ok(())
    }

    #[test]
    fn a_profile_from_before_keeps_its_curve_and_sounds_as_it_did() -> io::Result<()> {
        // What 0.4.3 wrote: the switch and the ten bands, nothing else.
        let path = scratch("before")?;
        let before = "{\"equalizer_on\": true, \
                      \"equalizer\": [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}";
        std::fs::write(&path, before)?;
        let settings = settings::load(&path);
        assert!(settings.equalizer_on);
        assert_eq!(settings.equalizer[..4], [6.0, 5.0, 4.0, 2.0]);
        // The old Bass boost is nobody's preset now: it is theirs.
        assert_eq!(named(&settings), Named::Custom);
        // No level was taken off before, and none is now.
        assert!(!settings.equalizer_headroom);
        assert_eq!(settings.equalizer_preamp, 0.0);
        assert!(settings.equalizer_presets.is_empty());
        // A new profile keeps headroom from the start.
        assert!(Settings::default().equalizer_headroom);
        // A curve written by hand out of range, or not a number, is held.
        std::fs::write(
            &path,
            "{\"equalizer\": [40, -40, 0, 0, 0, 0, 0, 0, 0, 0], \"equalizer_preamp\": 99}",
        )?;
        let settings = settings::load(&path);
        assert_eq!(settings.equalizer[..2], [12.0, -12.0]);
        assert_eq!(settings.equalizer_preamp, 12.0);
        Ok(())
    }
}
