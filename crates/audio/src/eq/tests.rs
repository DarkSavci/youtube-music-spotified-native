use std::f32::consts::PI;

use super::limiter::CEILING;
use super::*;

const RATE: u32 = 48_000;
/// Long enough for the lowest band to have settled twice over.
const SECONDS: usize = 2;

fn tone(hz: f32, amplitude: f32) -> Vec<f32> {
    (0..RATE as usize * SECONDS)
        .flat_map(|frame| {
            let value = amplitude * (2.0 * PI * hz * frame as f32 / RATE as f32).sin();
            [value, value]
        })
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// The level of a tone after the equalizer, relative to before, in dB:
/// what is measured, not what is worked out.
fn measured_db(equalizer: &mut Equalizer, hz: f32) -> f32 {
    let mut samples = tone(hz, 0.05);
    let before = rms(&samples);
    equalizer.process(&mut samples);
    // The second half, past the change arriving and the filters settling.
    20.0 * (rms(&samples[samples.len() / 2..]) / before).log10()
}

fn with(gains: [f32; 10]) -> Equalizer {
    let mut equalizer = Equalizer::new(RATE);
    equalizer.set(Settings {
        enabled: true,
        gains,
        ..Settings::default()
    });
    equalizer
}

fn one_band(index: usize, db: f32) -> [f32; 10] {
    let mut gains = [0.0; 10];
    gains[index] = db;
    gains
}

/// The frequency half way between two neighbouring bands.
fn between(band: usize) -> f32 {
    (BANDS[band] * BANDS[band + 1]).sqrt()
}

#[test]
fn each_band_gives_its_own_frequency_what_its_slider_says() {
    for (band, hz) in BANDS.into_iter().enumerate() {
        for db in [6.0, -9.0, 12.0] {
            let heard = measured_db(&mut with(one_band(band, db)), hz);
            assert!((heard - db).abs() < 0.15, "{hz} Hz at {db} dB gave {heard}");
        }
    }
}

#[test]
fn a_raised_band_leaves_its_neighbours_centres_where_they_were() {
    let gains = one_band(5, 9.0); // 1 kHz
    for hz in [250.0, 500.0, 2000.0, 4000.0] {
        let heard = measured_db(&mut with(gains), hz);
        assert!(heard.abs() < 0.15, "{hz} Hz moved by {heard}");
    }
}

#[test]
fn every_slider_raised_alike_raises_everything_alike() {
    // Set straight from the sliders this came out near +9 dB; the gains
    // are solved for so that it is the +6 that was asked for.
    let mut equalizer = with([6.0; 10]);
    for hz in BANDS {
        let heard = measured_db(&mut equalizer, hz);
        assert!((heard - 6.0).abs() < 0.15, "{hz} Hz gave {heard}");
    }
    for band in 0..9 {
        let heard = measured_db(&mut equalizer, between(band));
        assert!(
            (heard - 6.0).abs() < 0.7,
            "{} Hz gave {heard}",
            between(band)
        );
    }
}

#[test]
fn a_curve_of_ups_and_downs_is_met_at_every_centre() {
    let gains = [5.0, 4.0, 2.0, -1.0, -2.0, 0.0, 2.0, 3.0, 4.0, 3.0];
    let mut equalizer = with(gains);
    for (hz, db) in BANDS.into_iter().zip(gains) {
        let heard = measured_db(&mut equalizer, hz);
        assert!((heard - db).abs() < 0.15, "{hz} Hz gave {heard}, not {db}");
    }
}

#[test]
fn the_curve_that_is_drawn_is_the_one_that_is_heard() {
    let settings = Settings {
        enabled: true,
        gains: [6.0, 3.0, -4.0, 0.0, 2.0, -6.0, 1.0, 8.0, -3.0, 5.0],
        ..Settings::default()
    };
    let shape = Design::new(RATE).shape(&settings);
    let mut equalizer = Equalizer::new(RATE);
    equalizer.set(settings);
    for hz in [
        25.0, 45.0, 90.0, 180.0, 700.0, 1400.0, 3000.0, 6000.0, 11_000.0, 19_000.0,
    ] {
        let heard = measured_db(&mut equalizer, hz);
        let drawn = shape.db_at(hz);
        assert!(
            (heard - drawn).abs() < 0.05,
            "{hz} Hz: heard {heard}, drawn {drawn}"
        );
    }
}

#[test]
fn flat_or_switched_off_it_is_out_of_the_path_and_changes_not_a_bit() {
    let original = tone(1000.0, 0.5);
    let mut off = Equalizer::new(RATE);
    off.set(Settings {
        enabled: false,
        gains: [12.0; 10],
        preamp: 6.0,
        headroom: true,
    });
    let mut flat = Equalizer::new(RATE);
    flat.set(Settings {
        enabled: true,
        headroom: true,
        ..Settings::default()
    });
    for mut equalizer in [off, flat] {
        assert!(equalizer.is_idle());
        let mut samples = original.clone();
        equalizer.process(&mut samples);
        assert_eq!(samples, original);
    }
}

#[test]
fn switched_off_while_playing_it_leaves_the_path_once_the_change_has_arrived() {
    let mut equalizer = with([8.0; 10]);
    let mut samples = tone(1000.0, 0.05);
    equalizer.process(&mut samples);
    assert!(!equalizer.is_idle());
    equalizer.set(Settings::default());
    let original = tone(1000.0, 0.05);
    let mut samples = original.clone();
    equalizer.process(&mut samples);
    assert!(equalizer.is_idle());
    let mut samples = original.clone();
    equalizer.process(&mut samples);
    assert_eq!(samples, original);
}

/// The biggest jump between one sample and the next.
fn steepest(samples: &[f32]) -> f32 {
    samples
        .as_chunks::<2>()
        .0
        .windows(2)
        .map(|pair| (pair[1][0] - pair[0][0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn a_change_glides_in_without_a_step() {
    // A 1 kHz tone at 0.05 moves by at most 0.0065 a sample; raised 12 dB,
    // four times that. A change that landed at once would jump by the
    // whole difference in level, some 0.15.
    let hz = 1000.0;
    let mut equalizer = with(one_band(5, -12.0));
    let mut samples = tone(hz, 0.05);
    let (first, rest) = samples.split_at_mut(RATE as usize);
    equalizer.process(first);
    // Mid-wave, where a step would show most.
    equalizer.set(Settings {
        enabled: true,
        gains: one_band(5, 12.0),
        preamp: -3.0,
        headroom: false,
    });
    equalizer.process(rest);
    let settled = steepest(&rest[rest.len() / 2..]);
    let during = steepest(&rest[..RATE as usize / 5]);
    assert!(
        during <= settled * 1.05,
        "jumped {during}, settles at {settled}"
    );
    // And switched off mid-wave, the same.
    let mut samples = tone(hz, 0.05);
    equalizer.set(Settings::default());
    equalizer.process(&mut samples);
    assert!(steepest(&samples) <= settled * 1.05);
}

#[test]
fn a_slider_dragged_through_many_small_changes_stays_smooth() {
    let mut equalizer = Equalizer::new(RATE);
    let mut samples = tone(250.0, 0.05);
    // A change every five milliseconds, each before the last has arrived.
    for (step, block) in samples.chunks_mut(480).enumerate() {
        let db = (step as f32 * 0.25).min(12.0);
        equalizer.set(Settings {
            enabled: true,
            gains: one_band(3, db),
            ..Settings::default()
        });
        equalizer.process(block);
    }
    let settled = steepest(&samples[samples.len() / 2..]);
    assert!(steepest(&samples) <= settled * 1.05);
    let heard = 20.0 * (rms(&samples[samples.len() / 2..]) / rms(&tone(250.0, 0.05))).log10();
    assert!((heard - 12.0).abs() < 0.15, "ended at {heard} dB");
}

#[test]
fn the_preamp_raises_and_lowers_everything() {
    for preamp in [-6.0, 4.0] {
        let mut equalizer = Equalizer::new(RATE);
        equalizer.set(Settings {
            enabled: true,
            preamp,
            ..Settings::default()
        });
        for hz in [62.0, 1000.0, 8000.0] {
            let heard = measured_db(&mut equalizer, hz);
            assert!((heard - preamp).abs() < 0.05, "{hz} Hz gave {heard}");
        }
    }
}

#[test]
fn with_headroom_kept_nothing_comes_out_louder_than_it_went_in() {
    let settings = Settings {
        enabled: true,
        gains: [8.0, 6.0, 4.0, 1.0, 0.0, 0.0, 1.0, 3.0, 5.0, 6.0],
        preamp: 3.0,
        headroom: true,
    };
    let shape = Design::new(RATE).shape(&settings);
    assert!((shape.peak_db - 8.0).abs() < 0.6, "peak {}", shape.peak_db);
    assert!((shape.gain_db + shape.peak_db).abs() < 1e-4);
    let mut equalizer = Equalizer::new(RATE);
    equalizer.set(settings);
    for hz in [20.0, 31.0, 45.0, 125.0, 1000.0, 9000.0, 16_000.0] {
        assert!(measured_db(&mut equalizer, hz) < 0.05);
    }
    // A curve that only cuts needs no headroom, and the preamp may then
    // raise it as far as the top of the curve allows.
    let cut = Settings {
        gains: [-6.0; 10],
        ..settings
    };
    let shape = Design::new(RATE).shape(&cut);
    assert!(shape.gain_db > 2.9, "gain {}", shape.gain_db);
}

#[test]
fn the_limiter_holds_peaks_under_the_ceiling() {
    let mut equalizer = with(one_band(5, 12.0));
    let mut samples = tone(1000.0, 0.9);
    equalizer.process(&mut samples);
    let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
    assert!(peak <= CEILING + 1e-4, "peak {peak}");
}

#[test]
fn silence_leaves_the_filters_with_nothing_to_grind_through() {
    let mut equalizer = with([6.0; 10]);
    equalizer.process(&mut tone(62.0, 0.5));
    for _ in 0..4 {
        equalizer.process(&mut vec![0.0; RATE as usize * 2]);
    }
    let memory = equalizer.bands.iter().flat_map(|band| band.memory);
    assert!(memory.flatten().all(|value| value == 0.0));
}

#[test]
fn input_that_is_not_a_number_does_not_stay_in_the_filters() {
    let mut equalizer = with([6.0; 10]);
    equalizer.process(&mut [f32::NAN; 64]);
    let mut samples = tone(1000.0, 0.05);
    equalizer.process(&mut samples);
    assert!(samples.iter().all(|sample| sample.is_finite()));
}

#[test]
fn a_device_too_slow_for_the_top_bands_goes_without_them() {
    // At 22.05 kHz there is nothing above 11 kHz to shape.
    let settings = Settings {
        enabled: true,
        gains: [6.0; 10],
        ..Settings::default()
    };
    let shape = Design::new(22_050).shape(&settings);
    assert!((shape.db_at(1000.0) - 6.0).abs() < 0.2);
    assert!(shape.db_at(10_000.0).is_finite());
    let mut equalizer = Equalizer::new(22_050);
    equalizer.set(settings);
    let mut samples = tone(1000.0, 0.05);
    equalizer.process(&mut samples);
    assert!(samples.iter().all(|sample| sample.is_finite()));
}

#[test]
fn sliders_set_hard_against_each_other_still_give_stable_filters() {
    let gains = [
        12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0,
    ];
    let mut equalizer = with(gains);
    let mut samples = tone(1000.0, 0.05);
    equalizer.process(&mut samples);
    assert!(samples.iter().all(|sample| sample.abs() < 1.0));
}
