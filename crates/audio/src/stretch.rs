//! Playing faster or slower without the pitch moving.
//!
//! Resampling would change the speed and the pitch together. This keeps the
//! pitch by leaving the samples as they are and choosing which of them are
//! heard: the music is cut into short overlapping windows, and windows are
//! skipped (faster) or heard twice (slower). Each window is taken from the
//! place near where it is due that looks most like what the last one led
//! into, so the joins fall where the waveform already repeats and are not
//! heard. It is the method a browser's audio element uses, with the same
//! window and reach, which is what the old app's speed sounded like.

/// The length of one window, and how far either side of where a window is
/// due the best join is looked for.
const WINDOW_MS: u32 = 20;
const REACH_MS: u32 = 15;
/// The search looks at every fourth place and every second sample first,
/// then at every place and sample around the best of those.
const COARSE_STEP: usize = 4;
const COARSE_STRIDE: usize = 2;

/// Changes the speed of interleaved stereo as it passes through.
#[derive(Default)]
pub struct Stretcher {
    rate: u32,
    /// Frames in a window; each one starts half a window after the last.
    window: usize,
    hop: usize,
    reach: usize,
    /// A window's fade in and out. Two of them half a window apart add up
    /// to one everywhere, so overlapped windows of unchanged audio give the
    /// audio back.
    weights: Vec<f32>,
    /// The audio taken in and still needed, and the same as one channel,
    /// which is what the joins are judged on.
    input: Vec<f32>,
    mono: Vec<f32>,
    /// Where the next window is due, in frames from the start of `input`.
    due: f64,
    /// Where the audio that followed the last window begins: what the next
    /// one should sound like. `None` before the first.
    follows: Option<usize>,
    /// The second half of the last window, faded out, waiting to be laid
    /// under the first half of the next.
    tail: Vec<f32>,
}

impl Stretcher {
    /// Whether nothing is held: audio at normal speed can pass round it.
    pub fn is_idle(&self) -> bool {
        self.input.is_empty() && self.follows.is_none()
    }

    /// Frames taken in whose sound has not yet come out.
    pub fn held_frames(&self) -> usize {
        self.mono.len() - self.follows.unwrap_or(0).min(self.mono.len())
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.mono.clear();
        self.due = 0.0;
        self.follows = None;
    }

    /// Takes `samples` in and adds to `out` what is ready to be heard at
    /// `speed` times normal. At a speed of one, audio passes through
    /// untouched, after whatever was still held from another speed.
    pub fn process(&mut self, rate: u32, speed: f32, samples: &[f32], out: &mut Vec<f32>) {
        if speed == 1.0 {
            self.drain(out);
            out.extend_from_slice(samples);
            return;
        }
        self.fit(rate);
        self.input.extend_from_slice(samples);
        self.mono.extend(
            samples
                .as_chunks::<2>()
                .0
                .iter()
                .map(|frame| (frame[0] + frame[1]) * 0.5),
        );
        loop {
            let centre = self.due.round() as usize;
            let (first, last) = (centre.saturating_sub(self.reach), centre + self.reach);
            let needed = (last + self.window).max(self.follows.map_or(0, |at| at + self.window));
            if self.mono.len() < needed {
                break;
            }
            let at = match self.follows {
                None => centre,
                // What followed the last window is itself within reach: it
                // carries straight on, with no join at all.
                Some(follows) if (first..=last).contains(&follows) => follows,
                Some(follows) => self.most_alike(follows, first, last),
            };
            self.emit(at, out);
            self.follows = Some(at + self.hop);
            self.due += self.hop as f64 * f64::from(speed);
            self.forget();
        }
    }

