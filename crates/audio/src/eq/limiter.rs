//! The limiter that follows the equalizer.
//!
//! Raising bands, or two songs crossing, can push peaks past full scale,
//! which the device would clip. This turns the level down for as long as a
//! peak lasts and lets it back up gently. What stays under the ceiling
//! passes through untouched, bit for bit.

/// The level peaks are held under, a little below full scale.
pub(super) const CEILING: f32 = 0.98;
/// How long the level takes to come back up, in seconds.
const RELEASE: f32 = 0.1;

pub(super) struct Limiter {
    /// The gain: 1 when nothing is being held back.
    gain: f32,
    /// How much of the way back to 1 the gain moves per frame.
    release: f32,
}

impl Limiter {
    pub(super) fn new(sample_rate: f32) -> Self {
        Self {
            gain: 1.0,
            release: 1.0 - (-1.0 / (RELEASE * sample_rate)).exp(),
        }
    }

    /// Holds the peaks of interleaved stereo under the ceiling, in place.
    pub(super) fn hold(&mut self, samples: &mut [f32]) {
        for frame in samples.as_chunks_mut::<2>().0 {
            let peak = frame[0].abs().max(frame[1].abs());
            // Down at once for a peak, back up gently after it.
            let needed = if peak > CEILING { CEILING / peak } else { 1.0 };
            if needed < self.gain {
                self.gain = needed;
            } else {
                self.gain += (needed - self.gain) * self.release;
                // The last of the way is too small a step to be taken.
                if needed == 1.0 && self.gain > 0.9999 {
                    self.gain = 1.0;
                }
            }
            frame[0] *= self.gain;
            frame[1] *= self.gain;
        }
    }
}
