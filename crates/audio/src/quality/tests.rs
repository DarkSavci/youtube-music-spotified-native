//! The engine held to the suite, one test a check, so a failure names
//! what broke. Plus checks on the measuring itself: a ruler that is wrong
//! passes everything.

use super::checks::{self, Check};
use super::measure;
use super::signal;

fn holds(check: Check) {
    assert!(
        check.passed,
        "{}: {} (limit: {})\n{}",
        check.name, check.measured, check.limit, check.about
    );
}

#[test]
fn correction_is_clean() {
    holds(checks::correction_is_clean());
}

#[test]
fn correction_is_what_is_drawn() {
    holds(checks::correction_is_what_is_drawn());
}

#[test]
fn untouched_is_bit_exact() {
    holds(checks::untouched_is_bit_exact());
}

#[test]
fn full_scale_peaks_pass_untouched() {
    holds(checks::full_scale_peaks_pass_untouched());
}

#[test]
fn full_volume_is_unity() {
    holds(checks::full_volume_is_unity());
}

#[test]
fn equalizer_off_leaves_peaks_alone() {
    holds(checks::equalizer_off_leaves_peaks_alone());
}

#[test]
fn aac_to_48k_is_flat() {
    holds(checks::aac_to_48k_is_flat());
}

#[test]
fn aac_to_48k_is_clean() {
    holds(checks::aac_to_48k_is_clean());
}

#[test]
fn opus_to_44k_is_flat() {
    holds(checks::opus_to_44k_is_flat());
}

#[test]
fn opus_to_44k_is_clean() {
    holds(checks::opus_to_44k_is_clean());
}

#[test]
fn channels_stay_apart() {
    holds(checks::channels_stay_apart());
}

#[test]
fn loudness_cut_is_exact_and_clean() {
    holds(checks::loudness_cut_is_exact_and_clean());
}

#[test]
fn lower_volume_is_clean() {
    holds(checks::lower_volume_is_clean());
}

#[test]
fn equalizer_is_clean() {
    holds(checks::equalizer_is_clean());
}

#[test]
fn overs_are_held_at_full_scale() {
    holds(checks::overs_are_held_at_full_scale());
}

#[test]
fn quiet_detail_survives_16_bit() {
    holds(checks::quiet_detail_survives_16_bit());
}

#[test]
fn limiting_is_clean() {
    holds(checks::limiting_is_clean());
}

#[test]
fn the_fit_finds_a_tones_level_and_a_pure_tone_is_clean() {
    let samples = measure::channel(&signal::tone(48_000, 997.0, 0.5, 0.5), 0);
    let tone = measure::fit(&samples, 48_000, 997.0);
    assert!((tone.amplitude - 0.5).abs() < 1e-6, "{}", tone.amplitude);
    // Only f32 rounding is left: about -150 dB.
    assert!(tone.thd_n_db() < -130.0, "{}", tone.thd_n_db());
}

#[test]
fn the_fit_sees_distortion_at_the_level_it_is() {
    // A 1% second harmonic is THD of -40 dB.
    let samples: Vec<f32> = (0..48_000)
        .map(|n| {
            let t = 2.0 * std::f64::consts::PI * 1000.0 * f64::from(n) / 48_000.0;
            (0.5 * t.sin() + 0.005 * (2.0 * t).sin()) as f32
        })
        .collect();
    let thd = measure::fit(&samples, 48_000, 1000.0).thd_n_db();
    assert!((thd + 40.0).abs() < 0.1, "{thd}");
}

#[test]
fn a_null_counts_every_changed_sample_and_only_those() {
    let a = [0.1, 0.2, 0.3, 0.4];
    let b = [0.1, 0.25, 0.3, 0.0];
    let null = measure::null(&a, &b);
    assert_eq!(null.differing, 2);
    assert!((null.largest - 0.4).abs() < 1e-6);
    assert!(measure::null(&a, &a).exact());
}

#[test]
fn noise_is_the_same_each_time_and_within_its_peak() {
    let first = signal::noise(48_000, 0.999, 0.1);
    assert_eq!(first, signal::noise(48_000, 0.999, 0.1));
    let peak = measure::peak(&first);
    assert!(peak <= 0.999 && peak > 0.99, "{peak}");
}
