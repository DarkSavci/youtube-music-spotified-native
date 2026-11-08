//! The suite: each check sends a known signal through the chain, or
//! through the one stage it is about, and holds what comes out to a limit.
//!
//! The limits are set where a change stops being something anyone could
//! hear or a better device could show, not where the code happens to be:
//! where nothing should change, nothing may; where something must (a rate
//! changed, a level turned down), it must stay flat to within a tenth of a
//! decibel across the band and add nothing above -100 dB, under the noise
//! of a 16-bit recording.

use super::chain::settled_gain;
use super::measure::{self, Null, Tone};
use super::signal;
use super::{Chain, Setup};
use crate::eq::{self, Equalizer};
use crate::output::{Dither, LOOKAHEAD};

pub struct Check {
    pub name: &'static str,
    /// What it shows when it fails, and why that matters.
    pub about: &'static str,
    pub measured: String,
    pub limit: &'static str,
    pub passed: bool,
}

const OPUS: u32 = 48_000;
const AAC: u32 = 44_100;
/// Tones are measured at -6 dBFS: loud, with room left for a raised band.
const LEVEL: f64 = 0.5;
/// The most a level may stray where it should not change, in dB.
const FLAT_DB: f64 = 0.1;
/// The most that may come out besides the tone, in dB below it.
const CLEAN_DB: f64 = -100.0;
/// Frequencies a response is measured at, in hertz.
const BAND: [f64; 9] = [
    20.0, 50.0, 100.0, 1000.0, 5000.0, 10_000.0, 15_000.0, 18_000.0, 20_000.0,
];
/// Frequencies distortion is measured at: the middle, and up where the
/// resampler's filter does its work.
const DISTORTION_AT: [f64; 3] = [1000.0, 10_000.0, 19_000.0];

pub fn all() -> Vec<Check> {
    vec![
        untouched_is_bit_exact(),
        full_scale_peaks_pass_untouched(),
        full_volume_is_unity(),
        equalizer_off_leaves_peaks_alone(),
        aac_to_48k_is_flat(),
        aac_to_48k_is_clean(),
        opus_to_44k_is_flat(),
        opus_to_44k_is_clean(),
        channels_stay_apart(),
        loudness_cut_is_exact_and_clean(),
        lower_volume_is_clean(),
        equalizer_is_clean(),
        overs_are_held_at_full_scale(),
        quiet_detail_survives_16_bit(),
        limiting_is_clean(),
        correction_is_clean(),
        correction_is_what_is_drawn(),
    ]
}

/// A real headphone's correction: AutoEQ's for the Sennheiser HD 600, as
/// oratory1990 measured it. Ten filters, shelves at both ends.
pub fn hd_600() -> eq::Correction {
    use eq::FilterKind::{HighShelf, LowShelf, Peak};
    let filters = [
        (LowShelf, 105.0, 6.5, 0.70),
        (Peak, 125.0, -2.7, 0.55),
        (Peak, 8445.0, 3.3, 1.61),
        (Peak, 522.0, 0.7, 1.02),
        (Peak, 1298.0, -1.2, 2.14),
        (HighShelf, 10000.0, -3.1, 0.70),
        (Peak, 3158.0, -1.8, 3.67),
        (Peak, 2166.0, 0.9, 3.32),
        (Peak, 6639.0, 2.2, 5.82),
        (Peak, 5433.0, -1.2, 5.70),
    ];
    eq::Correction {
        filters: filters.map(|(kind, hz, db, q)| eq::Filter { kind, hz, db, q }),
    }
}

fn corrected() -> Setup {
    Setup {
        equalizer: eq::Settings {
            correction: hd_600(),
            ..eq::Settings::default()
        },
        ..Setup::untouched(OPUS, OPUS)
    }
}

pub fn correction_is_clean() -> Check {
    distortion_check(
        "Headphone correction adds no distortion",
        "Ten filters making a headphone neutral change the level of each \
         frequency and should add nothing besides.",
        corrected(),
    )
}

/// The frequencies the correction is compared with its curve at: across
/// the band, and on its filters' centres.
const CORRECTION_AT: [f64; 9] = [
    30.0, 105.0, 522.0, 1298.0, 3158.0, 5433.0, 6639.0, 8445.0, 15_000.0,
];

