//! Keeping yt-dlp current, at most once a day.
//!
//! yt-dlp tracks YouTube's changes, and a copy frozen when the app was
//! packaged is the usual reason songs stop playing. Its own `-U` does not
//! update the unpacked build the app ships, so this does what `-U` would:
//! read the release's list of checksums, and if the archive differs from
//! the one in use, download it, check it, and unpack it beside the app's
//! data. The newer copy is only staged here. It goes into use at the next
//! launch, before the core starts: never under a core that may be running
//! the old one at that moment.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use crate::update;

const RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const ARCHIVE: &str = "yt-dlp_win.zip";
const SUMS: &str = "SHA2-256SUMS";
const EXE: &str = "yt-dlp.exe";
/// The hash of the archive a copy was unpacked from, kept beside it.
const HASH: &str = "release.sha256";

/// How long after starting the first look waits, well clear of startup.
pub const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// What a look came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Looked for less than a day ago; not looked for again.
    NotDue,
    UpToDate,
    /// A newer copy is unpacked and waiting for the next launch.
    Staged,
}

impl Outcome {
    /// What to tell someone who asked for the update by hand.
    pub fn said(self) -> &'static str {
        match self {
            Outcome::NotDue | Outcome::UpToDate => "yt-dlp is up to date",
            Outcome::Staged => "yt-dlp was updated. Restart the app to use the new version.",
        }
    }
}

/// The folder the newer copies are kept in.
pub struct Resolver {
    root: PathBuf,
}

impl Resolver {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The time of the last look is the time this file was written.
    fn stamp(&self) -> PathBuf {
        self.root.join("checked")
    }

    fn current(&self) -> PathBuf {
        self.root.join("current")
    }

    fn next(&self) -> PathBuf {
        self.root.join("next")
    }

    /// The copy an update put in place, if there has been one. It is used
    /// in preference to the one that came with the app.
    pub fn updated_exe(&self) -> Option<PathBuf> {
        let exe = self.current().join(EXE);
        exe.exists().then_some(exe)
    }

    /// Whether an update is unpacked and waiting. The hash is written
    /// last: without it the unpacking never ended.
    pub fn staged(&self) -> bool {
        self.next().join(HASH).exists()
    }

    /// Moves a staged update into place. Called before the core starts, so
    /// that nothing can be running the copy it replaces.
    pub fn promote(&self) {
        if !self.staged() {
            return;
        }
        let next = self.next();
        let current = self.current();
        let moved = remove_dir(&current)
            .and_then(|()| std::fs::rename(&next, &current))
            .and_then(|()| std::fs::copy(current.join(HASH), self.in_use_hash()).map(drop));
        match moved {
            Ok(()) => log::info!("yt-dlp: the staged update is now in use"),
            Err(error) => log::warn!("yt-dlp: the staged update could not be applied: {error}"),
        }
    }

    fn in_use_hash(&self) -> PathBuf {
        self.root.join("current.sha256")
    }

    /// Whether a day has passed since the last look, or there has been none.
    pub fn due(&self, now: SystemTime) -> bool {
        let checked = std::fs::metadata(self.stamp()).and_then(|stamp| stamp.modified());
        match checked {
            Ok(checked) => now
                .duration_since(checked)
                .is_ok_and(|since| since >= CHECK_EVERY),
            Err(_) => true,
        }
    }

    /// Looks for a newer yt-dlp and stages it. `bundled` is the copy that
    /// came with the app. `force` looks even if the last look was today,
    /// for when it is asked for by hand. Blocks on the network, so it is
    /// called from a thread of its own.
    pub fn update(&self, bundled: Option<&Path>, force: bool) -> Result<Outcome, String> {
        if !force && !self.due(SystemTime::now()) {
            return Ok(Outcome::NotDue);
        }
        let text = |error: io::Error| error.to_string();
        std::fs::create_dir_all(&self.root).map_err(text)?;
        // Stamped before the look: a look that fails is not tried again
        // on every launch of a computer that is offline.
        std::fs::write(self.stamp(), b"").map_err(text)?;

        let agent = update::agent();
        let sums = update::fetch_text(&agent, &format!("{RELEASE}/{SUMS}"))?
            .ok_or("the release has no list of checksums")?;
        let expected = update::expected_hash(&sums, ARCHIVE)
            .ok_or("the release's checksums do not name the Windows build")?;

        let bundled_hash = bundled.and_then(Path::parent).map(|dir| dir.join(HASH));
        let staged_hash = self.next().join(HASH);
        let in_use = [Some(self.in_use_hash()), bundled_hash];
        if hash_in(in_use.into_iter().flatten()).as_deref() == Some(expected.as_str()) {
            return Ok(Outcome::UpToDate);
        }
        // Downloaded on an earlier look and still waiting for a launch.
        if hash_in([staged_hash]).as_deref() == Some(expected.as_str()) {
            return Ok(Outcome::Staged);
        }

        let archive = self.root.join(ARCHIVE);
        update::download(&agent, &format!("{RELEASE}/{ARCHIVE}"), &archive).map_err(text)?;
        let actual = update::hash_of(&archive).map_err(text)?;
        if actual != expected {
            let _ = std::fs::remove_file(&archive);
            return Err("the download did not match its checksum".into());
        }
        let next = self.next();
        remove_dir(&next).map_err(text)?;
        std::fs::create_dir_all(&next).map_err(text)?;
        let unpacked = unpack(&archive, &next);
        let _ = std::fs::remove_file(&archive);
        unpacked.map_err(text)?;
        if !next.join(EXE).exists() {
            return Err("the download has no yt-dlp in it".into());
        }
        std::fs::write(next.join(HASH), &expected).map_err(text)?;
        Ok(Outcome::Staged)
    }
}

