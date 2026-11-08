//! Test signals: interleaved stereo, every sample known in advance and the
//! same on every run.

use std::f64::consts::PI;

/// A sine of `amplitude` at `hz`, the same in both channels.
pub fn tone(rate: u32, hz: f64, amplitude: f64, seconds: f64) -> Vec<f32> {
    let frames = (f64::from(rate) * seconds) as usize;
    (0..frames)
        .flat_map(|frame| {
            let value = (amplitude * (2.0 * PI * hz * frame as f64 / f64::from(rate)).sin()) as f32;
            [value, value]
        })
        .collect()
}

/// White noise up to `peak`, left and right unrelated. Every value the
/// format holds turns up, which a tone would not exercise: noise peaking
/// just under full scale stands in for music mastered as loud as it goes.
pub fn noise(rate: u32, peak: f32, seconds: f64) -> Vec<f32> {
    let samples = (f64::from(rate) * seconds) as usize * 2;
    // xorshift64: fixed seed, so a failure can be run again exactly.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    (0..samples)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            // The top 24 bits, as -1 to 1.
            let unit = (state >> 40) as f32 / (1u64 << 23) as f32 - 1.0;
            unit * peak
        })
        .collect()
}

/// The same signal with the right channel silent.
pub fn left_only(mut samples: Vec<f32>) -> Vec<f32> {
    for frame in samples.as_chunks_mut::<2>().0 {
        frame[1] = 0.0;
    }
    samples
}
