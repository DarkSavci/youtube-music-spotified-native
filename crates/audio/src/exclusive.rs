//! The sound device held for this app alone (WASAPI exclusive mode).
//!
//! Shared, as cpal opens it, the device plays what the system's mixer makes
//! of every app's sound: at the rate the system chose for the device, after
//! whatever effects its driver adds (bass boost, "loudness equalisation",
//! spatial sound) and the system volume. Held exclusively, what the engine
//! hands over is what the device plays, sample for sample, and at a rate
//! asked for: 48 kHz where the device has it, the rate Opus decodes at, so
//! nothing is resampled at all.
//!
//! The costs are the listener's to weigh, which is why it is a setting and
//! off by default: every other app goes silent on that device while this
//! one holds it, and the system's volume no longer reaches it (the app's
//! own still does).
//!
//! A thread of its own owns the device, as COM needs it to, and is fed by
//! the same [`Feed`] a shared stream is: the ring, the volume and its
//! limiter, and dither for a device that takes 16-bit.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use wasapi::{
    AudioClient, Device, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat,
    calculate_period_100ns, initialize_mta,
};

use crate::output::{Dither, Feed};

/// The rates asked for, best first: Opus's own; then 44.1 kHz, AAC's.
/// After them, whatever the system runs the device at.
const RATES: [u32; 2] = [48_000, 44_100];

/// How long the thread waits for the device before looking at its flags.
const WAIT_MS: u32 = 100;

/// The frames converted in one go: the most [`Feed`] gives at once.
const PIECE: usize = 1024;

/// How the device takes its samples, best first. A float the device takes
/// as it is; 32 and 24 bits are cut far below anything heard; 16 bits is
/// dithered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sample {
    F32,
    I32,
    /// 24 bits in a 32-bit word, at its top.
    I24In32,
    /// 24 bits packed in three bytes.
    I24,
    I16,
}

impl Sample {
    const EVERY: [Sample; 5] = [
        Sample::F32,
        Sample::I32,
        Sample::I24In32,
        Sample::I24,
        Sample::I16,
    ];

    fn wave_format(self, rate: u32) -> WaveFormat {
        let (store, valid, kind) = match self {
            Sample::F32 => (32, 32, SampleType::Float),
            Sample::I32 => (32, 32, SampleType::Int),
            Sample::I24In32 => (32, 24, SampleType::Int),
            Sample::I24 => (24, 24, SampleType::Int),
            Sample::I16 => (16, 16, SampleType::Int),
        };
        WaveFormat::new(store, valid, kind, rate as usize, 2, None)
    }

    fn name(self) -> &'static str {
        match self {
            Sample::F32 => "f32",
            Sample::I32 => "i32",
            Sample::I24In32 | Sample::I24 => "i24",
            Sample::I16 => "i16",
        }
    }

    /// Writes `value`, from -1 to 1, as the device takes it.
    fn write(self, value: f32, dither: &mut Dither, out: &mut Vec<u8>) {
        let value = f64::from(value.clamp(-1.0, 1.0));
        match self {
            Sample::F32 => out.extend_from_slice(&(value as f32).to_le_bytes()),
            Sample::I32 => {
                let step = (value * f64::from(i32::MAX)).round() as i32;
                out.extend_from_slice(&step.to_le_bytes());
            }
            Sample::I24In32 => {
                let step = (value * 8_388_607.0).round() as i32;
                out.extend_from_slice(&(step << 8).to_le_bytes());
            }
            Sample::I24 => {
                let step = (value * 8_388_607.0).round() as i32;
                out.extend_from_slice(&step.to_le_bytes()[..3]);
            }
            Sample::I16 => {
                let stepped = dither.quantize_16_bit(value as f32);
                let step = (f64::from(stepped) * 32_768.0).clamp(-32_768.0, 32_767.0) as i16;
                out.extend_from_slice(&step.to_le_bytes());
            }
        }
    }
}

