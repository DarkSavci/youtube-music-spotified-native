//! The icon in the notification area: where the app lives while its window
//! is closed and the music plays on.

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::icon;

/// What the tray asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Bring the window back.
    Show,
    TogglePlay,
    Next,
    Previous,
    Quit,
}

/// Kept for as long as the icon should show; dropping it removes the icon.
pub struct Tray {
    _icon: TrayIcon,
}

impl Tray {
    /// Puts the icon up. `on_action` is called from the system's event
    /// handling. `None` if the system has no notification area to offer;
    /// the app then quits when its window closes, as it has nowhere to go.
    pub fn new(on_action: impl Fn(TrayAction) + Send + Sync + Clone + 'static) -> Option<Self> {
        let entries = [
            ("Show", Some(TrayAction::Show)),
            ("", None),
            ("Play or pause", Some(TrayAction::TogglePlay)),
            ("Next", Some(TrayAction::Next)),
            ("Previous", Some(TrayAction::Previous)),
            ("", None),
            ("Quit", Some(TrayAction::Quit)),
        ];
        let menu = Menu::new();
        let mut actions = Vec::new();
        for (label, action) in entries {
            let appended = match action {
                Some(action) => {
                    let item = MenuItem::new(label, true, None);
                    actions.push((item.id().clone(), action));
                    menu.append(&item)
                }
                None => menu.append(&PredefinedMenuItem::separator()),
            };
            appended
                .inspect_err(|error| log::warn!("tray menu: {error}"))
                .ok()?;
        }

        let size = 32;
        let image = tray_icon::Icon::from_rgba(icon::rgba(size), size, size)
            .inspect_err(|error| log::warn!("tray icon: {error}"))
            .ok()?;
        let tray = TrayIconBuilder::new()
            .with_icon(image)
            .with_tooltip(crate::APP_NAME)
            .with_menu(Box::new(menu))
            // A left click brings the window back; the menu is on the right.
            .with_menu_on_left_click(false)
            .build()
            .inspect_err(|error| log::warn!("no tray icon: {error}"))
            .ok()?;

        let on_menu = on_action.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let chosen = actions.iter().find(|(id, _)| *id == event.id);
            if let Some((_, action)) = chosen {
                on_menu(*action);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                on_action(TrayAction::Show);
            }
        }));
        Some(Self { _icon: tray })
    }
}
