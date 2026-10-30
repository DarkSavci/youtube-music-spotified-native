//! What a Linux desktop offers over D-Bus: whether a tray host is there,
//! the file manager, and notifications. A desktop without one of them
//! simply goes without it.

use std::collections::HashMap;

use zbus::blocking::{Connection, MessageIterator};
use zbus::zvariant::Value;
use zbus::{MatchRule, message};

const NOTIFICATIONS: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";
const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
/// What hosts a StatusNotifierItem. Without it an icon is shown to nobody.
const TRAY_HOST: &str = "org.kde.StatusNotifierWatcher";

/// Whether this session has a message bus to speak on.
pub fn has_session_bus() -> bool {
    Connection::session()
        .inspect_err(|error| log::debug!("no session bus: {error}"))
        .is_ok()
}

/// Whether the desktop has anywhere to show a tray icon. Plain GNOME does
/// not, and an app closed to a tray nobody can see could not be got back.
pub fn has_tray_host() -> bool {
    let answer = || -> zbus::Result<bool> {
        let bus = Connection::session()?;
        let reply = bus.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &TRAY_HOST,
        )?;
        reply.body().deserialize()
    };
    answer()
        .inspect_err(|error| log::debug!("no session bus to ask for a tray: {error}"))
        .unwrap_or(false)
}

/// Asks the file manager to show this file in its folder.
pub fn show_in_file_manager(uri: &str) -> zbus::Result<()> {
    let bus = Connection::session()?;
    bus.call_method(
        Some(FILE_MANAGER),
        "/org/freedesktop/FileManager1",
        Some(FILE_MANAGER),
        "ShowItems",
        &(vec![uri], ""),
    )?;
    Ok(())
}

/// Shows a notification and waits for what becomes of it: `on_click` is
/// called if it is clicked. Blocks, so it is called on a thread of its own.
pub fn notify(title: &str, body: &str, on_click: impl Fn()) -> zbus::Result<()> {
    let bus = Connection::session()?;
    let rule = MatchRule::builder()
        .msg_type(message::Type::Signal)
        .interface(NOTIFICATIONS)?
        .build();
    let signals = MessageIterator::for_match_rule(rule, &bus, None)?;
    let hints: HashMap<&str, Value<'_>> = HashMap::new();
    let reply = bus.call_method(
        Some(NOTIFICATIONS),
        NOTIFICATIONS_PATH,
        Some(NOTIFICATIONS),
        "Notify",
        &(
            crate::APP_NAME,
            0u32,
            super::identity::DESKTOP_ICON,
            title,
            body,
            vec!["default", "Restart and update"],
            hints,
            -1i32,
        ),
    )?;
    let id: u32 = reply.body().deserialize()?;
    for signal in signals.flatten() {
        let header = signal.header();
        let member = header.member().map(|member| member.as_str());
        match member {
            Some("ActionInvoked") => {
                let clicked = signal.body().deserialize::<(u32, String)>();
                if clicked.is_ok_and(|(clicked, _)| clicked == id) {
                    on_click();
                    break;
                }
            }
            Some("NotificationClosed") => {
                let closed = signal.body().deserialize::<(u32, u32)>();
                if closed.is_ok_and(|(closed, _)| closed == id) {
                    break;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// These want a session bus with the desktop's services on it, or none of
/// them, so they are run by hand: `dbus-run-session -- cargo test -- --ignored`
/// beside something that answers for those services.
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    #[ignore = "needs a notification service on the session bus"]
    fn a_click_on_a_notification_is_heard() {
        let clicked = AtomicBool::new(false);
        notify("Title", "Body", || clicked.store(true, Ordering::Relaxed)).expect("shown");
        assert!(clicked.load(Ordering::Relaxed));
    }

    #[test]
    #[ignore = "needs a file manager on the session bus"]
    fn the_file_manager_is_asked_to_show_a_file() {
        show_in_file_manager("file:///tmp/a%20b.zip").expect("asked");
    }

    #[test]
    #[ignore = "needs a session bus with no tray host on it"]
    fn with_nobody_to_show_an_icon_there_is_no_tray() {
        assert!(has_session_bus());
        assert!(!has_tray_host());
    }
}
