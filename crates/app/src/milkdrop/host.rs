//! The app's side of the MilkDrop window: starting the process it runs
//! in, giving it the sound, and stopping it.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Weak};

use spotified_audio::tap::Tap;

use super::ring::Ring;

/// The process while it runs, and what it is fed through.
struct Running {
    child: Child,
    ring: Arc<Ring>,
    /// The file the ring is in, to take away afterwards.
    file: PathBuf,
    /// The engine's tap the ring is fed from. The engine is made again
    /// when the account changes, and its tap with it.
    fed: Weak<Tap>,
}

/// The MilkDrop window, from the app's side. Dropping it closes the window.
#[derive(Default)]
pub struct Host {
    running: Option<Running>,
}

impl Host {
    /// Starts the window's process. `scratch` is where the file the sound
    /// is shared through is kept, and `presets` the folder of presets.
    pub fn open(&mut self, scratch: &Path, presets: &Path) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }
        let child = start(scratch, presets)?;
        self.running = Some(child);
        Ok(())
    }

    /// Whether the window is still there. One closed from inside it is
    /// noticed here, and tidied up after.
    pub fn is_running(&mut self) -> bool {
        let Some(running) = &mut self.running else {
            return false;
        };
        match running.child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                if !status.success() {
                    log::warn!("MilkDrop ended badly: {status}");
                }
                self.close();
                false
            }
            Err(error) => {
                log::warn!("MilkDrop cannot be asked how it is: {error}");
                self.close();
                false
            }
        }
    }

    /// Sends the window the sound of `tap` from now on, if it is not
    /// being sent already. Called every frame while the window is open:
    /// the tap is not there until playback is up, and changes with it.
    pub fn feed(&mut self, tap: &Arc<Tap>) {
        let Some(running) = &mut self.running else {
            return;
        };
        if running
            .fed
            .upgrade()
            .is_some_and(|fed| Arc::ptr_eq(&fed, tap))
        {
            return;
        }
        let ring = running.ring.clone();
        tap.set_sink(Some(Box::new(move |stereo, ahead, rate| {
            let frames = (ahead.as_secs_f64() * f64::from(rate)) as usize;
            ring.push(stereo, frames);
        })));
        running.fed = Arc::downgrade(tap);
    }

    /// Closes the window: asked first, and stopped if it does not go.
    pub fn close(&mut self) {
        let Some(mut running) = self.running.take() else {
            return;
        };
        if let Some(tap) = running.fed.upgrade() {
            tap.set_sink(None);
        }
        if matches!(running.child.try_wait(), Ok(None)) {
            if let Some(stdin) = &mut running.child.stdin {
                let _ = stdin.write_all(b"quit\n");
                let _ = stdin.flush();
            }
            // Its standard input closing says the same thing.
            drop(running.child.stdin.take());
            let asked = std::time::Instant::now();
            let patience = std::time::Duration::from_millis(600);
            while matches!(running.child.try_wait(), Ok(None)) && asked.elapsed() < patience {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if matches!(running.child.try_wait(), Ok(None)) {
                let _ = running.child.kill();
                let _ = running.child.wait();
            }
        }
        let _ = std::fs::remove_file(&running.file);
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(windows)]
fn start(scratch: &Path, presets: &Path) -> Result<Running, String> {
    let library = super::library().ok_or_else(|| {
        format!(
            "This copy of the app came without {}, which draws MilkDrop.",
            super::LIBRARY
        )
    })?;
    let said = |error: std::io::Error| error.to_string();
    std::fs::create_dir_all(scratch).map_err(said)?;
    std::fs::create_dir_all(presets).map_err(said)?;
    let file = scratch.join(format!("milkdrop-{}.ring", std::process::id()));
    let ring = Arc::new(Ring::create(&file).map_err(said)?);
    let program = std::env::current_exe().map_err(said)?;
    let mut command = Command::new(program);
    command
        .args(super::child::Args::command_line(&file, presets, &library))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(said)?;
    // What the window has to say goes in the app's log.
    if let Some(stderr) = child.stderr.take() {
        let reading = std::thread::Builder::new()
            .name("milkdrop-log".into())
            .spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    log::warn!("{line}");
                }
            });
        if let Err(error) = reading {
            log::warn!("MilkDrop will not be heard from: {error}");
        }
    }
    log::info!("MilkDrop opened, with presets from {}", presets.display());
    Ok(Running {
        child,
        ring,
        file,
        fed: Weak::new(),
    })
}

#[cfg(not(windows))]
fn start(_scratch: &Path, _presets: &Path) -> Result<Running, String> {
    Err("MilkDrop is only built for Windows.".into())
}
