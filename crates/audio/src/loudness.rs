//! Evening out how loud tracks are.
//!
//! YouTube knows each track's loudness, and the core passes it on. A track
//! louder than the target is turned down to it. A quieter one is left as it
//! is: turning it up could push peaks past full scale, and that needs a
//! limiter this pipeline does not have yet.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// What streaming services have settled on, in LUFS.
const TARGET_LUFS: f32 = -14.0;
/// The most a track is turned down. A reading that asks for more is more
/// likely wrong than the track is that loud.
const MOST_CUT_DB: f32 = 12.0;

/// A gain that may arrive after the track has started. It is fetched on a
/// thread of its own so the first sound never waits for it.
#[derive(Clone)]
pub struct Gain(Arc<AtomicU32>);

impl Gain {
    /// Unity, for when normalisation is off or nothing is known.
    pub fn unity() -> Self {
        Self(Arc::new(AtomicU32::new(1.0f32.to_bits())))
    }

    /// Unity now, and the track's own gain once the core has answered at
    /// `url`. Without an answer it stays at unity.
    pub fn fetch(agent: ureq::Agent, url: String) -> Self {
        let gain = Self::unity();
        let slot = gain.0.clone();
        let spawned = std::thread::Builder::new()
            .name("loudness".into())
            .spawn(move || {
                let body = agent
                    .get(&url)
                    .call()
                    .ok()
                    .and_then(|mut response| response.body_mut().read_to_string().ok());
                if let Some(lufs) = body.as_deref().and_then(loudness_in) {
                    slot.store(gain_for(lufs).to_bits(), Ordering::Relaxed);
                }
            });
        if let Err(error) = spawned {
            log::debug!("loudness not fetched: {error}");
        }
        gain
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// Scales `samples` from the gain the last chunk ended on to `to`, moving
/// evenly across the chunk so a gain that arrives mid-track is not heard as
/// a step. Returns `to`, for the next call.
pub fn apply(samples: &mut [f32], from: f32, to: f32) -> f32 {
    if from == 1.0 && to == 1.0 {
        return to;
    }
    let frames = (samples.len() / 2).max(1) as f32;
    for (index, frame) in samples.chunks_mut(2).enumerate() {
        let gain = from + (to - from) * (index as f32 / frames);
        for sample in frame {
            *sample *= gain;
        }
    }
    to
}

/// The linear gain for a track measured at `lufs`.
fn gain_for(lufs: f32) -> f32 {
    let db = (TARGET_LUFS - lufs).clamp(-MOST_CUT_DB, 0.0);
    10f32.powf(db / 20.0)
}

/// The number in `{"loudnessLkfs":-9.3}`. The core sends `{}` when it does
/// not know, which reads as `None`.
fn loudness_in(body: &str) -> Option<f32> {
    let after = body.split_once("\"loudnessLkfs\":")?.1;
    let number: String = after
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))
        .collect();
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loudness_is_read_from_the_cores_answer() {
        assert_eq!(loudness_in(r#"{"loudnessLkfs":-9.3}"#), Some(-9.3));
        assert_eq!(loudness_in(r#"{"loudnessLkfs": -14}"#), Some(-14.0));
        assert_eq!(loudness_in("{}"), None);
    }

    #[test]
    fn a_loud_track_is_turned_down_and_a_quiet_one_left_alone() {
        // Six decibels over the target is half the amplitude.
        assert!((gain_for(-8.0) - 0.501).abs() < 0.001);
        assert_eq!(gain_for(-14.0), 1.0);
        assert_eq!(gain_for(-23.0), 1.0);
    }

    #[test]
    fn an_implausible_reading_is_held_to_the_limit() {
        assert!((gain_for(20.0) - 10f32.powf(-12.0 / 20.0)).abs() < 1e-6);
    }

    #[test]
    fn a_new_gain_is_reached_across_the_chunk_not_at_once() {
        let mut samples = vec![1.0f32; 8];
        let ended_on = apply(&mut samples, 1.0, 0.5);
        assert_eq!(ended_on, 0.5);
        assert_eq!(samples[0], 1.0);
        assert!(samples[6] < samples[2]);
        assert!(samples[6] > 0.5);
    }
}
