//! The engine's thread: the decks, the device, and the loop that keeps
//! them matching the target.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError};

use super::{Decks, Event, Op, Target, crossfade_due, gain_for, reconcile};
use crate::clock::Clock;
use crate::deck::{Deck, DeckError, Poll};
use crate::eq::{self, Equalizer};
use crate::loudness::Gain;
use crate::output::Output;
use crate::silence::{self, Tail};
use crate::source;
use crate::tap::Tap;

const POSITION_INTERVAL: Duration = Duration::from_millis(250);
/// Playing with nothing to play for this long is reported as a stall.
const STALL_AFTER: Duration = Duration::from_secs(6);
/// How often the engine looks at its decks while there is work to do. The
/// device holds a quarter of a second, so this keeps it fed many times over
/// without waking a hundred times a second.
const TICK: Duration = Duration::from_millis(20);
/// After a pause, how long the device runs on so the fade-out is heard.
const SUSPEND_AFTER: Duration = Duration::from_millis(80);
/// How often a playing engine looks for a change of sound device.
const DEVICE_CHECK: Duration = Duration::from_secs(2);

/// Samples of silence stood in at a time for an incoming track that has
/// no audio ready during a fade.
const LATE_BLOCK: usize = 2048;

mod playing;
mod watch;

use playing::{Fade, Playing};

/// What the app sets while the engine runs, read by the engine's thread.
pub(super) struct Dials {
    /// Whether tracks loaded from now on are evened out in loudness.
    pub normalise: AtomicBool,
    /// The loudness they are evened out to, in LUFS, as `f32` bits.
    pub loudness_target: AtomicU32,
    /// How fast the music plays, as `f32` bits; 1 is normal.
    pub speed: AtomicU32,
    /// The equalizer's settings, as the app last set them.
    pub equalizer: Mutex<eq::Settings>,
}

pub(super) struct Worker {
    origin: String,
    dials: Arc<Dials>,
    /// The speed in force, as last read from the dials.
    speed: f32,
    /// Built with the device, whose sample rate its filters depend on.
    equalizer: Option<Equalizer>,
    /// Where what is played can be watched from.
    tap: Arc<Tap>,
    /// Where the track has reached, for whatever keeps time with it.
    clock: Arc<Clock>,
    /// Audio on its way to the device, copied here to be equalized.
    shaped: Vec<f32>,
    emit: Box<dyn Fn(Event)>,
    agent: ureq::Agent,
    /// Opened at the first load, so an app that never plays never touches
    /// the sound device.
    output: Option<Output>,
    target: Target,
    current: Option<Playing>,
    /// The track before, while it fades out under the current one.
    fading_out: Option<Fade>,
    next: Option<Deck>,
    /// A preload that failed is not tried again until the target names
    /// another: it would fail the same way every tick.
    failed_preload: Option<String>,
    /// The epoch a failure was last reported for; one report per epoch.
    failed_epoch: Option<u64>,
    /// The epoch an end was last reported for, likewise.
    ended_epoch: Option<u64>,
    last_position_report: Instant,
    /// When audio last reached the device, for the stall watch.
    last_audio: Instant,
    stall_reported: bool,
    /// When to stop the device after a pause.
    suspend_at: Option<Instant>,
    /// When the device was last checked for having changed.
    device_checked: Instant,
}

impl Worker {
    pub(super) fn new(
        origin: String,
        dials: Arc<Dials>,
        tap: Arc<Tap>,
        clock: Arc<Clock>,
        emit: Box<dyn Fn(Event)>,
    ) -> Self {
        let now = Instant::now();
        Self {
            origin,
            dials,
            speed: 1.0,
            equalizer: None,
            tap,
            clock,
            shaped: Vec::new(),
            emit,
            agent: source::agent(),
            output: None,
            target: Target::default(),
            current: None,
            fading_out: None,
            next: None,
            failed_preload: None,
            failed_epoch: None,
            ended_epoch: None,
            last_position_report: now,
            last_audio: now,
            stall_reported: false,
            suspend_at: None,
            device_checked: now,
        }
    }

