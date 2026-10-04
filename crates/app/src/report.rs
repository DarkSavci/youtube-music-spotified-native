//! The problem report: the zip someone sends when something breaks.
//!
//! Only the logs and a short summary go in: never the database, the
//! credentials or the cookie file. The summary says whether those exist,
//! which is all a diagnosis needs of them. Everything is scrubbed again on
//! its way in, so a log written by an earlier version of the app is as
//! safe to send as one written by this.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde::Deserialize;
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::redact::redact;
use crate::state::State;

/// How much of the newest logs a report carries.
const LOGS_MOST: u64 = 20 * 1024 * 1024;
const SUMMARY: &str = "info.txt";
/// How long the core has to say how it is.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);
/// How long yt-dlp has to say its version.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// Where a report was in the making of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    Working,
    Saved(PathBuf),
    Failed(String),
}

/// What the report is made from. Gathered on the UI thread, which knows
/// it; the making is done on another.
pub struct Request {
    pub logs: PathBuf,
    /// The folder the zip is written to.
    pub into: PathBuf,
    /// Where the core is listening, if it is.
    pub core: Option<String>,
    /// The yt-dlp in use.
    pub resolver: Option<PathBuf>,
    pub credentials: PathBuf,
    pub audio_cache: PathBuf,
    /// The app as it is now: see [`page_state`].
    pub page: Value,
}

/// What the app knows that the log may not: the player as it is now.
/// Titles and ids only. The queue itself is not sent, just its size and
/// where in it playback is; nor anything that names a person or a room.
pub fn page_state(state: &State) -> Value {
    let player = state.playback.as_ref().map(|playback| {
        let session = &playback.session;
        let queue = &session.queue;
        json!({
            "state": format!("{:?}", session.state),
            "track": playback.current().map(|track| json!({
                "id": track.id,
                "title": track.title,
            })),
            "index": queue.index,
            "queueLength": queue.items.len(),
            "unplayableInQueue": queue.items.iter().filter(|track| !track.playable).count(),
            "origin": queue.origin,
            "shuffle": session.shuffle,
            "repeat": format!("{:?}", session.repeat),
            "offline": playback.offline,
            "followingRoom": playback.following_room,
        })
    });
    let mut settings = serde_json::to_value(&state.settings).unwrap_or(Value::Null);
    if let Some(fields) = settings.as_object_mut() {
        // What was searched for, who is listened with and where are the
        // person's own business, and no help in finding a fault.
        fields.retain(|name, _| {
            !name.starts_with("together_")
                && !name.starts_with("recent_searches")
                && name != "device_id"
        });
    }
    json!({
        "page": format!("{:?}", state.nav.page()),
        "core": format!("{:?}", state.core),
        "signedIn": state.account.is_some(),
        "savedAccounts": state.accounts.accounts.len(),
        "inRoom": state.together.in_room(),
        "notice": state.notice.map(|notice| format!("{notice:?}")),
        "update": format!("{:?}", state.update),
        "player": player,
        "settings": settings,
    })
}

/// Builds the zip and returns where it is. Reads files, runs yt-dlp and
/// asks the core, so it is called from a thread of its own.
pub fn save(request: &Request) -> Result<PathBuf, String> {
    log::info!("problem report requested");
    log::logger().flush();
    let stamp = stamp(now_local());
    let staging = std::env::temp_dir().join(format!("ytms-diag-{stamp}-{}", std::process::id()));
    let result = build(request, &staging, &stamp).map_err(|error| error.to_string());
    let _ = std::fs::remove_dir_all(&staging);
    match &result {
        Ok(zip) => log::info!("problem report written to {}", zip.display()),
        Err(error) => log::error!("problem report failed: {error}"),
    }
    result
}

fn build(request: &Request, staging: &Path, stamp: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(staging)?;
    std::fs::write(staging.join(SUMMARY), summary(request))?;
    let mut names = vec![SUMMARY.to_owned()];
    let mut room = LOGS_MOST;
    for (name, size) in logs_newest_first(&request.logs) {
        // The newest log always goes, whatever its size.
        if size > room && names.len() > 1 {
            break;
        }
        let text = std::fs::read(request.logs.join(&name))?;
        let text = String::from_utf8_lossy(&text);
        let mut copy = io::BufWriter::new(std::fs::File::create(staging.join(&name))?);
        for line in text.lines() {
            writeln!(copy, "{}", redact(line))?;
        }
        copy.flush()?;
        names.push(name);
        room = room.saturating_sub(size);
    }

    std::fs::create_dir_all(&request.into)?;
    let zip = request.into.join(format!("ytms-diagnostics-{stamp}.zip"));
    // Windows ships bsdtar, which writes a zip when the name ends in one
    // (-a). The files are named rather than given as ".": that stores them
    // as "./app.log", and Explorer shows a zip of "./" entries as empty.
    let status = crate::resolver::system_tar()
        .args(["-a", "-c", "-f"])
        .arg(&zip)
        .arg("-C")
        .arg(staging)
        .args(&names)
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!("tar ended with {status}")));
    }
    Ok(zip)
}

