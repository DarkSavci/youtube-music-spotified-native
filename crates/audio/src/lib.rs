//! The audio engine: stream in, sound out.
//!
//! Nothing here knows about the UI. The app hands the engine a target and
//! receives events; the pieces below are usable and testable on their own.

pub mod clock;
mod deck;
pub mod decode;
pub mod engine;
pub mod eq;
mod loudness;
mod output;
mod resample;
mod silence;
pub mod source;
mod stretch;
pub mod tap;