/// The device held, playing on its thread.
pub(crate) struct Stream {
    stop: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Stream {
    /// Starts the device or stops it. Stopped, it is still held.
    pub(crate) fn run(&self, on: bool) {
        self.running.store(on, Ordering::Relaxed);
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What opening the device left to do: hand it the feed.
pub(crate) type Start = Box<dyn FnOnce(Feed) -> Result<(Stream, &'static str), String>>;

/// Takes the device `id` names (cpal's id, `wasapi:` and the endpoint), or
/// the system's, for this app alone, and says at what rate. Nothing plays
/// until the feed is handed over.
pub(crate) fn probe(id: Option<&str>) -> Result<(u32, Start), String> {
    let id = id.map(|id| id.strip_prefix("wasapi:").unwrap_or(id).to_owned());
    let (ready, opened) = mpsc::channel::<Result<(u32, Sample), String>>();
    let (give, fed) = mpsc::channel::<Feed>();
    let stop = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(false));
    let (stopping, run) = (stop.clone(), running.clone());
    let thread = std::thread::Builder::new()
        .name("exclusive output".into())
        .spawn(move || {
            let held = match hold(id.as_deref()) {
                Ok(held) => held,
                Err(why) => {
                    let _ = ready.send(Err(why));
                    return;
                }
            };
            let _ = ready.send(Ok((held.rate, held.sample)));
            if let Ok(feed) = fed.recv() {
                play(held, feed, &stopping, &run);
            }
        })
        .map_err(|error| format!("no thread for the device: {error}"))?;
    let (rate, sample) = match opened.recv() {
        Ok(Ok(opened)) => opened,
        Ok(Err(why)) => {
            let _ = thread.join();
            return Err(why);
        }
        Err(_) => return Err("the device's thread ended".into()),
    };
    let start: Start = Box::new(move |feed| {
        give.send(feed)
            .map_err(|_| "the device's thread ended".to_owned())?;
        let stream = Stream {
            stop,
            running,
            thread: Some(thread),
        };
        Ok((stream, sample.name()))
    });
    Ok((rate, start))
}

/// The device, held, and how.
struct Held {
    client: AudioClient,
    rate: u32,
    sample: Sample,
}

/// Opens the device exclusively at the best rate and format it takes.
fn hold(id: Option<&str>) -> Result<Held, String> {
    let _ = initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|e| format!("no devices: {e}"))?;
    let device = match id {
        Some(id) => enumerator.get_device(id),
        None => enumerator.get_default_device(Direction::Render),
    }
    .map_err(|e| format!("the device is not there: {e}"))?;
    let client = device
        .get_iaudioclient()
        .map_err(|e| format!("the device cannot be opened: {e}"))?;
    let shared_rate = client.get_mixformat().ok().map(|f| f.get_samplespersec());
    let mut rates = RATES.to_vec();
    if let Some(rate) = shared_rate.filter(|rate| !rates.contains(rate)) {
        rates.push(rate);
    }
    let chosen = rates.iter().find_map(|&rate| {
        Sample::EVERY.iter().find_map(|&sample| {
            let format = client
                .is_supported_exclusive_with_quirks(&sample.wave_format(rate))
                .ok()?;
            Some((rate, sample, format))
        })
    });
    let Some((rate, sample, format)) = chosen else {
        return Err("the device takes no stereo format exclusively".into());
    };
    let client = initialize(&device, client, &format)?;
    Ok(Held {
        client,
        rate,
        sample,
    })
}

/// Initializes the client for events at the device's own period, aligned
/// as the device needs: the second try is the one Microsoft documents for
/// a device that wants its buffer a particular size.
fn initialize(
    device: &Device,
    mut client: AudioClient,
    format: &WaveFormat,
) -> Result<AudioClient, String> {
    let (period, _) = client
        .get_device_period()
        .map_err(|e| format!("the device has no period: {e}"))?;
    let period = client
        .calculate_aligned_period_near(period, Some(128), format)
        .unwrap_or(period);
    let mode = StreamMode::EventsExclusive { period_hns: period };
    match client.initialize_client(format, Direction::Render, mode, None) {
        Ok(()) => return Ok(client),
        Err(error) => log::debug!("exclusive at {period}: {error}; trying the aligned size"),
    }
    let frames = client
        .get_buffer_size()
        .map_err(|e| format!("the device refused exclusive use: {e}"))?;
    let period = calculate_period_100ns(i64::from(frames), i64::from(format.get_samplespersec()));
    client = device
        .get_iaudioclient()
        .map_err(|e| format!("the device cannot be opened: {e}"))?;
    let mode = StreamMode::EventsExclusive { period_hns: period };
    client
        .initialize_client(format, Direction::Render, mode, None)
        .map_err(|e| format!("the device refused exclusive use: {e}"))?;
    Ok(client)
}

/// Plays until told to stop or the device goes.
fn play(held: Held, mut feed: Feed, stop: &AtomicBool, running: &AtomicBool) {
    let Held {
        client,
        rate,
        sample,
    } = held;
    let started = (|| {
        let event = client.set_get_eventhandle()?;
        let render = client.get_audiorenderclient()?;
        Ok::<_, wasapi::WasapiError>((event, render))
    })();
    let (event, render) = match started {
        Ok(started) => started,
        Err(error) => {
            log::warn!("exclusive output: {error}");
            feed.lost();
            return;
        }
    };
    let mut dither = Dither::new();
    let mut frames = [[0.0f32; 2]; PIECE];
    let mut bytes = Vec::new();
    let mut fill = |feed: &mut Feed, count: usize| -> Result<(), wasapi::WasapiError> {
        bytes.clear();
        let mut left = count;
        while left > 0 {
            let piece = &mut frames[..left.min(PIECE)];
            feed.next(piece);
            for &[l, r] in piece.iter() {
                sample.write(l, &mut dither, &mut bytes);
                sample.write(r, &mut dither, &mut bytes);
            }
            left -= piece.len();
        }
        render.write_to_device(&bytes, None)
    };
    let mut playing = false;
    while !stop.load(Ordering::Relaxed) {
        let wanted = running.load(Ordering::Relaxed);
        if wanted != playing {
            let changed = if wanted {
                // Primed first, so the device starts on music, not on
                // whatever its buffer held.
                client
                    .get_available_space_in_frames()
                    .and_then(|count| fill(&mut feed, count as usize))
                    .and_then(|()| client.start_stream())
            } else {
                client.stop_stream()
            };
            if let Err(error) = changed {
                log::warn!("exclusive output: {error}");
                feed.lost();
                return;
            }
            playing = wanted;
        }
        if !playing {
            std::thread::sleep(Duration::from_millis(u64::from(WAIT_MS) / 5));
            continue;
        }
        if event.wait_for_event(WAIT_MS).is_err() {
            // No event in a tenth of a second while running: the device
            // has stopped answering.
            log::warn!("exclusive output: the device stopped asking for audio");
            feed.lost();
            return;
        }
        let written = client.get_available_space_in_frames().and_then(|count| {
            // The buffer just handed over plays after the one playing.
            feed.delayed(Duration::from_secs_f64(f64::from(count) / f64::from(rate)));
            fill(&mut feed, count as usize)
        });
        if let Err(error) = written {
            log::warn!("exclusive output: {error}");
            feed.lost();
            return;
        }
    }
    if playing {
        let _ = client.stop_stream();
    }
}