    /// Adds to `out` everything still held, at normal speed, and starts
    /// afresh: at the end of a track, or on the way back to normal speed.
    pub fn drain(&mut self, out: &mut Vec<f32>) {
        let frames = self.mono.len();
        match self.follows {
            None => out.extend_from_slice(&self.input),
            // The last window's tail under the audio that followed it is
            // that audio, so it goes out as it came in.
            Some(follows) if frames >= follows + self.hop => {
                out.extend_from_slice(&self.input[follows * 2..]);
            }
            // Too little left to cover the tail: it fades out as it is.
            Some(follows) => {
                for index in 0..self.hop {
                    let weight = self.weights[index];
                    for channel in 0..2 {
                        let fresh = self
                            .input
                            .get((follows + index) * 2 + channel)
                            .map_or(0.0, |sample| sample * weight);
                        out.push(self.tail[index * 2 + channel] + fresh);
                    }
                }
            }
        }
        self.reset();
    }

    /// Sizes the windows for a sample rate, the first time and if the
    /// device changes.
    fn fit(&mut self, rate: u32) {
        if self.rate == rate && self.window > 0 {
            return;
        }
        self.reset();
        self.rate = rate;
        self.hop = ((rate * WINDOW_MS / 2000) as usize).max(1);
        self.window = self.hop * 2;
        self.reach = (rate * REACH_MS / 1000) as usize;
        self.weights = (0..self.window)
            .map(|index| {
                let turn = std::f32::consts::TAU * index as f32 / self.window as f32;
                0.5 - 0.5 * turn.cos()
            })
            .collect();
        self.tail = vec![0.0; self.window];
    }

    /// The start, between `first` and `last`, of the window most like the
    /// one at `wanted`.
    fn most_alike(&self, wanted: usize, first: usize, last: usize) -> usize {
        let coarse = self.best(wanted, first, last, COARSE_STEP, COARSE_STRIDE);
        let from = coarse.saturating_sub(COARSE_STEP - 1).max(first);
        let to = (coarse + COARSE_STEP - 1).min(last);
        self.best(wanted, from, to, 1, 1)
    }

    fn best(&self, wanted: usize, first: usize, last: usize, step: usize, stride: usize) -> usize {
        let target = &self.mono[wanted..wanted + self.window];
        let mut best = (first, f32::MIN);
        for at in (first..=last).step_by(step) {
            let candidate = &self.mono[at..at + self.window];
            let (mut alike, mut energy) = (0.0f32, 1e-9f32);
            for (a, b) in target.iter().zip(candidate).step_by(stride) {
                alike += a * b;
                energy += b * b;
            }
            // Scaled by the candidate's own loudness, so a loud passage is
            // not chosen merely for being loud.
            let score = alike / energy.sqrt();
            if score > best.1 {
                best = (at, score);
            }
        }
        best.0
    }

    /// Lays the window at `at` over the last one's tail: half a window
    /// comes out, and the other half waits for the next.
    fn emit(&mut self, at: usize, out: &mut Vec<f32>) {
        let block = &self.input[at * 2..(at + self.window) * 2];
        // The very first has nothing under it, and goes out as it is.
        let opening = self.follows.is_none();
        for index in 0..self.hop {
            let weight = if opening { 1.0 } else { self.weights[index] };
            for channel in 0..2 {
                let under = if opening {
                    0.0
                } else {
                    self.tail[index * 2 + channel]
                };
                out.push(under + block[index * 2 + channel] * weight);
            }
        }
        for index in 0..self.hop {
            let weight = self.weights[self.hop + index];
            for channel in 0..2 {
                self.tail[index * 2 + channel] = block[(self.hop + index) * 2 + channel] * weight;
            }
        }
    }

