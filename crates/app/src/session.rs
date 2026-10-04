//! Playback, joined up: the core's session on one side, the audio engine on
//! the other.
//!
//! The core decides what plays and says so in a projection. Each projection
//! goes two ways from here: its target to the engine, at once and without
//! waiting for a frame, and the whole of it to the UI. Commands from the UI
//! and reports from the engine go back to the core through one thread, so
//! they arrive in the order they were made.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};
use spotified_audio::engine::{self, Engine};
use spotified_client::Client;
use spotified_client::session::{Command, EngineEvent, EngineEventKind, Projection, Target};

/// How long to wait before following the session again after the stream
/// drops, doubling up to the second figure.
const RECONNECT: (Duration, Duration) = (Duration::from_secs(1), Duration::from_secs(15));

/// What the session tells the UI.
pub enum Update {
    Projection(Box<Projection>),
    /// A command failed or was refused; a sentence saying so.
    Refused(String),
}

enum Job {
    Command(Command),
    Report(EngineEvent),
    /// Tell the core the crossfade's length, in milliseconds.
    Transitions(u64),
    Stop,
}

pub struct Session {
    jobs: Sender<Job>,
    stopping: Arc<AtomicBool>,
    shared: Arc<Shared>,
}

struct Shared {
    client: Client,
    device_id: String,
    engine: Engine,
    /// The newest snapshot acted on, and whether the next is to be taken
    /// whatever its number: a core that restarted counts from one again.
    newest: Mutex<(u64, bool)>,
    deliver: Box<dyn Fn(Update) + Send + Sync>,
}

impl Shared {
    /// Acts on a snapshot unless a newer one already has been. A command's
    /// answer and the event stream both carry snapshots, in no fixed order.
    fn accept(&self, projection: Projection) {
        {
            let Ok(mut newest) = self.newest.lock() else {
                return;
            };
            let (version, resync) = *newest;
            if !resync && projection.state.version <= version {
                return;
            }
            *newest = (projection.state.version, false);
        }
        let duration_ms = projection
            .state
            .current()
            .map_or(0, |track| track.duration_ms);
        self.engine
            .apply(engine_target(&projection.target, duration_ms));
        (self.deliver)(Update::Projection(Box::new(projection)));
    }

    fn resync(&self) {
        if let Ok(mut newest) = self.newest.lock() {
            newest.1 = true;
        }
    }
}

impl Session {
    /// Registers with the core at `origin`, starts the engine, and follows
    /// the session. `deliver` is called from background threads.
    pub fn start(
        origin: &str,
        device_id: String,
        normalise_volume: bool,
        deliver: impl Fn(Update) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let (jobs, inbox) = unbounded();
        let reports = jobs.clone();
        let engine = Engine::start(origin.to_owned(), normalise_volume, move |event| {
            let _ = reports.send(Job::Report(engine_event(event)));
        })?;
        let shared = Arc::new(Shared {
            client: Client::new(origin),
            device_id,
            engine,
            newest: Mutex::new((0, true)),
            deliver: Box::new(deliver),
        });
        let stopping = Arc::new(AtomicBool::new(false));
        let handle = shared.clone();

        std::thread::Builder::new()
            .name("session-commands".into())
            .spawn({
                let shared = shared.clone();
                move || run_jobs(&shared, &inbox)
            })?;
        std::thread::Builder::new()
            .name("session-events".into())
            .spawn({
                let stopping = stopping.clone();
                move || follow(&shared, &stopping)
            })?;
        Ok(Self {
            jobs,
            stopping,
            shared: handle,
        })
    }

    pub fn set_equalizer(&self, enabled: bool, gains: [f32; 10]) {
        let settings = spotified_audio::eq::Settings { enabled, gains };
        self.shared.engine.set_equalizer(settings);
    }

    /// A window onto what is being played, for the visualizer.
    pub fn tap(&self) -> Arc<spotified_audio::tap::Tap> {
        self.shared.engine.tap()
    }

    /// How long tracks fade into one another; zero for no fade.
    pub fn set_crossfade(&self, seconds: u32) {
        let _ = self.jobs.send(Job::Transitions(u64::from(seconds) * 1000));
    }

    /// Takes effect from the next track.
    pub fn set_normalise_volume(&self, on: bool) {
        self.shared.engine.set_normalise(on);
    }

    pub fn send(&self, command: Command) {
        let _ = self.jobs.send(Job::Command(command));
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        // The engine holds a sender to this queue for its reports, so the
        // queue never closes by itself; the thread has to be told.
        let _ = self.jobs.send(Job::Stop);
    }
}

fn run_jobs(shared: &Shared, inbox: &Receiver<Job>) {
    for job in inbox {
        match job {
            Job::Command(command) => match shared.client.command(&shared.device_id, &command) {
                Ok(projection) => shared.accept(projection),
                Err(error) => {
                    log::warn!("command {command:?} failed: {error}");
                    (shared.deliver)(Update::Refused(error.to_string()));
                }
            },
            Job::Report(event) => {
                if let Err(error) = shared.client.engine_event(&shared.device_id, &event) {
                    log::debug!("engine report not delivered: {error}");
                }
            }
            Job::Transitions(crossfade_ms) => {
                if let Err(error) = shared.client.set_transitions(crossfade_ms) {
                    log::warn!("crossfade setting not delivered: {error}");
                }
            }
            Job::Stop => return,
        }
    }
}

/// Registers and follows the event stream, again each time it drops: the
/// core forgets a device whose stream has gone.
fn follow(shared: &Shared, stopping: &AtomicBool) {
    let mut wait = RECONNECT.0;
    while !stopping.load(Ordering::Relaxed) {
        shared.resync();
        match shared.client.register(&shared.device_id) {
            Ok(projection) => {
                wait = RECONNECT.0;
                shared.accept(projection);
                let followed = shared
                    .client
                    .events(&shared.device_id, |projection| shared.accept(projection));
                if let Err(error) = followed {
                    log::debug!("session stream: {error}");
                }
            }
            Err(error) => log::debug!("session register: {error}"),
        }
        if stopping.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(RECONNECT.1);
    }
}

fn engine_target(target: &Target, duration_ms: u64) -> engine::Target {
    engine::Target {
        epoch: target.epoch,
        video_id: target.video_id.clone(),
        start_at_ms: target.start_at_ms,
        playing: target.playing,
        preload_video_id: target.preload_video_id.clone(),
        volume: target.volume,
        duration_ms,
        crossfade_ms: target.transition.crossfade_ms(),
    }
}

fn engine_event(event: engine::Event) -> EngineEvent {
    let report = |kind, epoch| EngineEvent {
        kind,
        epoch,
        position_ms: 0,
        duration_ms: 0,
        reason: String::new(),
    };
    match event {
        engine::Event::Loaded { epoch, duration_ms } => EngineEvent {
            duration_ms,
            ..report(EngineEventKind::Loaded, epoch)
        },
        engine::Event::Position {
            epoch,
            position_ms,
            duration_ms,
        } => EngineEvent {
            position_ms,
            duration_ms,
            ..report(EngineEventKind::Position, epoch)
        },
        engine::Event::Ended { epoch } => report(EngineEventKind::Ended, epoch),
        engine::Event::Failed { epoch, reason } => EngineEvent {
            reason,
            ..report(EngineEventKind::Failed, epoch)
        },
        engine::Event::Stalled { epoch } => EngineEvent {
            reason: "stalled".into(),
            ..report(EngineEventKind::Stalled, epoch)
        },
        engine::Event::Blocked { epoch } => report(EngineEventKind::Blocked, epoch),
    }
}
