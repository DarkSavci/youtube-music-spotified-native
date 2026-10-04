//! The audio engine: stream in, sound out.
//!
//! Nothing here knows about the UI. The app hands the engine a target and
//! receives events; the pieces below are usable and testable on their own.

mod deck;
pub mod decode;
pub mod engine;
pub mod eq;
mod loudness;
mod output;
mod resample;
mod silence;
mod source;
pub mod tap;
