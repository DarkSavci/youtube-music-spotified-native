//! A ten-band equalizer and the limiter that follows it.
//!
//! Each band is a peaking filter an octave wide, centred where graphic
//! equalizers have had their sliders since the hi-fi rack. Raising bands
//! can push peaks past full scale, so the signal then goes through a
//! limiter that turns the level down for as long as a peak lasts.

use std::f32::consts::PI;

/// The centre of each band, in hertz.
pub const BANDS: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// The most a band is raised or lowered, in decibels.
pub const RANGE_DB: f32 = 12.0;
/// How wide each band is: an octave.
const Q: f32 = 1.41;

/// The level the limiter holds peaks under, a little below full scale.
const CEILING: f32 = 0.98;
/// How long the limiter takes to let the level back up, in seconds.
const RELEASE: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Settings {
    pub enabled: bool,
    /// Decibels for each of [`BANDS`].
    pub gains: [f32; 10],
}

/// One peaking filter, for both channels.
#[derive(Clone, Copy, Default)]
struct Band {
    // The filter's coefficients, normalised.
    b: [f32; 3],
    a: [f32; 2],
    /// The last two inputs and outputs of each channel.
    history: [[f32; 4]; 2],
}

impl Band {
    /// A peaking filter at `hz`, raising or lowering by `db` (from Robert
    /// Bristow-Johnson's Audio EQ Cookbook).
    fn peaking(hz: f32, db: f32, sample_rate: f32) -> Self {
        let amplitude = 10f32.powf(db / 40.0);
        let omega = 2.0 * PI * hz / sample_rate;
        let alpha = omega.sin() / (2.0 * Q);
        let a0 = 1.0 + alpha / amplitude;
        Self {
            b: [
                (1.0 + alpha * amplitude) / a0,
                -2.0 * omega.cos() / a0,
                (1.0 - alpha * amplitude) / a0,
            ],
            a: [-2.0 * omega.cos() / a0, (1.0 - alpha / amplitude) / a0],
            history: [[0.0; 4]; 2],
        }
    }

    fn process(&mut self, channel: usize, input: f32) -> f32 {
        let [x1, x2, y1, y2] = self.history[channel];
        let output =
            self.b[0] * input + self.b[1] * x1 + self.b[2] * x2 - self.a[0] * y1 - self.a[1] * y2;
        self.history[channel] = [input, x1, output, y1];
        output
    }
}

pub struct Equalizer {
    sample_rate: f32,
    settings: Settings,
    bands: Vec<Band>,
    /// The limiter's gain: 1 when nothing is being held back.
    limit: f32,
    /// How much of the way back to 1 the limiter's gain moves per frame.
    release: f32,
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate as f32;
        Self {
            sample_rate,
            settings: Settings::default(),
            bands: Vec::new(),
            limit: 1.0,
            release: 1.0 - (-1.0 / (RELEASE * sample_rate)).exp(),
        }
    }

    pub fn set(&mut self, settings: Settings) {
        if settings == self.settings {
            return;
        }
        self.settings = settings;
        // A flat band does nothing but cost time, so it is left out. A band
        // above what the device can carry cannot be built at all.
        let nyquist = self.sample_rate / 2.0;
        self.bands = BANDS
            .iter()
            .zip(settings.gains)
            .filter(|(hz, db)| settings.enabled && db.abs() > 0.05 && **hz < nyquist)
            .map(|(hz, db)| Band::peaking(*hz, db.clamp(-RANGE_DB, RANGE_DB), self.sample_rate))
            .collect();
    }

    /// Shapes interleaved stereo in place, then holds its peaks under the
    /// ceiling. With every band flat only the limiter runs, and it leaves
    /// alone what is already under the ceiling.
    pub fn process(&mut self, samples: &mut [f32]) {
        for frame in samples.as_chunks_mut::<2>().0 {
            for band in &mut self.bands {
                frame[0] = band.process(0, frame[0]);
                frame[1] = band.process(1, frame[1]);
            }
            let peak = frame[0].abs().max(frame[1].abs());
            // Down at once for a peak, back up gently after it.
            let needed = if peak > CEILING { CEILING / peak } else { 1.0 };
            if needed < self.limit {
                self.limit = needed;
            } else {
                self.limit += (needed - self.limit) * self.release;
            }
            frame[0] *= self.limit;
            frame[1] *= self.limit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn tone(hz: f32, amplitude: f32) -> Vec<f32> {
        (0..RATE as usize)
            .flat_map(|frame| {
                let value = amplitude * (2.0 * PI * hz * frame as f32 / RATE as f32).sin();
                [value, value]
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// The level of a tone after the equalizer, relative to before, in dB.
    fn change_db(equalizer: &mut Equalizer, hz: f32) -> f32 {
        let mut samples = tone(hz, 0.1);
        let before = rms(&samples);
        equalizer.process(&mut samples);
        // Past the filter's settling.
        20.0 * (rms(&samples[RATE as usize..]) / before).log10()
    }

    fn with_band(index: usize, db: f32) -> Equalizer {
        let mut gains = [0.0; 10];
        gains[index] = db;
        let mut equalizer = Equalizer::new(RATE);
        equalizer.set(Settings {
            enabled: true,
            gains,
        });
        equalizer
    }

    #[test]
    fn a_raised_band_raises_its_own_frequency_and_leaves_distant_ones() {
        let mut equalizer = with_band(5, 6.0); // 1 kHz
        assert!((change_db(&mut equalizer, 1000.0) - 6.0).abs() < 0.3);
        assert!(change_db(&mut with_band(5, 6.0), 62.0).abs() < 0.3);
        assert!(change_db(&mut with_band(5, 6.0), 12_000.0).abs() < 0.3);
    }

    #[test]
    fn a_lowered_band_lowers() {
        assert!((change_db(&mut with_band(2, -9.0), 125.0) + 9.0).abs() < 0.4);
    }

    #[test]
    fn switched_off_it_changes_nothing() {
        let mut equalizer = Equalizer::new(RATE);
        equalizer.set(Settings {
            enabled: false,
            gains: [12.0; 10],
        });
        let original = tone(1000.0, 0.5);
        let mut samples = original.clone();
        equalizer.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn the_limiter_holds_peaks_under_the_ceiling() {
        let mut equalizer = with_band(5, 12.0);
        let mut samples = tone(1000.0, 0.9);
        equalizer.process(&mut samples);
        let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
        assert!(peak <= CEILING + 1e-4, "peak {peak}");
    }
}
