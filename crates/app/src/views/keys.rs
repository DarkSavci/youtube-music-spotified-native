//! Keyboard shortcuts.
//!
//! The old app's table, key for key, with the sidebar's and the mini
//! player's own added. Typing is never intercepted: while a text field has
//! the caret every shortcut is left to it, so Space in the search field
//! types a space and does not pause the music. And the keys must match
//! exactly: Ctrl+Right is the next song and plain Right a seek, and neither
//! answers to the other.

use eframe::egui::{Context, Event, Key, Modifiers};

use crate::actions::Action;
use crate::state::Page;

const SEEK_STEP_MS: i64 = 5000;
const VOLUME_STEP: f32 = 0.05;
/// What a new playlist is called until it is given a name.
const NEW_PLAYLIST_NAME: &str = "My playlist";

/// What a shortcut is listed under in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Playback,
    Navigation,
    Interface,
}

impl Group {
    pub const EVERY: [Group; 3] = [Group::Playback, Group::Navigation, Group::Interface];

    pub fn label(self) -> &'static str {
        match self {
            Group::Playback => "Playback",
            Group::Navigation => "Navigation",
            Group::Interface => "Interface",
        }
    }
}

/// One shortcut: the keys, what it does, and how the list in Settings
/// describes it.
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: Key,
    pub action: fn() -> Action,
    pub description: &'static str,
    pub group: Group,
}

const ALT_SHIFT: Modifiers = Modifiers {
    alt: true,
    ctrl: false,
    shift: true,
    mac_cmd: false,
    command: false,
};

