//! What is measured: whether samples changed, and how a tone came through.

use std::f64::consts::PI;

/// One channel of interleaved stereo: 0 left, 1 right.
pub fn channel(samples: &[f32], which: usize) -> Vec<f32> {
    samples.iter().skip(which).step_by(2).copied().collect()
}

/// The largest sample, by size.
pub fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0, |most, sample| most.max(sample.abs()))
}

/// How many samples are larger than `level`.
pub fn above(samples: &[f32], level: f32) -> usize {
    samples.iter().filter(|sample| sample.abs() > level).count()
}

/// A level as a multiplier, in decibels.
pub fn db(level: f64) -> f64 {
    20.0 * level.log10()
}

/// What subtracting one signal from another leaves.
#[derive(Debug, Clone, Copy)]
pub struct Null {
    /// Samples that are not bit for bit the same.
    pub differing: usize,
    pub compared: usize,
    /// The largest difference, by size.
    pub largest: f32,
}

impl Null {
    pub fn exact(&self) -> bool {
        self.differing == 0
    }

    /// The largest difference relative to full scale: minus infinity when
    /// there is none.
    pub fn largest_db(&self) -> f64 {
        db(f64::from(self.largest))
    }
}

impl std::fmt::Display for Null {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.exact() {
            write!(f, "all {} samples identical", self.compared)
        } else {
            write!(
                f,
                "{} of {} samples changed, by up to {:.1} dBFS",
                self.differing,
                self.compared,
                self.largest_db()
            )
        }
    }
}

/// Compares `output` with `reference` over the length they share. Bits
/// are compared, not values, so a changed sign of zero counts too.
pub fn null(reference: &[f32], output: &[f32]) -> Null {
    let mut differing = 0;
    let mut largest = 0.0f32;
    for (a, b) in reference.iter().zip(output) {
        if a.to_bits() != b.to_bits() {
            differing += 1;
            largest = largest.max((a - b).abs());
        }
    }
    Null {
        differing,
        compared: reference.len().min(output.len()),
        largest,
    }
}

/// A tone as it came through: the sine at its frequency that best matches
/// the signal, and what is left once that is taken away (noise, distortion,
/// images and aliases, all together).
#[derive(Debug, Clone, Copy)]
pub struct Tone {
    pub amplitude: f64,
    pub residual_rms: f64,
}

impl Tone {
    /// The tone's peak level, in dBFS.
    pub fn level_db(&self) -> f64 {
        db(self.amplitude)
    }

    /// Everything that is not the tone, relative to the tone: THD+N.
    pub fn thd_n_db(&self) -> f64 {
        db(self.residual_rms / (self.amplitude / 2f64.sqrt()))
    }
}

/// Fits a sine at `hz` (and a constant offset) to one channel by least
/// squares. Exact for any length, whole cycles or not.
pub fn fit(samples: &[f32], rate: u32, hz: f64) -> Tone {
    let step = 2.0 * PI * hz / f64::from(rate);
    // The normal equations for x ≈ a·sin + b·cos + c.
    let mut m = [[0.0f64; 3]; 3];
    let mut v = [0.0f64; 3];
    for (index, &sample) in samples.iter().enumerate() {
        let (sin, cos) = (step * index as f64).sin_cos();
        let basis = [sin, cos, 1.0];
        for row in 0..3 {
            v[row] += basis[row] * f64::from(sample);
            for column in 0..3 {
                m[row][column] += basis[row] * basis[column];
            }
        }
    }
    let [a, b, c] = solve(m, v);
    let mut residual = 0.0;
    for (index, &sample) in samples.iter().enumerate() {
        let (sin, cos) = (step * index as f64).sin_cos();
        let left = f64::from(sample) - (a * sin + b * cos + c);
        residual += left * left;
    }
    Tone {
        amplitude: a.hypot(b),
        residual_rms: (residual / samples.len().max(1) as f64).sqrt(),
    }
}

/// Gaussian elimination with partial pivoting, for three unknowns.
fn solve(mut m: [[f64; 3]; 3], mut v: [f64; 3]) -> [f64; 3] {
    for column in 0..3 {
        let pivot = (column..3)
            .max_by(|&x, &y| m[x][column].abs().total_cmp(&m[y][column].abs()))
            .unwrap_or(column);
        m.swap(column, pivot);
        v.swap(column, pivot);
        if m[column][column] == 0.0 {
            continue;
        }
        for row in column + 1..3 {
            let factor = m[row][column] / m[column][column];
            let pivot_row = m[column];
            for (value, above) in m[row].iter_mut().zip(pivot_row).skip(column) {
                *value -= factor * above;
            }
            v[row] -= factor * v[column];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let known: f64 = (row + 1..3).map(|k| m[row][k] * x[k]).sum();
        x[row] = if m[row][row] == 0.0 {
            0.0
        } else {
            (v[row] - known) / m[row][row]
        };
    }
    x
}