    pub(super) fn run(mut self, inbox: &Receiver<Target>) {
        loop {
            // With nothing in motion the thread sleeps until told otherwise,
            // so a paused app does no work here at all.
            let received = if self.busy() {
                inbox.recv_timeout(TICK)
            } else {
                inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };
            match received {
                // Only the latest target matters.
                Ok(target) => self.apply(inbox.try_iter().last().unwrap_or(target)),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.tick();
            self.tell_clock();
        }
    }

    /// Whether anything needs watching between targets.
    fn busy(&self) -> bool {
        self.target.playing
            || self.suspend_at.is_some()
            || self.current.as_ref().is_some_and(|playing| !playing.ready)
    }

    fn position_ms(&self) -> u64 {
        let Some(playing) = &self.current else {
            return 0;
        };
        // What waits at the device is that long in the hearing, and longer
        // or shorter than that in the track.
        let queued_ms = self.output.as_ref().map_or(0, |output| {
            let heard = output.queued_frames() as f64 * 1000.0 / f64::from(output.sample_rate());
            (heard * f64::from(self.speed)) as u64
        });
        playing.fed_until_ms.saturating_sub(queued_ms)
    }

    fn apply(&mut self, target: Target) {
        let decks = Decks {
            current: self.current.as_ref().map(|playing| playing.deck.video_id()),
            position_ms: self.position_ms(),
            ended: self.current.as_ref().is_some_and(|playing| playing.ended),
            next: self.next.as_ref().map(Deck::video_id),
        };
        let ops = reconcile(decks, &self.target, &target);
        if target.epoch != self.target.epoch {
            self.failed_epoch = None;
        }
        if target.preload_video_id != self.target.preload_video_id {
            self.failed_preload = None;
        }
        let was_playing = self.target.playing;
        let new_epoch = target.epoch != self.target.epoch;
        self.target = target;
        for op in ops {
            self.perform(op);
        }
        // A track the engine moved into by itself was ready before the core
        // named it; the core still needs to hear that it is.
        if new_epoch
            && self.on_target()
            && let Some(playing) = self.current.as_ref().filter(|playing| playing.ready)
        {
            (self.emit)(Event::Loaded {
                epoch: self.target.epoch,
                duration_ms: playing.duration_ms,
            });
        }
        self.set_playing(was_playing);
        if let Some(output) = &self.output {
            output.set_gain(gain_for(self.target.volume));
        }
    }

    fn perform(&mut self, op: Op) {
        match op {
            Op::Stop => {
                self.fading_out = None;
                self.current = None;
                self.next = None;
                self.flush();
            }
            Op::Load { start_ms } => {
                // A skip cuts: whatever was fading out goes with it.
                self.fading_out = None;
                self.flush();
                let id = self.target.video_id.clone();
                self.current = self
                    .spawn_deck(&id, start_ms, false)
                    .map(|deck| Playing::new(deck, start_ms));
                self.restart_watches();
            }
            Op::Promote => {
                self.fading_out = None;
                self.flush();
                self.current = self.next.take().map(|deck| Playing::new(deck, 0));
                self.restart_watches();
            }
            Op::Seek(position_ms) => {
                self.fading_out = None;
                self.flush();
                if let Some(playing) = &mut self.current {
                    playing.deck.seek(position_ms);
                    playing.fed_until_ms = position_ms;
                    playing.clear();
                    playing.ended = false;
                }
                self.restart_watches();
            }
            Op::Preload => {
                let id = self.target.preload_video_id.clone();
                self.next = if self.failed_preload.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    self.spawn_deck(&id, 0, true)
                };
            }
            Op::DropPreload => self.next = None,
        }
    }

    fn spawn_deck(&mut self, video_id: &str, start_ms: u64, preload: bool) -> Option<Deck> {
        if self.output.is_none() {
            match Output::open() {
                Ok(output) => {
                    self.equalizer = Some(Equalizer::new(output.sample_rate()));
                    self.tap.set_sample_rate(output.sample_rate());
                    self.output = Some(output);
                }
                Err(error) => {
                    log::error!("{error}");
                    self.fail(DeckError::Decode(error));
                    return None;
                }
            }
        }
        let rate = self.output.as_ref()?.sample_rate();
        let query = if preload { "?preload=1" } else { "" };
        let url = format!("{}/v1/stream/{video_id}{query}", self.origin);
        let gain = if self.dials.normalise.load(Ordering::Relaxed) {
            let url = format!("{}/v1/tracks/{video_id}/loudness", self.origin);
            let target = f32::from_bits(self.dials.loudness_target.load(Ordering::Relaxed));
            Gain::fetch(self.agent.clone(), url, target)
        } else {
            Gain::unity()
        };
        Some(Deck::spawn(
            self.agent.clone(),
            url,
            video_id.to_owned(),
            start_ms,
            rate,
            gain,
        ))
    }

