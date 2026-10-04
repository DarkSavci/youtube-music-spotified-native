//! A song's music video, as pictures to draw.
//!
//! There is no sound here and no clock: the audio engine plays the song
//! and says where it is, and this hands over the picture for that moment.
//! The core serves the film (H.264, without its sound); `mp4` reads it,
//! `decode` turns it into pictures with what Windows brings, `sync` picks
//! the one that is due, and `worker` is the thread it all happens on.
//!
//! Nothing here can take the app down with it. A film that will not load
//! or decode is a `Status::Failed` and a line in the log, and the cover
//! stays where the picture would have been.

#[cfg(windows)]
mod decode;
#[cfg(windows)]
mod fetch;
mod mp4;
pub mod sync;
#[cfg(windows)]
mod worker;

use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;

use eframe::egui::Color32;

pub use sync::Clock;

/// One picture of the film, in the pixels egui draws.
pub struct Picture {
    /// When it shows, in 100 ns.
    pub time: i64,
    pub size: [usize; 2],
    pub pixels: Vec<Color32>,
}

impl sync::Timed for Picture {
    fn time(&self) -> i64 {
        self.time
    }
}

/// How long a picture nobody is looking at keeps its decoder. It sleeps
/// meanwhile, but holds the graphics card's device, a few pictures and a
/// piece of the file; past this they are let go, and looking again starts
/// afresh. A paused song is not looked at either, once it has been drawn.
pub const LET_GO_AFTER: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// No picture yet: the film is being found and opened.
    Loading,
    Showing,
    /// Nobody looked for long enough that the decoder was let go. Another
    /// player picks up where this one stopped.
    Asleep,
    /// There will be no picture, and why, for the log.
    Failed(String),
}

/// A film being decoded for as long as this is held.
pub struct Player {
    #[cfg(windows)]
    shared: std::sync::Arc<worker::Shared>,
}

impl Player {
    /// Starts on the film at `address`, the core's address for the video's
    /// stream. `wake` is called when there is something new to draw.
    #[cfg(windows)]
    pub fn start(address: String, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let shared = std::sync::Arc::new(worker::Shared::new(Instant::now()));
        let spawned = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("video".to_owned())
                .spawn(move || worker::run(&shared, &address, &wake))
        };
        if let Err(error) = spawned {
            log::warn!("video: no thread to decode on: {error}");
            shared.lock().status = Status::Failed(error.to_string());
        }
        Self { shared }
    }

    #[cfg(not(windows))]
    pub fn start(_address: String, _wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {}
    }

    /// Says where the song is and how tall the picture is drawn, in pixels
    /// of the screen. Called for every frame the picture is on screen, and
    /// not otherwise: that the calls have stopped is how the decoder knows
    /// to.
    #[cfg(windows)]
    pub fn tell(&self, clock: Clock, height: u32) {
        let mut inner = self.shared.lock();
        let news = inner.clock.surprised_by(&clock)
            || inner.height != height
            || clock.read.saturating_duration_since(inner.asked) > Duration::from_millis(500);
        inner.clock = clock;
        inner.height = height;
        inner.asked = clock.read;
        drop(inner);
        if news {
            self.shared.changed.notify_all();
        }
    }

    #[cfg(not(windows))]
    pub fn tell(&self, _clock: Clock, _height: u32) {}

    /// The picture to show at `position`, when it is not the one already
    /// shown.
    #[cfg(windows)]
    pub fn take(&self, position: i64) -> Option<Picture> {
        let mut inner = self.shared.lock();
        let picture = sync::due(&mut inner.waiting, position)?;
        inner.shown = Some(picture.time);
        drop(inner);
        // There is room for another now.
        self.shared.changed.notify_all();
        Some(picture)
    }

    #[cfg(not(windows))]
    pub fn take(&self, _position: i64) -> Option<Picture> {
        None
    }

    /// How long until the next picture is due.
    #[cfg(windows)]
    pub fn until_next(&self, position: i64, speed: f32) -> Option<Duration> {
        sync::until_next(&self.shared.lock().waiting, position, speed)
    }

    #[cfg(not(windows))]
    pub fn until_next(&self, _position: i64, _speed: f32) -> Option<Duration> {
        None
    }

    #[cfg(windows)]
    pub fn status(&self) -> Status {
        self.shared.lock().status.clone()
    }

    #[cfg(not(windows))]
    pub fn status(&self) -> Status {
        Status::Failed("music videos need Windows".to_owned())
    }
}

#[cfg(windows)]
impl Drop for Player {
    fn drop(&mut self) {
        // Not waited for: the thread may be in the middle of a request, and
        // leaves by itself when that returns.
        self.shared.end();
    }
}

#[cfg(all(test, windows))]
mod tests;
