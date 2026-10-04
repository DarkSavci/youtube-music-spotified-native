//! The sound device.
//!
//! The engine writes stereo frames into a lock-free ring; the device's
//! callback drains it. The callback never locks and never allocates. Volume
//! and pause are applied there, so both take effect within one device
//! buffer rather than after everything already queued.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};

/// Audio queued ahead of the device. Long enough that a busy moment on the
/// engine thread is not heard, short enough that a seek is.
const RING_SECONDS: f32 = 0.25;
/// How fast the gain follows a change, per frame. About 10 ms to settle at
/// 48 kHz: no click on pause or a volume step, no audible lag.
const GAIN_SLEW: f32 = 0.004;

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
}

pub struct Output {
    stream: cpal::Stream,
    producer: Producer<f32>,
    shared: Arc<Shared>,
    sample_rate: u32,
    capacity: usize,
    /// Which device this is, to tell when the system's choice has moved on.
    /// `None` where the device has no identity to compare.
    device: Option<String>,
}

/// The identity of the device the system plays through now.
fn default_device() -> Option<(cpal::Device, Option<String>)> {
    let device = cpal::default_host().default_output_device()?;
    let id = device.id().ok().map(|id| id.to_string());
    Some((device, id))
}

impl Output {
    /// Opens the default output device in its own format.
    pub fn open() -> Result<Self, String> {
        let (device, id) = default_device().ok_or("no sound output device")?;
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
    /// longer the one the system plays through (headphones went in or out).
    pub fn stale(&self) -> bool {
        if self.shared.lost.load(Ordering::Relaxed) {
            return true;
        }
        match (&self.device, default_device()) {
            (Some(opened), Some((_, Some(now)))) => *opened != now,
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
    let errors = shared.clone();
    let callback = move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
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
