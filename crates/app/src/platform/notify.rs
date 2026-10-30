//! A notification from the system itself, for the one thing worth saying
//! while the window may be out of sight: an update has been downloaded.
//!
//! A Mac has none here: Apple's notifications may not serve an app that is
//! not signed by a developer, and the toast inside the window says the same.
//!
//! Windows files a notification under the application id the process
//! claimed ([`super::identity`]) and shows it only for an app with a Start
//! menu shortcut carrying that id, which the installer makes. A copy run
//! from a build folder has no such shortcut and is shown nothing; the
//! toast inside the window says the same there.

/// Says that `version` is downloaded and waits to be installed. A click on
/// the notification calls `on_click`, from a thread of the system's.
pub fn update_ready(version: &str, on_click: impl Fn() + Send + 'static) {
    let title = format!("{} {version} is ready", crate::APP_NAME);
    let body = "It installs when you quit. Click to restart and update now.";
    #[cfg(windows)]
    if let Err(error) = win::show(&markup(&title, body), on_click) {
        // No shortcut with the app's id, or notifications switched off.
        log::debug!("no notification was shown: {error}");
    }
    #[cfg(target_os = "linux")]
    {
        // Waits on the notification, for its click.
        let spawned = std::thread::Builder::new()
            .name("notify".into())
            .spawn(move || {
                if let Err(error) = super::linux::notify(&title, body, on_click) {
                    log::debug!("no notification was shown: {error}");
                }
            });
        if let Err(error) = spawned {
            log::debug!("no notification was shown: {error}");
        }
    }
    #[cfg(target_os = "macos")]
    let _ = (title, body, on_click);
}

/// The notification as Windows reads it: two lines of text.
#[cfg(windows)]
fn markup(title: &str, body: &str) -> String {
    format!(
        "<toast><visual><binding template=\"ToastGeneric\">\
         <text>{}</text><text>{}</text></binding></visual></toast>",
        escaped(title),
        escaped(body)
    )
}

/// Text as it may stand inside a tag.
#[cfg(windows)]
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(windows)]
mod win {
    use std::sync::Mutex;

    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::{HSTRING, IInspectable};

    /// The notification last shown. Its click is heard only while the
    /// object that was shown is still held.
    static SHOWN: Mutex<Option<ToastNotification>> = Mutex::new(None);

    pub(super) fn show(
        markup: &str,
        on_click: impl Fn() + Send + 'static,
    ) -> windows::core::Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(markup))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        let clicked = TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, _| {
            on_click();
            Ok(())
        });
        toast.Activated(&clicked)?;
        let id = HSTRING::from(super::super::identity::APP_ID);
        ToastNotificationManager::CreateToastNotifierWithId(&id)?.Show(&toast)?;
        if let Ok(mut shown) = SHOWN.lock() {
            *shown = Some(toast);
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_notification_is_two_lines_with_nothing_in_them_read_as_markup() {
        let said = markup("Rock & <Roll> 1.2.3 is ready", "Click to update.");
        assert_eq!(
            said,
            "<toast><visual><binding template=\"ToastGeneric\">\
             <text>Rock &amp; &lt;Roll&gt; 1.2.3 is ready</text>\
             <text>Click to update.</text></binding></visual></toast>"
        );
    }
}
