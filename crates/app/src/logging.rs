//! One log file per launch, and panics written to it.
//!
//! Release builds abort on panic, so the hook is the only record of one.
//! Times are seconds since launch: what a performance question needs, with
//! no clock or time-zone dependency.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Logs kept, counting the one just opened.
const KEPT: usize = 10;

struct FileLogger {
    file: Mutex<File>,
    started: Instant,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // Verbose means this app's own detail. The libraries under it log
        // every request and frame at debug, which would bury that.
        let ours = metadata.target().starts_with("spotified") || metadata.target() == "core";
        let level = if ours {
            self.level
        } else {
            log::LevelFilter::Warn
        };
        metadata.level() <= level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{:9.3}] {:5} {}: {}\n",
            self.started.elapsed().as_secs_f64(),
            record.level(),
            record.target(),
            record.args()
        );
        // A debug build has a console; a release build has none to write to.
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Starts logging to a new file in `dir` and returns its path.
pub fn init(dir: &Path, started: Instant, verbose: bool) -> io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let path = dir.join(format!("app-{stamp}.log"));
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    let logger = FileLogger {
        file: Mutex::new(File::create(&path)?),
        started,
        level,
    };
    log::set_boxed_logger(Box::new(logger)).map_err(io::Error::other)?;
    log::set_max_level(level);
    std::panic::set_hook(Box::new(|panic| {
        log::error!("panic: {panic}");
        log::logger().flush();
    }));
    prune(dir);
    Ok(path)
}

/// Removes all but the newest logs. The names sort by age, being a prefix
/// and a timestamp.
fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "log"))
        .collect();
    logs.sort();
    let excess = logs.len().saturating_sub(KEPT);
    for old in logs.into_iter().take(excess) {
        let _ = std::fs::remove_file(old);
    }
}
