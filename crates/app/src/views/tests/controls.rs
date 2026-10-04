//! The speed, the volume, the keys, the full-screen player and the notice.

use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, TouchPhase};
use egui_kittest::kittest::NodeT;

use super::*;
use crate::state::Notice;

#[test]
fn the_speed_button_opens_a_panel_whose_presets_set_the_speed() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Playback speed: 1×").click();
    harness.run();
    harness.get_by_label("1.5").click();
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::SetSpeed(speed) if *speed == 1.5)
    ));
}

#[test]
fn the_panels_steps_move_the_speed_one_step_either_way() {
    let mut state = playing(state());
    state.settings.playback_speed = 1.25;
    let mut harness = harness(state);
    harness.get_by_label("Playback speed: 1.25×").click();
    harness.run();
    harness.get_by_label("Faster").click();
    harness.run();
    harness.get_by_label("Slower").click();
    harness.run();
    let asked_for: Vec<f32> = harness
        .state()
        .actions
        .iter()
        .filter_map(|action| match action {
            Action::SetSpeed(speed) => Some(*speed),
            _ => None,
        })
        .collect();
    assert_eq!(asked_for, [1.3, 1.2]);
}

#[test]
fn in_a_room_the_speed_button_says_why_it_will_not_open() {
    let mut state = playing(state());
    state.settings.playback_speed = 2.0;
    state.together.phase = crate::together::Phase::Joined;
    let mut harness = harness(state);
    harness
        .get_by_label("Playback speed: 1× in Listen Together")
        .click();
    harness.run();
    // The ask is refused in `apply`, with a sentence; no panel opens.
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetSpeed(_)
    )));
    assert!(harness.query_by_label("Faster").is_none());
}

#[test]
fn the_time_at_the_end_of_the_bar_switches_to_what_is_left() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Show remaining time").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleRemainingTime
    )));
}

#[test]
fn the_wheel_over_the_volume_turns_it_up_and_down() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Mute").hover();
    harness.run();
    for notches in [1.0, -1.0] {
        harness.event(Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: vec2(0.0, notches),
            phase: TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        harness.run();
    }
    let steps: Vec<f32> = harness
        .state()
        .actions
        .iter()
        .filter_map(|action| match action {
            Action::VolumeBy(step) => Some(*step),
            _ => None,
        })
        .collect();
    assert_eq!(steps, [0.05, -0.05]);
}

#[test]
fn the_wheel_elsewhere_leaves_the_volume_alone() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Pause").hover();
    harness.run();
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::VolumeBy(_)
    )));
}

#[test]
fn with_boost_the_slider_runs_to_twice_full() {
    let mut state = playing(state());
    state.settings.volume_boost = true;
    let mut harness = harness(state);
    // The far end of the bar is 200%.
    let slider = harness.get_by_label("volume");
    let bounds = slider.rect();
    harness.hover_at(bounds.right_center() - vec2(1.0, 0.0));
    harness.drag_at(bounds.right_center() - vec2(1.0, 0.0));
    harness.drop_at(bounds.right_center() - vec2(1.0, 0.0));
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::SetVolume(level) if *level > 1.9)
    ));
}

fn press(harness: &mut Harness<'_, Fixture>, modifiers: Modifiers, key: Key) {
    harness.event(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    harness.run();
}

#[test]
fn the_old_keys_do_what_the_old_app_did_with_them() {
    let mut harness = harness(playing(state()));
    press(&mut harness, Modifiers::NONE, Key::F);
    press(&mut harness, Modifiers::NONE, Key::P);
    press(&mut harness, Modifiers::NONE, Key::ArrowRight);
    press(&mut harness, Modifiers::COMMAND, Key::S);
    press(&mut harness, Modifiers::COMMAND, Key::K);
    press(&mut harness, Modifiers::COMMAND, Key::Comma);
    press(&mut harness, Modifiers::COMMAND, Key::N);
    let actions = &harness.state().actions;
    assert!(matches!(
        &actions[..],
        [
            Action::ToggleFullscreenPlayer,
            Action::ToggleMiniPlayer,
            Action::SeekBy(5000),
            Action::SaveCurrent,
            Action::FocusSearch,
            Action::Open(Page::Settings),
            Action::NewPlaylist { name, .. },
        ] if name == "My playlist"
    ));
}

#[test]
fn no_shortcut_fires_while_a_field_is_being_typed_in() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Search music").focus();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::F);
    press(&mut harness, Modifiers::COMMAND, Key::ArrowRight);
    press(&mut harness, Modifiers::COMMAND, Key::Comma);
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::ToggleFullscreenPlayer | Action::Next | Action::Open(Page::Settings)
    )));
}

