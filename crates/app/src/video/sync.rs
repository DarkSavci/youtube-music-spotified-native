//! Keeping the picture with the sound.
//!
//! The audio engine's clock is the only clock. Nothing here plays: the
//! picture shown is whichever one the song's position falls in, so a pause
//! holds it, a seek moves it and a faster song runs it faster, with no rate
//! of its own to drift by. All of it is arithmetic on times, and tested.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::mp4::SECOND;

/// How far ahead of the last picture decoded the song may be before it is
/// quicker to jump there than to decode up to it.
const JUMP_AHEAD: i64 = 2 * SECOND;
/// How far behind the oldest picture in hand the song may be and still
/// count as the same place: a report from the engine can be a little old.
const JUMP_BACK: i64 = SECOND / 4;
/// A waiting picture this far behind the song was made for somewhere the
/// song has since jumped away from, and is not shown at all.
const STALE: i64 = SECOND;
/// The most late pictures dropped in a row. A decoder that cannot keep up
/// then shows what it has, late, rather than nothing at all.
pub const MOST_DROPPED: u32 = 12;

/// Where the song is, as the window last said, in 100 ns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub position: i64,
    pub playing: bool,
    pub speed: f32,
    pub read: Instant,
}

impl Clock {
    pub fn stopped(now: Instant) -> Self {
        Self {
            position: 0,
            playing: false,
            speed: 1.0,
            read: now,
        }
    }

    /// Where the song is at `now`: on from the reading while it plays.
    pub fn at(&self, now: Instant) -> i64 {
        if !self.playing {
            return self.position;
        }
        let passed = now.saturating_duration_since(self.read).as_secs_f64();
        self.position + (passed * f64::from(self.speed) * SECOND as f64) as i64
    }

    /// Whether `later` says something this reading did not foretell: a
    /// pause, a change of speed, or a position that is not where this one
    /// would have got to. That is what the decoder has to be woken for.
    pub fn surprised_by(&self, later: &Clock) -> bool {
        self.playing != later.playing
            || self.speed != later.speed
            || (self.at(later.read) - later.position).abs() > JUMP_BACK
    }
}

/// Something that shows from `time` on.
pub trait Timed {
    fn time(&self) -> i64;
}

/// What the decoder has in hand since it last jumped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// The earliest time it could still show: the picture on screen, or
    /// where decoding began.
    pub from: i64,
    /// The end of the last picture decoded.
    pub to: i64,
}

/// Whether to jump to `position` rather than decode on towards it.
pub fn must_jump(position: i64, span: Option<Span>) -> bool {
    match span {
        None => true,
        Some(span) => position < span.from - JUMP_BACK || position > span.to + JUMP_AHEAD,
    }
}

/// Whether a picture just decoded has already had its time. One that
/// `position` falls in is kept: it is the picture to show now.
pub fn is_late(time: i64, duration: i64, position: i64) -> bool {
    time + duration.max(1) <= position
}

/// Takes from the waiting pictures the one to show at `position`: the last
/// whose time has come. Those before it had theirs while nobody looked, and
/// go. `None` when the next is still to come, and the one on screen stays;
/// and when the song has jumped ahead of everything waiting, which goes
/// unseen while the decoder follows.
pub fn due<P: Timed>(waiting: &mut VecDeque<P>, position: i64) -> Option<P> {
    let mut show = None;
    while waiting
        .front()
        .is_some_and(|picture| picture.time() <= position)
    {
        show = waiting.pop_front();
    }
    show.filter(|picture| position - picture.time() < STALE)
}

/// How long until the next waiting picture is due, on the wall's clock: a
/// song played faster comes to it sooner.
pub fn until_next<P: Timed>(waiting: &VecDeque<P>, position: i64, speed: f32) -> Option<Duration> {
    let next = waiting.front()?;
    let ahead = (next.time() - position).max(0) as f64 / SECOND as f64;
    Some(Duration::from_secs_f64(ahead / f64::from(speed.max(0.1))))
}

/// The height to make pictures at for a surface `wanted` pixels tall: the
/// first step of a ladder that covers it, and never more than the film has.
/// A ladder, so dragging the window's edge does not remake the converter
/// at every pixel.
pub fn height_for(wanted: u32, native: u32) -> u32 {
    const LADDER: [u32; 5] = [240, 360, 480, 720, 1080];
    LADDER
        .into_iter()
        .find(|step| *step >= wanted)
        .unwrap_or(native)
        .min(native)
}

/// The size of a picture `height` tall with the film's shape, in whole
/// pairs of pixels, which is what a converter can make.
pub fn size_for(height: u32, native: (u32, u32)) -> (u32, u32) {
    if height >= native.1 || native.1 == 0 {
        return native;
    }
    let width = (u64::from(native.0) * u64::from(height) / u64::from(native.1)) as u32;
    ((width & !1).max(2), (height & !1).max(2))
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