/// The first hash that can be read from these files.
fn hash_in(files: impl IntoIterator<Item = PathBuf>) -> Option<String> {
    files.into_iter().find_map(|file| {
        let hash = std::fs::read_to_string(file).ok()?;
        let hash = hash.trim();
        (!hash.is_empty()).then(|| hash.to_lowercase())
    })
}

fn remove_dir(dir: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// The `tar` that Windows ships, which reads zip archives too.
pub(crate) fn system_tar() -> Command {
    let windows = std::env::var_os("SystemRoot").map_or_else(|| "C:/Windows".into(), PathBuf::from);
    let mut command = Command::new(windows.join("System32").join("tar.exe"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::sidecar::hide_console(&mut command);
    command
}

fn unpack(archive: &Path, into: &Path) -> io::Result<()> {
    let status = system_tar()
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("tar ended with {status}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> Resolver {
        let dir =
            std::env::temp_dir().join(format!("spotified-resolver-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Resolver::new(dir)
    }

    #[test]
    fn a_look_is_due_when_there_has_been_none() {
        let resolver = scratch("never");
        assert!(resolver.due(SystemTime::now()));
    }

    #[test]
    fn a_look_is_not_due_again_until_a_day_has_passed() {
        let resolver = scratch("day");
        std::fs::write(resolver.stamp(), b"").expect("write");
        let now = SystemTime::now();
        assert!(!resolver.due(now));
        assert!(!resolver.due(now + Duration::from_secs(23 * 60 * 60)));
        assert!(resolver.due(now + Duration::from_secs(25 * 60 * 60)));
        // A clock set back is not a reason to look.
        assert!(!resolver.due(now - Duration::from_secs(60 * 60)));
    }

    #[test]
    fn a_look_that_is_not_due_touches_nothing_and_asks_nobody() {
        let resolver = scratch("skip");
        std::fs::write(resolver.stamp(), b"").expect("write");
        assert_eq!(resolver.update(None, false), Ok(Outcome::NotDue));
        assert!(!resolver.next().exists());
    }

    #[test]
    fn a_staged_update_goes_into_use_at_the_next_launch() {
        let resolver = scratch("promote");
        let next = resolver.next();
        std::fs::create_dir_all(&next).expect("a folder");
        std::fs::write(next.join(EXE), b"new").expect("write");
        std::fs::write(next.join(HASH), b"abc").expect("write");
        std::fs::create_dir_all(resolver.current()).expect("a folder");
        std::fs::write(resolver.current().join(EXE), b"old").expect("write");
        assert!(resolver.updated_exe().is_some());

        resolver.promote();
        let exe = resolver.updated_exe().expect("the updated copy");
        assert_eq!(std::fs::read(exe).expect("read"), b"new");
        assert!(!next.exists());
        assert_eq!(
            hash_in([resolver.in_use_hash()]).as_deref(),
            Some("abc"),
            "the hash in use is what later looks compare with"
        );
    }

    #[test]
    fn an_unpacking_that_never_ended_is_not_put_into_use() {
        let resolver = scratch("half");
        std::fs::create_dir_all(resolver.next()).expect("a folder");
        std::fs::write(resolver.next().join(EXE), b"half").expect("write");
        resolver.promote();
        assert_eq!(resolver.updated_exe(), None);
    }

    #[test]
    fn the_hash_in_use_is_the_updates_before_the_bundles() {
        let resolver = scratch("hash");
        let bundle = resolver.root.join("bundle.sha256");
        std::fs::write(&bundle, "BUNDLED\n").expect("write");
        let files = || [resolver.in_use_hash(), bundle.clone()];
        assert_eq!(hash_in(files()).as_deref(), Some("bundled"));
        std::fs::write(resolver.in_use_hash(), "updated").expect("write");
        assert_eq!(hash_in(files()).as_deref(), Some("updated"));
    }

    /// The real thing, against yt-dlp's releases: a look stages the newest
    /// build, a second look finds it already staged, and the next launch
    /// puts it into use, after which there is nothing newer to fetch.
    #[test]
    #[ignore = "downloads yt-dlp from GitHub; run with --ignored"]
    fn the_newest_release_is_staged_then_put_into_use() {
        let resolver = scratch("live");
        assert_eq!(resolver.update(None, false), Ok(Outcome::Staged));
        assert!(resolver.staged());
        assert_eq!(resolver.updated_exe(), None, "not in use until a launch");
        assert_eq!(resolver.update(None, true), Ok(Outcome::Staged));

        resolver.promote();
        let exe = resolver.updated_exe().expect("the updated copy");
        let version = std::process::Command::new(exe)
            .arg("--version")
            .output()
            .expect("yt-dlp runs");
        assert!(version.status.success());
        assert!(!version.stdout.is_empty());
        assert_eq!(resolver.update(None, true), Ok(Outcome::UpToDate));
        // And a look that is not asked for by hand waits for tomorrow.
        assert_eq!(resolver.update(None, false), Ok(Outcome::NotDue));
        let _ = std::fs::remove_dir_all(&resolver.root);
    }

    #[test]
    fn the_windows_build_is_found_in_the_releases_checksums() {
        let sums = "aa11  yt-dlp\nbb22  yt-dlp_win.zip\ncc33  yt-dlp_win_x86.zip\n";
        assert_eq!(
            update::expected_hash(sums, ARCHIVE).as_deref(),
            Some("bb22")
        );
    }
}