pub fn correction_is_what_is_drawn() -> Check {
    let setup = corrected();
    let shape = eq::Design::new(setup.device_rate).shape(&setup.equalizer);
    let worst = CORRECTION_AT
        .iter()
        .map(|&hz| {
            let heard = tone_through(setup, hz).level_db() - measure::db(LEVEL);
            let drawn = f64::from(shape.db_at(hz as f32) + shape.gain_db);
            (heard - drawn, hz)
        })
        .max_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        .unwrap_or((0.0, 0.0));
    let peak = shape.peak_db + shape.gain_db;
    Check {
        name: "Headphone correction is the curve drawn, under full scale",
        about: "What a tone comes out at is what the correction's curve says, \
                and the level is taken down so its highest point stays at \
                full scale.",
        measured: format!(
            "{:+.3} dB off at {} Hz (worst); top of the curve {peak:+.2} dB",
            worst.0, worst.1
        ),
        limit: "within ±0.1 dB, top at most 0 dB",
        passed: worst.0.abs() <= FLAT_DB && peak <= 0.0,
    }
}

/// The chain's output for `input`, past the first tenth of a second, when
/// the resampler and the equalizer have settled.
fn through(setup: Setup, input: &[f32]) -> Vec<f32> {
    let output = Chain::new(setup).process(input);
    let skip = (setup.device_rate as usize / 10) * 2;
    output[skip.min(output.len())..].to_vec()
}

/// How a tone at `hz` comes through `setup`, measured on the left.
fn tone_through(setup: Setup, hz: f64) -> Tone {
    let input = signal::tone(setup.source_rate, hz, LEVEL, 1.0);
    let output = through(setup, &input);
    measure::fit(&measure::channel(&output, 0), setup.device_rate, hz)
}

fn null_check(name: &'static str, about: &'static str, null: Null) -> Check {
    Check {
        name,
        about,
        measured: null.to_string(),
        limit: "no sample changed",
        passed: null.exact(),
    }
}

/// The largest departure from `LEVEL` across the band, in dB, and where.
fn response(setup: Setup) -> (f64, f64) {
    BAND.iter()
        .map(|&hz| (tone_through(setup, hz).level_db() - measure::db(LEVEL), hz))
        .max_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        .unwrap_or((0.0, 0.0))
}

fn response_check(name: &'static str, about: &'static str, setup: Setup) -> Check {
    let (worst, hz) = response(setup);
    Check {
        name,
        about,
        measured: format!("{worst:+.3} dB at {hz} Hz (worst, 20 Hz to 20 kHz)"),
        limit: "within ±0.1 dB",
        passed: worst.abs() <= FLAT_DB,
    }
}

/// THD+N at each of `DISTORTION_AT`, in dB: the worst, and all of them
/// written out, since how it grows with frequency says where it comes from.
fn distortion(setup: Setup) -> (f64, String) {
    let measured: Vec<(f64, f64)> = DISTORTION_AT
        .iter()
        .map(|&hz| (hz, tone_through(setup, hz).thd_n_db()))
        .collect();
    let worst = measured
        .iter()
        .map(|&(_, thd)| thd)
        .fold(f64::NEG_INFINITY, f64::max);
    let all = measured
        .iter()
        .map(|(hz, thd)| format!("{thd:.1} dB at {} kHz", hz / 1000.0))
        .collect::<Vec<_>>()
        .join(", ");
    (worst, format!("THD+N {all}"))
}

fn distortion_check(name: &'static str, about: &'static str, setup: Setup) -> Check {
    let (worst, measured) = distortion(setup);
    Check {
        name,
        about,
        measured,
        limit: "at most -100 dB",
        passed: worst <= CLEAN_DB,
    }
}

pub fn untouched_is_bit_exact() -> Check {
    let input = signal::noise(OPUS, 0.5, 2.0);
    let output = Chain::new(Setup::untouched(OPUS, OPUS)).process(&input);
    null_check(
        "Untouched path is bit-exact",
        "With every effect off, at full volume, on a 48 kHz device, an Opus \
         track should reach the device exactly as decoded.",
        measure::null(&input, &output),
    )
}

pub fn full_scale_peaks_pass_untouched() -> Check {
    // Peaks to 0.999: loud masters, and Opus decoding, go this high.
    let input = signal::noise(OPUS, 0.999, 2.0);
    let output = Chain::new(Setup::untouched(OPUS, OPUS)).process(&input);
    null_check(
        "Peaks near full scale pass untouched",
        "Music mastered loud has peaks just under full scale. Nothing is \
         raised, so nothing needs limiting, and none should be.",
        measure::null(&input, &output),
    )
}