    fn flush(&self) {
        if let Some(output) = &self.output {
            output.flush();
        }
    }

    fn restart_watches(&mut self) {
        self.last_audio = Instant::now();
        self.stall_reported = false;
    }

    fn set_playing(&mut self, was_playing: bool) {
        let Some(output) = &self.output else {
            return;
        };
        if self.target.playing {
            self.suspend_at = None;
            output.play();
            if !was_playing {
                self.restart_watches();
            }
        } else if was_playing {
            output.pause();
            self.suspend_at = Some(Instant::now() + SUSPEND_AFTER);
        }
    }

    fn tick(&mut self) {
        if let (Some(equalizer), Ok(settings)) = (&mut self.equalizer, self.dials.equalizer.lock())
        {
            equalizer.set(*settings);
        }
        self.speed = f32::from_bits(self.dials.speed.load(Ordering::Relaxed));
        if self.suspend_at.is_some_and(|at| Instant::now() >= at) {
            self.suspend_at = None;
            if let Some(output) = &self.output {
                output.suspend();
            }
        }
        self.follow_device();
        self.start_crossfade();
        self.feed();
        self.watch_preload();
        self.report();
    }

    /// Whether the playing deck holds the track the target names. It does
    /// not for the moment between a gapless change and the core catching up.
    fn on_target(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|playing| playing.deck.video_id() == self.target.video_id)
    }

    /// Moves decoded audio to the device while there is room for it.
    fn feed(&mut self) {
        let (Some(output), Some(playing)) = (&mut self.output, &mut self.current) else {
            return;
        };
        let mut failure = None;
        let (rate, speed) = (output.sample_rate(), self.speed);
        loop {
            if playing.pending.is_none() {
                match playing.deck.poll() {
                    Poll::Chunk {
                        samples,
                        position_ms,
                    } => {
                        let frames = (samples.len() / 2) as u64;
                        let ends_ms = position_ms + frames * 1000 / u64::from(rate);
                        playing.fed_until_ms = ends_ms;
                        let quiet = match playing.quiet_to_skip {
                            0 => 0,
                            _ => silence::leading_quiet(&samples),
                        };
                        if quiet == samples.len() {
                            // Nothing in it to hear: on to the next.
                            playing.quiet_to_skip = playing.quiet_to_skip.saturating_sub(frames);
                            continue;
                        }
                        playing.quiet_to_skip = 0;
                        playing.take(samples, quiet, rate, speed);
                        // What is held back to change its speed has not
                        // been handed on, and is not counted as fed.
                        playing.fed_until_ms = ends_ms.saturating_sub(playing.held_ms(rate));
                        if playing.pending.is_none() {
                            continue;
                        }
                    }
                    Poll::Ready { duration_ms } => {
                        playing.ready = true;
                        playing.duration_ms = duration_ms.unwrap_or(0);
                        if playing.deck.video_id() == self.target.video_id {
                            (self.emit)(Event::Loaded {
                                epoch: self.target.epoch,
                                duration_ms: playing.duration_ms,
                            });
                        }
                        continue;
                    }
                    Poll::Pending if self.fading_out.is_some() => {}
                    Poll::Pending => break,
                    Poll::Ended => {
                        playing.ended = true;
                        playing.take_rest();
                        if playing.pending.is_none() {
                            break;
                        }
                    }
                    Poll::Failed(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            // Audio is only handed over while playing: a paused deck keeps
            // what it has decoded and resumes from exactly there.
            if !self.target.playing {
                break;
            }
            let room = output.free() & !1;
            if room == 0 {
                break;
            }
            self.shaped.clear();
            match &mut playing.pending {
                Some((samples, sent)) => {
                    let end = (*sent + room).min(samples.len());
                    self.shaped.extend_from_slice(&samples[*sent..end]);
                    *sent = end;
                    if end == samples.len() {
                        playing.pending = None;
                    }
                }
                // The incoming track is late. Silence stands in for it, so
                // the one fading out carries on rather than stopping dead.
                None if self.fading_out.is_some() => {
                    self.shaped.resize(room.min(LATE_BLOCK), 0.0);
                }
                None => break,
            }
            if let Some(fade) = &mut self.fading_out {
                fade.mix_into(&mut self.shaped, rate, speed);
                if fade.finished() {
                    self.fading_out = None;
                }
            }
            // Shaped on the way out rather than at decode, so a moved slider
            // is heard as soon as what is queued has played.
            if let Some(equalizer) = &mut self.equalizer {
                equalizer.process(&mut self.shaped);
            }
            let queued = output.queued_frames() + self.shaped.len() / 2;
            let ahead = Duration::from_secs_f64(queued as f64 / f64::from(output.sample_rate()));
            self.tap.write(&self.shaped, ahead + output.delay());
            output.push(&self.shaped);
            self.last_audio = Instant::now();
            self.stall_reported = false;
        }
        if let Some(error) = failure {
            self.fail(error);
        } else if self
            .current
            .as_ref()
            .is_some_and(|playing| playing.ended && playing.pending.is_none())
        {
            self.finish();
        }
    }

    /// Begins fading into the next track when the current one is the
    /// fade's length from its end and the next is ready. The end is
    /// reported here, at the start of the fade: the core moves on, and
    /// names the track that is already fading in.
    fn start_crossfade(&mut self) {
        if !self.target.playing || self.fading_out.is_some() || !self.on_target() {
            return;
        }
        let Some(playing) = &mut self.current else {
            return;
        };
        if self.target.crossfade_ms == 0 || !playing.ready {
            return;
        }
        // The end is read through a connection of its own, once, and only
        // when there is a fade to place by it.
        let tail = playing.tail.get_or_insert_with(|| {
            let url = format!("{}/v1/stream/{}", self.origin, playing.deck.video_id());
            Tail::listen(self.agent.clone(), url)
        });
        let music_ends_ms = tail.music_ends_ms();
        let duration_ms = if playing.duration_ms > 0 {
            playing.duration_ms
        } else {
            self.target.duration_ms
        };
        let next_is_ready = self
            .next
            .as_ref()
            .is_some_and(|deck| deck.video_id() == self.target.preload_video_id);
        // The fade is placed against where the music ends, which for a
        // track that trails off into silence is before the track does.
        let music_ends_ms = music_ends_ms.map_or(duration_ms, |end_ms| end_ms.min(duration_ms));
        let due = crossfade_due(
            playing.fed_until_ms,
            music_ends_ms,
            self.target.crossfade_ms,
            self.speed,
        );
        let (true, true, Some(output)) = (next_is_ready, due, &self.output) else {
            return;
        };
        let rate = u64::from(output.sample_rate());
        let total = self.target.crossfade_ms * rate / 1000;
        let quiet = silence::MOST_LEAD_MS * rate / 1000;
        let incoming = self
            .next
            .take()
            .map(|deck| Playing::faded_into(deck, quiet));
        if let Some(outgoing) = std::mem::replace(&mut self.current, incoming) {
            self.fading_out = Some(Fade {
                outgoing,
                total,
                done: 0,
            });
        }
        self.restart_watches();
        self.ended_epoch = Some(self.target.epoch);
        (self.emit)(Event::Ended {
            epoch: self.target.epoch,
        });
    }

    /// The playing deck has run out. Reports the end once, and if the next
    /// track is ready, carries straight on into it: that is what gapless is.
    fn finish(&mut self) {
        if !self.on_target() {
            return;
        }
        let next_is_ready = self
            .next
            .as_ref()
            .is_some_and(|deck| deck.video_id() == self.target.preload_video_id);
        // Without gapless the core is waited for, as it is for any track.
        if next_is_ready && self.target.gapless {
            self.current = self.next.take().map(|deck| Playing::new(deck, 0));
            self.restart_watches();
            (self.emit)(Event::Ended {
                epoch: self.target.epoch,
            });
            return;
        }
        // Otherwise the end is when the last of it has left the device.
        let drained = self
            .output
            .as_ref()
            .is_none_or(|output| output.queued_frames() == 0);
        if drained && self.ended_epoch != Some(self.target.epoch) {
            // One report per epoch: the core answers with a new target.
            self.ended_epoch = Some(self.target.epoch);
            (self.emit)(Event::Ended {
                epoch: self.target.epoch,
            });
        }
    }
}
