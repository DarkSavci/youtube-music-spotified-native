//! The player bar, the mini player, the queue and the lyrics.

use super::*;

#[test]
fn the_play_button_pauses_what_is_playing() {
    let mut harness = harness(playing(state()));
    harness.get_by_label("Pause").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
}

#[test]
fn the_transport_is_inert_with_nothing_queued() {
    let mut harness = harness(state());
    harness.get_by_label("Play").click();
    harness.run();
    assert!(harness.state().actions.is_empty());
}

#[test]
fn the_big_button_pauses_the_playlist_that_is_playing() {
    let mut state = playing(on_playlist());
    if let Some(playback) = &mut state.playback {
        playback.session.queue.origin = "Road trip".into();
    }
    let mut harness = harness(state);
    // Two controls now say "Pause": this one and the player bar's.
    let buttons: Vec<_> = harness.get_all_by_label("Pause").collect();
    assert_eq!(buttons.len(), 2);
    buttons[0].click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
}

#[test]
fn clicking_a_timed_lyric_seeks_to_it() {
    use spotified_client::models::{LyricLine, Lyrics};

    let mut state = playing(state());
    state.settings.panel = crate::settings::RightPanel::Lyrics;
    state.lyrics = crate::state::TrackLyrics {
        track_id: "a".into(),
        words: Loadable::Loaded(Some(Lyrics {
            synced: true,
            lines: vec![
                LyricLine {
                    at_ms: 1000,
                    text: "The first line".into(),
                },
                LyricLine {
                    at_ms: 42_000,
                    text: "A later line".into(),
                },
            ],
            ..Lyrics::default()
        })),
    };
    let mut harness = harness(state);
    harness.get_by_label("A later line").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Seek(42_000)
    )));
}

#[test]
fn the_lyrics_panel_can_take_the_whole_window_and_give_it_back() {
    let mut state = playing(state());
    state.settings.panel = crate::settings::RightPanel::Lyrics;
    // Settled, not loading: a spinner would keep the window redrawing.
    state.lyrics.words = Loadable::Loaded(None);
    let mut harness = harness(state);
    harness.get_by_label("Full screen").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetLyricsFullscreen(true)
    )));

    let mut state = playing(self::state());
    state.lyrics_fullscreen = true;
    state.lyrics.words = Loadable::Loaded(None);
    let mut harness = self::harness(state);
    harness.get_by_label("Leave full screen").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetLyricsFullscreen(false)
    )));
}

#[test]
fn the_mini_player_steers_playback_and_leads_back() {
    // Tall: every control is on show, with no pointer needed to bring it.
    let mut harness = mini_harness(playing(on_playlist()), vec2(360.0, 580.0));
    harness.get_by_label("Pause").click();
    harness.get_by_label("Next").click();
    harness.get_by_label("Queue").click();
    harness.get_by_label("Open app").click();
    harness.get_by_label("Close mini player").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
    assert!(asked(&harness, |action| matches!(action, Action::Next)));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetMiniPanel(crate::state::MiniPanel::Queue)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ShowMainWindow
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleMiniPlayer
    )));
}

#[test]
fn a_mini_player_as_a_strip_keeps_the_transport() {
    let mut harness = mini_harness(playing(on_playlist()), vec2(420.0, 80.0));
    harness.get_by_label("Previous").click();
    harness.get_by_label("Pause").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::Previous)));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
    // No room on a strip for the queue's button.
    assert!(harness.query_by_label("Queue").is_none());
}

#[test]
fn a_tall_mini_player_has_the_speed_beside_its_window_buttons_with_the_whole_panel() {
    let mut harness = mini_harness(playing(on_playlist()), vec2(360.0, 580.0));
    let speed = harness.get_by_label("Playback speed: 1×").rect();
    let pin = harness.get_by_label_contains("on top: click").rect();
    assert!(speed.right() <= pin.left() + 1.0);
    assert!((speed.center().y - pin.center().y).abs() < 1.0);

    harness.get_by_label("Playback speed: 1×").click();
    harness.run();
    // The whole panel: the slider is there, with its steps.
    assert!(harness.query_all_by_label("Playback speed").count() > 0);
    harness.get_by_label("1.5").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetSpeed(speed) if *speed == 1.5
    )));
}

#[test]
fn a_strip_of_a_mini_player_opens_the_speed_as_one_row_of_presets() {
    let mut harness = mini_harness(playing(on_playlist()), vec2(560.0, 80.0));
    harness.get_by_label("Playback speed: 1×").click();
    harness.run();
    // No room for the slider: the presets between a step down and a step up.
    assert!(harness.query_by_label("Playback speed").is_none());
    let (slower, faster) = (
        harness.get_by_label("Slower").rect(),
        harness.get_by_label("Faster").rect(),
    );
    let preset = harness.get_by_label("1.25").rect();
    assert!(slower.right() < preset.left() && preset.right() < faster.left());
    assert!((slower.center().y - preset.center().y).abs() < 1.0);
    // And all of it inside the window.
    assert!(slower.left() >= 0.0 && faster.right() <= 560.0);
    harness.get_by_label("Faster").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetSpeed(speed) if (*speed - 1.05).abs() < 1e-4
    )));
}

#[test]
fn a_strip_too_narrow_for_it_leaves_the_speed_out() {
    let harness = mini_harness(playing(on_playlist()), vec2(480.0, 80.0));
    assert!(harness.query_by_label("Playback speed: 1×").is_none());
    // The transport stays.
    assert!(harness.query_by_label("Pause").is_some());
}

#[test]
fn the_player_bar_leads_to_the_mini_player() {
    let mut harness = harness(playing(on_playlist()));
    harness.get_by_label("Mini player").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleMiniPlayer
    )));
}
