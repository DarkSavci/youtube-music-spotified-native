//! Starting with a Linux desktop: an entry in the XDG autostart folder.

use std::io;
use std::path::{Path, PathBuf};

const ENTRY: &str = "dev.darksavci.spotified.native.desktop";

fn entry() -> Option<PathBuf> {
    Some(
        directories::BaseDirs::new()?
            .config_dir()
            .join("autostart")
            .join(ENTRY),
    )
}

/// Adds or removes the entry.
pub fn set(enabled: bool) -> io::Result<()> {
    let file = entry().ok_or_else(|| io::Error::other("no home directory"))?;
    if !enabled {
        return match std::fs::remove_file(file) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(file, text(&crate::platform::shell::launcher()?))
}

/// Whether the app will start at sign-in. Desktops switch an entry off by
/// adding `Hidden=true` to it.
pub fn is_enabled() -> bool {
    entry()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .is_some_and(|text| !switched_off(&text))
}

/// Points an existing entry at this copy. One written by a copy that has
/// since moved would start nothing, or the old copy.
pub fn refresh() {
    let Some(file) = entry() else {
        return;
    };
    let Ok(old) = std::fs::read_to_string(&file) else {
        return;
    };
    let Ok(program) = crate::platform::shell::launcher() else {
        return;
    };
    if switched_off(&old) || old == text(&program) {
        return;
    }
    match std::fs::write(file, text(&program)) {
        Ok(()) => log::info!("the startup entry now points at this copy"),
        Err(error) => log::warn!("the startup entry could not be moved: {error}"),
    }
}

fn switched_off(entry: &str) -> bool {
    entry.lines().any(|line| line.trim() == "Hidden=true")
}

/// The entry for `program`, quoted as the Desktop Entry specification asks.
fn text(program: &Path) -> String {
    let quoted = program
        .display()
        .to_string()
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('$', "\\\\$")
        .replace('`', "\\\\`");
    format!(
        "[Desktop Entry]\nType=Application\nName={}\nExec=\"{quoted}\" --hidden\n\
         Icon={}\nTerminal=false\n",
        crate::APP_NAME,
        crate::platform::identity::DESKTOP_ICON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_starts_this_copy_hidden_with_its_path_quoted() {
        let text = text(Path::new("/home/ada/My Apps/app.AppImage"));
        assert!(text.contains("Exec=\"/home/ada/My Apps/app.AppImage\" --hidden\n"));
        assert!(text.starts_with("[Desktop Entry]\n"));
    }

    #[test]
    fn an_entry_a_desktop_switched_off_reads_as_off() {
        assert!(switched_off("[Desktop Entry]\nHidden=true\n"));
        assert!(!switched_off("[Desktop Entry]\nHidden=false\n"));
    }
}
