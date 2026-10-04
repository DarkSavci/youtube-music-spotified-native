//! One log file per launch, and panics written to it.
//!
//! Release builds abort on panic, so the hook is the only record of one.
//! Times are seconds since launch: what a performance question needs, with
//! no clock or time-zone dependency.
//!
//! Each launch's file is named for when it started, as the Electron app
//! named them (`app-20261004-153012.log`), so "what happened this morning"
//! is one file and not the tail of a shared one. A launch that outgrows
//! its file goes on in a numbered part, and old launches are deleted by
//! count and by size, never the one being written.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

/// One launch's file is cut here and continued in a numbered part.
const MAX_BYTES: u64 = 5 * 1024 * 1024;
/// Log files kept, this launch's included.
const KEPT: usize = 20;
/// And the folder as a whole stays under this.
const MAX_TOTAL_BYTES: u64 = 40 * 1024 * 1024;

/// The file being written, and what it takes to go on in the next.
struct Sink {
    file: File,
    written: u64,
    dir: PathBuf,
    /// This launch's file name without ".log"; parts after the first add
    /// "-2", "-3" and so on.
    base: String,
    part: u32,
}

impl Sink {
    fn path(&self) -> PathBuf {
        self.dir.join(part_name(&self.base, self.part))
    }

    fn write(&mut self, line: &str) {
        let length = line.len() as u64;
        if self.written > 0 && self.written + length > MAX_BYTES {
            self.next_part();
        }
        if self.file.write_all(line.as_bytes()).is_ok() {
            self.written += length;
        }
    }

    /// Goes on in the next part. A part that cannot be made leaves the
    /// launch writing where it was: a long file is better than none.
    fn next_part(&mut self) {
        let next = self.dir.join(part_name(&self.base, self.part + 1));
        let Ok(file) = File::create(&next) else {
            return;
        };
        let _ = self.file.flush();
        self.file = file;
        self.written = 0;
        self.part += 1;
        prune(&self.dir, &next);
    }
}

/// `app-20261004-153012.log`, then `app-20261004-153012-2.log`.
fn part_name(base: &str, part: u32) -> String {
    match part {
        1 => format!("{base}.log"),
        part => format!("{base}-{part}.log"),
    }
}

struct FileLogger {
    sink: Mutex<Sink>,
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
        // Scrubbed on the way in: cookies and signed addresses are never
        // written down, so there are none to leak later.
        let said = record.args().to_string();
        let line = format!(
            "[{:9.3}] {:5} {}: {}\n",
            self.started.elapsed().as_secs_f64(),
            record.level(),
            record.target(),
            crate::redact::redact(&said)
        );
        // A debug build has a console; a release build has none to write to.
        if cfg!(debug_assertions) {
            eprint!("{line}");
        }
        if let Ok(mut sink) = self.sink.lock() {
            sink.write(&line);
        }
    }

    fn flush(&self) {
        if let Ok(mut sink) = self.sink.lock() {
            let _ = sink.file.flush();
        }
    }
}

/// Starts logging to a new file in `dir` and returns its path.
pub fn init(dir: &Path, started: Instant, verbose: bool) -> io::Result<PathBuf> {
    let sink = open(dir, &crate::report::stamp(crate::report::now_local()))?;
    let path = sink.path();
    let level = if verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    let logger = FileLogger {
        sink: Mutex::new(sink),
        started,
        level,
    };
    log::set_boxed_logger(Box::new(logger)).map_err(io::Error::other)?;
    log::set_max_level(level);
    std::panic::set_hook(Box::new(|panic| {
        log::error!("panic: {panic}");
        log::logger().flush();
    }));
    prune(dir, &path);
    Ok(path)
}

/// Opens this launch's file, named for `stamp`.
fn open(dir: &Path, stamp: &str) -> io::Result<Sink> {
    let mut base = format!("app-{stamp}");
    // Two launches in the same second (a quick relaunch) must not share a
    // file.
    if dir.join(part_name(&base, 1)).exists() {
        base = format!("{base}-{}", std::process::id());
    }
    Ok(Sink {
        file: File::create(dir.join(part_name(&base, 1)))?,
        written: 0,
        dir: dir.to_path_buf(),
        base,
        part: 1,
    })
}

