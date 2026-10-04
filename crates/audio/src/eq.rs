//! A ten-band equalizer, its preamp, and the limiter that follows them.
//!
//! The bands sit where graphic equalizers have had their sliders since the
//! hi-fi rack, an octave apart. What each filter is, and how the ten are
//! chosen so that the curve passes through the sliders, is in [`design`].
//!
//! Where it stands among the stages a track goes through:
//!
//! 1. decoded, and resampled to the device's rate;
//! 2. turned down to the loudness target (per track, so two songs crossing
//!    are each at their own level);
//! 3. stretched to the playback speed;
//! 4. mixed with the track it fades from or into;
//! 5. **equalized, and the preamp applied** (here);
//! 6. limited, so nothing the equalizer or a crossfade raised clips (here);
//! 7. copied to the visualizer's tap, and queued for the device;
//! 8. scaled by the volume in the device's callback, where boost past 100%
//!    has a limiter of its own.
//!
//! It comes after the mix so that there is one equalizer however many
//! decks play, and before the volume so that the volume never changes how
//! hard the limiter works.
//!
//! Nothing here allocates, and switched off or flat the equalizer is not
//! in the path at all: the samples are not read.

mod design;
mod limiter;
#[cfg(test)]
mod tests;

pub use design::{Biquad, Design, Shape};
use limiter::Limiter;

/// The centre of each band, in hertz.
pub const BANDS: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// The most a band, or the preamp, is raised or lowered, in decibels.
pub const RANGE_DB: f32 = 12.0;

/// How long a change takes to arrive, in seconds. Long enough that a
/// dragged slider is a glide and not a staircase, short enough to be heard
/// as the hand moves.
const RAMP: f32 = 0.03;
/// Filter memory smaller than this is nothing: it is cleared, so that a
/// long silence does not leave the filters grinding through numbers too
/// small for the processor to handle quickly.
const NOTHING: f64 = 1e-30;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Settings {
    pub enabled: bool,
    /// Decibels for each of [`BANDS`].
    pub gains: [f32; 10],
    /// Decibels the whole signal is raised or lowered by.
    pub preamp: f32,
    /// Turn the level down by as much as the curve raises it, so that
    /// nothing comes out louder than it went in and the limiter is left
    /// with nothing to do.
    pub headroom: bool,
}

/// One band while it plays: the filter in force, where it is heading, and
/// what it remembers of each channel.
#[derive(Clone, Copy)]
struct Band {
    now: Biquad,
    target: Biquad,
    /// What `now` moves by each frame of a ramp.
    step: [f64; 5],
    /// Two numbers a channel: the filter in its transposed form, which
    /// stays quiet while its coefficients move.
    memory: [[f64; 2]; 2],
    /// Left out of the path while it is flat and staying so.
    active: bool,
}

impl Band {
    const IDLE: Band = Band {
        now: Biquad::FLAT,
        target: Biquad::FLAT,
        step: [0.0; 5],
        memory: [[0.0; 2]; 2],
        active: false,
    };

    /// Heads for `target` over `frames`. Every filter on the way is
    /// stable: each lies on the straight line between two that are.
    fn head_for(&mut self, target: Biquad, frames: f64) {
        self.target = target;
        self.active = self.active || target != Biquad::FLAT;
        let (from, to) = (flatten(&self.now), flatten(&target));
        self.step = std::array::from_fn(|at| (to[at] - from[at]) / frames);
    }

    fn advance(&mut self) {
        for (value, step) in self.now.b.iter_mut().zip(&self.step[..3]) {
            *value += step;
        }
        for (value, step) in self.now.a.iter_mut().zip(&self.step[3..]) {
            *value += step;
        }
    }

    /// Arrives: exactly the target, whatever the steps added up to.
    fn settle(&mut self) {
        self.now = self.target;
        if self.target == Biquad::FLAT {
            *self = Band::IDLE;
        }
    }

    fn filter(&mut self, channel: usize, input: f64) -> f64 {
        let [b0, b1, b2] = self.now.b;
        let [a1, a2] = self.now.a;
        let memory = &mut self.memory[channel];
        let output = b0 * input + memory[0];
        memory[0] = b1 * input - a1 * output + memory[1];
        memory[1] = b2 * input - a2 * output;
        output
    }