#[test]
fn asking_for_the_search_from_the_keyboard_puts_the_caret_in_it() {
    let mut state = state();
    state.search_focus = 1;
    let harness = harness(state);
    assert!(harness.get_by_label("Search music").is_focused());
}

fn fullscreen() -> State {
    let mut state = playing(state());
    state.fullscreen_player = true;
    if let Some(playback) = &mut state.playback {
        playback.session.queue.origin = "Road trip".into();
    }
    state
}

#[test]
fn the_full_screen_player_names_the_song_and_where_it_is_from() {
    let harness = harness(fullscreen());
    // The window is the player's alone: the top bar is not drawn.
    assert!(harness.query_by_label("Search music").is_none());
    for label in ["Pause", "Next", "Previous", "Shuffle", "Repeat", "Share"] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    assert!(harness.query_by_label("Playback speed: 1×").is_some());
    assert!(harness.query_by_label("Show remaining time").is_some());
}

#[test]
fn the_full_screen_player_is_left_by_its_button_or_by_escape() {
    let leaves = |action: &Action| matches!(action, Action::SetFullscreenPlayer(false));
    let mut harness = harness(fullscreen());
    harness.get_by_label("Exit full screen").click();
    harness.run();
    assert!(asked(&harness, leaves));

    let mut harness = self::harness(fullscreen());
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert!(asked(&harness, leaves));
}

#[test]
fn the_full_screen_player_closes_when_nothing_is_left_to_play() {
    let mut state = state();
    state.fullscreen_player = true;
    let harness = harness(state);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetFullscreenPlayer(false)
    )));
    // And the window is the app's again meanwhile.
    assert!(harness.query_by_label("Search music").is_some());
}

#[test]
fn a_notice_says_why_playback_stopped_and_can_be_dismissed() {
    let mut state = playing(state());
    state.notice = Some(Notice::RateLimited);
    let mut harness = harness(state);
    assert!(
        harness
            .query_by_label_contains("YouTube is rate-limiting this device")
            .is_some()
    );
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::DismissNotice
    )));
}

#[test]
fn a_screen_reader_is_told_what_starts_playing() {
    let mut state = playing(state());
    if let Some(playback) = &mut state.playback {
        playback.session.queue.items[0].artists = vec![spotified_client::models::ArtistRef {
            name: "The Band".into(),
            ..Default::default()
        }];
    }
    let harness = harness(state);
    let said = harness.get_by_label("Playing First song by The Band");
    assert_eq!(
        said.accesskit_node().live(),
        eframe::egui::accesskit::Live::Polite
    );
}

#[test]
fn the_new_settings_are_switches_on_the_page() {
    let mut state = state();
    state.nav.open(Page::Settings);
    let mut harness = harness(state);
    for (label, wanted) in [
        ("Gapless playback", "SetGapless(false)"),
        ("Autoplay", "SetAutoplay(false)"),
        ("Volume boost", "SetVolumeBoost(true)"),
        ("Resume on launch", "SetResumeOnLaunch(false)"),
        (
            "Continue from YouTube Music",
            "SetContinueFromYouTubeMusic(true)",
        ),
        ("Send listening to YouTube", "SetReportToYouTube(false)"),
    ] {
        // Some sit below the fold of the test window.
        switch(&harness, label).scroll_to_me();
        harness.run();
        switch(&harness, label).click();
        harness.run();
        let last = harness
            .state()
            .actions
            .last()
            .map(|action| format!("{action:?}"));
        assert_eq!(last.as_deref(), Some(wanted));
    }
}
