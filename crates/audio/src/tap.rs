//! A window onto what is being played, for drawing it.
//!
//! The engine copies the audio it hands to the device into a ring here; the
//! app reads the stretch being heard and turns it into a spectrum. Nothing
//! is copied unless someone has asked to look.
//!
//! What is handed to the device is not heard yet: a quarter of a second is
//! queued ahead of it, and the device has a delay of its own. So each copy
//! comes with when it will be heard, and the reader looks back from the
//! newest sample to the one at the ear now. Drawing the newest instead made
//! the picture run a quarter of a second ahead of the music.

use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Samples a spectrum is made from. A power of two, for the transform, and
/// long enough to tell bass notes apart: about 23 Hz a step at 48 kHz.
pub const WINDOW: usize = 2048;
/// Samples kept: what is queued ahead of the device, its own delay, and a
/// window besides. Two thirds of a second at 48 kHz.
const RING: usize = 32_768;
/// How long a spectrum takes to reach the eye once read: on average half
/// the wait for the next drawing, and a frame for the screen to show it.
const TO_THE_EYE: Duration = Duration::from_millis(30);

struct Ring {
    /// The latest mono samples.
    samples: Vec<f32>,
    /// Where the next one goes.
    at: usize,
    /// When the newest sample reaches the ear.
    heard: Option<Instant>,
}

/// Somewhere the whole sound is sent as it is played: interleaved stereo,
/// how long until the last of it is heard, and the rate it is at.
pub type Sink = Box<dyn Fn(&[f32], Duration, u32) + Send + Sync>;

pub struct Tap {
    watching: AtomicBool,
    /// Where the sound itself is sent, when something wants more of it
    /// than the window kept here: a visualizer in a process of its own.
    sink: Mutex<Option<Sink>>,
    /// The rate the samples are at: the device's.
    sample_rate: AtomicU32,
    /// The bitrate of the stream being played, in kilobits a second;
    /// nought when it is not known.
    bitrate: AtomicU32,
    ring: Mutex<Ring>,
}

impl Default for Tap {
    fn default() -> Self {
        Self {
            watching: AtomicBool::new(false),
            sink: Mutex::new(None),
            sample_rate: AtomicU32::new(48_000),
            bitrate: AtomicU32::new(0),
            ring: Mutex::new(Ring {
                samples: vec![0.0; RING],
                at: 0,
                heard: None,
            }),
        }
    }
}

impl Tap {
    /// Whether anyone is looking. The engine skips the copy when not.
    pub fn set_watching(&self, watching: bool) {
        self.watching.store(watching, Ordering::Relaxed);
    }

    /// The rate of the device being played through; until there is one,
    /// the usual 48 kHz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    /// Sends the whole sound to `sink` from now on, or with `None` to
    /// nobody. It is called on the engine's thread with every block on
    /// its way out, and must not keep it waiting.
    pub fn set_sink(&self, sink: Option<Sink>) {
        if let Ok(mut held) = self.sink.lock() {
            *held = sink;
        }
    }

    /// The bitrate of the stream being played, in kilobits a second: its
    /// size over its length. Nought when that is not known.
    pub fn bitrate(&self) -> u32 {
        self.bitrate.load(Ordering::Relaxed)
    }

    pub fn set_bitrate(&self, kbps: u32) {
        self.bitrate.store(kbps, Ordering::Relaxed);
    }

    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Relaxed);
    }

    /// Called by the engine with interleaved stereo on its way out, and
    /// how long it is until the last of it is heard: what is queued before
    /// it, its own length, and the device's delay.
    pub fn write(&self, stereo: &[f32], ahead: Duration) {
        if let Ok(sink) = self.sink.lock()
            && let Some(sink) = sink.as_ref()
        {
            sink(stereo, ahead, self.sample_rate());
        }
        if !self.watching.load(Ordering::Relaxed) {
            return;
        }
        let Ok(mut ring) = self.ring.lock() else {
            return;
        };
        let ring = &mut *ring;
        for frame in stereo.as_chunks::<2>().0 {
            ring.samples[ring.at] = (frame[0] + frame[1]) * 0.5;
            ring.at = (ring.at + 1) % RING;
        }
        ring.heard = Some(Instant::now() + ahead);
    }

    /// How loud each of `bars` bands is right now, low to high, from 0 to 1. The bands are spaced as pitch is heard: evenly in octaves.
    pub fn spectrum(&self, bars: usize) -> Vec<f32> {
        let mut real = [0.0f32; WINDOW];
        {
            let Ok(ring) = self.ring.lock() else {
                return vec![0.0; bars];
            };
            let skip = unheard(ring.heard, self.sample_rate());
            let start = ring.at + RING - WINDOW - skip;
            let window = &tables().window;
            for (index, slot) in real.iter_mut().enumerate() {
                // Oldest first, shaped so the ends of the stretch do not
                // show up as a splash across every band.
                *slot = ring.samples[(start + index) % RING] * window[index];
            }
        }
        let magnitudes = magnitudes(&mut real);
        bands(&magnitudes, bars, self.sample_rate.load(Ordering::Relaxed))
    }

    /// The sound itself as it reaches the ear about now: `points` samples
    /// from -1 to 1, oldest first, each `stride` samples after the last.
    /// For an oscilloscope.
    pub fn wave(&self, points: usize, stride: usize) -> Vec<f32> {
        let stride = stride.max(1);
        let span = (points * stride).min(RING / 2);
        let Ok(ring) = self.ring.lock() else {
            return vec![0.0; points];
        };
        let skip = unheard(ring.heard, self.sample_rate()).min(RING - span);
        let start = ring.at + 2 * RING - span - skip;
        (0..points)
            .map(|point| ring.samples[(start + (point * stride) % span.max(1)) % RING])
            .collect()
    }
}

