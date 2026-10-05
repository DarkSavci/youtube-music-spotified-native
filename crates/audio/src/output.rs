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

/// How fast the limiter lets go, per frame: about a tenth of a second at
/// 48 kHz, slow enough that it is not heard pumping.
const LIMITER_RELEASE: f32 = 0.0002;

/// Holds boosted audio under full scale.
///
/// Volume past 100% multiplies the music beyond what it was mastered to,
/// and a peak that passes full scale is clipped by the device, which is
/// heard as distortion. This turns the level down the moment a peak would
/// pass and lets it back up slowly, so loud passages are flattened rather
/// than broken. It does nothing to audio that stays under full scale.
struct Limiter {
    gain: f32,
}

impl Limiter {
    fn apply(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.gain += (1.0 - self.gain) * LIMITER_RELEASE;
        // The last of the way is too small a step to be taken; it is given.
        if self.gain > 0.999 {
            self.gain = 1.0;
        }
        let peak = left.abs().max(right.abs());
        if peak * self.gain > 1.0 {
            self.gain = 1.0 / peak;
        }
        (left * self.gain, right * self.gain)
    }

    /// Whether it is holding the level down at the moment.
    fn at_work(&self) -> bool {
        self.gain < 1.0
    }
}

/// What the engine thread and the device callback share.
struct Shared {
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
    stream: cpal::Stream,
    producer: Producer<f32>,
    shared: Arc<Shared>,
    sample_rate: u32,
    capacity: usize,
    /// Which device this is, to tell when the one to play through has moved
    /// on. `None` where the device has no identity to compare.
    device: Option<String>,
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
    pub fn open(wanted: Option<&str>) -> Result<Self, String> {
        let chosen = wanted.and_then(|id| Some((chosen_device(id)?, Some(id.to_owned()))));
        let (device, id) = chosen
            .or_else(default_device)
            .ok_or("no sound output device")?;
        let supported = device
            .default_output_config()
            .map_err(|error| format!("the sound device has no usable format: {error}"))?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let sample_rate = config.sample_rate;
        let capacity = (sample_rate as f32 * RING_SECONDS) as usize * 2;
        let (producer, consumer) = RingBuffer::new(capacity);
        let shared = Arc::new(Shared {
            gain: AtomicU32::new(1.0f32.to_bits()),
            paused: AtomicBool::new(true),
            flush: AtomicBool::new(false),
            lost: AtomicBool::new(false),
            delay: AtomicU32::new(0),
        });
        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, config, consumer, shared.clone()),
            SampleFormat::I16 => build::<i16>(&device, config, consumer, shared.clone()),
            SampleFormat::I32 => build::<i32>(&device, config, consumer, shared.clone()),
            other => Err(format!("unsupported sample format {other}")),
        }?;
        Ok(Self {
            stream,
            producer,
            shared,
            sample_rate,
            capacity,
            device: id,
        })
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
        if let Err(error) = self.stream.play() {
            log::warn!("the sound device would not start: {error}");
        }
    }

    /// Fades out. The device keeps running until `suspend`, so the fade is
    /// heard.
    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
    }

    /// Stops the device itself, so a paused app does no audio work at all.
    pub fn suspend(&self) {
        if let Err(error) = self.stream.pause() {
            log::debug!("the sound device would not pause: {error}");
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut consumer: Consumer<f32>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut gain = 0.0f32;
    let mut limiter = Limiter { gain: 1.0 };
    let errors = shared.clone();
    let callback = move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
        let stamp = info.timestamp();
        let delay = stamp.playback.duration_since(stamp.callback);
        let micros = u32::try_from(delay.as_micros()).unwrap_or(u32::MAX);
        shared.delay.store(micros, Ordering::Relaxed);
        if shared.flush.swap(false, Ordering::Relaxed) {
            let queued = consumer.slots();
            if let Ok(chunk) = consumer.read_chunk(queued) {
                chunk.commit_all();
            }
        }
        let paused = shared.paused.load(Ordering::Relaxed);
        let target = if paused {
            0.0
        } else {
            f32::from_bits(shared.gain.load(Ordering::Relaxed))
        };
        for frame in data.chunks_mut(channels) {
            gain += (target - gain) * GAIN_SLEW;
            // While paused and silent, the queue is left for the resume.
            let (left, right) = if paused && gain < 1e-4 {
                (0.0, 0.0)
            } else {
                match (consumer.pop(), consumer.pop()) {
                    // Only boost can pass full scale, so at an ordinary
                    // volume the limiter is not in the path at all.
                    (Ok(left), Ok(right)) if gain > 1.0 || limiter.at_work() => {
                        limiter.apply(left * gain, right * gain)
                    }
                    (Ok(left), Ok(right)) => (left * gain, right * gain),
                    _ => (0.0, 0.0),
                }
            };
            match frame {
                [only] => *only = T::from_sample((left + right) * 0.5),
                [first, second, rest @ ..] => {
                    *first = T::from_sample(left);
                    *second = T::from_sample(right);
                    rest.fill(T::from_sample(0.0));
                }
                [] => {}
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
    fn a_boosted_peak_is_held_at_full_scale_and_the_rest_left_alone() {
        let mut limiter = Limiter { gain: 1.0 };
        assert_eq!(limiter.apply(0.5, -0.25), (0.5, -0.25));
        assert!(!limiter.at_work());
        let (left, right) = limiter.apply(1.6, -0.8);
        assert!((left - 1.0).abs() < 1e-6);
        // Both channels come down together, so the picture does not shift.
        assert!((right + 0.5).abs() < 1e-6);
        assert!(limiter.at_work());
    }

    #[test]
    fn the_level_comes_back_once_the_peaks_have_passed() {
        let mut limiter = Limiter { gain: 1.0 };
        limiter.apply(2.0, 2.0);
        let (soon, _) = limiter.apply(0.5, 0.5);
        assert!(soon < 0.3);
        // A second later it has let go.
        for _ in 0..48_000 {
            limiter.apply(0.5, 0.5);
        }
        assert!(!limiter.at_work());
    }
}
