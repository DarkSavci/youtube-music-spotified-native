//! The sound device.
//!
//! The engine writes stereo frames into a lock-free ring; the device's
//! callback drains it. The callback never locks and never allocates. Volume
//! and pause are applied there, so both take effect within one device
//! buffer rather than after everything already queued.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};

/// Audio queued ahead of the device. Long enough that a busy moment on the
/// engine thread is not heard, short enough that a seek is.
const RING_SECONDS: f32 = 0.25;
/// How fast the gain follows a change, per frame. About 10 ms to settle at
/// 48 kHz: no click on pause or a volume step, no audible lag.
const GAIN_SLEW: f32 = 0.004;

/// How far ahead the limiter looks, in frames: 2 ms at 48 kHz. Time to
/// bring the level down smoothly before a peak, rather than at once on it.
pub(crate) const LOOKAHEAD: usize = 96;
/// How long the level stays down after the last peak that needed it, in
/// frames: 50 ms at 48 kHz, longer than a cycle of the lowest bass, so a
/// held tone is held steady instead of pumped once a cycle.
const LIMITER_HOLD: u32 = 2400;
/// How fast it then lets go, per frame: a time constant of 50 ms at 48 kHz.
const LIMITER_RELEASE: f32 = 1.0 / 2400.0;
/// Frames of the ring the device callback looks at in one go: what it
/// plays, and the lookahead past it. Fixed, so the callback never
/// allocates; a longer callback is played in pieces.
const PIECE: usize = 1024;

/// How close the gain must come to its target to be given the rest: a
/// thousandth of a decibel at full volume, far too small to hear as a step.
/// Without it the glide stalls short, where a step rounds to nothing in
/// `f32`, and at 100% every sample would be scaled by 0.9999926.
const GAIN_SNAP: f32 = 1e-4;

/// Holds audio under full scale.
///
/// Three things pass it: volume past 100%; two songs crossing; and loud
/// masters themselves, whose decoded peaks can go over (to +1.6 dBFS in
/// tracks measured). A peak past full scale is clipped by the device,
/// which is heard as distortion.
///
/// It looks [`LOOKAHEAD`] frames ahead in what is queued, and brings the
/// level down in a straight line to arrive at what the coming peak needs
/// just as the peak does; holds it there while peaks keep coming; and lets
/// it back up after [`LIMITER_HOLD`]. The one before it cut the level on
/// the peak itself and let it straight back up, which pumped a held bass
/// note once a cycle (THD+N -49 dB at 50 Hz). Audio under full scale, away
/// from a peak it has had to hold, passes bit for bit.
struct Limiter {
    gain: f32,
    /// Frames left before letting go.
    held: u32,
}

impl Limiter {
    const NEW: Limiter = Limiter { gain: 1.0, held: 0 };

    /// One frame, already at the volume. `ahead` is what follows it,
    /// interleaved, before the volume, which `volume` is.
    fn apply(&mut self, frame: [f32; 2], ahead: &[f32], volume: f32) -> [f32; 2] {
        // The deepest cut the window needs, and how many frames off.
        let mut need = 1.0f32;
        let mut at = 0;
        let now = frame[0].abs().max(frame[1].abs());
        if now > 1.0 {
            need = 1.0 / now;
        }
        for (index, next) in ahead.as_chunks::<2>().0.iter().take(LOOKAHEAD).enumerate() {
            let peak = next[0].abs().max(next[1].abs()) * volume;
            if peak * need > 1.0 {
                need = 1.0 / peak;
                at = index + 1;
            }
        }
        if need < self.gain {
            // Down in a straight line, to arrive as the peak does.
            self.gain -= (self.gain - need) / (at as f32 + 1.0);
            self.held = LIMITER_HOLD;
        } else if need < 1.0 {
            self.held = LIMITER_HOLD;
        } else if self.held > 0 {
            self.held -= 1;
        } else if self.gain < 1.0 {
            self.gain += (1.0 - self.gain) * LIMITER_RELEASE;
            // The last of the way is too small a step to be taken; it is
            // given.
            if self.gain > 0.999 {
                self.gain = 1.0;
            }
        }
        let out = [frame[0] * self.gain, frame[1] * self.gain];
        // A volume moving within the window can leave a sliver over: that
        // is clipped, where the device would have clipped all of it.
        [out[0].clamp(-1.0, 1.0), out[1].clamp(-1.0, 1.0)]
    }
}

/// The volume as the device callback applies it, frame by frame: a gain
/// that glides to where it is asked to be, and the limiter that holds
/// peaks under full scale.
pub(crate) struct Volume {
    gain: f32,
    limiter: Limiter,
}

