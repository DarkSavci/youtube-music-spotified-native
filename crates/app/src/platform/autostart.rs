//! Starting with Windows.
//!
//! The system runs whatever is listed under the user's own `Run` key at
//! sign-in. The entry is the path of this executable with `--hidden`, so
//! the app comes up in the tray rather than in the person's face.
//!
//! The system is the source of truth, not a saved preference: the entry
//! can be switched off in Task Manager without the app knowing, which
//! Windows notes in a second key and leaves the entry itself in place.

use std::io;
use std::process::{Command, Stdio};

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
/// Where Task Manager's Startup tab keeps what it has switched off.
const APPROVED_KEY: &str =
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
/// The entry's name. Distinct from the Electron app's, which has its own.
const ENTRY: &str = "YoutubeMusicSpotifiedNative";

/// Adds or removes the entry.
pub fn set(enabled: bool) -> io::Result<()> {
    if !enabled {
        // Removing an entry that is not there is the wanted outcome already.
        let _ = reg().args(["delete", RUN_KEY, "/v", ENTRY, "/f"]).status();
        return Ok(());
    }
    write(&launch_command()?)?;
    // Turning it on also undoes a switch-off in Task Manager, which is
    // what someone flipping the switch back on means.
    let _ = reg()
        .args(["delete", APPROVED_KEY, "/v", ENTRY, "/f"])
        .status();
    Ok(())
}

/// Whether the app will start at sign-in: the entry is there, and Task
/// Manager has not switched it off.
pub fn is_enabled() -> bool {
    query(RUN_KEY).is_some_and(|run| command_in(&run).is_some())
        && query(APPROVED_KEY).is_none_or(|approved| !switched_off(&approved))
}

/// Points an existing entry at this executable. One written by a copy that
/// has since moved would start nothing, or the old copy. Does nothing
/// unless there is an entry and it names another path; whether it was
/// switched off is left as it was.
pub fn refresh() {
    let Some(command) = query(RUN_KEY).and_then(|run| command_in(&run)) else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    if launches(&command, &exe.display().to_string()) {
        return;
    }
    match launch_command().and_then(|launch| write(&launch)) {
        Ok(()) => log::info!("the startup entry now points at this copy"),
        Err(error) => log::warn!("the startup entry could not be moved: {error}"),
    }
}

fn launch_command() -> io::Result<String> {
    let exe = std::env::current_exe()?;
    Ok(format!("\"{}\" --hidden", exe.display()))
}

fn write(launch: &str) -> io::Result<()> {
    let status = reg()
        .args([
            "add", RUN_KEY, "/v", ENTRY, "/t", "REG_SZ", "/d", launch, "/f",
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other("the startup entry could not be written"))
    }
}

/// What `reg query` prints for the entry under `key`; `None` if it is not
/// there.
fn query(key: &str) -> Option<String> {
    let output = reg()
        .args(["query", key, "/v", ENTRY])
        .stdout(Stdio::piped())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The command in `reg query`'s answer for a text value.
fn command_in(answer: &str) -> Option<String> {
    answer.lines().find_map(|line| {
        let (_, value) = line
            .split_once("REG_EXPAND_SZ")
            .or_else(|| line.split_once("REG_SZ"))?;
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

/// Whether `reg query`'s answer for the approval says "switched off": its
/// bytes then begin with 03.
fn switched_off(answer: &str) -> bool {
    answer
        .lines()
        .find_map(|line| line.split_once("REG_BINARY"))
        .is_some_and(|(_, bytes)| bytes.trim().starts_with("03"))
}

/// Whether `command`, the executable quoted or not and then its arguments,
/// starts the executable at `exe`.
fn launches(command: &str, exe: &str) -> bool {
    command
        .replace('"', "")
        .to_lowercase()
        .starts_with(&exe.to_lowercase())
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

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run\r\n    \
        YoutubeMusicSpotifiedNative    REG_SZ    \"C:\\Apps\\Spotified\\app.exe\" --hidden\r\n\r\n";

    #[test]
    fn the_command_is_read_out_of_what_reg_prints() {
        assert_eq!(
            command_in(RUN).as_deref(),
            Some("\"C:\\Apps\\Spotified\\app.exe\" --hidden")
        );
        assert_eq!(command_in("ERROR: the value was not found"), None);
    }

    #[test]
    fn an_entry_switched_off_in_task_manager_reads_as_off() {
        let off = "    YoutubeMusicSpotifiedNative    REG_BINARY    030000000000000000000000";
        let on = "    YoutubeMusicSpotifiedNative    REG_BINARY    020000000000000000000000";
        assert!(switched_off(off));
        assert!(!switched_off(on));
        assert!(!switched_off(""));
    }

    #[test]
    fn an_entry_for_a_copy_that_has_moved_is_told_from_this_ones() {
        let command = "\"C:\\Apps\\Spotified\\app.exe\" --hidden";
        assert!(launches(command, "c:\\apps\\spotified\\APP.exe"));
        assert!(launches(
            "C:\\Apps\\Spotified\\app.exe --hidden",
            "C:\\Apps\\Spotified\\app.exe"
        ));
        assert!(!launches(command, "D:\\Elsewhere\\app.exe"));
    }
}