const fn shortcut(
    modifiers: Modifiers,
    key: Key,
    group: Group,
    description: &'static str,
    action: fn() -> Action,
) -> Shortcut {
    Shortcut {
        modifiers,
        key,
        action,
        description,
        group,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const CTRL: Modifiers = Modifiers::COMMAND;
const ALT: Modifiers = Modifiers::ALT;

pub const SHORTCUTS: &[Shortcut] = &[
    shortcut(NONE, Key::Space, Group::Playback, "Play / pause", || {
        Action::TogglePlay
    }),
    shortcut(CTRL, Key::ArrowRight, Group::Playback, "Next track", || {
        Action::Next
    }),
    shortcut(
        CTRL,
        Key::ArrowLeft,
        Group::Playback,
        "Previous track",
        || Action::Previous,
    ),
    shortcut(
        NONE,
        Key::ArrowRight,
        Group::Playback,
        "Seek forward 5s",
        || Action::SeekBy(SEEK_STEP_MS),
    ),
    shortcut(
        NONE,
        Key::ArrowLeft,
        Group::Playback,
        "Seek back 5s",
        || Action::SeekBy(-SEEK_STEP_MS),
    ),
    shortcut(CTRL, Key::ArrowUp, Group::Playback, "Volume up", || {
        Action::VolumeBy(VOLUME_STEP)
    }),
    shortcut(CTRL, Key::ArrowDown, Group::Playback, "Volume down", || {
        Action::VolumeBy(-VOLUME_STEP)
    }),
    shortcut(NONE, Key::M, Group::Playback, "Mute", || Action::ToggleMute),
    shortcut(NONE, Key::S, Group::Playback, "Shuffle", || {
        Action::ToggleShuffle
    }),
    shortcut(NONE, Key::R, Group::Playback, "Repeat mode", || {
        Action::CycleRepeat
    }),
    shortcut(
        CTRL,
        Key::S,
        Group::Playback,
        "Save the current track",
        || Action::SaveCurrent,
    ),
    shortcut(CTRL, Key::H, Group::Navigation, "Home", || {
        Action::Open(Page::Home)
    }),
    shortcut(NONE, Key::Slash, Group::Navigation, "Search", || {
        Action::Open(Page::Search)
    }),
    shortcut(CTRL, Key::L, Group::Navigation, "Your library", || {
        Action::Open(Page::Stats)
    }),
    shortcut(ALT, Key::ArrowLeft, Group::Navigation, "Back", || {
        Action::Back
    }),
    shortcut(ALT, Key::ArrowRight, Group::Navigation, "Forward", || {
        Action::Forward
    }),
    shortcut(ALT_SHIFT, Key::H, Group::Navigation, "Home", || {
        Action::Open(Page::Home)
    }),
    shortcut(CTRL, Key::K, Group::Navigation, "Focus search", || {
        Action::FocusSearch
    }),
    shortcut(
        ALT_SHIFT,
        Key::L,
        Group::Navigation,
        "Your listening",
        || Action::Open(Page::Stats),
    ),
    shortcut(CTRL, Key::Comma, Group::Navigation, "Settings", || {
        Action::Open(Page::Settings)
    }),
    shortcut(NONE, Key::Q, Group::Interface, "Toggle queue", || {
        Action::ToggleQueue
    }),
    shortcut(NONE, Key::L, Group::Interface, "Lyrics", || {
        Action::ToggleLyrics
    }),
    shortcut(
        NONE,
        Key::F,
        Group::Interface,
        "Now playing, full screen",
        || Action::ToggleFullscreenPlayer,
    ),
    // "M" is mute; "P" for picture-in-picture, which is what it is.
    shortcut(NONE, Key::P, Group::Interface, "Mini player", || {
        Action::ToggleMiniPlayer
    }),
    shortcut(CTRL, Key::M, Group::Interface, "Mini player", || {
        Action::ToggleMiniPlayer
    }),
    shortcut(CTRL, Key::N, Group::Interface, "New playlist", || {
        Action::NewPlaylist {
            name: NEW_PLAYLIST_NAME.to_owned(),
            track_ids: Vec::new(),
        }
    }),
    shortcut(
        CTRL,
        Key::B,
        Group::Interface,
        "Collapse or widen the sidebar",
        || Action::ToggleSidebar,
    ),
];

/// The shortcut these keys are, if they are one. Ctrl is Cmd on a Mac.
fn matching(modifiers: Modifiers, key: Key) -> Option<&'static Shortcut> {
    SHORTCUTS
        .iter()
        .find(|shortcut| shortcut.key == key && modifiers.matches_exact(shortcut.modifiers))
}

pub fn handle(ctx: &Context, actions: &mut Vec<Action>) {
    // The caret is in a field: every key is the field's.
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    // A menu is open: the arrows, Enter and Space move through it.
    if super::widgets::menu::is_open(ctx) {
        return;
    }
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            match matching(*modifiers, *key) {
                Some(shortcut) => {
                    actions.push((shortcut.action)());
                    // Taken: nothing under the pointer answers it as well.
                    false
                }
                None => true,
            }
        });
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
    fn keys_must_match_exactly_to_be_a_shortcut() {
        let does = |modifiers, key| matching(modifiers, key).map(|shortcut| shortcut.description);
        assert_eq!(does(NONE, Key::ArrowRight), Some("Seek forward 5s"));
        assert_eq!(does(CTRL, Key::ArrowRight), Some("Next track"));
        assert_eq!(does(ALT, Key::ArrowRight), Some("Forward"));
        // Shift with an arrow selects rows; it is nobody's shortcut.
        assert_eq!(does(Modifiers::SHIFT, Key::ArrowRight), None);
        assert_eq!(does(NONE, Key::S), Some("Shuffle"));
        assert_eq!(does(CTRL, Key::S), Some("Save the current track"));
        assert_eq!(does(ALT_SHIFT, Key::L), Some("Your listening"));
        assert_eq!(does(ALT, Key::L), None);
    }

    #[test]
    fn the_old_apps_table_is_all_here() {
        let has = |modifiers, key| matching(modifiers, key).is_some();
        for (modifiers, key) in [
            (NONE, Key::Space),
            (CTRL, Key::ArrowRight),
            (CTRL, Key::ArrowLeft),
            (NONE, Key::ArrowRight),
            (NONE, Key::ArrowLeft),
            (CTRL, Key::ArrowUp),
            (CTRL, Key::ArrowDown),
            (NONE, Key::M),
            (NONE, Key::S),
            (NONE, Key::R),
            (CTRL, Key::H),
            (NONE, Key::Slash),
            (CTRL, Key::L),
            (NONE, Key::Q),
            (CTRL, Key::S),
            (ALT, Key::ArrowLeft),
            (ALT, Key::ArrowRight),
            (ALT_SHIFT, Key::H),
            (CTRL, Key::K),
            (ALT_SHIFT, Key::L),
            (CTRL, Key::Comma),
            (NONE, Key::L),
            (NONE, Key::F),
            (NONE, Key::P),
            (CTRL, Key::N),
        ] {
            assert!(has(modifiers, key), "{modifiers:?} {key:?}");
        }
    }
}
