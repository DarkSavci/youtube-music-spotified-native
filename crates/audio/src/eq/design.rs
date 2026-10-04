//! The equalizer's filters: what each band is, and which ten of them give
//! the curve the sliders ask for.
//!
//! A band's filter is not as narrow as its slider: raised by itself it
//! lifts its neighbours' frequencies a little too, so ten of them set
//! straight from the sliders overshoot (every slider at +6 dB came out near
//! +9). The gains the filters are given are therefore solved for, so that
//! the response at each band's centre is what its slider says. The same
//! filters are what the panel draws, so the curve on screen is the one
//! that is heard.

use std::f64::consts::{PI, SQRT_2};

use super::{BANDS, RANGE_DB, Settings};

/// How many bands there are.
const COUNT: usize = BANDS.len();
/// How wide a peaking band is: wider than the octave the bands are apart.
/// Wider still and neighbours raised alike join more smoothly, but
/// neighbours set against each other need more from their filters than
/// there is to give. At this, ten sliders at +6 dB dip half a decibel
/// between bands, and sliders alternating between the two ends of the
/// range are met to about a decibel.
const Q: f64 = 1.0;
/// Where the top band's shelf is half way up: half an octave under its
/// slider's frequency, so that it has all but arrived by there.
const SHELF_BELOW: f64 = SQRT_2;
/// A band is only built this far under half the sample rate; nearer than
/// that and a filter cannot be told from one at the limit itself.
const NYQUIST_SHARE: f64 = 0.9;
/// The most a filter is given, however the sliders are set against one
/// another. Neighbours at opposite ends ask for more than either says.
const MOST_DB: f64 = 2.0 * RANGE_DB as f64;
/// A filter asked for less than this is left out: it would cost time to
/// change nothing that can be heard.
const LEAST_DB: f64 = 0.01;
/// The gain the bands' overlap is measured at.
const PROBE_DB: f64 = 6.0;
/// How many times the solved gains are corrected against the response
/// they really give. The overlap is not quite proportional to the gain,
/// so the first answer is a few tenths of a decibel out.
const ROUNDS: usize = 4;
/// The frequencies the curve's highest point is looked for at.
const PEAK_POINTS: usize = 96;

/// A second-order filter, normalised so that its `a0` is 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biquad {
    pub(super) b: [f64; 3],
    pub(super) a: [f64; 2],
}

impl Biquad {
    /// The filter that changes nothing.
    pub const FLAT: Biquad = Biquad {
        b: [1.0, 0.0, 0.0],
        a: [0.0, 0.0],
    };

    /// A bell at `hz`, raising or lowering by `db` (Robert
    /// Bristow-Johnson's Audio EQ Cookbook).
    fn peaking(hz: f64, db: f64, rate: f64) -> Self {
        let amplitude = 10f64.powf(db / 40.0);
        let omega = 2.0 * PI * hz / rate;
        let alpha = omega.sin() / (2.0 * Q);
        let a0 = 1.0 + alpha / amplitude;
        Self {
            b: [
                (1.0 + alpha * amplitude) / a0,
                -2.0 * omega.cos() / a0,
                (1.0 - alpha * amplitude) / a0,
            ],
            a: [-2.0 * omega.cos() / a0, (1.0 - alpha / amplitude) / a0],
        }
    }

    /// A shelf that raises or lowers everything above `hz` by `db`, as
    /// gently as a shelf can without a bump (the Cookbook's, slope 1).
    fn high_shelf(hz: f64, db: f64, rate: f64) -> Self {
        let amplitude = 10f64.powf(db / 40.0);
        let omega = 2.0 * PI * hz / rate;
        let cos = omega.cos();
        let alpha = omega.sin() / 2.0 * SQRT_2;
        let lean = 2.0 * amplitude.sqrt() * alpha;
        let (more, less) = (amplitude + 1.0, amplitude - 1.0);
        let a0 = more - less * cos + lean;
        Self {
            b: [
                amplitude * (more + less * cos + lean) / a0,
                -2.0 * amplitude * (less + more * cos) / a0,
                amplitude * (more + less * cos - lean) / a0,
            ],
            a: [
                2.0 * (less - more * cos) / a0,
                (more - less * cos - lean) / a0,
            ],
        }
    }

