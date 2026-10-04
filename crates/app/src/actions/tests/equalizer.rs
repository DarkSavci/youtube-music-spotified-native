//! The equalizer: what its panel asks for, what is heard and what is kept.

use super::*;
use crate::equalizer::{Ask, Named, PRESETS, Saved, named};

const HEARD: [Effect; 2] = [Effect::SaveSettings, Effect::ApplyEqualizer];

fn ask(state: &mut State, ask: Ask) -> Vec<Effect> {
    apply(state, Action::Equalizer(ask))
}

fn last_toast(state: &State) -> Option<&str> {
    state.toasts.last().map(|toast| toast.text.as_str())
}

#[test]
fn moving_a_band_switches_it_on_and_holds_the_range_and_the_step() {
    let mut state = ready();
    // Heard at once, and not written down until the hand lets go.
    assert_eq!(
        ask(&mut state, Ask::Band(0, 40.0)),
        [Effect::ApplyEqualizer]
    );
    assert!(state.settings.equalizer_on);
    assert_eq!(state.settings.equalizer[0], 12.0);
    ask(&mut state, Ask::Band(3, -4.3));
    assert_eq!(state.settings.equalizer[3], -4.5);
    assert_eq!(ask(&mut state, Ask::Keep), [Effect::SaveSettings]);
    assert!(ask(&mut state, Ask::Band(99, 1.0)).is_empty());
}

#[test]
fn the_preamp_is_a_slider_like_the_bands() {
    let mut state = ready();
    assert_eq!(ask(&mut state, Ask::Preamp(-3.2)), [Effect::ApplyEqualizer]);
    assert_eq!(state.settings.equalizer_preamp, -3.0);
    assert!(state.settings.equalizer_on);
    ask(&mut state, Ask::Preamp(99.0));
    assert_eq!(state.settings.equalizer_preamp, 12.0);
}

#[test]
fn choosing_a_preset_sets_every_band_and_moving_one_makes_it_custom() {
    let mut state = ready();
    let (name, rock) = PRESETS[5];
    assert_eq!(ask(&mut state, Ask::Curve(rock)), HEARD);
    assert!(state.settings.equalizer_on);
    assert_eq!(named(&state.settings).label(), name);
    ask(&mut state, Ask::Band(2, 6.0));
    assert_eq!(named(&state.settings), Named::Custom);
    // Moved back, it is the preset again.
    ask(&mut state, Ask::Band(2, rock[2]));
    assert_eq!(named(&state.settings).label(), "Rock");
}

#[test]
fn the_switch_and_the_headroom_are_kept_and_heard() {
    let mut state = ready();
    assert_eq!(ask(&mut state, Ask::On(true)), HEARD);
    assert!(state.settings.equalizer_on);
    assert_eq!(ask(&mut state, Ask::Headroom(false)), HEARD);
    assert!(!state.settings.equalizer_headroom);
}

#[test]
fn a_reset_flattens_the_sliders_and_leaves_the_switch_and_the_saved_curves() {
    let mut state = ready();
    ask(&mut state, Ask::Curve(PRESETS[1].1));
    ask(&mut state, Ask::Preamp(-4.0));
    ask(&mut state, Ask::Name);
    ask(&mut state, Ask::Typed("Car".into()));
    ask(&mut state, Ask::Save);
    assert_eq!(ask(&mut state, Ask::Reset), HEARD);
    assert_eq!(state.settings.equalizer, [0.0; 10]);
    assert_eq!(state.settings.equalizer_preamp, 0.0);
    assert!(state.settings.equalizer_on);
    assert_eq!(state.settings.equalizer_presets.len(), 1);
}