impl Volume {
    /// Silent, so the first sound fades in.
    pub(crate) fn new() -> Self {
        Self {
            gain: 0.0,
            limiter: Limiter::NEW,
        }
    }

    /// Moves the gain a frame's step towards `target`, and says where it is.
    pub(crate) fn step(&mut self, target: f32) -> f32 {
        self.gain += (target - self.gain) * GAIN_SLEW;
        if (target - self.gain).abs() < GAIN_SNAP {
            self.gain = target;
        }
        self.gain
    }

    /// Steps towards `target` for as long as `frames` would take: where
    /// the gain is once that much has played.
    pub(crate) fn settle(&mut self, target: f32, frames: usize) {
        for _ in 0..frames {
            self.step(target);
        }
    }

    /// One frame at the gain in force; `ahead` is what is queued after it,
    /// interleaved, for the limiter to see coming.
    pub(crate) fn apply(&mut self, frame: [f32; 2], ahead: &[f32]) -> [f32; 2] {
        // At a gain of exactly 1 and under full scale, every multiplication
        // is by 1: the samples come out as they went in.
        let gain = self.gain;
        self.limiter
            .apply([frame[0] * gain, frame[1] * gain], ahead, gain)
    }
}

/// Rounds to the steps of a 16-bit device, with triangular dither.
///
/// cpal turns a float into an `i16` by cutting off what is past the step
/// below, towards zero. Everything within two steps of silence comes out as
/// silence, and quiet passages pick up distortion where the cut falls.
/// Rounding to the nearest step after adding noise of up to one step either
/// way, shaped as a triangle, leaves an even hiss at -96 dBFS instead: the
/// tail of a fade and detail below the last step stay audible in it.
pub(crate) struct Dither {
    state: u32,
}

impl Dither {
    pub(crate) fn new() -> Self {
        Self { state: 0x2545_F491 }
    }

    /// A float that is exactly one of the 65,536 steps, so cpal's cut has
    /// nothing to cut.
    pub(crate) fn quantize_16_bit(&mut self, sample: f32) -> f32 {
        let noise = self.uniform() - self.uniform();
        let step = (sample * 32_768.0 + noise)
            .round()
            .clamp(-32_768.0, 32_767.0);
        step / 32_768.0
    }

