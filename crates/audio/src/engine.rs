//! The engine: handed the state playback should be in, it gets there.
//!
//! The core owns the queue and decides what plays. It sends a [`Target`];
//! the engine loads, seeks, plays or pauses until it matches, and reports
//! what happens as [`Event`]s. What to do about a target is worked out by
//! [`reconcile`], a pure function, so those rules are tested without a
//! sound device or a network.

mod worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::{Sender, unbounded};

use worker::Worker;

use crate::eq;
use crate::tap::Tap;

/// How far the engine may be from the target's position before it seeks.
/// Less than this is ordinary drift between two clocks, not a request.
const SEEK_TOLERANCE_MS: u64 = 1500;

/// What playback should be, as the core states it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Target {
    /// Changes whenever the track does, or the same track is started again.
    pub epoch: u64,
    /// Empty when nothing is to play.
    pub video_id: String,
    pub start_at_ms: u64,
    pub playing: bool,
    /// The track after this one, to have ready. Empty when there is none.
    pub preload_video_id: String,
    /// 0 to 1, or up to 2 with boost.
    pub volume: f32,
    /// The track's length as its listing gives it, for when the stream
    /// itself does not say. 0 when unknown.
    pub duration_ms: u64,
    /// How long one track fades into the next at its end. 0 for none.
    pub crossfade_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Loaded {
        epoch: u64,
        duration_ms: u64,
    },
    Position {
        epoch: u64,
        position_ms: u64,
        duration_ms: u64,
    },
    Ended {
        epoch: u64,
    },
    Failed {
        epoch: u64,
        reason: String,
    },
    /// Playing, but no audio is arriving.
    Stalled {
        epoch: u64,
    },
    /// YouTube is refusing requests; the track itself is not at fault.
    Blocked {
        epoch: u64,
    },
}

/// The decks as [`reconcile`] needs to see them.
#[derive(Debug, Default, Clone, Copy)]
pub struct Decks<'a> {
    /// The track on the playing deck.
    pub current: Option<&'a str>,
    pub position_ms: u64,
    /// The playing deck has run to its end.
    pub ended: bool,
    /// The track on the preloaded deck.
    pub next: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Nothing is to play: drop both decks.
    Stop,
    /// Load the target's track on the playing deck from this position.
    Load {
        start_ms: u64,
    },
    /// The preloaded deck holds the target's track: make it the playing one.
    Promote,
    Seek(u64),
    /// Load the target's next track on the preload deck.
    Preload,
    DropPreload,
}

/// What must change for the decks to match `target`. `previous` is the
/// target the engine last acted on.
pub fn reconcile(decks: Decks<'_>, previous: &Target, target: &Target) -> Vec<Op> {
    if target.video_id.is_empty() {
        return vec![Op::Stop];
    }
    let mut ops = Vec::new();
    let mut next = decks.next;
    if decks.current != Some(target.video_id.as_str()) {
        if next == Some(target.video_id.as_str()) {
            ops.push(Op::Promote);
            next = None;
            if target.start_at_ms > SEEK_TOLERANCE_MS {
                ops.push(Op::Seek(target.start_at_ms));
            }
        } else {
            ops.push(Op::Load {
                start_ms: target.start_at_ms,
            });
        }
    } else if previous.video_id == target.video_id {
        // The same track under a new epoch, after it ended, is the core
        // starting it again: repeat one.
        let restarted = decks.ended && target.epoch != previous.epoch;
        let moved = decks.position_ms.abs_diff(target.start_at_ms) > SEEK_TOLERANCE_MS;
        if restarted || moved {
            ops.push(Op::Seek(target.start_at_ms));
        }
    }
    // Otherwise the deck already plays a track the core has only now named:
    // the engine ran into it gaplessly. However far in it is by the time the
    // core catches up, it is left playing; a seek to the start would be
    // heard as a stutter.
    let wanted = (!target.preload_video_id.is_empty()).then_some(target.preload_video_id.as_str());
    match (next, wanted) {
        (Some(have), Some(want)) if have == want => {}
        (_, Some(_)) => ops.push(Op::Preload),
        (Some(_), None) => ops.push(Op::DropPreload),
        (None, None) => {}
    }
    ops
}

/// Whether it is time to start fading into the next track: the fade's
/// length before the end. A track too short to hold a fade at each end is
/// left to end plainly.
pub fn crossfade_due(position_ms: u64, duration_ms: u64, crossfade_ms: u64) -> bool {
    crossfade_ms > 0 && duration_ms > crossfade_ms * 2 && position_ms + crossfade_ms >= duration_ms
}

/// The slider's position as a gain. Loudness is heard on a curve, so the
/// lower half of the slider is given more of the range; past 1 is plain
/// boost.
pub fn gain_for(volume: f32) -> f32 {
    if volume <= 1.0 {
        volume.max(0.0).powf(1.0 / 0.6)
    } else {
        volume.min(2.0)
    }
}

pub struct Engine {
    targets: Sender<Target>,
    normalise: Arc<AtomicBool>,
    equalizer: Arc<Mutex<eq::Settings>>,
    tap: Arc<Tap>,
}

impl Engine {
    /// Starts the engine thread. `origin` is the core's address; `emit` is
    /// called on the engine thread with each event, in order. `normalise`
    /// evens out loudness between tracks.
    pub fn start(
        origin: String,
        normalise: bool,
        emit: impl Fn(Event) + Send + 'static,
    ) -> std::io::Result<Self> {
        let (targets, inbox) = unbounded();
        let normalise = Arc::new(AtomicBool::new(normalise));
        let switch = normalise.clone();
        let equalizer = Arc::new(Mutex::new(eq::Settings::default()));
        let bands = equalizer.clone();
        let tap = Arc::new(Tap::default());
        let window = tap.clone();
        std::thread::Builder::new()
            .name("engine".into())
            .spawn(move || {
                Worker::new(origin, switch, bands, window, Box::new(emit)).run(&inbox)
            })?;
        Ok(Self {
            targets,
            normalise,
            equalizer,
            tap,
        })
    }

    /// A window onto what is being played, for a visualizer.
    pub fn tap(&self) -> Arc<Tap> {
        self.tap.clone()
    }

    /// Heard within a quarter of a second: what is already queued for the
    /// device plays out first.
    pub fn set_equalizer(&self, settings: eq::Settings) {
        if let Ok(mut shared) = self.equalizer.lock() {
            *shared = settings;
        }
    }

    /// Takes effect from the next track loaded.
    pub fn set_normalise(&self, on: bool) {
        self.normalise.store(on, Ordering::Relaxed);
    }

    pub fn apply(&self, target: Target) {
        // The thread ends only when this is dropped.
        let _ = self.targets.send(target);
    }
}

#[cfg(test)]
mod tests;