#[test]
fn a_curve_is_saved_under_the_name_typed_for_it() {
    let mut state = ready();
    ask(&mut state, Ask::Band(0, 3.0));
    assert!(ask(&mut state, Ask::Name).is_empty());
    assert!(state.equalizer_naming.is_some());
    // Nothing typed yet: the field stays for a name.
    assert!(ask(&mut state, Ask::Save).is_empty());
    assert!(state.equalizer_naming.is_some());
    ask(&mut state, Ask::Typed("  In the car ".into()));
    assert_eq!(ask(&mut state, Ask::Save), [Effect::SaveSettings]);
    assert!(state.equalizer_naming.is_none());
    let saved = &state.settings.equalizer_presets;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].name, "In the car");
    assert_eq!(saved[0].gains[0], 3.0);
    assert_eq!(named(&state.settings), Named::Saved("In the car"));
    assert_eq!(last_toast(&state), Some("Saved “In the car”"));
}

#[test]
fn a_built_in_presets_name_cannot_be_taken() {
    let mut state = ready();
    ask(&mut state, Ask::Band(0, 3.0));
    ask(&mut state, Ask::Name);
    ask(&mut state, Ask::Typed("rock".into()));
    assert!(ask(&mut state, Ask::Save).is_empty());
    assert!(state.settings.equalizer_presets.is_empty());
    // The field stays, for another name.
    assert!(state.equalizer_naming.is_some());
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
    ask(&mut state, Ask::Cancel);
    assert!(state.equalizer_naming.is_none());
}

#[test]
fn saving_under_a_name_already_used_changes_that_curve() {
    let mut state = ready();
    state.settings.equalizer_presets.push(Saved {
        name: "Car".into(),
        gains: [1.0; 10],
    });
    ask(&mut state, Ask::Band(4, -6.0));
    ask(&mut state, Ask::Name);
    ask(&mut state, Ask::Typed("Car".into()));
    ask(&mut state, Ask::Save);
    let saved = &state.settings.equalizer_presets;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].gains[4], -6.0);
}

#[test]
fn a_saved_curve_is_renamed_but_not_onto_another() {
    let mut state = ready();
    for name in ["Car", "Kitchen"] {
        state.settings.equalizer_presets.push(Saved {
            name: name.into(),
            gains: [1.0; 10],
        });
    }
    ask(&mut state, Ask::Rename("Car".into()));
    let naming = state.equalizer_naming.as_ref();
    assert_eq!(naming.map(|naming| naming.name.as_str()), Some("Car"));
    ask(&mut state, Ask::Typed("Kitchen".into()));
    assert!(ask(&mut state, Ask::Save).is_empty());
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
    ask(&mut state, Ask::Typed("Van".into()));
    assert_eq!(ask(&mut state, Ask::Save), [Effect::SaveSettings]);
    let names: Vec<_> = state
        .settings
        .equalizer_presets
        .iter()
        .map(|saved| saved.name.as_str())
        .collect();
    assert_eq!(names, ["Van", "Kitchen"]);
}

#[test]
fn deleting_a_saved_curve_leaves_the_sliders_where_they_are() {
    let mut state = ready();
    let curve = [2.0; 10];
    state.settings.equalizer = curve;
    state.settings.equalizer_presets.push(Saved {
        name: "Car".into(),
        gains: curve,
    });
    assert_eq!(
        ask(&mut state, Ask::Delete("Car".into())),
        [Effect::SaveSettings]
    );
    assert!(state.settings.equalizer_presets.is_empty());
    assert_eq!(state.settings.equalizer, curve);
    assert_eq!(named(&state.settings), Named::Custom);
    assert!(ask(&mut state, Ask::Delete("Car".into())).is_empty());
}

#[test]
fn resetting_the_preferences_keeps_the_curves_someone_saved() {
    let mut state = ready();
    state.settings.equalizer_presets.push(Saved {
        name: "Car".into(),
        gains: [1.0; 10],
    });
    ask(&mut state, Ask::Curve(PRESETS[1].1));
    ask(&mut state, Ask::Headroom(false));
    apply(&mut state, Action::ResetPreferences);
    assert!(!state.settings.equalizer_on);
    assert_eq!(state.settings.equalizer, [0.0; 10]);
    assert!(state.settings.equalizer_headroom);
    assert_eq!(state.settings.equalizer_presets.len(), 1);
}