    /// 0 to 1, by xorshift32: fast, and plenty for noise.
    fn uniform(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        (self.state >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// What the engine thread and the device callback share.
pub(crate) struct Shared {
    /// The gain to move towards, as `f32` bits.
    gain: AtomicU32,
    /// Fade to silence and stop taking from the ring.
    paused: AtomicBool,
    /// Throw away what is queued: it belongs to before a seek or a skip.
    flush: AtomicBool,
    /// The device reported an error: unplugged, or taken away by the system.
    lost: AtomicBool,
    /// How long the device takes to play what it is handed, in
    /// microseconds, as it last said.
    delay: AtomicU32,
}

pub struct Output {
    stream: Stream,
    producer: Producer<f32>,
    shared: Arc<Shared>,
    sample_rate: u32,
    capacity: usize,
    /// Which device this is, to tell when the one to play through has moved
    /// on. `None` where the device has no identity to compare.
    device: Option<String>,
    format: Format,
}

/// What plays the ring: a stream cpal runs, through the system's mixer, or
/// the device held for this app alone.
enum Stream {
    Shared(cpal::Stream),
    #[cfg(windows)]
    Exclusive(crate::exclusive::Stream),
}

/// How the device was opened, for the listener to see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    pub sample_rate: u32,
    /// `f32`, `i16`, `i24`, `i32`, as the device takes its samples.
    pub sample_format: &'static str,
    /// Held for this app alone, past the system's mixer and effects.
    pub exclusive: bool,
    /// Exclusive was asked for and could not be had: why.
    pub exclusive_refused: Option<String>,
}

/// A sound device there is to play through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputDevice {
    /// What the system knows it by: the same after a restart, and after it
    /// has been unplugged and put back.
    pub id: String,
    pub name: String,
}

/// The devices there are to play through now.
pub fn output_devices() -> Vec<OutputDevice> {
    let devices = match cpal::default_host().output_devices() {
        Ok(devices) => devices,
        Err(error) => {
            log::warn!("the sound devices could not be listed: {error}");
            return Vec::new();
        }
    };
    devices
        .filter_map(|device| {
            let id = device.id().ok()?.to_string();
            let name = match device.description() {
                Ok(description) => description.name().to_owned(),
                Err(_) => id.clone(),
            };
            Some(OutputDevice { id, name })
        })
        .collect()
}

/// Opens the device `id` names (the system's with `None`) exclusively for
/// a moment, playing silence, and says how it opened: whether exclusive
/// use was had, and at what rate and format. For a report.
pub fn try_exclusive(id: Option<&str>) -> Result<Format, String> {
    let mut output = Output::open(id, true)?;
    output.set_gain(0.0);
    let silence = vec![0.0; output.free().min(9600)];
    output.push(&silence);
    output.play();
    std::thread::sleep(Duration::from_millis(200));
    output.pause();
    Ok(output.format().clone())
}

/// The identity of the device the system plays through now.
fn default_device() -> Option<(cpal::Device, Option<String>)> {
    let device = cpal::default_host().default_output_device()?;
    let id = device.id().ok().map(|id| id.to_string());
    Some((device, id))
}

/// The device chosen by its id, if it is there to play through now.
fn chosen_device(id: &str) -> Option<cpal::Device> {
    let id = id.parse::<cpal::DeviceId>().ok()?;
    cpal::default_host().device_by_id(&id)
}

impl Output {
    /// Opens the device `wanted` names, in its own format; the one the
    /// system plays through when none is named, or the one named has gone.
    /// `exclusive` asks for it to be held for this app alone, where the
    /// system allows that; otherwise, or if it will not, it is shared.
    pub fn open(wanted: Option<&str>, exclusive: bool) -> Result<Self, String> {
        let chosen = wanted.and_then(|id| Some((chosen_device(id)?, Some(id.to_owned()))));
        let (device, id) = chosen
            .or_else(default_device)
            .ok_or("no sound output device")?;
        let mut refused = None;
        if exclusive {
            match Self::open_exclusive(id.as_deref()) {
                Ok(mut output) => {
                    output.device = id;
                    return Ok(output);
                }
                Err(why) => {
                    log::warn!("exclusive output refused, sharing the device: {why}");
                    refused = Some(why);
                }
            }
        }
        let supported = device
            .default_output_config()
            .map_err(|error| format!("the sound device has no usable format: {error}"))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let (producer, consumer, shared, capacity) = Self::ring(sample_rate);
        let feed = Feed::new(consumer, shared.clone());
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, feed, false),
            SampleFormat::I16 => build::<i16>(&device, config, feed, true),
            // Cut at the 32-bit step, far below what a float holds: nothing
            // to dither.
            SampleFormat::I32 => build::<i32>(&device, config, feed, false),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        let sample_format = match format {
            SampleFormat::F32 => "f32",
            SampleFormat::I16 => "i16",
            _ => "i32",
        };
        Ok(Self {
            stream: Stream::Shared(stream),
            producer,
            shared,
            sample_rate,
            capacity,
            device: id,
            format: Format {
                sample_rate,
                sample_format,
                exclusive: false,
                exclusive_refused: refused,
            },
        })
    }

    /// The ring and the flags, before there is a device to drain them.
    fn ring(sample_rate: u32) -> (Producer<f32>, Consumer<f32>, Arc<Shared>, usize) {
        let capacity = (sample_rate as f32 * RING_SECONDS) as usize * 2;
        let (producer, consumer) = RingBuffer::new(capacity);
        let shared = Arc::new(Shared {
            gain: AtomicU32::new(1.0f32.to_bits()),
            paused: AtomicBool::new(true),
            flush: AtomicBool::new(false),
            lost: AtomicBool::new(false),
            delay: AtomicU32::new(0),
        });
        (producer, consumer, shared, capacity)
    }

    #[cfg(windows)]
    fn open_exclusive(id: Option<&str>) -> Result<Self, String> {
        // The ring is sized once the rate is known; the stream is handed
        // its far end.
        let (sample_rate, open) = crate::exclusive::probe(id)?;
        let (producer, consumer, shared, capacity) = Self::ring(sample_rate);
        let feed = Feed::new(consumer, shared.clone());
        let (stream, sample_format) = open(feed)?;
        Ok(Self {
            stream: Stream::Exclusive(stream),
            producer,
            shared,
            sample_rate,
            capacity,
            device: None,
            format: Format {
                sample_rate,
                sample_format,
                exclusive: true,
                exclusive_refused: None,
            },
        })
    }

    #[cfg(not(windows))]
    fn open_exclusive(_id: Option<&str>) -> Result<Self, String> {
        Err("exclusive output is only made on Windows so far".into())
    }

