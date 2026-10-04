//! The icon in the notification area: where the app lives while its window
//! is closed and the music plays on.
//!
//! A click opens the flyout, the player's controls in a small window by
//! the icon. A double click brings the main window back. The right button
//! has a menu for the things menus are for: quick transport, the mini
//! player, showing or hiding the window, a waiting update, quitting. The
//! tooltip says what is playing.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::shell::Area;
use crate::icon;

/// Windows cuts a tooltip off past this many characters, mid-word.
const TOOLTIP_MOST: usize = 127;
/// A menu item cannot wrap, and a long title would push the menu across
/// the screen; the tooltip carries the whole line.
const MENU_LINE_MOST: usize = 48;

/// What the tray asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    /// Bring the window back.
    Show,
    /// Put the window away, leaving the app in the tray.
    Hide,
    /// Open the flyout by the icon, which is here on the screen; or close
    /// it if it is open.
    Flyout(Area),
    TogglePlay,
    Next,
    Previous,
    /// Like what is playing, or take the like back.
    Like,
    /// Open the mini player, or close it.
    Mini,
    /// Restart into the update that is waiting.
    InstallUpdate,
    Quit,
}

impl TrayAction {
    /// The menu items that ask for something, by the ids they are given.
    const IN_MENU: [(&'static str, TrayAction); 9] = [
        ("show", TrayAction::Show),
        ("hide", TrayAction::Hide),
        ("toggle", TrayAction::TogglePlay),
        ("next", TrayAction::Next),
        ("previous", TrayAction::Previous),
        ("like", TrayAction::Like),
        ("mini", TrayAction::Mini),
        ("update", TrayAction::InstallUpdate),
        ("quit", TrayAction::Quit),
    ];

    fn id(self) -> &'static str {
        Self::IN_MENU
            .iter()
            .find(|(_, action)| *action == self)
            .map_or("", |(id, _)| id)
    }
}

/// What the icon's tooltip and menu are made from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// The song in the player: its title, and who it is by.
    pub track: Option<(String, String)>,
    pub playing: bool,
    /// Signed in, so a like has somewhere to go.
    pub can_like: bool,
    pub liked: bool,
    pub mini_open: bool,
    pub window_visible: bool,
    /// The version of an update that is downloaded and waiting.
    pub update: Option<String>,
}

impl Snapshot {
    /// What is playing, in a line.
    fn line(&self) -> Option<String> {
        let (title, artist) = self.track.as_ref()?;
        Some(if artist.is_empty() {
            title.clone()
        } else {
            format!("{title} — {artist}")
        })
    }

    fn tooltip(&self) -> String {
        let line = self.line().unwrap_or_else(|| crate::APP_NAME.to_owned());
        shortened(&line, TOOLTIP_MOST)
    }

    /// The menu, top to bottom, as it stands at this moment.
    fn menu(&self) -> Vec<Entry> {
        let has_track = self.track.is_some();
        let mut menu = Vec::new();
        if let Some(line) = self.line() {
            menu.push(Entry::Heading(shortened(&line, MENU_LINE_MOST)));
            menu.push(Entry::Separator);
        }
        let play = if self.playing { "Pause" } else { "Play" };
        let transport = [
            (play, TrayAction::TogglePlay),
            ("Next", TrayAction::Next),
            ("Previous", TrayAction::Previous),
        ];
        for (label, action) in transport {
            menu.push(Entry::Item {
                label: label.to_owned(),
                enabled: has_track,
                action,
            });
        }
        if has_track && self.can_like {
            let label = if self.liked {
                "Remove from Liked Music"
            } else {
                "Add to Liked Music"
            };
            menu.push(Entry::item(label, TrayAction::Like));
        }
        menu.push(Entry::Separator);
        menu.push(Entry::Check {
            label: "Mini player".to_owned(),
            checked: self.mini_open,
            action: TrayAction::Mini,
        });
        menu.push(if self.window_visible {
            Entry::item("Hide window", TrayAction::Hide)
        } else {
            Entry::item(&format!("Open {}", crate::APP_NAME), TrayAction::Show)
        });
        menu.push(Entry::Separator);
        if let Some(version) = &self.update {
            let label = format!("Restart to update to {version}");
            menu.push(Entry::item(&label, TrayAction::InstallUpdate));
        }
        let quit = format!("Quit {}", crate::APP_NAME);
        menu.push(Entry::item(&quit, TrayAction::Quit));
        menu
    }
}

