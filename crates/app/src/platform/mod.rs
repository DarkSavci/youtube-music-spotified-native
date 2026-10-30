//! What the app asks of the operating system. Every call into a system's
//! own interfaces is in here, behind functions that are safe to call and
//! that fail by saying so in the log, never by stopping the app.

pub mod autostart;
pub mod identity;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod media_keys;
pub mod notify;
pub mod pick;
pub mod region;
pub mod shell;
// Only Windows has these buttons; elsewhere nothing calls this.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod taskbar;
pub mod tray;
#[cfg(target_os = "linux")]
pub mod x11;