    /// What the filter does to a tone at `hz`, in decibels.
    pub fn db_at(&self, hz: f64, rate: f64) -> f64 {
        if *self == Self::FLAT {
            return 0.0;
        }
        let omega = 2.0 * PI * hz / rate;
        let (cos, cos2) = (omega.cos(), (2.0 * omega).cos());
        let [b0, b1, b2] = self.b;
        let [a1, a2] = self.a;
        let above =
            b0 * b0 + b1 * b1 + b2 * b2 + 2.0 * (b0 * b1 + b1 * b2) * cos + 2.0 * b0 * b2 * cos2;
        let below = 1.0 + a1 * a1 + a2 * a2 + 2.0 * (a1 + a1 * a2) * cos + 2.0 * a2 * cos2;
        // A notch's very bottom is nothing at all, which has no logarithm.
        10.0 * (above / below).max(1e-12).log10()
    }
}

/// The filters for one setting of the sliders, and the level that goes
/// with them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub(super) filters: [Biquad; COUNT],
    /// What the whole signal is turned up or down by, in decibels: the
    /// preamp, less whatever keeps the curve's top at full scale.
    pub gain_db: f32,
    /// The highest the curve reaches, in decibels, before that. Under
    /// nought for a curve that only lowers.
    pub peak_db: f32,
    rate: f64,
}

impl Shape {
    /// What the ten bands together do to a tone at `hz`, in decibels: the
    /// curve, without the level.
    pub fn db_at(&self, hz: f32) -> f32 {
        let hz = f64::from(hz);
        self.filters
            .iter()
            .map(|filter| filter.db_at(hz, self.rate))
            .sum::<f64>() as f32
    }
}

/// Everything about the filters that depends on the device's sample rate
/// alone, worked out once.
#[derive(Debug, Clone)]
pub struct Design {
    rate: f64,
    /// The bands the device can carry.
    usable: [bool; COUNT],
    /// Turns the decibels wanted at the centres into the decibels to give
    /// the filters: the inverse of how much each band leaks into the rest.
    solve: [[f64; COUNT]; COUNT],
}

impl Design {
    pub fn new(sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let mut design = Self {
            rate,
            usable: [true; COUNT],
            solve: identity(),
        };
        let limit = rate / 2.0 * NYQUIST_SHARE;
        for (band, usable) in design.usable.iter_mut().enumerate() {
            *usable = turning_point(band) < limit;
        }
        // What a decibel on each band does at every band's centre.
        let mut leak = identity();
        for (band, _) in BANDS
            .iter()
            .enumerate()
            .filter(|(band, _)| design.usable[*band])
        {
            let filter = design.band(band, PROBE_DB);
            for (centre, hz) in BANDS
                .iter()
                .enumerate()
                .filter(|(at, _)| design.usable[*at])
            {
                leak[centre][band] = filter.db_at(f64::from(*hz), rate) / PROBE_DB;
            }
        }
        design.solve = inverse(leak).unwrap_or_else(identity);
        design
    }

    /// One band's filter at `db`. The top band is a shelf, the rest bells.
    ///
    /// The top is a shelf because a bell so near half the sample rate is
    /// squeezed out of shape, and differently on a 44.1 kHz device than on
    /// a 48 kHz one; a shelf is the same on both, and "more air" is what
    /// the last slider is reached for. The bottom stays a bell: a shelf
    /// there would raise what is under hearing as well, which no speaker
    /// plays and which only uses up headroom.
    fn band(&self, band: usize, db: f64) -> Biquad {
        if db.abs() < LEAST_DB || !self.usable[band] {
            Biquad::FLAT
        } else if band == COUNT - 1 {
            Biquad::high_shelf(turning_point(band), db, self.rate)
        } else {
            Biquad::peaking(f64::from(BANDS[band]), db, self.rate)
        }
    }

    fn filters(&self, gains: &[f64; COUNT]) -> [Biquad; COUNT] {
        std::array::from_fn(|band| self.band(band, gains[band]))
    }

