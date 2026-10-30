//! Starting with the system.
//!
//! Each system has its own place for what starts at sign-in, and is the
//! source of truth about it, not a saved preference. What is listed is this
//! app with `--hidden`, so it comes up in the tray rather than in the
//! person's face.

#[cfg(target_os = "linux")]
#[path = "autostart_linux.rs"]
mod system;
#[cfg(target_os = "macos")]
#[path = "autostart_macos.rs"]
mod system;
#[cfg(windows)]
#[path = "autostart_windows.rs"]
mod system;

pub use system::{is_enabled, refresh, set};
