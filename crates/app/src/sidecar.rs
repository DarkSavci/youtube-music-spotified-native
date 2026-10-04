//! The Go core, run as a child process.
//!
//! The core picks a free port and says which in its log; the app reads that
//! line from the child's stderr. The child's stdin is a pipe the app holds
//! open: closing it asks the core for a clean stop, and a crashed app closes
//! it too, so the core is never orphaned.

use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

const CORE_BINARY: &str = if cfg!(windows) {
    "spotified-core.exe"
} else {
    "spotified-core"
};

const YTDLP: &str = "vendor/yt-dlp/yt-dlp.exe";
const DENO: &str = "vendor/deno/deno.exe";

/// How long a clean stop may take before the core is killed. Its own
/// shutdown allows five seconds for the final resume save.
const STOP_GRACE: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreStatus {
    Starting,
    /// Listening at this origin, for example `http://127.0.0.1:51234`.
    Ready {
        origin: String,
    },
    Failed(String),
}

pub struct SidecarConfig {
    pub credentials: PathBuf,
    pub database: PathBuf,
    /// Where the songs played are kept. One place whoever is signed in: a
    /// song is the same song for every account.
    pub cache: PathBuf,
    /// A yt-dlp newer than the one that came with the app, when an update
    /// has put one in place.
    pub resolver: Option<PathBuf>,
    /// Serve recorded responses from this directory instead of YouTube.
    pub fixtures: Option<PathBuf>,
}

pub struct Sidecar {
    child: Child,
    stdin: Option<ChildStdin>,
}

/// Starts the core and reports its status from a reader thread: `Ready` once
/// it is listening, `Failed` if it stops before that.
pub fn spawn(
    config: &SidecarConfig,
    report: impl Fn(CoreStatus) + Send + 'static,
) -> io::Result<Sidecar> {
    let program = locate(CORE_BINARY).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the playback service is not next to the app",
        )
    })?;
    let mut command = Command::new(program);
    command
        .args(["-addr", "127.0.0.1:0", "-exit-with-stdin"])
        .arg("-credentials")
        .arg(&config.credentials)
        .arg("-db")
        .arg(&config.database)
        .arg("-cache")
        .arg(&config.cache)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(fixtures) = &config.fixtures {
        command
            .args(["-catalog", "fixture", "-fixtures"])
            .arg(fixtures);
    }
    // yt-dlp resolves streams and needs deno to run YouTube's player code.
    // Without them the core looks on PATH, and playback fails if they are
    // not there either.
    let resolver = config.resolver.clone().or_else(bundled_resolver);
    for (flag, tool, found) in [("-ytdlp", YTDLP, resolver), ("-deno", DENO, locate(DENO))] {
        match found {
            Some(path) => {
                command.arg(flag).arg(path);
            }
            None => log::warn!("{tool} is missing; run scripts/fetch-tools.ps1"),
        }
    }
    hide_console(&mut command);

    let mut child = command.spawn()?;
    let stdin = child.stdin.take();
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("the playback service has no log pipe"))?;
    std::thread::Builder::new()
        .name("core-log".into())
        .spawn(move || follow_log(BufReader::new(stderr), report))?;
    Ok(Sidecar { child, stdin })
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        // Closing stdin is the request to stop.
        drop(self.stdin.take());
        let deadline = Instant::now() + STOP_GRACE;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        log::warn!("the playback service did not stop in time; ending it");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Copies the core's log into ours and watches it for the listening line.
fn follow_log(log: impl BufRead, report: impl Fn(CoreStatus)) {
    let mut ready = false;
    for line in log.lines().map_while(Result::ok) {
        // The core's warnings are kept in every log, not only a verbose
        // one: they are what explains a song that would not play.
        if line.contains("level=ERROR") {
            log::error!(target: "core", "{line}");
        } else if line.contains("level=WARN") {
            log::warn!(target: "core", "{line}");
        } else {
            log::debug!(target: "core", "{line}");
        }
        if !ready && let Some(addr) = listening_addr(&line) {
            ready = true;
            report(CoreStatus::Ready {
                origin: format!("http://{addr}"),
            });
        }
    }
    // The log ends when the core does. One that was stopped on purpose
    // says this to nobody: the app has stopped listening to it by then.
    report(CoreStatus::Failed(if ready {
        "The playback service stopped. Restart the app to carry on.".into()
    } else {
        "The playback service stopped before it was ready.".into()
    }));
}

