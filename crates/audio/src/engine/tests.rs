//! What the engine does about a target, checked without a device.

use super::*;

fn target(video: &str, preload: &str) -> Target {
    Target {
        epoch: 1,
        video_id: video.into(),
        preload_video_id: preload.into(),
        playing: true,
        volume: 1.0,
        ..Target::default()
    }
}

#[test]
fn nothing_to_play_stops_the_decks() {
    let decks = Decks {
        current: Some("a"),
        ..Decks::default()
    };
    assert_eq!(
        reconcile(decks, &target("a", ""), &Target::default()),
        [Op::Stop]
    );
}

#[test]
fn a_new_track_is_loaded_and_its_successor_preloaded() {
    let ops = reconcile(Decks::default(), &Target::default(), &target("a", "b"));
    assert_eq!(ops, [Op::Load { start_ms: 0 }, Op::Preload]);
}

#[test]
fn a_track_already_preloaded_is_promoted_not_loaded() {
    let decks = Decks {
        current: Some("a"),
        next: Some("b"),
        ..Decks::default()
    };
    assert_eq!(
        reconcile(decks, &target("a", "b"), &target("b", "c")),
        [Op::Promote, Op::Preload]
    );
}

#[test]
fn the_track_the_engine_moved_into_by_itself_is_left_playing() {
    // After a gapless change the deck already holds the new target.
    // The core was slow to say so: the track is already seconds in.
    let decks = Decks {
        current: Some("b"),
        position_ms: 2500,
        ..Decks::default()
    };
    let mut next = target("b", "");
    next.epoch = 2;
    assert!(reconcile(decks, &target("a", "b"), &next).is_empty());
}

#[test]
fn ordinary_drift_is_not_a_seek_but_a_moved_position_is() {
    let decks = Decks {
        current: Some("a"),
        position_ms: 61_000,
        ..Decks::default()
    };
    let mut near = target("a", "");
    near.start_at_ms = 60_000;
    assert!(reconcile(decks, &target("a", ""), &near).is_empty());
    let mut far = target("a", "");
    far.start_at_ms = 120_000;
    assert_eq!(
        reconcile(decks, &target("a", ""), &far),
        [Op::Seek(120_000)]
    );
}

#[test]
fn a_short_track_that_ended_starts_again_under_a_new_epoch() {
    let decks = Decks {
        current: Some("a"),
        position_ms: 900,
        ended: true,
        ..Decks::default()
    };
    let mut again = target("a", "");
    again.epoch = 2;
    assert_eq!(reconcile(decks, &target("a", ""), &again), [Op::Seek(0)]);
}

#[test]
fn a_preload_no_longer_wanted_is_dropped() {
    let decks = Decks {
        current: Some("a"),
        next: Some("b"),
        ..Decks::default()
    };
    let previous = target("a", "b");
    assert_eq!(
        reconcile(decks, &previous, &target("a", "")),
        [Op::DropPreload]
    );
    assert_eq!(
        reconcile(decks, &previous, &target("a", "c")),
        [Op::Preload]
    );
    assert!(reconcile(decks, &previous, &target("a", "b")).is_empty());
}

#[test]
fn the_volume_slider_is_tapered_and_boost_is_plain() {
    assert_eq!(gain_for(0.0), 0.0);
    assert_eq!(gain_for(1.0), 1.0);
    assert!(gain_for(0.5) < 0.35);
    assert_eq!(gain_for(1.5), 1.5);
    assert_eq!(gain_for(9.0), 2.0);
}

#[test]
fn a_fade_starts_its_own_length_before_the_end() {
    assert!(!crossfade_due(100_000, 200_000, 6000));
    assert!(crossfade_due(194_000, 200_000, 6000));
    assert!(crossfade_due(199_000, 200_000, 6000));
}

#[test]
fn there_is_no_fade_when_it_is_off_or_the_track_is_too_short_for_one() {
    assert!(!crossfade_due(199_000, 200_000, 0));
    assert!(!crossfade_due(9000, 10_000, 6000));
    // A stream that never said how long it is cannot be faded out of.
    assert!(!crossfade_due(199_000, 0, 6000));
}
