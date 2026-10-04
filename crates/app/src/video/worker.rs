//! The thread a picture is decoded on, and the one that fetches for it.
//!
//! The decoder keeps a few pictures ready ahead of the song and then waits
//! to be relieved of one. Nobody taking them is how it knows to stop: with
//! the window hidden or the picture closed the song's position is no longer
//! told, the decoder's idea of it stands still, the queue stays full and
//! the thread sleeps, whatever the song does meanwhile.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::decode::{Decoded, Decoder, Runtime};
use super::fetch::{Failure, Source};
use super::mp4::{self, Sample, Segment, Track};
use super::sync::{self, Clock, Span};
use super::{LET_GO_AFTER, Picture, Status};

/// Pictures kept ready ahead of the song. Enough to ride out a busy
/// moment, few enough that a film at full size is not much memory.
const READY: usize = 4;
/// How long the song is taken to have played on since the window last
/// said where it was. Past this nobody is watching, and the position
/// stands still rather than run on by guesswork.
const UNWATCHED: Duration = Duration::from_millis(700);
/// How long a wait lasts before the thread looks again for itself.
const LOOK_AGAIN: Duration = Duration::from_millis(250);
/// How much of the file is asked for first; the opening of these files is
/// a kilobyte or two, and a second request fetches the rest if not.
const OPENING: u64 = 4096;
/// What the core is asked for when the pictures are decoded in software:
/// full size costs most of a processor core that way.
const SOFTWARE_HEIGHT: u32 = 720;

pub struct Shared {
    pub inner: Mutex<Inner>,
    /// Something the decoder waits on has changed.
    pub changed: Condvar,
    /// There is a piece of the file to fetch.
    ordered: Condvar,
    pub stop: AtomicBool,
}

pub struct Inner {
    pub clock: Clock,
    /// How tall the surface is, in pixels of the screen.
    pub height: u32,
    /// When the window last asked.
    pub asked: Instant,
    pub waiting: VecDeque<Picture>,
    /// The time of the picture on screen.
    pub shown: Option<i64>,
    pub status: Status,
    order: Option<Order>,
    fetched: Option<(usize, Result<Vec<u8>, String>)>,
}

#[derive(Clone, Copy)]
struct Order {
    serial: u64,
    index: usize,
    segment: Segment,
}

impl Inner {
    /// Where the song is now, as far as the window has vouched for it.
    fn position(&self) -> i64 {
        self.clock.at(Instant::now().min(self.asked + UNWATCHED))
    }
}

impl Shared {
    pub fn new(now: Instant) -> Self {
        Self {
            inner: Mutex::new(Inner {
                clock: Clock::stopped(now),
                height: 0,
                asked: now,
                waiting: VecDeque::new(),
                shown: None,
                status: Status::Loading,
                order: None,
                fetched: None,
            }),
            changed: Condvar::new(),
            ordered: Condvar::new(),
            stop: AtomicBool::new(false),
        }
    }

    /// A panic on another thread while it held the lock leaves nothing
    /// half-written that matters here, so the lock is taken regardless.
    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn end(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.changed.notify_all();
        self.ordered.notify_all();
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

enum Fault {
    Stopped,
    /// The file could not be had: the same would happen in software.
    Fetch(String),
    /// The decoder gave up: worth another go without the graphics card.
    Decode(String),
}

impl From<Failure> for Fault {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Stopped => Fault::Stopped,
            Failure::Failed(reason) => Fault::Fetch(reason),
        }
    }
}

/// Runs until told to stop or until the picture cannot be had. `address`
/// is the core's address for the video's stream; `wake` redraws the window.
pub fn run(shared: &Arc<Shared>, address: &str, wake: &(dyn Fn() + Send + Sync)) {
    let outcome = Runtime::start()
        .map_err(Fault::Decode)
        .and_then(
            |_runtime| match play(shared, &format!("{address}?codec=h264"), true, wake) {
                Err(Fault::Decode(reason)) => {
                    log::warn!("video: {reason}; decoding in software instead");
                    let smaller = format!("{address}?codec=h264&height={SOFTWARE_HEIGHT}");
                    play(shared, &smaller, false, wake)
                }
                other => other,
            },
        );
    if let Err(Fault::Fetch(reason) | Fault::Decode(reason)) = outcome {
        log::warn!("video: {reason}");
        let mut inner = shared.lock();
        inner.status = Status::Failed(reason);
        inner.waiting.clear();
        drop(inner);
        wake();
    }
    shared.end();
}

