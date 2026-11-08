//! Changes a stereo stream's sample rate.
//!
//! AAC tracks are 44.1 kHz and most output devices run at 48 kHz; Opus is
//! 48 kHz, and some devices run at 44.1. Each output sample is a
//! windowed-sinc weighted sum of the input around it.
//!
//! The filter is made for the two rates: flat to 20 kHz, past which
//! neither codec carries anything, and silent by the lower Nyquist limit,
//! with everything that would fold back held 120 dB down. A Kaiser window
//! gives that in the fewest taps, about 170 for 44.1 to 48 kHz. Its weights
//! are worked out once for a fine set of fractional positions, and each
//! output sample blends the two rows either side of where it falls. Taking
//! the nearest row instead puts the timing out by up to half a row, which
//! was measured as distortion rising 6 dB an octave, to -57 dB at 19 kHz.
//!
//! The tables are shared: every deck resampling between the same two
//! rates uses one.

use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

const CHANNELS: usize = 2;
/// Fractional positions the weights are tabulated for. With the blend
/// between neighbours, what is left of the timing error is under -120 dB.
const PHASES: usize = 1024;
/// Kept flat up to here, in hertz, or to 90% of the lower Nyquist limit
/// when that is lower.
const PASS_HZ: f64 = 20_000.0;
/// How far down everything past the lower Nyquist limit is held, in dB.
const REJECTION_DB: f64 = 120.0;
/// One input frame, in the 32.32 fixed point positions are kept in.
const ONE: u64 = 1 << 32;

/// The weights for one pair of rates.
struct Table {
    /// Input frames that contribute to one output frame. Even.
    taps: usize,
    /// `PHASES + 1` rows of `taps` weights: the last is the first moved on
    /// a frame, so every position has a row either side.
    weights: Vec<f32>,
}

pub struct Resampler {
    /// Input frames advanced per output frame, in 32.32 fixed point.
    /// Whole numbers, so the same input gives the same output however it
    /// is cut into pieces.
    step: u64,
    table: Arc<Table>,
    /// Input not yet fully used, interleaved, led by the frames before it
    /// that the filter still reaches back to.
    buffer: Vec<f32>,
    /// Where the next output frame falls, in frames from the buffer's
    /// start, in the same fixed point.
    position: u64,
    /// The end of the input has been flushed through.
    finished: bool,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        let ratio = f64::from(to_rate) / f64::from(from_rate);
        let mut resampler = Self {
            step: (ONE as f64 / ratio).round() as u64,
            table: table(from_rate, to_rate),
            buffer: Vec::new(),
            position: 0,
            finished: false,
        };
        resampler.reset();
        resampler
    }

    /// Forgets what came before, for a seek: the audio on either side of it
    /// is unrelated.
    pub fn reset(&mut self) {
        // Silence stands in for the frames before the first, so the first
        // output frame lines up with the first input frame.
        let half = self.table.taps / 2;
        self.buffer.clear();
        self.buffer.resize((half - 1) * CHANNELS, 0.0);
        self.position = (half as u64 - 1) * ONE;
        self.finished = false;
    }

    /// The input has ended: gives the output still held back for the input
    /// that would have followed, with silence standing in for it. Without
    /// this a track loses its last few milliseconds. Once only, until the
    /// next `reset`.
    pub fn finish(&mut self) -> Vec<f32> {
        let mut output = Vec::new();
        if !self.finished {
            self.finished = true;
            let silence = vec![0.0; self.table.taps / 2 * CHANNELS];
            self.process(&silence, &mut output);
        }
        output
    }

    /// Takes interleaved stereo `input` and appends what can be produced
    /// from it to `output`. The tail that still needs later input is kept
    /// for the next call.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        self.buffer.extend_from_slice(input);
        let frames = self.buffer.len() / CHANNELS;
        let taps = self.table.taps;
        let half = taps / 2;
        loop {
            let centre = (self.position / ONE) as usize;
            if centre + half >= frames {
                break;
            }
            // The row the position falls after, and how far towards the
            // next.
            let scaled = (self.position % ONE) * PHASES as u64;
            let row = (scaled / ONE) as usize;
            let blend = (scaled % ONE) as f32 / ONE as f32;
            let before = &self.table.weights[row * taps..(row + 1) * taps];
            let after = &self.table.weights[(row + 1) * taps..(row + 2) * taps];
            let first = (centre + 1 - half) * CHANNELS;
            let window = &self.buffer[first..first + taps * CHANNELS];
            let (mut left, mut right) = (0.0f32, 0.0f32);
            let frames_in = window.as_chunks::<CHANNELS>().0;
            for ((frame, a), b) in frames_in.iter().zip(before).zip(after) {
                let weight = a + (b - a) * blend;
                left += frame[0] * weight;
                right += frame[1] * weight;
            }
            output.push(left);
            output.push(right);
            self.position += self.step;
        }
        // Drop the frames no later output can reach.
        let keep_from = ((self.position / ONE) as usize + 1).saturating_sub(half);
        self.buffer.drain(..keep_from * CHANNELS);
        self.position -= keep_from as u64 * ONE;
    }
}

/// The table for these two rates: made the first time, shared after.
fn table(from_rate: u32, to_rate: u32) -> Arc<Table> {
    type Made = Vec<((u32, u32), Arc<Table>)>;
    static MADE: Mutex<Made> = Mutex::new(Vec::new());
    let mut made = MADE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, table)) = made
        .iter()
        .find(|(rates, _)| *rates == (from_rate, to_rate))
    {
        return table.clone();
    }
    let table = Arc::new(design(from_rate, to_rate));
    made.push(((from_rate, to_rate), table.clone()));
    table
}