    /// How the device was opened.
    pub fn format(&self) -> &Format {
        &self.format
    }

    /// Whether this was opened as `exclusive` asks: held when asked for and
    /// not refused, shared when not asked for.
    pub fn opened_as(&self, exclusive: bool) -> bool {
        self.format.exclusive == exclusive || (exclusive && self.format.exclusive_refused.is_some())
    }

    /// Whether to open the output again: this device has gone, or it is no
    /// longer the one to play through. That is the one `wanted` names while
    /// it is there, and otherwise the one the system plays through
    /// (headphones went in or out).
    pub fn stale(&self, wanted: Option<&str>) -> bool {
        if self.shared.lost.load(Ordering::Relaxed) {
            return true;
        }
        let Some(opened) = self.device.as_deref() else {
            return false;
        };
        if wanted == Some(opened) {
            return false;
        }
        // Newly chosen, or chosen before and now plugged back in.
        if wanted.is_some_and(|id| chosen_device(id).is_some()) {
            return true;
        }
        match default_device() {
            Some((_, Some(now))) => opened != now,
            // No device at all: nothing better to move to.
            _ => false,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Samples the ring can take now.
    pub fn free(&self) -> usize {
        self.producer.slots()
    }

    /// Frames queued and not yet played.
    pub fn queued_frames(&self) -> usize {
        (self.capacity - self.producer.slots()) / 2
    }

    /// How long after leaving the queue a frame is heard, by the device's
    /// own account: what draws the music waits this long too.
    pub fn delay(&self) -> Duration {
        Duration::from_micros(u64::from(self.shared.delay.load(Ordering::Relaxed)))
    }

    /// Queues interleaved stereo. The caller checks `free` first.
    pub fn push(&mut self, samples: &[f32]) {
        if let Ok(mut chunk) = self.producer.write_chunk(samples.len()) {
            let (first, second) = chunk.as_mut_slices();
            first.copy_from_slice(&samples[..first.len()]);
            second.copy_from_slice(&samples[first.len()..]);
            chunk.commit_all();
        }
    }

    pub fn set_gain(&self, gain: f32) {
        self.shared.gain.store(gain.to_bits(), Ordering::Relaxed);
    }

    /// Drops what is queued. Heard as a short fade, not a click, because
    /// the callback's gain slews.
    pub fn flush(&self) {
        self.shared.flush.store(true, Ordering::Relaxed);
    }

    pub fn play(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
        match &self.stream {
            Stream::Shared(stream) => {
                if let Err(error) = stream.play() {
                    log::warn!("the sound device would not start: {error}");
                }
            }
            #[cfg(windows)]
            Stream::Exclusive(stream) => stream.run(true),
        }
    }

    /// Fades out. The device keeps running until `suspend`, so the fade is
    /// heard.
    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
    }

    /// Stops the device itself, so a paused app does no audio work at all.
    pub fn suspend(&self) {
        match &self.stream {
            Stream::Shared(stream) => {
                if let Err(error) = stream.pause() {
                    log::debug!("the sound device would not pause: {error}");
                }
            }
            #[cfg(windows)]
            Stream::Exclusive(stream) => stream.run(false),
        }
    }
}

/// What the device takes from the ring, frame by frame: the queue, the
/// volume with its limiter, and the flags the engine sets. The same for a
/// stream cpal runs and for one held exclusively.
pub(crate) struct Feed {
    consumer: Consumer<f32>,
    shared: Arc<Shared>,
    volume: Volume,
    /// What is about to be played, and the lookahead past it, copied out
    /// of the ring without taking it.
    window: [f32; (PIECE + LOOKAHEAD) * 2],
}

impl Feed {
    fn new(consumer: Consumer<f32>, shared: Arc<Shared>) -> Self {
        Self {
            consumer,
            shared,
            volume: Volume::new(),
            window: [0.0; (PIECE + LOOKAHEAD) * 2],
        }
    }

    /// The device will play what it is handed now `delay` from now.
    pub(crate) fn delayed(&self, delay: Duration) {
        let micros = u32::try_from(delay.as_micros()).unwrap_or(u32::MAX);
        self.shared.delay.store(micros, Ordering::Relaxed);
    }

    /// The device has gone: unplugged, or taken by the system. Said by the
    /// exclusive stream, which only Windows has; cpal says it itself.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn lost(&self) {
        self.shared.lost.store(true, Ordering::Relaxed);
    }