    /// The filters whose curve passes through what the sliders say, with
    /// the level the settings ask for. Switched off, or with nothing
    /// moved, it is the shape that changes nothing.
    pub fn shape(&self, settings: &Settings) -> Shape {
        let flat = Shape {
            filters: [Biquad::FLAT; COUNT],
            gain_db: 0.0,
            peak_db: 0.0,
            rate: self.rate,
        };
        if !settings.enabled {
            return flat;
        }
        let wanted: [f64; COUNT] = std::array::from_fn(|band| {
            let db = settings.gains[band];
            let db = if db.is_finite() { db } else { 0.0 };
            f64::from(db.clamp(-RANGE_DB, RANGE_DB))
        });
        let mut shape = flat;
        if wanted.iter().any(|db| db.abs() >= LEAST_DB) {
            let mut given = [0.0; COUNT];
            let mut short = wanted;
            for _ in 0..ROUNDS {
                for (band, gain) in given.iter_mut().enumerate() {
                    let more: f64 = (0..COUNT).map(|at| self.solve[band][at] * short[at]).sum();
                    *gain = (*gain + more).clamp(-MOST_DB, MOST_DB);
                }
                shape.filters = self.filters(&given);
                for (band, hz) in BANDS.iter().enumerate() {
                    short[band] = if self.usable[band] {
                        wanted[band] - f64::from(shape.db_at(*hz))
                    } else {
                        0.0
                    };
                }
            }
            shape.peak_db = self.peak_db(&shape);
        }
        let preamp = if settings.preamp.is_finite() {
            settings.preamp.clamp(-RANGE_DB, RANGE_DB)
        } else {
            0.0
        };
        // With headroom kept, nothing comes out louder than it went in:
        // what the curve and the preamp raise is taken off again.
        let over = if settings.headroom {
            (shape.peak_db + preamp).max(0.0)
        } else {
            0.0
        };
        shape.gain_db = preamp - over;
        shape
    }

    /// The highest the curve reaches across what can be heard.
    fn peak_db(&self, shape: &Shape) -> f32 {
        let top = (self.rate / 2.0 * NYQUIST_SHARE).min(20_000.0) as f32;
        let octaves = (top / 20.0).log2();
        (0..PEAK_POINTS)
            .map(|point| 20.0 * 2f32.powf(octaves * point as f32 / (PEAK_POINTS - 1) as f32))
            .map(|hz| shape.db_at(hz))
            .fold(f32::NEG_INFINITY, f32::max)
    }
}

/// The frequency a band's filter is built at: its centre, or for the top
/// band where its shelf turns.
fn turning_point(band: usize) -> f64 {
    let centre = f64::from(BANDS[band]);
    if band == COUNT - 1 {
        centre / SHELF_BELOW
    } else {
        centre
    }
}

fn identity() -> [[f64; COUNT]; COUNT] {
    std::array::from_fn(|row| std::array::from_fn(|column| if row == column { 1.0 } else { 0.0 }))
}

/// The inverse of a small matrix, by elimination. `None` for one that has
/// none, which a matrix of overlapping bands never is.
fn inverse(mut matrix: [[f64; COUNT]; COUNT]) -> Option<[[f64; COUNT]; COUNT]> {
    let mut inverse = identity();
    for column in 0..COUNT {
        let pivot = (column..COUNT).max_by(|a, b| {
            matrix[*a][column]
                .abs()
                .total_cmp(&matrix[*b][column].abs())
        })?;
        if matrix[pivot][column].abs() < 1e-9 {
            return None;
        }
        matrix.swap(pivot, column);
        inverse.swap(pivot, column);
        let scale = matrix[column][column];
        for at in 0..COUNT {
            matrix[column][at] /= scale;
            inverse[column][at] /= scale;
        }
        for row in (0..COUNT).filter(|row| *row != column) {
            let factor = matrix[row][column];
            for at in 0..COUNT {
                matrix[row][at] -= factor * matrix[column][at];
                inverse[row][at] -= factor * inverse[column][at];
            }
        }
    }
    Some(inverse)
}