/// A Kaiser-windowed sinc low-pass, in input frames, that passes up to
/// `PASS_HZ` and stops by the lower Nyquist limit, tabulated for each of
/// `PHASES + 1` fractional positions and scaled so each row sums to one:
/// loudness unchanged.
fn design(from_rate: u32, to_rate: u32) -> Table {
    let from = f64::from(from_rate);
    let nyquist = f64::from(from_rate.min(to_rate)) / 2.0;
    let pass = PASS_HZ.min(0.9 * nyquist);
    // The cutoff halfway across the transition, as a fraction of the
    // input's Nyquist limit.
    let cutoff = (pass + nyquist) / from;
    // Kaiser's estimates of the length and the shape that give the
    // rejection over a transition this wide.
    let width = 2.0 * PI * (nyquist - pass) / from;
    let length = (REJECTION_DB - 7.95) / (2.285 * width) + 1.0;
    let taps = (length / 2.0).ceil() as usize * 2;
    let beta = 0.1102 * (REJECTION_DB - 8.7);
    let half = (taps / 2) as f64;
    let mut weights = Vec::with_capacity((PHASES + 1) * taps);
    for phase in 0..=PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let row: Vec<f64> = (0..taps)
            .map(|tap| {
                // Distance, in input frames, from this tap to the output.
                let distance = tap as f64 - (half - 1.0) - fraction;
                sinc(distance * cutoff) * kaiser(distance / half, beta)
            })
            .collect();
        let sum: f64 = row.iter().sum();
        weights.extend(row.iter().map(|weight| (weight / sum) as f32));
    }
    Table { taps, weights }
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// The Kaiser window over `-1..=1`, zero outside it.
fn kaiser(x: f64, beta: f64) -> f64 {
    if x.abs() > 1.0 {
        return 0.0;
    }
    bessel_i0(beta * (1.0 - x * x).sqrt()) / bessel_i0(beta)
}

/// The modified Bessel function of the first kind, order zero, by its
/// series: the terms fall fast enough for any shape used here.
fn bessel_i0(x: f64) -> f64 {
    let quarter = x * x / 4.0;
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..100u32 {
        term *= quarter / f64::from(k * k);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_1_SQRT_2;

    use super::*;

    fn sine(rate: u32, hz: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|frame| {
                let value = (2.0 * PI * hz * frame as f64 / f64::from(rate)).sin() as f32;
                [value, value]
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn a_second_of_audio_stays_a_second() {
        let mut resampler = Resampler::new(44_100, 48_000);
        let mut output = Vec::new();
        // Fed in uneven pieces, as a decoder delivers them.
        for piece in sine(44_100, 1000.0, 44_100).chunks(2 * 1023) {
            resampler.process(piece, &mut output);
        }
        // Short by the frames the filter still looks ahead to until the
        // end is flushed through; then whole, to the frame.
        let held = output.len() / CHANNELS;
        assert!((47_850..48_000).contains(&held), "{held} frames");
        output.extend(resampler.finish());
        let frames = output.len() / CHANNELS;
        assert!((47_999..=48_001).contains(&frames), "{frames} frames");
        assert!(resampler.finish().is_empty());
        resampler.reset();
        resampler.process(&sine(44_100, 1000.0, 4410), &mut output);
        assert!(!resampler.finish().is_empty());
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_level() {
        let mut resampler = Resampler::new(44_100, 48_000);
        let mut output = Vec::new();
        resampler.process(&sine(44_100, 1000.0, 44_100), &mut output);
        let left: Vec<f32> = output.iter().step_by(2).copied().collect();
        let settled = &left[1000..];
        assert!(
            (rms(settled) - FRAC_1_SQRT_2).abs() < 0.01,
            "rms {}",
            rms(settled)
        );
        let crossings = settled
            .windows(2)
            .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
            .count() as f64;
        let hz = crossings / (settled.len() as f64 / 48_000.0);
        assert!((hz - 1000.0).abs() < 2.0, "{hz} Hz");
    }

    #[test]
    fn a_high_tone_survives_downsampling_to_a_rate_that_holds_it() {
        let mut resampler = Resampler::new(48_000, 44_100);
        let mut output = Vec::new();
        resampler.process(&sine(48_000, 15_000.0, 48_000), &mut output);
        let level = rms(&output[2000..]);
        assert!((level - FRAC_1_SQRT_2).abs() < 0.03, "rms {level}");
    }

    #[test]
    fn pieces_join_without_a_seam() {
        let input = sine(44_100, 440.0, 8000);
        let mut whole = Vec::new();
        Resampler::new(44_100, 48_000).process(&input, &mut whole);
        let mut pieces = Vec::new();
        let mut resampler = Resampler::new(44_100, 48_000);
        for piece in input.chunks(2 * 333) {
            resampler.process(piece, &mut pieces);
        }
        assert_eq!(whole, pieces);
    }

    #[test]
    fn the_filter_is_sized_to_the_rates() {
        // Narrower transitions need longer filters; both well past 32.
        let up = design(44_100, 48_000).taps;
        let down = design(48_000, 44_100).taps;
        assert!((150..200).contains(&up), "{up} taps");
        assert!(down > up, "{down} taps");
    }
}
