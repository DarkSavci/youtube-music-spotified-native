//! The Sound quality card: the presets and headphone correction.

use std::sync::Arc;

use eframe::egui::Event;
use eframe::egui::accesskit::Role;

use super::*;
use crate::headphones;
use crate::settings::SoundPreset;

/// The Settings page, scrolled to the card.
fn card(mut state: State) -> Harness<'static, Fixture> {
    state.nav.open(Page::Settings);
    state.held_scroll = Some(1000.0);
    harness(state)
}

const LIST: &str = "- [Sennheiser HD 600](./oratory1990/over-ear/Sennheiser%20HD%20600) by oratory1990\n\
    - [Sennheiser HD 600](./crinacle/GRAS%2043AG-7%20over-ear/Sennheiser%20HD%20600) by crinacle on GRAS 43AG-7\n";

#[test]
fn typing_a_headphone_asks_for_the_list_and_a_match_is_chosen_by_a_click() {
    let mut harness = card(state());
    harness.get_by_label("Search headphones").focus();
    harness.run();
    harness.event(Event::Text("hd 600".into()));
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SearchHeadphones(query) if query == "hd 600"
    )));

    let mut state = state();
    state.headphones.query = "hd 600".into();
    state.headphones.list = Some(Ok(Arc::new(headphones::parse_index(LIST))));
    let mut harness = card(state);
    // The preferred reviewer's measurement first.
    harness
        .get_by_label("Sennheiser HD 600  \u{00b7}  oratory1990")
        .click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ChooseHeadphones(chosen) if chosen.source == "oratory1990"
    )));
}

#[test]
fn a_chosen_correction_can_be_switched_off_and_a_preset_chosen() {
    let mut state = state();
    state.settings.headphones = Some(headphones::Correction {
        name: "Sennheiser HD 600".into(),
        source: "oratory1990".into(),
        filters: Vec::new(),
    });
    let mut harness = card(state);
    harness
        .get_by_role_and_label(Role::CheckBox, "Headphone correction")
        .click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetHeadphonesOn(false)
    )));
    harness.get_by_label("Sound preset").click();
    harness.run();
    harness.get_by_label("Best quality").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetSoundPreset(SoundPreset::BestQuality)
    )));
}