/// The logs in `dir` with their sizes, newest first.
fn logs_newest_first(dir: &Path) -> Vec<(String, u64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut logs: Vec<(SystemTime, String, u64)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let named = name.starts_with("app") && name.ends_with(".log");
            let about = entry.metadata().ok()?;
            named.then_some((about.modified().ok()?, name, about.len()))
        })
        .collect();
    logs.sort_by(|one, other| other.0.cmp(&one.0).then_with(|| other.1.cmp(&one.1)));
    logs.into_iter()
        .map(|(_, name, size)| (name, size))
        .collect()
}

/// The summary that heads the report: what someone reading the log would
/// ask first. Nothing in it says whose account it is.
fn summary(request: &Request) -> String {
    let local = now_local();
    let utc = local.to_offset(time::UtcOffset::UTC);
    let offset = local.offset();
    let development = if crate::sidecar::packaged() {
        ""
    } else {
        " (development)"
    };
    let processors = std::thread::available_parallelism().map_or(0, usize::from);
    let processor = std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".into());
    let memory = match crate::platform::shell::memory() {
        Some((total, free)) => format!(
            "{} GB total, {} MB free",
            (total as f64 / f64::from(1u32 << 30)).round(),
            free >> 20
        ),
        None => "unknown".into(),
    };
    let resolver = match &request.resolver {
        Some(exe) => {
            let version = resolver_version(exe);
            let version = version.as_deref().unwrap_or("did not answer --version");
            format!("{version} at {}", exe.display())
        }
        None => "not found".into(),
    };
    let present = |file: PathBuf| if file.exists() { "present" } else { "absent" };
    let cookies = request.credentials.with_file_name("yt-dlp-cookies.txt");
    let page = serde_json::to_string_pretty(&request.page).unwrap_or_default();
    let lines = [
        "Youtube Music Spotified diagnostics".to_owned(),
        format!(
            "created:     {} UTC (local {} {:+03}:{:02})",
            clock(utc),
            clock(local),
            offset.whole_hours(),
            offset.minutes_past_hour().abs()
        ),
        String::new(),
        format!(
            "app:         {} native{development}",
            env!("CARGO_PKG_VERSION")
        ),
        format!(
            "os:          {} {}",
            os_version().unwrap_or_else(|| "Windows".into()),
            std::env::consts::ARCH
        ),
        format!("cpu:         {processor} x{processors}"),
        format!("memory:      {memory}"),
        String::new(),
        format!("core:        {}", core_health(request.core.as_deref())),
        format!("yt-dlp:      {resolver}"),
        format!("credentials: {}", present(request.credentials.clone())),
        format!("yt cookies:  {}", present(cookies)),
        format!("song cache:  {}", folder_size(&request.audio_cache)),
        String::new(),
        "page state:".to_owned(),
        page,
    ];
    format!("{}\n", redact(&lines.join("\n")))
}

/// What the core says of itself, in a line.
fn core_health(origin: Option<&str>) -> String {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Health {
        #[serde(default)]
        uptime: String,
        #[serde(default)]
        have_credentials: bool,
    }
    let Some(origin) = origin else {
        return "not running".into();
    };
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(HEALTH_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let mut response = match agent.get(format!("{origin}/v1/health")).call() {
        Ok(response) => response,
        Err(error) => return format!("unreachable ({error})"),
    };
    if !response.status().is_success() {
        return format!("HTTP {}", response.status().as_u16());
    }
    match response.body_mut().read_json::<Health>() {
        Ok(health) => format!(
            "ok uptime={} signedIn={}",
            health.uptime, health.have_credentials
        ),
        Err(error) => format!("answered, but not as expected ({error})"),
    }
}

/// How many files a folder holds and what they come to.
fn folder_size(folder: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return "none".into();
    };
    let sizes: Vec<u64> = entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|about| about.is_file())
        .map(|about| about.len())
        .collect();
    let total: u64 = sizes.iter().sum();
    format!(
        "{} files, {} MB",
        sizes.len(),
        (total as f64 / f64::from(1u32 << 20)).round()
    )
}

/// What yt-dlp calls its version; `None` if it will not say in time.
fn resolver_version(exe: &Path) -> Option<String> {
    answer(Command::new(exe).arg("--version"), VERSION_TIMEOUT)
}

/// What Windows calls its version, as `ver` prints it.
fn os_version() -> Option<String> {
    answer(Command::new("cmd").args(["/c", "ver"]), HEALTH_TIMEOUT)
}

/// Runs a command and returns what it printed, trimmed. `None` if it
/// failed, printed nothing, or took longer than `limit`, when it is ended.
fn answer(command: &mut Command, limit: Duration) -> Option<String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::sidecar::hide_console(command);
    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = child.wait_with_output().ok()?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (output.status.success() && !said.is_empty()).then_some(said)
}

/// Now, by this computer's clock; by Greenwich's where the system will not
/// say how far apart they are.
pub(crate) fn now_local() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

/// A time as a file name carries it: `20261004-153012`.
pub(crate) fn stamp(at: OffsetDateTime) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// A time as a person reads it: `2026-10-04 15:30:12`.
fn clock(at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

#[cfg(test)]
mod tests;