/// How many of the newest samples to leave out, so that the window's
/// middle, where it weighs the most, is what will be at the ear when the
/// drawing is seen.
fn unheard(heard: Option<Instant>, sample_rate: u32) -> usize {
    let ahead = heard
        .map(|heard| heard.saturating_duration_since(Instant::now() + TO_THE_EYE))
        .unwrap_or_default();
    let frames = (ahead.as_secs_f32() * sample_rate as f32) as usize;
    frames.saturating_sub(WINDOW / 2).min(RING - WINDOW)
}

/// What the transform needs on every run and that never changes, worked
/// out once: thousands of sines a frame are the costly part otherwise.
struct Tables {
    /// A Hann window.
    window: [f32; WINDOW],
    /// The sine and cosine of each step of half a turn backwards.
    turns: [(f32, f32); WINDOW / 2],
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let step = 2.0 * PI / WINDOW as f32;
        Tables {
            window: std::array::from_fn(|index| 0.5 - 0.5 * (step * index as f32).cos()),
            turns: std::array::from_fn(|index| (-step * index as f32).sin_cos()),
        }
    })
}

/// The strength of each frequency in `real`: an in-place radix-2 transform.
fn magnitudes(real: &mut [f32; WINDOW]) -> [f32; WINDOW / 2] {
    let mut imaginary = [0.0f32; WINDOW];
    // Reorder by bit-reversed index, then combine in doubling spans.
    let bits = WINDOW.trailing_zeros();
    for index in 0..WINDOW {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        if reversed > index {
            real.swap(index, reversed);
        }
    }
    let turns = &tables().turns;
    let mut span = 2;
    while span <= WINDOW {
        // A span's turns are every `stride`th of the whole window's.
        let stride = WINDOW / span;
        for start in (0..WINDOW).step_by(span) {
            for offset in 0..span / 2 {
                let (sin, cos) = turns[offset * stride];
                let (even, odd) = (start + offset, start + offset + span / 2);
                let twisted_real = real[odd] * cos - imaginary[odd] * sin;
                let twisted_imaginary = real[odd] * sin + imaginary[odd] * cos;
                real[odd] = real[even] - twisted_real;
                imaginary[odd] = imaginary[even] - twisted_imaginary;
                real[even] += twisted_real;
                imaginary[even] += twisted_imaginary;
            }
        }
        span *= 2;
    }
    let mut out = [0.0f32; WINDOW / 2];
    for (index, slot) in out.iter_mut().enumerate() {
        // Scaled so a full-scale tone reads about 1.
        *slot = real[index].hypot(imaginary[index]) / (WINDOW as f32 / 4.0);
    }
    out
}

