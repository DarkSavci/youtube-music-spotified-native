//! A report on what the app does to the sound, in three parts:
//!
//! 1. the suite in `spotified_audio::quality::checks`: known signals sent
//!    through the engine's stages and held to limits;
//! 2. the sound device: the rate and format the system runs it at, which
//!    decide whether tracks have to be resampled;
//! 3. real tracks, when files are given: how loud their peaks run, and
//!    whether they reach the device exactly as decoded with every effect
//!    off.
//!
//!     cargo run -p spotified-audio --example quality --release -- [tracks...]
//!
//! Cached tracks are the `.audio` files in the app's `audio-cache` folder
//! (`%APPDATA%\SpotifiedNative\audio-cache` on Windows). It exits with 1
//! when a check fails.

use std::fs::File;
use std::process::ExitCode;

use cpal::traits::{DeviceTrait, HostTrait};
use spotified_audio::decode::Decoder;
use spotified_audio::quality::{Chain, Setup, checks, measure};

fn main() -> ExitCode {
    let checks = checks::all();
    println!("Engine checks\n");
    for check in &checks {
        let mark = if check.passed { "pass" } else { "FAIL" };
        println!("  {mark}  {}", check.name);
        println!("        {}", check.measured);
        if !check.passed {
            println!("        limit: {}", check.limit);
            println!("        {}", check.about);
        }
    }
    let failed = checks.iter().filter(|check| !check.passed).count();
    println!("\n  {} of {} passed\n", checks.len() - failed, checks.len());

    let device_rate = device();

    let files: Vec<String> = std::env::args().skip(1).collect();
    if !files.is_empty() {
        println!("Tracks, with every effect off, at full volume\n");
        for path in &files {
            track(path, device_rate.unwrap_or(48_000));
        }
    }

    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Describes the device the system plays through, and gives its rate.
fn device() -> Option<u32> {
    println!("Sound device\n");
    let Some(device) = cpal::default_host().default_output_device() else {
        println!("  none found\n");
        return None;
    };
    let name = device
        .description()
        .map(|description| description.name().to_owned())
        .unwrap_or_else(|_| "unnamed".into());
    let config = match device.default_output_config() {
        Ok(config) => config,
        Err(error) => {
            println!("  {name}: no usable format ({error})\n");
            return None;
        }
    };
    let rate = config.sample_rate();
    println!(
        "  {name}: {rate} Hz, {} channels, {:?}",
        config.channels(),
        config.sample_format()
    );
    if rate == 48_000 {
        println!("  Opus tracks play at their own rate; AAC tracks are resampled.");
    } else {
        println!(
            "  Every Opus track is resampled from 48000 Hz to {rate}. Setting the \
             device to 48000 Hz in the system's sound settings avoids it."
        );
    }
    if config.sample_format() != cpal::SampleFormat::F32 {
        println!(
            "  The device takes {:?}: samples are converted from 32-bit float, \
             with dither at 16 bits.",
            config.sample_format()
        );
    }
    // Held for a moment, playing silence: what exclusive output would get.
    match spotified_audio::try_exclusive(None) {
        Ok(format) if format.exclusive => println!(
            "  Exclusive output: available, at {} Hz, {}",
            format.sample_rate, format.sample_format
        ),
        Ok(format) => println!(
            "  Exclusive output: refused ({}); shared instead",
            format.exclusive_refused.unwrap_or_default()
        ),
        Err(error) => println!("  Exclusive output: the device would not open: {error}"),
    }
    println!();
    Some(rate)
}

fn track(path: &str, device_rate: u32) {
    let name = std::path::Path::new(path)
        .file_name()
        .map_or(path.into(), |name| name.to_string_lossy());
    let opened = File::open(path)
        .map_err(|error| error.to_string())
        .and_then(|file| Decoder::open(Box::new(file)).map_err(|error| error.to_string()));
    let mut decoder = match opened {
        Ok(decoder) => decoder,
        Err(error) => {
            println!("  {name}: {error}\n");
            return;
        }
    };
    let format = decoder.format();
    let mut decoded = Vec::new();
    loop {
        match decoder.next_chunk() {
            Ok(Some(chunk)) => decoded.extend_from_slice(chunk.samples),
            Ok(None) => break,
            Err(error) => {
                println!("  {name}: stopped decoding: {error}");
                break;
            }
        }
    }
    let seconds =
        decoded.len() as f64 / format.channels.max(1) as f64 / f64::from(format.sample_rate);
    println!(
        "  {name}: {} Hz, {} channels, {:.0} s",
        format.sample_rate, format.channels, seconds
    );

    let peak = measure::peak(&decoded);
    let near = measure::above(&decoded, 0.98);
    let over = measure::above(&decoded, 1.0);
    let share = |count: usize| 100.0 * count as f64 / decoded.len().max(1) as f64;
    println!(
        "    peak {:+.2} dBFS; {near} samples above 0.98 ({:.3}%), {over} above full scale ({:.3}%)",
        measure::db(f64::from(peak)),
        share(near),
        share(over),
    );

    let setup = Setup {
        channels: format.channels,
        ..Setup::untouched(format.sample_rate, device_rate)
    };
    // Whole, so the limiter sees what is coming as it does on the device.
    let output = Chain::new(setup).process(&decoded);
    if format.sample_rate == device_rate && format.channels == 2 {
        println!("    to the device: {}", measure::null(&decoded, &output));
    } else {
        println!(
            "    to the device: resampled from {} to {device_rate} Hz, so not bit-exact by \
             design; peak after {:+.2} dBFS",
            format.sample_rate,
            measure::db(f64::from(measure::peak(&output)))
        );
    }
    println!();
}