fn play(
    shared: &Arc<Shared>,
    url: &str,
    hardware: bool,
    wake: &(dyn Fn() + Send + Sync),
) -> Result<(), Fault> {
    let source = Arc::new(Source::new(url.to_owned()));
    let mut opening = source.opening(OPENING, &shared.stop)?;
    if let Some(length) = mp4::opening_length(&opening).filter(|length| *length > opening.len()) {
        opening = source.opening(length as u64, &shared.stop)?;
    }
    let (track, segments) =
        mp4::open(&opening).map_err(|fault| Fault::Fetch(format!("not a film: {fault}")))?;
    let native = (track.width, track.height);
    let size = sync::size_for(sync::height_for(shared.lock().height, native.1), native);
    let decoder = Decoder::new(native, size, hardware).map_err(Fault::Decode)?;
    log::info!(
        "video: {}x{} shown at {}x{}, decoded {}",
        native.0,
        native.1,
        size.0,
        size.1,
        if decoder.hardware() {
            "on the graphics card"
        } else {
            "in software"
        }
    );
    let fetcher = {
        let (shared, source) = (shared.clone(), source.clone());
        std::thread::Builder::new()
            .name("video-fetch".to_owned())
            .spawn(move || fetch(&shared, &source))
    };
    if let Err(error) = fetcher {
        return Err(Fault::Fetch(format!("no thread to fetch on: {error}")));
    }
    let stream = Stream {
        shared,
        track,
        segments,
        decoder,
        current: None,
        need: 0,
        span: None,
        dropped: 0,
        serial: 0,
        ended: false,
        unit: Vec::new(),
        jumped: None,
        wake,
    };
    stream.run()
}

