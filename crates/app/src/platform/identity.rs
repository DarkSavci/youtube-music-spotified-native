//! Who this app is to Windows, and how Windows dresses what it draws for
//! it.
//!
//! The taskbar groups windows, pins shortcuts and files notifications by
//! an application id. Left to itself Windows makes one up from the path of
//! the executable, which changes when the app is installed elsewhere and
//! is not what the installer's shortcuts carry; so the id is said outright,
//! here and on those shortcuts (`packaging/windows/spotified.iss`), and it
//! is this app's own, not the Electron app's, beside which it installs.
//!
//! The menu on the tray icon is drawn by Windows, light unless told
//! otherwise. It is told to follow the app's theme.

/// The application id. The installer stamps the same one on its shortcuts.
pub const APP_ID: &str = "dev.darksavci.youtubemusicspotified.native";

/// Tells Windows the app's id. Once, before any window is made.
pub fn claim() {
    #[cfg(windows)]
    win::claim();
}

/// Has the menus Windows draws for this app (the tray icon's) follow the
/// theme: dark with a dark one, light with a light one.
pub fn dress_menus(dark: bool) {
    #[cfg(windows)]
    win::dress_menus(dark);
    #[cfg(not(windows))]
    let _ = dark;
}

#[cfg(windows)]
mod win {
    use windows::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
    };
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    use windows::core::{HSTRING, PCSTR, w};

    pub(super) fn claim() {
        let id = HSTRING::from(super::APP_ID);
        // SAFETY: the id is a terminated string that outlives the call.
        if let Err(error) = unsafe { SetCurrentProcessExplicitAppUserModelID(&id) } {
            log::warn!("the application id was not taken: {error}");
        }
    }

    /// `SetPreferredAppMode` and `FlushMenuThemes`, which uxtheme.dll
    /// exports by number alone (Windows 10 1903 and later). They are how
    /// every program with a dark menu gets one: Windows has no documented
    /// way to ask for it.
    const SET_PREFERRED_APP_MODE: usize = 135;
    const FLUSH_MENU_THEMES: usize = 136;
    /// The modes that hold whatever the system's own setting is.
    const FORCE_DARK: i32 = 2;
    const FORCE_LIGHT: i32 = 3;

    pub(super) fn dress_menus(dark: bool) {
        // SAFETY: the library is the system's own, loaded from its folder.
        let Ok(uxtheme) =
            (unsafe { LoadLibraryExW(w!("uxtheme.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) })
        else {
            return;
        };
        // An export known only by number is asked for with the number in
        // place of the name's address.
        let by_number = |number: usize| PCSTR(number as *const u8);
        // SAFETY: the handle is the library just loaded; a number it does
        // not export gives `None`, on a Windows too old to have these.
        let (set, flush) = unsafe {
            (
                GetProcAddress(uxtheme, by_number(SET_PREFERRED_APP_MODE)),
                GetProcAddress(uxtheme, by_number(FLUSH_MENU_THEMES)),
            )
        };
        let (Some(set), Some(flush)) = (set, flush) else {
            log::debug!("this Windows has no dark menus to ask for");
            return;
        };
        let mode = if dark { FORCE_DARK } else { FORCE_LIGHT };
        // SAFETY: these are the two exports' signatures as Windows has had
        // them since they appeared: the mode in and the old mode out, and
        // nothing in or out.
        unsafe {
            let set: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(set);
            let flush: unsafe extern "system" fn() = std::mem::transmute(flush);
            set(mode);
            flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_installers_shortcuts_carry_the_id_the_app_claims() {
        let script = include_str!("../../../../packaging/windows/spotified.iss");
        let stamped = format!("AppUserModelID: \"{APP_ID}\"");
        // The Start menu's shortcut and the desktop's.
        assert_eq!(script.matches(&stamped).count(), 2, "{stamped}");
    }

    #[test]
    fn the_id_is_not_the_electron_apps() {
        // They are installed side by side and must not share a taskbar
        // button or each other's notifications.
        assert_ne!(APP_ID, "dev.darksavci.youtubemusicspotified");
        assert!(APP_ID.len() <= 128 && !APP_ID.contains(' '));
    }
}
