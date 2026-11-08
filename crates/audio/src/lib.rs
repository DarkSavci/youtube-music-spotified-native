//! The audio engine: stream in, sound out.
//!
//! Nothing here knows about the UI. The app hands the engine a target and
//! receives events; the pieces below are usable and testable on their own.

pub mod clock;
mod deck;
pub mod decode;
pub mod engine;
pub mod eq;
#[cfg(windows)]
mod exclusive;
mod loudness;
mod output;
pub mod quality;
mod resample;
mod silence;
pub mod source;
mod stretch;
pub mod tap;

pub use output::{Format as OutputFormat, OutputDevice, output_devices, try_exclusive};