/// Deletes the oldest logs past the count or the size the folder is held
/// to, never `live`, the one being written. Age is when a file was last
/// written, so the logs older versions named otherwise are counted too.
fn prune(dir: &Path, live: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<(SystemTime, PathBuf, u64)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let named = name.starts_with("app") && name.ends_with(".log");
            let about = entry.metadata().ok()?;
            named.then_some((about.modified().ok()?, entry.path(), about.len()))
        })
        .collect();
    // Newest first, and by name where two were written in the same moment.
    logs.sort_by(|one, other| other.0.cmp(&one.0).then_with(|| other.1.cmp(&one.1)));
    let (mut kept, mut total) = (0, 0);
    for (_, path, size) in logs {
        let room = kept < KEPT && total + size <= MAX_TOTAL_BYTES;
        if path == live || room {
            kept += 1;
            total += size;
        } else {
            // One another program holds open goes at the next launch.
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("spotified-logs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    /// A log of `size` bytes last written `age` seconds ago.
    fn log_of(dir: &Path, name: &str, size: usize, age: u64) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, vec![b'x'; size]).expect("a file");
        let file = File::options().write(true).open(&path).expect("open");
        let then = SystemTime::now() - Duration::from_secs(age);
        file.set_modified(then).expect("a time");
        path
    }

    fn names(dir: &Path) -> Vec<String> {
        let entries = std::fs::read_dir(dir).expect("a folder").flatten();
        let mut names: Vec<String> = entries
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_launch_is_named_for_when_it_started() {
        let dir = scratch("named");
        let sink = open(&dir, "20261004-153012").expect("a log");
        assert_eq!(sink.path(), dir.join("app-20261004-153012.log"));
        // A second launch in the same second gets a file of its own.
        let again = open(&dir, "20261004-153012").expect("a log");
        assert_ne!(again.path(), sink.path());
        assert!(again.path().exists() && sink.path().exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_launch_that_outgrows_its_file_goes_on_in_a_numbered_part() {
        let dir = scratch("parts");
        let mut sink = open(&dir, "20261004-153012").expect("a log");
        let line = "x".repeat(1024 * 1024);
        for _ in 0..6 {
            sink.write(&line);
        }
        assert_eq!(
            names(&dir),
            ["app-20261004-153012-2.log", "app-20261004-153012.log"]
        );
        // The first part holds what fitted, the second the line that did not.
        let first = std::fs::metadata(dir.join("app-20261004-153012.log")).expect("a file");
        assert_eq!(first.len(), 5 * 1024 * 1024);
        assert_eq!(sink.written, 1024 * 1024);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_oldest_logs_go_once_there_are_more_than_are_kept() {
        let dir = scratch("count");
        for age in 1..=25u64 {
            log_of(&dir, &format!("app-{age:02}.log"), 10, age * 60);
        }
        let live = log_of(&dir, "app-live.log", 10, 0);
        // Not a log of this app's: left alone.
        log_of(&dir, "notes.txt", 10, 9_000);
        prune(&dir, &live);
        let left = names(&dir);
        assert_eq!(left.len(), KEPT + 1);
        assert!(left.contains(&"app-live.log".to_owned()));
        assert!(left.contains(&"app-19.log".to_owned()));
        assert!(!left.contains(&"app-20.log".to_owned()));
        assert!(left.contains(&"notes.txt".to_owned()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_folder_is_held_to_a_size_but_the_log_being_written_is_never_deleted() {
        let dir = scratch("size");
        let big = 15 * 1024 * 1024;
        for age in 1..=4u64 {
            log_of(&dir, &format!("app-{age}.log"), big, age * 60);
        }
        // The oldest of all, and still the one being written.
        let live = log_of(&dir, "app-live.log", big, 9_000);
        prune(&dir, &live);
        // Two of fifteen megabytes fit under forty, and the live one stays
        // though it is over.
        assert_eq!(names(&dir), ["app-1.log", "app-2.log", "app-live.log"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
