//! The engine's path from decoder to device, without the threads, the
//! network or the device.
//!
//! Each stage is the one the engine runs, called as the engine calls it:
//! the deck's (to stereo, resampled, loudness), the worker's (equalizer,
//! balance) and the device callback's (volume). The order is the one
//! `eq.rs` sets out. Crossfade, speed and skipping silence are left out:
//! they are off when the sound is to be left as it is, and change it by
//! design when on.

use crate::deck::to_stereo;
use crate::engine::{balance_gains, gain_for};
use crate::eq::{Equalizer, Settings};
use crate::loudness;
use crate::output::Volume;
use crate::resample::Resampler;

/// How the stages are set: what the listener's settings, the track and the
/// device make of them.
#[derive(Debug, Clone, Copy)]
pub struct Setup {
    /// The rate the track decodes at: 48 kHz for Opus, 44.1 for AAC.
    pub source_rate: u32,
    pub channels: usize,
    /// The rate the device runs at.
    pub device_rate: u32,
    /// The track's loudness gain, as a multiplier: 1 with normalisation off.
    pub loudness: f32,
    pub equalizer: Settings,
    /// -1 all left, 1 all right.
    pub balance: f32,
    /// The volume slider: 0 to 1, or to 2 with boost.
    pub volume: f32,
}

impl Setup {
    /// Everything that changes the sound off, at full volume: the track
    /// exactly as it is, but for what the device's rate forces.
    pub fn untouched(source_rate: u32, device_rate: u32) -> Self {
        Self {
            source_rate,
            channels: 2,
            device_rate,
            loudness: 1.0,
            equalizer: Settings::default(),
            balance: 0.0,
            volume: 1.0,
        }
    }
}

pub struct Chain {
    setup: Setup,
    resampler: Option<Resampler>,
    equalizer: Equalizer,
    volume: Volume,
    /// The loudness gain in force.
    applied: f32,
}

/// How long the volume has been gliding when measuring starts: long past
/// any change, as it is a moment into a song.
const SETTLED_SECONDS: usize = 5;

impl Chain {
    pub fn new(setup: Setup) -> Self {
        let mut equalizer = Equalizer::new(setup.device_rate);
        equalizer.set(setup.equalizer);
        let mut volume = Volume::new();
        volume.settle(
            gain_for(setup.volume),
            SETTLED_SECONDS * setup.device_rate as usize,
        );
        Self {
            resampler: (setup.source_rate != setup.device_rate)
                .then(|| Resampler::new(setup.source_rate, setup.device_rate)),
            equalizer,
            volume,
            applied: setup.loudness,
            setup,
        }
    }

    /// Takes decoded audio, interleaved in the track's own channels, and
    /// gives back what would be handed to the device: stereo, at its rate.
    /// Meant to be fed a track in order, in pieces of any size.
    pub fn process(&mut self, decoded: &[f32]) -> Vec<f32> {
        let mut stereo = Vec::with_capacity(decoded.len());
        to_stereo(decoded, self.setup.channels, &mut stereo);
        let mut samples = match &mut self.resampler {
            Some(resampler) => {
                let mut resampled = Vec::with_capacity(stereo.len() * 2);
                resampler.process(&stereo, &mut resampled);
                resampled
            }
            None => stereo,
        };
        self.applied = loudness::apply(&mut samples, self.applied, self.setup.loudness);
        self.equalizer.process(&mut samples);
        if self.setup.balance != 0.0 {
            let (left, right) = balance_gains(self.setup.balance);
            for frame in samples.as_chunks_mut::<2>().0 {
                frame[0] *= left;
                frame[1] *= right;
            }
        }
        // The device callback sees what is queued after each frame; here,
        // what follows in the same piece. Fed a whole track, the same.
        let target = gain_for(self.setup.volume);
        let queued = samples.clone();
        for (index, frame) in samples.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            self.volume.step(target);
            *frame = self.volume.apply(*frame, &queued[(index + 1) * 2..]);
        }
        samples
    }
}

/// The gain the device callback settles at for a slider at `volume`: what
/// every sample is multiplied by once a song is under way.
pub(super) fn settled_gain(volume: f32, rate: u32) -> f32 {
    let mut gain = Volume::new();
    gain.settle(gain_for(volume), SETTLED_SECONDS * rate as usize);
    gain.step(gain_for(volume))
}
