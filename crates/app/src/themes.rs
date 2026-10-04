//! Themes: the built-in dark and light palettes, and any number of others
//! read from a folder of small JSON files.
//!
//! A theme file names a base and the colours it changes:
//!
//! ```json
//! { "base": "dark", "colors": { "window": "#1a1b26", "accent": "#7aa2f7" } }
//! ```
//!
//! Colours it leaves out are the base's. A handful of well-known palettes
//! are written into the folder the first time the app runs, as examples to
//! copy as much as themes to use.

use std::path::Path;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::theme::{self, Palette};

/// A theme file larger than this is not a theme file.
const MOST_BYTES: u64 = 64 * 1024;
/// Nor does anyone have more themes than this.
const MOST_THEMES: usize = 128;

/// Which of the built-in palettes to wear when no custom one is chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Choice {
    /// Whichever of light and dark the system is set to.
    System,
    Light,
    #[default]
    Dark,
}

impl Choice {
    pub const EVERY: [Choice; 3] = [Choice::System, Choice::Light, Choice::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Choice::System => "Follow system",
            Choice::Light => "Light",
            Choice::Dark => "Dark",
        }
    }
}

/// A theme read from the folder: its file's name, and its colours.
#[derive(Debug, Clone, PartialEq)]
pub struct Custom {
    /// The file's name, which is what the settings remember.
    pub file: String,
    pub palette: Palette,
}

impl Custom {
    /// The name shown: the file's, without its ending, with spaces.
    pub fn label(&self) -> String {
        let stem = self.file.strip_suffix(".json").unwrap_or(&self.file);
        stem.replace(['-', '_'], " ")
    }
}

/// The palette to wear: the chosen custom theme if it is there, otherwise
/// the built-in one the choice names. `system_dark` is what the system is
/// set to.
pub fn palette(
    choice: Choice,
    custom: Option<&str>,
    themes: &[Custom],
    system_dark: bool,
) -> Palette {
    if let Some(theme) = custom.and_then(|file| themes.iter().find(|theme| theme.file == file)) {
        return theme.palette;
    }
    let dark = match choice {
        Choice::System => system_dark,
        Choice::Light => false,
        Choice::Dark => true,
    };
    if dark { theme::DARK } else { theme::LIGHT }
}

#[derive(Deserialize)]
#[serde(default)]
struct File {
    base: String,
    colors: std::collections::BTreeMap<String, String>,
}

impl Default for File {
    fn default() -> Self {
        Self {
            base: "dark".into(),
            colors: Default::default(),
        }
    }
}

/// Reads a theme. `None` if it is not JSON of the right shape; a colour it
/// misnames or misspells is passed over, and the base's stands.
pub fn parse(text: &str) -> Option<Palette> {
    let file: File = serde_json::from_str(text).ok()?;
    let mut palette = match file.base.as_str() {
        "light" => theme::LIGHT,
        _ => theme::DARK,
    };
    for (name, value) in &file.colors {
        if let Some(color) = hex(value) {
            set(&mut palette, name, color);
        }
    }
    Some(palette)
}

fn set(palette: &mut Palette, name: &str, color: Color32) {
    let slot = match name {
        "window" => &mut palette.window,
        "panel" => &mut palette.panel,
        "surface" => &mut palette.surface,
        "surface_hover" => &mut palette.surface_hover,
        "surface_active" => &mut palette.surface_active,
        "outline" => &mut palette.outline,
        "text" => &mut palette.text,
        "secondary" => &mut palette.secondary,
        "dim" => &mut palette.dim,
        "accent" => &mut palette.accent,
        "accent_hover" => &mut palette.accent_hover,
        "on_accent" => &mut palette.on_accent,
        "danger" => &mut palette.danger,
        "warning" => &mut palette.warning,
        "overlay" => &mut palette.overlay,
        "shadow" => &mut palette.shadow,
        _ => return,
    };
    *slot = color;
}

/// `#RRGGBB` or `#RRGGBBAA`.
fn hex(text: &str) -> Option<Color32> {
    let digits = text.trim().strip_prefix('#')?;
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => {
            let [_, red, green, blue] = value.to_be_bytes();
            Some(Color32::from_rgb(red, green, blue))
        }
        8 => {
            let [red, green, blue, alpha] = value.to_be_bytes();
            Some(Color32::from_rgba_unmultiplied(red, green, blue, alpha))
        }
        _ => None,
    }
}

/// Every theme in `folder`, by name. A file that does not read as a theme
/// is left out and said so in the log.
pub fn list(folder: &Path) -> Vec<Custom> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut themes: Vec<Custom> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            if !file.ends_with(".json") || entry.metadata().ok()?.len() > MOST_BYTES {
                return None;
            }
            let text = std::fs::read_to_string(entry.path()).ok()?;
            match parse(&text) {
                Some(palette) => Some(Custom { file, palette }),
                None => {
                    log::warn!("{file} is not a theme this app can read");
                    None
                }
            }
        })
        .take(MOST_THEMES)
        .collect();
    themes.sort_by_key(|theme| theme.file.to_lowercase());
    themes
}

