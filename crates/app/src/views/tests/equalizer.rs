//! The equalizer's panel: the switch, the presets, the sliders, and the
//! curves saved under a name.

use eframe::egui::{Event, Key, Modifiers, MouseWheelUnit, TouchPhase};

use super::*;
use crate::equalizer::{Ask, Naming, PRESETS, Saved};

/// Everything the panel asked for, in order.
fn asks(harness: &Harness<'_, Fixture>) -> Vec<Ask> {
    let actions = harness.state().actions.iter();
    actions
        .filter_map(|action| match action {
            Action::Equalizer(ask) => Some(ask.clone()),
            _ => None,
        })
        .collect()
}

/// The panel, opened over the page from the player bar's button.
fn opened(state: State) -> Harness<'static, Fixture> {
    let mut harness = harness(state);
    harness.get_by_label("Equalizer").click();
    harness.run();
    harness
}

fn rock() -> State {
    let mut state = state();
    state.settings.equalizer_on = true;
    state.settings.equalizer = PRESETS[5].1;
    state
}

fn wheel(harness: &mut Harness<'_, Fixture>, notches: f32) {
    harness.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, notches),
        phase: TouchPhase::Move,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

#[test]
fn the_player_bars_button_opens_the_panel_and_its_switch_switches() {
    let mut harness = opened(state());
    assert!(harness.query_by_label("Equalizer preset: Flat").is_some());
    harness.get_by_label("Equalizer on").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::On(true)]);
}

#[test]
fn the_queues_heading_opens_the_same_panel() {
    let mut state = state();
    state.settings.panel = crate::settings::RightPanel::Queue;
    let mut harness = harness(state);
    harness.get_by_label("Queue equalizer").click();
    harness.run();
    harness.get_by_label("Jazz").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Curve(PRESETS[7].1)]);
}

#[test]
fn settings_shows_the_panel_too() {
    let mut state = state();
    state.nav.open(Page::Settings);
    state.held_scroll = Some(860.0);
    let mut harness = harness(state);
    harness.get_by_label("Equalizer on").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::On(true)]);
}

#[test]
fn choosing_a_preset_asks_for_its_curve() {
    let mut harness = opened(state());
    harness.get_by_label("Rock").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Curve(PRESETS[5].1)]);
    // The one already set is not asked for again.
    let mut harness = opened(rock());
    assert!(harness.query_by_label("Equalizer preset: Rock").is_some());
    harness.get_by_label("Rock").click();
    harness.run();
    assert!(asks(&harness).is_empty());
}

#[test]
fn a_band_moved_off_its_preset_is_called_custom_and_can_be_saved() {
    let mut state = rock();
    state.settings.equalizer[2] = -3.0;
    let mut harness = opened(state);
    assert!(harness.query_by_label("Equalizer preset: Custom").is_some());
    harness
        .get_by_label("Save as preset equalizer preset")
        .click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Name]);
    // A built-in curve has nothing to save.
    let harness = opened(rock());
    let save = harness.query_by_label("Save as preset equalizer preset");
    assert!(save.is_none());
}

#[test]
fn pressing_in_a_bands_column_sets_it_there_and_writes_it_down_once_let_go() {
    let mut harness = opened(state());
    let column = harness.get_by_label("1 kHz").rect();
    // The top of a slider's travel, where it is +12 dB.
    let top = column.center_top() + vec2(0.0, 14.0);
    harness.hover_at(top);
    harness.drag_at(top);
    harness.run();
    assert_eq!(asks(&harness), [Ask::Band(5, 12.0)]);
    harness.drop_at(top);
    harness.run();
    assert_eq!(asks(&harness), [Ask::Band(5, 12.0), Ask::Keep]);
}

#[test]
fn a_right_click_puts_a_band_back_to_nought() {
    let mut harness = opened(rock());
    harness.get_by_label("31 Hz").click_secondary();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Band(0, 0.0), Ask::Keep]);
}

#[test]
fn the_wheel_over_a_slider_moves_it_a_step() {
    let mut harness = opened(rock());
    harness.get_by_label("16 kHz").hover();
    harness.run();
    wheel(&mut harness, -1.0);
    // Rock has 16 kHz at +4.
    assert_eq!(asks(&harness), [Ask::Band(9, 3.5), Ask::Keep]);
}

#[test]
fn on_the_settings_page_the_wheel_scrolls_the_page_and_moves_no_slider() {
    let mut state = rock();
    state.nav.open(Page::Settings);
    state.held_scroll = Some(860.0);
    let mut harness = harness(state);
    harness.get_by_label("1 kHz").hover();
    harness.run();
    wheel(&mut harness, -1.0);
    assert!(asks(&harness).is_empty());
}

#[test]
fn the_arrow_keys_move_the_slider_that_has_the_keyboard() {
    let mut harness = opened(state());
    harness.get_by_label("Preamp").focus();
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(asks(&harness), [Ask::Preamp(-0.5), Ask::Keep]);
}

#[test]
fn the_reset_asks_for_one_and_has_nothing_to_do_when_all_is_flat() {
    let mut harness = opened(rock());
    harness.get_by_label("Reset equalizer").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Reset]);
    let mut harness = opened(state());
    harness.get_by_label("Reset equalizer").click();
    harness.run();
    assert!(asks(&harness).is_empty());
}

#[test]
fn a_name_is_typed_and_saved() {
    let mut state = rock();
    state.settings.equalizer[2] = -3.0;
    state.equalizer_naming = Some(Naming::default());
    let mut harness = opened(state);
    // Nothing typed: there is nothing to save under.
    harness.get_by_label("Save equalizer preset").click();
    harness.run();
    assert!(asks(&harness).is_empty());
    super::together::type_into(&mut harness, "Preset name", "Car");
    assert_eq!(asks(&harness), [Ask::Typed("Car".into())]);

    let mut state = rock();
    state.equalizer_naming = Some(Naming {
        renaming: None,
        name: "Car".into(),
    });
    let mut harness = opened(state);
    harness.get_by_label("Save equalizer preset").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Save]);
}

#[test]
fn a_saved_curve_is_a_chip_that_can_be_renamed_and_deleted() {
    let gains = [2.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0];
    let saved = || {
        let mut state = state();
        state.settings.equalizer_presets.push(Saved {
            name: "Car".into(),
            gains,
        });
        state
    };
    let mut harness = opened(saved());
    harness.get_by_label("Car").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Curve(gains)]);
    // Only the curve that is set offers them.
    assert!(harness.query_by_label("Rename equalizer preset").is_none());

    let mut state = saved();
    state.settings.equalizer = gains;
    let mut harness = opened(state);
    assert!(harness.query_by_label("Equalizer preset: Car").is_some());
    harness.get_by_label("Rename equalizer preset").click();
    harness.run();
    harness.get_by_label("Delete equalizer preset").click();
    harness.run();
    let wanted = [Ask::Rename("Car".into()), Ask::Delete("Car".into())];
    assert_eq!(asks(&harness), wanted);
}

#[test]
fn the_headroom_switch_asks_for_the_other_setting() {
    let mut harness = opened(state());
    harness.get_by_label("Prevent clipping").click();
    harness.run();
    assert_eq!(asks(&harness), [Ask::Headroom(false)]);
}