/// `text`, cut to `most` characters with an ellipsis if it is longer.
fn shortened(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let kept: String = text.chars().take(most - 1).collect();
    format!("{kept}…")
}

#[derive(Debug, Clone, PartialEq)]
enum Entry {
    /// What is playing: said, not clickable.
    Heading(String),
    Separator,
    Item {
        label: String,
        enabled: bool,
        action: TrayAction,
    },
    Check {
        label: String,
        checked: bool,
        action: TrayAction,
    },
}

impl Entry {
    fn item(label: &str, action: TrayAction) -> Self {
        Entry::Item {
            label: label.to_owned(),
            enabled: true,
            action,
        }
    }
}

/// Kept for as long as the icon should show; dropping it removes the icon.
pub struct Tray {
    icon: TrayIcon,
    /// What the tooltip and the menu were last made from.
    shown: Option<Snapshot>,
}

impl Tray {
    /// Puts the icon up. `on_action` is called from the system's event
    /// handling. `None` if the system has no notification area to offer;
    /// the app then quits when its window closes, as it has nowhere to go.
    pub fn new(on_action: impl Fn(TrayAction) + Send + Sync + Clone + 'static) -> Option<Self> {
        let size = 32;
        let image = tray_icon::Icon::from_rgba(icon::rgba(size), size, size)
            .inspect_err(|error| log::warn!("tray icon: {error}"))
            .ok()?;
        let icon = TrayIconBuilder::new()
            .with_icon(image)
            .with_tooltip(crate::APP_NAME)
            // A left click opens the flyout; the menu is on the right.
            .with_menu_on_left_click(false)
            .build()
            .inspect_err(|error| log::warn!("no tray icon: {error}"))
            .ok()?;

        let on_menu = on_action.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let chosen = TrayAction::IN_MENU
                .iter()
                .find(|(id, _)| event.id == *id)
                .map(|(_, action)| *action);
            if let Some(action) = chosen {
                on_menu(action);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } => {
                let (left, top) = (rect.position.x as f32, rect.position.y as f32);
                on_action(TrayAction::Flyout(Area {
                    left,
                    top,
                    right: left + rect.size.width as f32,
                    bottom: top + rect.size.height as f32,
                }));
            }
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => on_action(TrayAction::Show),
            _ => {}
        }));
        let mut tray = Self { icon, shown: None };
        tray.show(Snapshot::default());
        Some(tray)
    }

    /// Makes the tooltip and the menu say what is so now, if they do not.
    pub fn show(&mut self, snapshot: Snapshot) {
        if self.shown.as_ref() == Some(&snapshot) {
            return;
        }
        if let Err(error) = self.icon.set_tooltip(Some(snapshot.tooltip())) {
            log::debug!("tray tooltip: {error}");
        }
        match build(&snapshot.menu()) {
            Ok(menu) => self.icon.set_menu(Some(Box::new(menu))),
            Err(error) => log::warn!("tray menu: {error}"),
        }
        self.shown = Some(snapshot);
    }

    /// Where the icon is on the screen, when the system will say.
    pub fn place(&self) -> Option<Area> {
        let rect = self.icon.rect()?;
        let (left, top) = (rect.position.x as f32, rect.position.y as f32);
        Some(Area {
            left,
            top,
            right: left + rect.size.width as f32,
            bottom: top + rect.size.height as f32,
        })
    }
}