pub fn full_volume_is_unity() -> Check {
    let gain = settled_gain(1.0, OPUS);
    Check {
        name: "Full volume is a gain of exactly 1",
        about: "The device callback glides the gain to the volume. At 100% it \
                must arrive at 1, or every sample is scaled.",
        measured: format!(
            "settles at {gain:.9} ({:+.6} dB)",
            measure::db(f64::from(gain))
        ),
        limit: "exactly 1",
        passed: gain == 1.0,
    }
}

pub fn equalizer_off_leaves_peaks_alone() -> Check {
    let input = signal::noise(OPUS, 0.999, 2.0);
    let mut output = input.clone();
    Equalizer::new(OPUS).process(&mut output);
    null_check(
        "Equalizer off leaves peaks alone",
        "Switched off, the equalizer should be out of the path, its limiter \
         with it.",
        measure::null(&input, &output),
    )
}

pub fn aac_to_48k_is_flat() -> Check {
    response_check(
        "AAC resampled to 48 kHz: flat",
        "AAC tracks are 44.1 kHz and most devices run at 48: every AAC track \
         goes through the resampler.",
        Setup::untouched(AAC, OPUS),
    )
}

pub fn aac_to_48k_is_clean() -> Check {
    distortion_check(
        "AAC resampled to 48 kHz: clean",
        "What the resampler adds: rounding, and images of the tone above the \
         old band folded back.",
        Setup::untouched(AAC, OPUS),
    )
}

pub fn opus_to_44k_is_flat() -> Check {
    response_check(
        "Opus resampled to 44.1 kHz: flat",
        "On a device set to 44.1 kHz, Opus tracks are resampled down.",
        Setup::untouched(OPUS, AAC),
    )
}

pub fn opus_to_44k_is_clean() -> Check {
    distortion_check(
        "Opus resampled to 44.1 kHz: clean",
        "What resampling down adds, near the top of the band where the \
         filter cuts.",
        Setup::untouched(OPUS, AAC),
    )
}

pub fn channels_stay_apart() -> Check {
    let input = signal::left_only(signal::noise(AAC, 0.5, 1.0));
    let output = through(Setup::untouched(AAC, OPUS), &input);
    let leaked = measure::peak(&measure::channel(&output, 1));
    Check {
        name: "Channels stay apart",
        about: "Sound in one channel must not leak into the other, through the \
                resampler included.",
        measured: if leaked == 0.0 {
            "right channel silent".into()
        } else {
            format!(
                "right channel reaches {:.1} dBFS",
                measure::db(f64::from(leaked))
            )
        },
        limit: "right channel silent",
        passed: leaked == 0.0,
    }
}

pub fn loudness_cut_is_exact_and_clean() -> Check {
    // A track at -8 LUFS brought to -14.
    let cut_db = -6.0;
    let setup = Setup {
        loudness: 10f32.powf(cut_db / 20.0),
        ..Setup::untouched(OPUS, OPUS)
    };
    let tone = tone_through(setup, 1000.0);
    let error = tone.level_db() - measure::db(LEVEL) - f64::from(cut_db);
    Check {
        name: "Loudness cut is exact and clean",
        about: "Normalisation turns a loud track down. It should do that and \
                nothing else.",
        measured: format!("off by {error:+.4} dB, THD+N {:.1} dB", tone.thd_n_db()),
        limit: "within ±0.01 dB, THD+N at most -100 dB",
        passed: error.abs() <= 0.01 && tone.thd_n_db() <= CLEAN_DB,
    }
}

pub fn lower_volume_is_clean() -> Check {
    let setup = Setup {
        volume: 0.5,
        ..Setup::untouched(OPUS, OPUS)
    };
    let tone = tone_through(setup, 1000.0);
    Check {
        name: "Volume below 100% is clean",
        about: "The volume is a multiplication in 32-bit float: quieter, \
                nothing added.",
        measured: format!("THD+N {:.1} dB", tone.thd_n_db()),
        limit: "at most -100 dB",
        passed: tone.thd_n_db() <= CLEAN_DB,
    }
}

