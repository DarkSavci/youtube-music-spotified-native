//! Keyboard shortcuts.
//!
//! The same keys as Spotify's desktop app and Spotifast. Single letters and
//! Space belong to a text field while one has the focus; the combinations
//! with Ctrl or Alt work everywhere.

use eframe::egui::{Context, Key, Modifiers};

use crate::actions::Action;

const SEEK_STEP_MS: i64 = 10_000;
const VOLUME_STEP: f32 = 0.05;

/// One shortcut: the keys, whether typing takes precedence, what it does,
/// and how the shortcut list describes it.
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
    /// Left to a text field while one is being typed in.
    pub yields_to_typing: bool,
    pub action: fn() -> Action,
    pub description: &'static str,
}

const fn plain(key: Key, action: fn() -> Action, description: &'static str) -> Shortcut {
    Shortcut {
        modifiers: Modifiers::NONE,
        key,
        yields_to_typing: true,
        action,
        description,
    }
}

const fn held(
    modifiers: Modifiers,
    key: Key,
    action: fn() -> Action,
    description: &'static str,
) -> Shortcut {
    Shortcut {
        modifiers,
        key,
        yields_to_typing: false,
        action,
        description,
    }
}

/// Shift with an arrow, which in a text field selects text instead.
const fn shifted(key: Key, action: fn() -> Action, description: &'static str) -> Shortcut {
    Shortcut {
        modifiers: Modifiers::SHIFT,
        key,
        yields_to_typing: true,
        action,
        description,
    }
}

pub const SHORTCUTS: &[Shortcut] = &[
    plain(Key::Space, || Action::TogglePlay, "Play or pause"),
    held(
        Modifiers::COMMAND,
        Key::ArrowRight,
        || Action::Next,
        "Next song",
    ),
    held(
        Modifiers::COMMAND,
        Key::ArrowLeft,
        || Action::Previous,
        "Previous song",
    ),
    shifted(
        Key::ArrowRight,
        || Action::SeekBy(SEEK_STEP_MS),
        "Forward 10 seconds",
    ),
    shifted(
        Key::ArrowLeft,
        || Action::SeekBy(-SEEK_STEP_MS),
        "Back 10 seconds",
    ),
    held(
        Modifiers::COMMAND,
        Key::ArrowUp,
        || Action::VolumeBy(VOLUME_STEP),
        "Volume up",
    ),
    held(
        Modifiers::COMMAND,
        Key::ArrowDown,
        || Action::VolumeBy(-VOLUME_STEP),
        "Volume down",
    ),
    plain(Key::M, || Action::ToggleMute, "Mute"),
    plain(Key::S, || Action::ToggleShuffle, "Shuffle"),
    plain(Key::R, || Action::CycleRepeat, "Repeat"),
    plain(Key::Q, || Action::ToggleQueue, "Queue"),
    plain(Key::L, || Action::ToggleLyrics, "Lyrics"),
    held(
        Modifiers::COMMAND,
        Key::B,
        || Action::ToggleSidebar,
        "Show or hide the sidebar",
    ),
    held(
        Modifiers::COMMAND,
        Key::M,
        || Action::ToggleMiniPlayer,
        "Mini player",
    ),
    held(Modifiers::ALT, Key::ArrowLeft, || Action::Back, "Back"),
    held(
        Modifiers::ALT,
        Key::ArrowRight,
        || Action::Forward,
        "Forward",
    ),
];

pub fn handle(ctx: &Context, actions: &mut Vec<Action>) {
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input_mut(|input| {
        for shortcut in SHORTCUTS {
            if typing && shortcut.yields_to_typing {
                continue;
            }
            if input.consume_key(shortcut.modifiers, shortcut.key) {
                actions.push((shortcut.action)());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_two_shortcuts_share_their_keys() {
        for (index, first) in SHORTCUTS.iter().enumerate() {
            for second in &SHORTCUTS[index + 1..] {
                assert!(
                    (first.modifiers, first.key) != (second.modifiers, second.key),
                    "{} and {} share keys",
                    first.description,
                    second.description
                );
            }
        }
    }

    #[test]
    fn letters_and_space_are_left_to_a_text_field() {
        for shortcut in SHORTCUTS {
            if shortcut.modifiers == Modifiers::NONE {
                assert!(shortcut.yields_to_typing, "{}", shortcut.description);
            }
        }
    }
}
