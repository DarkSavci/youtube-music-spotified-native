//! Starting with a Mac: a launch agent in the person's own library.

use std::io;
use std::path::{Path, PathBuf};

const LABEL: &str = "dev.darksavci.spotified.native";

fn agent() -> Option<PathBuf> {
    Some(
        directories::BaseDirs::new()?
            .home_dir()
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}

/// Adds or removes the agent. It takes effect at the next sign-in, which is
/// when it is wanted, so nothing is loaded now.
pub fn set(enabled: bool) -> io::Result<()> {
    let file = agent().ok_or_else(|| io::Error::other("no home directory"))?;
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

pub fn is_enabled() -> bool {
    agent().is_some_and(|file| file.exists())
}

/// Points an existing agent at this copy, should the app have moved.
pub fn refresh() {
    let Some(file) = agent() else {
        return;
    };
    let Ok(old) = std::fs::read_to_string(&file) else {
        return;
    };
    let Ok(program) = crate::platform::shell::launcher() else {
        return;
    };
    if old == text(&program) {
        return;
    }
    match std::fs::write(file, text(&program)) {
        Ok(()) => log::info!("the startup entry now points at this copy"),
        Err(error) => log::warn!("the startup entry could not be moved: {error}"),
    }
}

fn text(program: &Path) -> String {
    let program = program
        .display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         <key>Label</key><string>{LABEL}</string>\n\
         <key>ProgramArguments</key><array><string>{program}</string>\
         <string>--hidden</string></array>\n\
         <key>RunAtLoad</key><true/>\n\
         </dict>\n</plist>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_starts_this_copy_hidden() {
        let text = text(Path::new("/Applications/A & B.app/Contents/MacOS/app"));
        assert!(text.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/app</string>"));
        assert!(text.contains("<string>--hidden</string>"));
    }
}