/// The address in the core's `msg="spotifier listening" addr=…` line.
fn listening_addr(line: &str) -> Option<&str> {
    if !line.contains("msg=\"spotifier listening\"") {
        return None;
    }
    line.split_whitespace()
        .find_map(|field| field.strip_prefix("addr="))
}

/// Runs the core for one job instead of as a server, and returns the line
/// of JSON it answers with; or what it said went wrong.
pub fn one_shot(arguments: &[(&str, &Path)]) -> Result<String, String> {
    let program = locate(CORE_BINARY).ok_or("the playback service is not next to the app")?;
    let mut command = Command::new(program);
    for (flag, path) in arguments {
        command.arg(flag).arg(path);
    }
    command.stdin(Stdio::null());
    hide_console(&mut command);
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        // Its last line is the reason; the rest is the log leading to it.
        let said = String::from_utf8_lossy(&output.stderr);
        let reason = said.lines().rev().find(|line| !line.trim().is_empty());
        Err(reason
            .unwrap_or("the playback service gave no reason")
            .to_owned())
    }
}

/// The yt-dlp that came with the app.
pub fn bundled_resolver() -> Option<PathBuf> {
    locate(YTDLP)
}

/// Whether this copy was put together to be run, with the core beside the
/// executable, as against built in a working tree.
pub fn packaged() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(CORE_BINARY).exists()))
        .unwrap_or(false)
}

/// Finds a file shipped with the app: beside the executable when installed,
/// or under `target/` and the repository root in a development build.
pub fn locate(relative: impl AsRef<Path>) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let relative = relative.as_ref();
    [
        exe_dir.join(relative),
        // target/debug/spotified.exe -> target/core/
        exe_dir.join("../core").join(relative),
        // target/debug/spotified.exe -> repository root
        exe_dir.join("../..").join(relative),
    ]
    .into_iter()
    .find(|candidate| candidate.exists())
}

/// A console child of a windowed app would flash a terminal window.
pub(crate) fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const LISTENING: &str = "time=2026-10-03T20:47:05.911+03:00 level=INFO \
        msg=\"spotifier listening\" addr=127.0.0.1:63152 catalog=fixture";

    #[test]
    fn the_listening_line_gives_the_bound_address() {
        assert_eq!(listening_addr(LISTENING), Some("127.0.0.1:63152"));
    }

    #[test]
    fn other_lines_carrying_an_address_are_not_the_signal() {
        let line = "level=INFO msg=\"upstream call\" addr=1.2.3.4:443";
        assert_eq!(listening_addr(line), None);
    }

    fn statuses(log: &str) -> Vec<CoreStatus> {
        let seen = RefCell::new(Vec::new());
        follow_log(log.as_bytes(), |status| seen.borrow_mut().push(status));
        seen.into_inner()
    }

    #[test]
    fn readiness_is_reported_once() {
        let log = format!("level=INFO msg=starting\n{LISTENING}\n{LISTENING}\n");
        let seen = statuses(&log);
        let ready = CoreStatus::Ready {
            origin: "http://127.0.0.1:63152".into(),
        };
        assert_eq!(seen.iter().filter(|status| **status == ready).count(), 1);
        assert_eq!(seen.first(), Some(&ready));
    }

    #[test]
    fn a_core_that_stops_while_in_use_is_said_to_have_stopped() {
        let seen = statuses(&format!("{LISTENING}\n"));
        assert!(matches!(
            seen.as_slice(),
            [CoreStatus::Ready { .. }, CoreStatus::Failed(_)]
        ));
    }

    #[test]
    fn a_core_that_exits_early_has_failed() {
        let seen = statuses("level=ERROR msg=listen err=\"address in use\"\n");
        assert!(matches!(seen.as_slice(), [CoreStatus::Failed(_)]));
    }
}