pub fn equalizer_is_clean() -> Check {
    let mut gains = [0.0; 10];
    gains[5] = 6.0;
    let setup = Setup {
        equalizer: eq::Settings {
            enabled: true,
            gains,
            preamp: 0.0,
            headroom: true,
            ..eq::Settings::default()
        },
        ..Setup::untouched(OPUS, OPUS)
    };
    let (worst, measured) = distortion(setup);
    Check {
        name: "Equalizer adds no distortion",
        about: "With a band raised, the filters change the level of each \
                frequency and should add nothing besides.",
        measured: format!("{measured}, with +6 dB at 1 kHz"),
        limit: "at most -100 dB",
        passed: worst <= CLEAN_DB,
    }
}

pub fn overs_are_held_at_full_scale() -> Check {
    // Music under full scale, with a burst over it at one second, as loud
    // masters decode.
    let mut input = signal::noise(OPUS, 0.5, 3.0);
    let at = OPUS as usize * 2;
    for (index, sample) in input[at..at + 40].iter_mut().enumerate() {
        *sample = if index % 2 == 0 { 1.1 } else { -1.1 };
    }
    let output = Chain::new(Setup::untouched(OPUS, OPUS)).process(&input);
    let peak = measure::peak(&output);
    // Untouched until the limiter starts to see it coming.
    let lead = (LOOKAHEAD + 1) * 2;
    let before = measure::null(&input[..at - lead], &output[..at - lead]);
    // The last sample changed: where the limiter has let go.
    let last = input
        .iter()
        .zip(&output)
        .rposition(|(a, b)| a.to_bits() != b.to_bits())
        .unwrap_or(at);
    let held_ms = (last - at) / 2 * 1000 / OPUS as usize;
    Check {
        name: "Peaks over full scale are held, not clipped",
        about: "Decoded loud masters go over full scale. They should be brought \
                down smoothly, and the music away from them left alone.",
        measured: format!(
            "peak {:+.2} dBFS; untouched before, and from {held_ms} ms after",
            measure::db(f64::from(peak))
        ),
        limit: "peak at most 0 dBFS, untouched until 2 ms before and within a second after",
        passed: peak <= 1.0 && before.exact() && held_ms < 1000,
    }
}

pub fn quiet_detail_survives_16_bit() -> Check {
    // A tone at 0.6 of a 16-bit step: below the last step there is.
    let amplitude = 0.6 / 32_768.0;
    let input = measure::channel(&signal::tone(OPUS, 1000.0, amplitude, 2.0), 0);
    let mut dither = Dither::new();
    let dithered: Vec<f32> = input.iter().map(|&s| dither.quantize_16_bit(s)).collect();
    // What cpal's own conversion makes of it.
    let cut: Vec<f32> = input
        .iter()
        .map(|&s| f32::from((s * 32_768.0) as i16) / 32_768.0)
        .collect();
    let kept = measure::fit(&dithered, OPUS, 1000.0).amplitude / amplitude;
    let kept_cut = measure::fit(&cut, OPUS, 1000.0).amplitude / amplitude;
    Check {
        name: "Quiet detail survives a 16-bit device",
        about: "Below the last 16-bit step, dither keeps a tone audible in \
                hiss where cutting to the step leaves silence.",
        measured: format!(
            "tone kept at {:.0}% with dither, {:.0}% cut without",
            kept * 100.0,
            kept_cut * 100.0
        ),
        limit: "kept within 10%",
        passed: (kept - 1.0).abs() < 0.1,
    }
}

pub fn limiting_is_clean() -> Check {
    // Tones 1 dB over full scale, low and middle: the limiter has to hold
    // them down for as long as they last.
    let over = 10f64.powf(1.0 / 20.0);
    let measured: Vec<(f64, f64)> = [50.0, 1000.0]
        .iter()
        .map(|&hz| {
            let input = signal::tone(OPUS, hz, over, 2.0);
            let output = Chain::new(Setup::untouched(OPUS, OPUS)).process(&input);
            // Past the first half second, once it has taken hold.
            let left = measure::channel(&output[OPUS as usize..], 0);
            (hz, measure::fit(&left, OPUS, hz).thd_n_db())
        })
        .collect();
    let worst = measured
        .iter()
        .map(|&(_, thd)| thd)
        .fold(f64::NEG_INFINITY, f64::max);
    Check {
        name: "Limiting adds little distortion",
        about: "Held down, a tone over full scale should come out a quieter \
                tone, not one with the limiter's pumping on it.",
        measured: format!(
            "THD+N {}",
            measured
                .iter()
                .map(|(hz, thd)| format!("{thd:.1} dB at {hz} Hz"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        limit: "at most -80 dB",
        passed: worst <= -80.0,
    }
}
