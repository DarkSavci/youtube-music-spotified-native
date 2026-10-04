//! Changes a stereo stream's sample rate.
//!
//! AAC tracks are 44.1 kHz and most output devices run at 48 kHz. Each output
//! sample is a windowed-sinc weighted sum of the input around it, with the
//! weights for a set of fractional positions worked out once. Thirty-two
//! taps keep the passband flat to the edge of hearing at a cost of a few
//! million multiplications a second.

use std::f64::consts::PI;

const CHANNELS: usize = 2;
/// Input frames that contribute to one output frame.
const TAPS: usize = 32;
/// Fractional positions the weights are tabulated for.
const PHASES: usize = 512;
/// How much of the band below the lower Nyquist limit is kept. The rest is
/// the filter's roll-off.
const PASSBAND: f64 = 0.94;
/// One input frame, in the 32.32 fixed point positions are kept in.
const ONE: u64 = 1 << 32;

pub struct Resampler {
    /// Input frames advanced per output frame, in 32.32 fixed point.
    /// Whole numbers, so the same input gives the same output however it
    /// is cut into pieces.
    step: u64,
    /// `PHASES` rows of `TAPS` weights.
    table: Vec<f32>,
    /// Input not yet fully used, interleaved, led by the frames before it
    /// that the filter still reaches back to.
    buffer: Vec<f32>,
    /// Where the next output frame falls, in frames from the buffer's
    /// start, in the same fixed point.
    position: u64,
}

impl Resampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        let ratio = f64::from(to_rate) / f64::from(from_rate);
        let mut resampler = Self {
            step: (ONE as f64 / ratio).round() as u64,
            table: weights(ratio.min(1.0) * PASSBAND),
            buffer: Vec::new(),
            position: 0,
        };
        resampler.reset();
        resampler
    }

    /// Forgets what came before, for a seek: the audio on either side of it
    /// is unrelated.
    pub fn reset(&mut self) {
        // Silence stands in for the frames before the first, so the first
        // output frame lines up with the first input frame.
        self.buffer.clear();
        self.buffer.resize((TAPS / 2 - 1) * CHANNELS, 0.0);
        self.position = (TAPS as u64 / 2 - 1) * ONE;
    }

    /// Takes interleaved stereo `input` and appends what can be produced
    /// from it to `output`. The tail that still needs later input is kept
    /// for the next call.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        self.buffer.extend_from_slice(input);
        let frames = self.buffer.len() / CHANNELS;
        loop {
            let centre = (self.position / ONE) as usize;
            if centre + TAPS / 2 >= frames {
                break;
            }
            let phase = ((self.position % ONE) * PHASES as u64 / ONE) as usize;
            let weights = &self.table[phase * TAPS..(phase + 1) * TAPS];
            let first = (centre + 1 - TAPS / 2) * CHANNELS;
            let window = &self.buffer[first..first + TAPS * CHANNELS];
            let (mut left, mut right) = (0.0f32, 0.0f32);
            for (frame, weight) in window.as_chunks::<CHANNELS>().0.iter().zip(weights) {
                left += frame[0] * weight;
                right += frame[1] * weight;
            }
            output.push(left);
            output.push(right);
            self.position += self.step;
        }
        // Drop the frames no later output can reach.
        let keep_from = ((self.position / ONE) as usize + 1).saturating_sub(TAPS / 2);
        self.buffer.drain(..keep_from * CHANNELS);
        self.position -= keep_from as u64 * ONE;
    }
}

/// The filter weights for each fractional position: a sinc low-pass at
/// `cutoff` (a fraction of the input's Nyquist limit) under a Blackman
/// window, scaled so each row sums to one and loudness is unchanged.
fn weights(cutoff: f64) -> Vec<f32> {
    let mut table = Vec::with_capacity(PHASES * TAPS);
    for phase in 0..PHASES {
        let fraction = phase as f64 / PHASES as f64;
        let row: Vec<f64> = (0..TAPS)
            .map(|tap| {
                // Distance, in input frames, from this tap to the output.
                let distance = tap as f64 - (TAPS / 2 - 1) as f64 - fraction;
                sinc(distance * cutoff) * blackman(distance / (TAPS / 2) as f64)
            })
            .collect();
        let sum: f64 = row.iter().sum();
        table.extend(row.iter().map(|weight| (weight / sum) as f32));
    }
    table
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// The Blackman window over `-1..=1`, zero outside it.
fn blackman(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    0.42 + 0.5 * (PI * x).cos() + 0.08 * (2.0 * PI * x).cos()
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
        let frames = output.len() / CHANNELS;
        assert!((47_950..=48_000).contains(&frames), "{frames} frames");
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
}
