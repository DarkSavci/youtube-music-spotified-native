//! What the app asks of the operating system. Every call into Windows'
//! own interfaces is in here, behind functions that are safe to call and
//! that fail by saying so in the log, never by stopping the app.

pub mod autostart;
pub mod identity;
pub mod media_keys;
pub mod notify;
pub mod shell;
pub mod taskbar;
pub mod tray;
