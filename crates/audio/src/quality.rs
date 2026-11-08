//! Measuring what the engine does to the sound.
//!
//! The stages a track passes through between the decoder and the device
//! are put together here in the engine's order, from the same code the
//! engine runs, and signals whose every sample is known are sent through
//! them. What comes out is compared with what went in: sample for sample
//! where nothing should have changed, and by the level of a tone and what
//! is left besides it where something should.
//!
//! [`checks`] is the suite, each check with the limit it is held to. The
//! tests fail when one is not met; `examples/quality.rs` prints them all,
//! beside what the same measurements make of real tracks and of the sound
//! device.
//!
//! What this cannot see is anything after the samples leave the app: the
//! system's mixer, its effects and the device itself.

mod chain;
pub mod checks;
pub mod measure;
pub mod signal;
#[cfg(test)]
mod tests;

pub use chain::{Chain, Setup};