    /// Lets go of the input no later window can reach back to.
    fn forget(&mut self) {
        let earliest = (self.due.round() as usize).saturating_sub(self.reach);
        let done = earliest.min(self.follows.unwrap_or(0)).min(self.mono.len());
        if done == 0 {
            return;
        }
        self.input.drain(..done * 2);
        self.mono.drain(..done);
        self.due -= done as f64;
        self.follows = self.follows.map(|at| at - done);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    const PITCH: f32 = 440.0;
    /// What is still held when the input stops comes out at normal speed,
    /// so a length may be out by that much: a tenth of a second of samples
    /// at most, however long the input.
    const END: f32 = RATE as f32 * 2.0 * 0.1;

    /// `seconds` of a sine on both channels.
    fn sine(seconds: f32) -> Vec<f32> {
        let frames = (RATE as f32 * seconds) as usize;
        (0..frames)
            .flat_map(|frame| {
                let sample =
                    (std::f32::consts::TAU * PITCH * frame as f32 / RATE as f32).sin() * 0.5;
                [sample, sample]
            })
            .collect()
    }

    /// Runs `input` through at `speed`, in chunks of the uneven sizes a
    /// decoder hands over.
    fn stretched(input: &[f32], speed: f32) -> Vec<f32> {
        let mut stretcher = Stretcher::default();
        let mut out = Vec::new();
        let mut rest = input;
        for size in [1920, 2048, 882, 4096].iter().cycle() {
            if rest.is_empty() {
                break;
            }
            let (chunk, later) = rest.split_at((*size).min(rest.len()));
            stretcher.process(RATE, speed, chunk, &mut out);
            rest = later;
        }
        stretcher.drain(&mut out);
        out
    }

    /// The pitch of the left channel, from how often it crosses zero going up.
    fn pitch_of(samples: &[f32]) -> f32 {
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let crossings = left
            .windows(2)
            .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
            .count();
        crossings as f32 * RATE as f32 / left.len() as f32
    }

    #[test]
    fn a_tone_keeps_its_pitch_and_its_length_follows_the_speed() {
        let input = sine(3.0);
        for speed in [0.5, 0.75, 1.25, 1.5, 2.0, 3.0] {
            let out = stretched(&input, speed);
            let expected = input.len() as f32 / speed;
            let off = (out.len() as f32 - expected).abs();
            assert!(
                off < END,
                "at {speed}: {} samples, not {expected}",
                out.len()
            );
            let pitch = pitch_of(&out);
            assert!((pitch - PITCH).abs() < 4.0, "at {speed}: {pitch} Hz");
        }
    }

    #[test]
    fn the_joins_are_not_heard_as_clicks() {
        // A sine never moves faster than this from one sample to the next;
        // a bad join would.
        let steepest = std::f32::consts::TAU * PITCH / RATE as f32 * 0.5;
        for speed in [0.5, 1.5, 2.0] {
            let out = stretched(&sine(2.0), speed);
            let jump = out
                .windows(4)
                .step_by(2)
                .map(|pair| (pair[2] - pair[0]).abs())
                .fold(0.0, f32::max);
            assert!(jump < steepest * 1.5, "at {speed}: a step of {jump}");
            let loudest = out
                .iter()
                .fold(0.0f32, |most, sample| most.max(sample.abs()));
            assert!(loudest < 0.55, "at {speed}: a peak of {loudest}");
        }
    }

    #[test]
    fn normal_speed_passes_the_audio_through_untouched() {
        let input = sine(0.5);
        assert_eq!(stretched(&input, 1.0), input);
    }

    #[test]
    fn going_back_to_normal_speed_loses_nothing_that_was_held() {
        let input = sine(1.0);
        let (fast, normal) = input.split_at(input.len() / 2);
        let mut stretcher = Stretcher::default();
        let mut out = Vec::new();
        stretcher.process(RATE, 2.0, fast, &mut out);
        assert!(stretcher.held_frames() > 0);
        stretcher.process(RATE, 1.0, normal, &mut out);
        assert!(stretcher.is_idle());
        // Half at double speed and half at normal: three quarters as long.
        let expected = input.len() as f32 * 0.75;
        assert!((out.len() as f32 - expected).abs() < END);
        assert!((pitch_of(&out) - PITCH).abs() < 4.0);
    }

    #[test]
    fn what_is_held_is_counted_until_it_is_heard() {
        let mut stretcher = Stretcher::default();
        let mut out = Vec::new();
        stretcher.process(RATE, 1.5, &sine(0.01), &mut out);
        // Too little for one window: all of it waits.
        assert!(out.is_empty());
        assert_eq!(stretcher.held_frames(), 480);
        stretcher.reset();
        assert_eq!(stretcher.held_frames(), 0);
        assert!(stretcher.is_idle());
    }
}
