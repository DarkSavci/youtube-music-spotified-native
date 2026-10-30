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
        position_ms: 60_800,
        ..Decks::default()
    };
    let mut near = target("a", "");
    near.seek_to_ms = 60_000;
    near.seek_seq = 1;
    assert!(reconcile(decks, &target("a", ""), &near).is_empty());
    let mut far = target("a", "");
    far.seek_to_ms = 120_000;
    far.seek_seq = 1;
    assert_eq!(
        reconcile(decks, &target("a", ""), &far),
        [Op::Seek(120_000)]
    );
}

#[test]
fn a_position_echoed_back_is_not_a_seek() {
    // Six seconds into the track, the core publishes the 0 it named it with.
    let decks = Decks {
        current: Some("b"),
        position_ms: 6000,
        ..Decks::default()
    };
    let mut named = target("b", "");
    named.epoch = 2;
    named.seek_seq = 3;
    assert!(reconcile(decks, &named, &named.clone()).is_empty());
    // Nor does a late report far from the deck count, however far.
    let mut late = named.clone();
    late.start_at_ms = 90_000;
    assert!(reconcile(decks, &named, &late).is_empty());
}

#[test]
fn a_seek_goes_where_it_named_though_a_later_report_says_otherwise() {
    // The projection that carried the seek was dropped, and a report from
    // before it has since overwritten the position.
    let decks = Decks {
        current: Some("a"),
        position_ms: 12_000,
        ..Decks::default()
    };
    let mut late = target("a", "");
    late.seek_seq = 4;
    late.seek_to_ms = 95_000;
    late.start_at_ms = 12_000;
    assert_eq!(
        reconcile(decks, &target("a", ""), &late),
        [Op::Seek(95_000)]
    );
}

#[test]
fn a_deck_with_nothing_loaded_starts_where_the_session_stands() {
    // The seek named is long past: the track has played on since.
    let mut going = target("a", "");
    going.seek_seq = 4;
    going.seek_to_ms = 30_000;
    going.start_at_ms = 150_000;
    assert_eq!(
        reconcile(Decks::default(), &Target::default(), &going),
        [Op::Load { start_ms: 150_000 }]
    );
}

#[test]
fn the_same_song_played_again_starts_again() {
    let decks = Decks {
        current: Some("a"),
        position_ms: 60_000,
        ..Decks::default()
    };
    let mut again = target("a", "");
    again.epoch = 2;
    again.seek_seq = 1;
    assert_eq!(reconcile(decks, &target("a", ""), &again), [Op::Seek(0)]);
}

#[test]
fn a_request_for_about_where_the_deck_is_moves_nothing() {
    // A room's follower is corrected only when it has really drifted.
    let decks = Decks {
        current: Some("a"),
        position_ms: 60_800,
        ..Decks::default()
    };
    let mut close = target("a", "");
    close.seek_to_ms = 60_000;
    close.seek_seq = 1;
    assert!(reconcile(decks, &target("a", ""), &close).is_empty());
}

#[test]
fn a_followers_real_drift_is_corrected_rather_than_asked_for_forever() {
    // The room corrects past a second of drift, so a deck that far out must move.
    let decks = Decks {
        current: Some("a"),
        position_ms: 61_200,
        ..Decks::default()
    };
    let mut drifted = target("a", "");
    drifted.seek_to_ms = 60_000;
    drifted.seek_seq = 1;
    assert_eq!(
        reconcile(decks, &target("a", ""), &drifted),
        [Op::Seek(60_000)]
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
    assert!(!crossfade_due(100_000, 200_000, 6000, 1.0));
    assert!(crossfade_due(194_000, 200_000, 6000, 1.0));
    assert!(crossfade_due(199_000, 200_000, 6000, 1.0));
}

#[test]
fn there_is_no_fade_when_it_is_off_or_the_track_is_too_short_for_one() {
    assert!(!crossfade_due(199_000, 200_000, 0, 1.0));
    assert!(!crossfade_due(9000, 10_000, 6000, 1.0));
    // A stream that never said how long it is cannot be faded out of.
    assert!(!crossfade_due(199_000, 0, 6000, 1.0));
}

#[test]
fn a_fade_is_as_long_in_the_hearing_at_any_speed() {
    // Six seconds of hearing is twelve of the track at double speed, and
    // three at half.
    assert!(!crossfade_due(187_000, 200_000, 6000, 2.0));
    assert!(crossfade_due(188_000, 200_000, 6000, 2.0));
    assert!(!crossfade_due(196_000, 200_000, 6000, 0.5));
    assert!(crossfade_due(197_000, 200_000, 6000, 0.5));
    // Twenty seconds holds a fade at each end at normal speed, and not at
    // double, where they would take twenty-four of the track between them.
    assert!(crossfade_due(19_000, 20_000, 6000, 1.0));
    assert!(!crossfade_due(19_000, 20_000, 6000, 2.0));
}

#[test]
fn a_promoted_track_far_into_its_target_is_sought_to() {
    let decks = Decks {
        current: Some("a"),
        next: Some("b"),
        ..Decks::default()
    };
    let mut resumed = target("b", "");
    resumed.start_at_ms = 90_000;
    assert_eq!(
        reconcile(decks, &target("a", "b"), &resumed),
        [Op::Promote, Op::Seek(90_000)]
    );
}

#[test]
fn a_paused_target_loads_its_track_all_the_same() {
    // A queue put in place paused (resumed at launch, picked up from
    // another device) is loaded where it stood, ready for play.
    let mut paused = target("a", "b");
    paused.playing = false;
    paused.start_at_ms = 42_000;
    assert_eq!(
        reconcile(Decks::default(), &Target::default(), &paused),
        [Op::Load { start_ms: 42_000 }, Op::Preload]
    );
}