/// Fetches the pieces the decoder orders, one at a time, until told to stop.
fn fetch(shared: &Shared, source: &Source) {
    let mut done = 0;
    loop {
        let order = {
            let mut inner = shared.lock();
            loop {
                if shared.stopped() {
                    return;
                }
                match inner.order {
                    Some(order) if order.serial != done => break order,
                    _ => {}
                }
                inner = shared
                    .ordered
                    .wait(inner)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let segment = order.segment;
        let bytes = match source.part(segment.offset, segment.size, &shared.stop) {
            Ok(bytes) => Ok(bytes),
            Err(Failure::Stopped) => return,
            Err(Failure::Failed(reason)) => Err(reason),
        };
        done = order.serial;
        let mut inner = shared.lock();
        // An order placed since is for somewhere else; this one is stale.
        if inner.order.is_some_and(|latest| latest.serial == done) {
            inner.fetched = Some((order.index, bytes));
        }
        drop(inner);
        shared.changed.notify_all();
    }
}

/// A piece of the file in hand, and how far into it decoding has got.
struct Loaded {
    index: usize,
    bytes: Vec<u8>,
    samples: Vec<Sample>,
    next: usize,
}

enum Next {
    Picture(Decoded),
    /// The piece wanted is on its way.
    Waiting,
    /// The film is over.
    End,
}

struct Stream<'a> {
    shared: &'a Arc<Shared>,
    track: Track,
    segments: Vec<Segment>,
    decoder: Decoder,
    current: Option<Loaded>,
    /// The piece to decode next when none is in hand.
    need: usize,
    /// What has been decoded since the last jump.
    span: Option<Span>,
    /// Late pictures dropped in a row.
    dropped: u32,
    serial: u64,
    ended: bool,
    /// One picture's bytes in the form the decoder reads, reused.
    unit: Vec<u8>,
    /// When the last jump was made and where to, until its first picture.
    jumped: Option<(Instant, i64)>,
    wake: &'a (dyn Fn() + Send + Sync),
}

impl Stream<'_> {
    fn run(mut self) -> Result<(), Fault> {
        let end = self
            .segments
            .last()
            .map_or(0, |last| last.start + last.duration - 1);
        loop {
            let mut inner = self.shared.lock();
            if self.shared.stopped() {
                return Err(Fault::Stopped);
            }
            if inner.asked.elapsed() > LET_GO_AFTER {
                inner.waiting.clear();
                inner.status = Status::Asleep;
                return Err(Fault::Stopped);
            }
            let position = inner.position().clamp(0, end);
            let span = self.span.map(|span| Span {
                from: inner.shown.unwrap_or(span.from),
                to: span.to,
            });
            if sync::must_jump(position, span) {
                inner.waiting.clear();
                inner.shown = None;
                drop(inner);
                self.jump(position);
                continue;
            }
            if inner.waiting.len() >= READY || self.ended {
                drop(self.wait(inner));
                continue;
            }
            let native = (self.track.width, self.track.height);
            let size = sync::size_for(sync::height_for(inner.height, native.1), native);
            drop(inner);
            self.decoder.resize(size);
            match self.next()? {
                Next::Picture(decoded) => self.keep(decoded)?,
                Next::Waiting => {
                    let inner = self.shared.lock();
                    if inner.fetched.is_none() {
                        drop(self.wait(inner));
                    }
                }
                Next::End => self.ended = true,
            }
        }
    }

    /// Sleeps until something changes, or for a moment at most.
    fn wait<'g>(&self, inner: MutexGuard<'g, Inner>) -> MutexGuard<'g, Inner> {
        self.shared
            .changed
            .wait_timeout(inner, LOOK_AGAIN)
            .map(|(inner, _)| inner)
            .unwrap_or_else(|poisoned| poisoned.into_inner().0)
    }

    /// Starts again from the picture that stands alone before `position`.
    fn jump(&mut self, position: i64) {
        let index = mp4::segment_at(&self.segments, position);
        self.decoder.flush();
        match &mut self.current {
            Some(current) if current.index == index => current.next = 0,
            _ => self.current = None,
        }
        self.need = index;
        let start = self.segments.get(index).map_or(0, |segment| segment.start);
        self.span = Some(Span {
            from: start,
            to: start,
        });
        self.dropped = 0;
        self.ended = false;
        self.jumped = Some((Instant::now(), position));
    }

    /// Keeps a decoded picture for the window, unless its time has passed.
    fn keep(&mut self, decoded: Decoded) -> Result<(), Fault> {
        let (time, duration) = (decoded.time, decoded.duration);
        if let Some(span) = &mut self.span {
            span.to = span.to.max(time + duration);
        }
        let position = self.shared.lock().position();
        if sync::is_late(time, duration, position) && self.dropped < sync::MOST_DROPPED {
            self.dropped += 1;
            return Ok(());
        }
        self.dropped = 0;
        if let Some((since, to)) = self.jumped.take() {
            log::debug!(
                "video: jumped to {} ms, first picture ({} ms) after {} ms",
                to / 10_000,
                time / 10_000,
                since.elapsed().as_millis()
            );
        }
        let mut pixels = Vec::new();
        self.decoder
            .convert(decoded, &mut pixels)
            .map_err(Fault::Decode)?;
        let (width, height) = self.decoder.size();
        let mut inner = self.shared.lock();
        let first = inner.waiting.is_empty();
        inner.status = Status::Showing;
        inner.waiting.push_back(Picture {
            time,
            size: [width as usize, height as usize],
            pixels,
        });
        drop(inner);
        if first {
            // The window may be asleep: a paused song redraws for nothing
            // else, and this is the picture it is waiting for.
            (self.wake)();
        }
        Ok(())
    }

    /// The next picture out of the decoder, feeding it as it asks.
    fn next(&mut self) -> Result<Next, Fault> {
        // A decoder that neither takes a picture nor gives one has stuck.
        let mut refused = 0;
        loop {
            if let Some(decoded) = self.decoder.pull().map_err(Fault::Decode)? {
                return Ok(Next::Picture(decoded));
            }
            let Some(current) = &mut self.current else {
                match self.obtain()? {
                    Some(loaded) => self.current = Some(loaded),
                    None => return Ok(Next::Waiting),
                }
                continue;
            };
            let Some(sample) = current.samples.get(current.next).copied() else {
                if current.index + 1 >= self.segments.len() {
                    return Ok(Next::End);
                }
                self.need = current.index + 1;
                self.current = None;
                continue;
            };
            let bytes = current
                .bytes
                .get(sample.offset..sample.offset + sample.size)
                .ok_or_else(|| Fault::Fetch("a picture lies outside its piece".to_owned()))?;
            mp4::annex_b(&self.track, bytes, sample.key, &mut self.unit)
                .map_err(|fault| Fault::Fetch(fault.to_owned()))?;
            let taken = self
                .decoder
                .push(&self.unit, sample.time, sample.duration)
                .map_err(Fault::Decode)?;
            if taken {
                current.next += 1;
                refused = 0;
            } else {
                refused += 1;
                if refused > 3 {
                    return Err(Fault::Decode("the decoder is stuck".to_owned()));
                }
            }
        }
    }

    /// The piece needed, if it has been fetched; ordered if it has not.
    /// Once it is in hand the one after it is ordered, to be ready in time.
    fn obtain(&mut self) -> Result<Option<Loaded>, Fault> {
        let index = self.need;
        let mut inner = self.shared.lock();
        let ready = inner
            .fetched
            .as_ref()
            .is_some_and(|(fetched, _)| *fetched == index);
        if !ready {
            if inner.order.is_none_or(|order| order.index != index) {
                inner.fetched = None;
                self.order(&mut inner, index);
            }
            return Ok(None);
        }
        let bytes = match inner.fetched.take() {
            Some((_, Ok(bytes))) => bytes,
            Some((_, Err(reason))) => return Err(Fault::Fetch(reason)),
            None => return Ok(None),
        };
        self.order(&mut inner, index + 1);
        drop(inner);
        let samples = mp4::samples(&self.track, &bytes)
            .map_err(|fault| Fault::Fetch(format!("not a film: {fault}")))?;
        Ok(Some(Loaded {
            index,
            bytes,
            samples,
            next: 0,
        }))
    }

    fn order(&mut self, inner: &mut Inner, index: usize) {
        let Some(segment) = self.segments.get(index).copied() else {
            return;
        };
        self.serial += 1;
        inner.order = Some(Order {
            serial: self.serial,
            index,
            segment,
        });
        self.shared.ordered.notify_all();
    }
}