    /// Fills `frames` with what plays next, at its volume: silence while
    /// paused, or when the queue runs dry. Never more than [`PIECE`] frames
    /// at a time; a caller with more asks again.
    pub(crate) fn next(&mut self, frames: &mut [[f32; 2]]) {
        if self.shared.flush.swap(false, Ordering::Relaxed) {
            let queued = self.consumer.slots();
            if let Ok(chunk) = self.consumer.read_chunk(queued) {
                chunk.commit_all();
            }
        }
        let paused = self.shared.paused.load(Ordering::Relaxed);
        let target = if paused {
            0.0
        } else {
            f32::from_bits(self.shared.gain.load(Ordering::Relaxed))
        };
        let count = frames.len().min(PIECE);
        let frames = &mut frames[..count];
        // Peek: what this piece plays and what the limiter must see.
        let peeked = self.consumer.slots().min((frames.len() + LOOKAHEAD) * 2) & !1;
        if let Ok(chunk) = self.consumer.read_chunk(peeked) {
            let (first, second) = chunk.as_slices();
            self.window[..first.len()].copy_from_slice(first);
            self.window[first.len()..peeked].copy_from_slice(second);
        }
        let mut taken = 0;
        for frame in frames {
            let gain = self.volume.step(target);
            // While paused and silent, the queue is left for the resume.
            *frame = if paused && gain < 1e-4 || taken * 2 >= peeked {
                [0.0, 0.0]
            } else {
                let at = taken * 2;
                taken += 1;
                let window = &self.window;
                self.volume
                    .apply([window[at], window[at + 1]], &window[at + 2..peeked])
            };
        }
        // Only what was played leaves the ring.
        if let Ok(chunk) = self.consumer.read_chunk(taken * 2) {
            chunk.commit_all();
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut feed: Feed,
    sixteen_bit: bool,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut dither = Dither::new();
    let mut frames = [[0.0f32; 2]; PIECE];
    let errors = feed.shared.clone();
    let callback = move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
        let stamp = info.timestamp();
        feed.delayed(stamp.playback.duration_since(stamp.callback));
        for piece in data.chunks_mut(channels * PIECE) {
            let played = &mut frames[..piece.len() / channels];
            feed.next(played);
            for (frame, &[left, right]) in piece.chunks_mut(channels).zip(played.iter()) {
                let mut sample = |value: f32| {
                    T::from_sample(if sixteen_bit {
                        dither.quantize_16_bit(value)
                    } else {
                        value
                    })
                };
                match frame {
                    [only] => *only = sample((left + right) * 0.5),
                    [first, second, rest @ ..] => {
                        *first = sample(left);
                        *second = sample(right);
                        rest.fill(T::from_sample(0.0));
                    }
                    [] => {}
                }
            }
        }
    };
    let on_error = move |error| {
        log::warn!("sound device: {error}");
        errors.lost.store(true, Ordering::Relaxed);
    };
    device
        .build_output_stream(config, callback, on_error, None)
        .map_err(|error| format!("the sound device could not be opened: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_peak_is_met_at_full_scale_and_what_is_far_from_it_left_alone() {
        let mut limiter = Limiter::NEW;
        // Nothing over in sight: untouched, bit for bit.
        assert_eq!(limiter.apply([0.5, -0.25], &[0.4, 0.4], 1.0), [0.5, -0.25]);
        // A peak at 1.6 four frames ahead: the level comes down over them.
        let ahead = [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 1.6, -0.8];
        let [before, _] = limiter.apply([0.5, 0.5], &ahead, 1.0);
        assert!(before < 0.5 && before > 1.0 / 1.6 * 0.5, "{before}");
        for at in (0..ahead.len()).step_by(2).take(3) {
            limiter.apply([ahead[at], ahead[at + 1]], &ahead[at + 2..], 1.0);
        }
        let [left, right] = limiter.apply([1.6, -0.8], &[], 1.0);
        assert!((left - 1.0).abs() < 1e-6, "{left}");
        // Both channels come down together, so the picture does not shift.
        assert!((right + 0.5).abs() < 1e-6, "{right}");
    }

    #[test]
    fn the_level_is_held_then_comes_back_once_the_peaks_have_passed() {
        let mut limiter = Limiter::NEW;
        limiter.apply([2.0, 2.0], &[], 1.0);
        let [held, _] = limiter.apply([0.5, 0.5], &[], 1.0);
        assert!((held - 0.25).abs() < 1e-6, "{held}");
        // A second later it has let go.
        for _ in 0..48_000 {
            limiter.apply([0.5, 0.5], &[], 1.0);
        }
        assert_eq!(limiter.gain, 1.0);
    }
}
