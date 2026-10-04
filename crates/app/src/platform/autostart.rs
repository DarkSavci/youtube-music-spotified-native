//! Starting with Windows.
//!
//! The system runs whatever is listed under the user's own `Run` key at
//! sign-in. The entry is the path of this executable with `--hidden`, so
//! the app comes up in the tray rather than in the person's face.

use std::io;
use std::process::{Command, Stdio};

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
/// The entry's name. Distinct from the Electron app's, which has its own.
const ENTRY: &str = "YoutubeMusicSpotifiedNative";

/// Adds or removes the entry.
pub fn set(enabled: bool) -> io::Result<()> {
    let mut reg = reg();
    if enabled {
        let exe = std::env::current_exe()?;
        let launch = format!("\"{}\" --hidden", exe.display());
        reg.args([
            "add", RUN_KEY, "/v", ENTRY, "/t", "REG_SZ", "/d", &launch, "/f",
        ]);
    } else {
        reg.args(["delete", RUN_KEY, "/v", ENTRY, "/f"]);
    }
    let status = reg.status()?;
    // Removing an entry that is not there is the wanted outcome already.
    if status.success() || !enabled {
        Ok(())
    } else {
        Err(io::Error::other("the startup entry could not be written"))
    }
}

/// Whether the entry is there. Asked of the system rather than remembered,
/// since it can be switched off from Task Manager too.
pub fn is_enabled() -> bool {
    reg()
        .args(["query", RUN_KEY, "/v", ENTRY])
        .status()
        .is_ok_and(|status| status.success())
}

/// `reg.exe`, quiet and without a console window of its own.
fn reg() -> Command {
    let mut command = Command::new("reg");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::sidecar::hide_console(&mut command);
    command
}