fn build(entries: &[Entry]) -> tray_icon::menu::Result<Menu> {
    let menu = Menu::new();
    for entry in entries {
        match entry {
            Entry::Heading(text) => menu.append(&MenuItem::new(text, false, None))?,
            Entry::Separator => menu.append(&PredefinedMenuItem::separator())?,
            Entry::Item {
                label,
                enabled,
                action,
            } => menu.append(&MenuItem::with_id(action.id(), label, *enabled, None))?,
            Entry::Check {
                label,
                checked,
                action,
            } => menu.append(&CheckMenuItem::with_id(
                action.id(),
                label,
                true,
                *checked,
                None,
            ))?,
        }
    }
    Ok(menu)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(snapshot: &Snapshot) -> Vec<String> {
        snapshot
            .menu()
            .into_iter()
            .map(|entry| match entry {
                Entry::Heading(text) => format!("[{text}]"),
                Entry::Separator => "-".to_owned(),
                Entry::Item { label, enabled, .. } if enabled => label,
                Entry::Item { label, .. } => format!("({label})"),
                Entry::Check { label, checked, .. } if checked => format!("{label} ✓"),
                Entry::Check { label, .. } => label,
            })
            .collect()
    }

    fn playing() -> Snapshot {
        Snapshot {
            track: Some(("One More Time".into(), "Daft Punk".into())),
            playing: true,
            window_visible: true,
            ..Snapshot::default()
        }
    }

    #[test]
    fn with_nothing_playing_the_transport_is_there_but_dimmed() {
        assert_eq!(
            labels(&Snapshot::default()),
            [
                "(Play)",
                "(Next)",
                "(Previous)",
                "-",
                "Mini player",
                "Open Youtube Music Spotified",
                "-",
                "Quit Youtube Music Spotified"
            ]
        );
    }

    #[test]
    fn the_menu_says_what_is_playing_and_offers_to_pause_it() {
        assert_eq!(
            labels(&playing())[..6],
            [
                "[One More Time — Daft Punk]",
                "-",
                "Pause",
                "Next",
                "Previous",
                "-"
            ]
        );
        assert!(labels(&playing()).contains(&"Hide window".to_owned()));
    }

    #[test]
    fn a_like_is_offered_only_with_an_account_and_says_which_way_it_goes() {
        let signed_in = Snapshot {
            can_like: true,
            ..playing()
        };
        assert!(labels(&signed_in).contains(&"Add to Liked Music".to_owned()));
        let liked = Snapshot {
            liked: true,
            ..signed_in
        };
        assert!(labels(&liked).contains(&"Remove from Liked Music".to_owned()));
        assert!(
            !labels(&playing())
                .iter()
                .any(|label| label.contains("Liked"))
        );
    }

    #[test]
    fn a_waiting_update_is_offered_above_quit() {
        let waiting = Snapshot {
            update: Some("0.4.0".into()),
            mini_open: true,
            ..playing()
        };
        let labels = labels(&waiting);
        assert_eq!(
            labels[labels.len() - 2..],
            ["Restart to update to 0.4.0", "Quit Youtube Music Spotified"]
        );
        assert!(labels.contains(&"Mini player ✓".to_owned()));
    }

    #[test]
    fn the_tooltip_says_what_is_playing_or_names_the_app() {
        assert_eq!(Snapshot::default().tooltip(), "Youtube Music Spotified");
        assert_eq!(playing().tooltip(), "One More Time — Daft Punk");
        let untitled = Snapshot {
            track: Some(("Song".into(), String::new())),
            ..Snapshot::default()
        };
        assert_eq!(untitled.tooltip(), "Song");
    }

    #[test]
    fn a_long_line_is_cut_where_windows_would_cut_it_but_with_an_ellipsis() {
        let long = Snapshot {
            track: Some(("é".repeat(200), String::new())),
            ..Snapshot::default()
        };
        let tooltip = long.tooltip();
        assert_eq!(tooltip.chars().count(), TOOLTIP_MOST);
        assert!(tooltip.ends_with('…'));
        let Some(Entry::Heading(line)) = long.menu().into_iter().next() else {
            panic!("the menu starts with what is playing");
        };
        assert_eq!(line.chars().count(), MENU_LINE_MOST);
    }

    #[test]
    fn every_menu_action_has_an_id_of_its_own() {
        for (id, action) in TrayAction::IN_MENU {
            assert_eq!(action.id(), id);
        }
    }
}