/// Gathers the transform's evenly spaced frequencies into `bars` bands
/// spaced evenly in octaves, from the bass to the top of hearing.
fn bands(magnitudes: &[f32; WINDOW / 2], bars: usize, sample_rate: u32) -> Vec<f32> {
    const LOWEST_HZ: f32 = 50.0;
    const HIGHEST_HZ: f32 = 16_000.0;
    /// The quietest level that still shows.
    const FLOOR_DB: f32 = -50.0;
    let hz_per_bin = sample_rate as f32 / WINDOW as f32;
    let edge = |bar: usize| {
        let hz = LOWEST_HZ * (HIGHEST_HZ / LOWEST_HZ).powf(bar as f32 / bars as f32);
        ((hz / hz_per_bin) as usize).clamp(1, WINDOW / 2 - 1)
    };
    (0..bars)
        .map(|bar| {
            let (from, to) = (edge(bar), edge(bar + 1).max(edge(bar) + 1));
            let loudest = magnitudes[from..to.min(WINDOW / 2)]
                .iter()
                .fold(0.0f32, |loudest, level| loudest.max(*level));
            // In decibels, as loudness is heard: music spreads its energy
            // thinly, and drawn in proportion only the kick would show.
            let decibels = 20.0 * loudest.max(f32::MIN_POSITIVE).log10();
            (1.0 - decibels / FLOOR_DB).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn tone(hz: f32) -> Vec<f32> {
        (0..WINDOW * 2)
            .flat_map(|frame| {
                let value = (2.0 * PI * hz * frame as f32 / RATE as f32).sin();
                [value, value]
            })
            .collect()
    }

    #[test]
    fn nothing_is_copied_until_someone_looks() {
        let tap = Tap::default();
        tap.write(&tone(1000.0), Duration::ZERO);
        assert!(tap.spectrum(32).iter().all(|level| *level == 0.0));
    }

    #[test]
    fn a_tone_lights_the_band_it_falls_in() {
        let tap = Tap::default();
        tap.set_watching(true);
        tap.write(&tone(1000.0), Duration::ZERO);
        let spectrum = tap.spectrum(32);
        let loudest = loudest(&spectrum);
        // 1 kHz sits a little past half way from 50 Hz to 16 kHz in octaves.
        assert!((15..=18).contains(&loudest), "bar {loudest}");
        assert!(spectrum[loudest] > 0.6);
        assert!(spectrum[2] < 0.1 && spectrum[30] < 0.1);
    }

    #[test]
    fn silence_is_flat() {
        let tap = Tap::default();
        tap.set_watching(true);
        tap.write(&vec![0.0; WINDOW * 2], Duration::ZERO);
        assert!(tap.spectrum(16).iter().all(|level| *level == 0.0));
    }

    fn loudest(spectrum: &[f32]) -> usize {
        spectrum
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(bar, _)| bar)
            .expect("bars")
    }

    #[test]
    fn what_is_drawn_is_what_is_heard_not_what_is_queued() {
        let tap = Tap::default();
        tap.set_watching(true);
        let low: Vec<f32> = tone(1000.0).repeat(RING / WINDOW);
        tap.write(&low, Duration::ZERO);
        // A fifth of a second of a higher tone, the last of it a quarter
        // of a second from the ear: the low one is still what is heard.
        let high = &tone(5000.0).repeat(4)[..9600 * 2];
        tap.write(high, Duration::from_millis(250));
        let low_bar = loudest(&tap.spectrum(32));
        assert!((15..=18).contains(&low_bar), "bar {low_bar}");
        // With nothing queued ahead of it, the high one is.
        tap.write(high, Duration::ZERO);
        let high_bar = loudest(&tap.spectrum(32));
        assert!(high_bar > 22, "bar {high_bar}");
    }
}

#[cfg(test)]
mod wave_tests {
    use super::*;

    #[test]
    fn the_wave_is_the_sound_itself_and_flat_with_none() {
        let tap = Tap::default();
        assert!(tap.wave(75, 7).iter().all(|sample| *sample == 0.0));
        tap.set_watching(true);
        // A ramp in both channels, so each sample says where it was.
        let stereo: Vec<f32> = (0..4096)
            .flat_map(|index| [index as f32 / 4096.0; 2])
            .collect();
        tap.write(&stereo, Duration::ZERO);
        let wave = tap.wave(75, 7);
        assert_eq!(wave.len(), 75);
        // Oldest first, seven samples apart.
        assert!(wave.windows(2).all(|pair| pair[1] > pair[0]));
        assert!((wave[1] - wave[0] - 7.0 / 4096.0).abs() < 1e-5);
    }

    #[test]
    fn the_bitrate_is_what_the_engine_last_said() {
        let tap = Tap::default();
        assert_eq!(tap.bitrate(), 0);
        tap.set_bitrate(128);
        assert_eq!(tap.bitrate(), 128);
    }
}

#[cfg(test)]
mod sink_tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn the_whole_sound_goes_to_whoever_asked_for_it_watched_or_not() {
        let tap = Tap::default();
        let heard = Arc::new(AtomicUsize::new(0));
        let count = heard.clone();
        tap.set_sink(Some(Box::new(move |stereo, ahead, rate| {
            assert_eq!((ahead, rate), (Duration::from_millis(40), 48_000));
            count.fetch_add(stereo.len(), Ordering::Relaxed);
        })));
        tap.write(&[0.5; 64], Duration::from_millis(40));
        assert_eq!(heard.load(Ordering::Relaxed), 64);
        tap.set_sink(None);
        tap.write(&[0.5; 64], Duration::from_millis(40));
        assert_eq!(heard.load(Ordering::Relaxed), 64);
    }
}