/// Palettes people know by name, as theme files.
const PRESETS: [(&str, &str); 6] = [
    (
        "catppuccin-mocha.json",
        r##"{ "base": "dark", "colors": { "window": "#11111b", "panel": "#1e1e2e", "surface": "#313244", "surface_hover": "#45475a", "surface_active": "#585b70", "outline": "#313244", "text": "#cdd6f4", "secondary": "#a6adc8", "dim": "#6c7086", "accent": "#cba6f7", "accent_hover": "#d9bcfa", "on_accent": "#11111b", "overlay": "#313244" } }"##,
    ),
    (
        "gruvbox.json",
        r##"{ "base": "dark", "colors": { "window": "#1d2021", "panel": "#282828", "surface": "#32302f", "surface_hover": "#3c3836", "surface_active": "#504945", "outline": "#3c3836", "text": "#ebdbb2", "secondary": "#bdae93", "dim": "#928374", "accent": "#b8bb26", "accent_hover": "#c9cc3f", "on_accent": "#1d2021", "overlay": "#3c3836" } }"##,
    ),
    (
        "nord.json",
        r##"{ "base": "dark", "colors": { "window": "#242933", "panel": "#2e3440", "surface": "#3b4252", "surface_hover": "#434c5e", "surface_active": "#4c566a", "outline": "#3b4252", "text": "#eceff4", "secondary": "#d8dee9", "dim": "#7b88a1", "accent": "#88c0d0", "accent_hover": "#8fbcbb", "on_accent": "#2e3440", "overlay": "#3b4252" } }"##,
    ),
    (
        "rose-pine.json",
        r##"{ "base": "dark", "colors": { "window": "#12101a", "panel": "#191724", "surface": "#1f1d2e", "surface_hover": "#26233a", "surface_active": "#403d52", "outline": "#26233a", "text": "#e0def4", "secondary": "#908caa", "dim": "#6e6a86", "accent": "#ebbcba", "accent_hover": "#f2cfcd", "on_accent": "#191724", "overlay": "#26233a" } }"##,
    ),
    (
        "spotify-green.json",
        r##"{ "base": "dark", "colors": { "accent": "#1ed760", "accent_hover": "#3ce87a", "on_accent": "#0a140e" } }"##,
    ),
    (
        "tokyo-night.json",
        r##"{ "base": "dark", "colors": { "window": "#16161e", "panel": "#1a1b26", "surface": "#24283b", "surface_hover": "#292e42", "surface_active": "#3b4261", "outline": "#292e42", "text": "#c0caf5", "secondary": "#a9b1d6", "dim": "#565f89", "accent": "#7aa2f7", "accent_hover": "#9ab8ff", "on_accent": "#16161e", "overlay": "#24283b" } }"##,
    ),
];

/// Makes the folder and puts the presets in it, the first time only: a
/// preset someone deleted stays deleted.
pub fn write_presets(folder: &Path) {
    if folder.exists() {
        return;
    }
    if let Err(error) = std::fs::create_dir_all(folder) {
        log::warn!("the themes folder could not be made: {error}");
        return;
    }
    for (file, text) in PRESETS {
        if let Err(error) = std::fs::write(folder.join(file), text) {
            log::warn!("{file} could not be written: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_changes_the_colours_it_names_and_keeps_the_rest() {
        let text = r##"{ "base": "light", "colors": { "accent": "#7aa2f7", "nonsense": "#000000", "text": "blue" } }"##;
        let palette = parse(text).expect("a theme");
        assert_eq!(palette.accent, Color32::from_rgb(0x7a, 0xa2, 0xf7));
        assert_eq!(palette.text, theme::LIGHT.text);
        assert!(!palette.dark);
    }

    #[test]
    fn a_theme_with_no_base_is_built_on_dark() {
        let palette = parse(r#"{ "colors": {} }"#).expect("a theme");
        assert_eq!(palette, theme::DARK);
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn colours_are_six_or_eight_hex_digits() {
        assert_eq!(hex("#ff0033"), Some(Color32::from_rgb(255, 0, 51)));
        assert!(hex("#ff003380").is_some());
        assert_eq!(hex("ff0033"), None);
        assert_eq!(hex("#fff"), None);
    }

    #[test]
    fn every_preset_reads_as_a_theme() {
        for (file, text) in PRESETS {
            assert!(parse(text).is_some(), "{file}");
        }
    }

    #[test]
    fn the_chosen_custom_theme_wins_and_a_missing_one_falls_back() {
        let nord = Custom {
            file: "nord.json".into(),
            palette: Palette {
                accent: Color32::from_rgb(1, 2, 3),
                ..theme::DARK
            },
        };
        let themes = [nord.clone()];
        assert_eq!(nord.label(), "nord");
        let worn = palette(Choice::Light, Some("nord.json"), &themes, false);
        assert_eq!(worn, nord.palette);
        let gone = palette(Choice::Light, Some("gone.json"), &themes, true);
        assert_eq!(gone, theme::LIGHT);
        assert_eq!(palette(Choice::System, None, &themes, true), theme::DARK);
    }
}