    /// Clears memory that has faded to nothing, or that bad input broke.
    fn tidy(&mut self) {
        for value in self.memory.iter_mut().flatten() {
            if !(value.abs() >= NOTHING && value.is_finite()) {
                *value = 0.0;
            }
        }
    }
}

fn flatten(filter: &Biquad) -> [f64; 5] {
    let [b0, b1, b2] = filter.b;
    let [a1, a2] = filter.a;
    [b0, b1, b2, a1, a2]
}

pub struct Equalizer {
    design: Design,
    settings: Settings,
    bands: [Band; 10],
    /// The level in force, as a multiplier, and where it is heading.
    gain: f64,
    gain_target: f64,
    gain_step: f64,
    /// A change takes this many frames to arrive, and this many are left.
    ramp: u32,
    ramp_left: u32,
    limiter: Limiter,
}

impl Equalizer {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            design: Design::new(sample_rate),
            settings: Settings::default(),
            bands: [Band::IDLE; 10],
            gain: 1.0,
            gain_target: 1.0,
            gain_step: 0.0,
            ramp: ((sample_rate as f32 * RAMP) as u32).max(1),
            ramp_left: 0,
            limiter: Limiter::new(sample_rate as f32),
        }
    }

    /// Takes new settings. The filters and the level glide to them from
    /// wherever they are, so a slider dragged, or the whole thing switched
    /// off, never clicks.
    pub fn set(&mut self, settings: Settings) {
        if settings == self.settings {
            return;
        }
        self.settings = settings;
        let shape = self.design.shape(&settings);
        let gain = 10f64.powf(f64::from(shape.gain_db) / 20.0);
        let headed = self.bands.iter().map(|band| &band.target);
        // A setting that comes to the same sound (a slider moved while it
        // is switched off) is no change to glide to.
        if gain == self.gain_target && headed.eq(&shape.filters) {
            return;
        }
        let frames = f64::from(self.ramp);
        for (band, target) in self.bands.iter_mut().zip(shape.filters) {
            band.head_for(target, frames);
        }
        self.gain_target = gain;
        self.gain_step = (self.gain_target - self.gain) / frames;
        self.ramp_left = self.ramp;
    }

    /// Whether the equalizer is out of the path: off, or flat, with no
    /// change still arriving.
    pub fn is_idle(&self) -> bool {
        self.ramp_left == 0 && self.gain == 1.0 && self.bands.iter().all(|band| !band.active)
    }

    /// Shapes interleaved stereo in place, then holds its peaks under the
    /// ceiling. Idle, the samples go to the limiter as they came, and it
    /// leaves alone what is already under the ceiling.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.is_idle() {
            let frames = samples.as_chunks_mut::<2>().0;
            let ramped = frames.len().min(self.ramp_left as usize);
            let (moving, steady) = frames.split_at_mut(ramped);
            self.glide(moving);
            self.shape(steady);
            for band in &mut self.bands {
                band.tidy();
            }
        }
        self.limiter.hold(samples);
    }

    /// The frames during which a change is arriving: every filter and the
    /// level move a step a frame.
    fn glide(&mut self, frames: &mut [[f32; 2]]) {
        for frame in frames {
            self.gain += self.gain_step;
            let mut pair = [f64::from(frame[0]), f64::from(frame[1])];
            for band in self.bands.iter_mut().filter(|band| band.active) {
                band.advance();
                pair = [band.filter(0, pair[0]), band.filter(1, pair[1])];
            }
            *frame = [(pair[0] * self.gain) as f32, (pair[1] * self.gain) as f32];
            self.ramp_left -= 1;
            if self.ramp_left == 0 {
                self.gain = self.gain_target;
                for band in &mut self.bands {
                    band.settle();
                }
            }
        }
    }

    /// The frames with everything at rest, a band at a time: each filter
    /// runs over the block with its numbers held in registers.
    fn shape(&mut self, frames: &mut [[f32; 2]]) {
        if frames.is_empty() {
            return;
        }
        for band in self.bands.iter_mut().filter(|band| band.active) {
            for frame in frames.iter_mut() {
                frame[0] = band.filter(0, f64::from(frame[0])) as f32;
                frame[1] = band.filter(1, f64::from(frame[1])) as f32;
            }
        }
        if self.gain != 1.0 {
            let gain = self.gain as f32;
            for frame in frames.iter_mut() {
                frame[0] *= gain;
                frame[1] *= gain;
            }
        }
    }
}
