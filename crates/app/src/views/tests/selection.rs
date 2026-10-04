//! Text that can be selected and copied: the release notes, and the rest
//! of what is prose and not a row to click.

use eframe::egui::{self, OutputCommand};

use super::*;
use crate::state::Dialog;

/// What the last frame put on the clipboard.
fn copied(harness: &Harness<'_, Fixture>) -> Option<String> {
    let commands = &harness.output().platform_output.commands;
    commands.iter().find_map(|command| match command {
        OutputCommand::CopyText(text) => Some(text.clone()),
        _ => None,
    })
}

/// Drags across the label that says `text`, from its start to its end.
fn select(harness: &mut Harness<'_, Fixture>, text: &str) {
    let rect = harness.get_by_label(text).rect();
    let (from, to) = (rect.left_center(), rect.right_center());
    harness.drag_at(from + egui::vec2(1.0, 0.0));
    harness.step();
    harness.hover_at(rect.center());
    harness.step();
    harness.hover_at(to - egui::vec2(1.0, 0.0));
    harness.step();
    harness.drop_at(to - egui::vec2(1.0, 0.0));
    harness.run();
}

/// The number of the newest release: one line, and only once on screen.
fn newest() -> String {
    let releases = crate::changelog::releases();
    releases.first().expect("a release").version.clone()
}

#[test]
fn the_release_notes_over_the_page_can_be_selected_and_copied() {
    let mut state = state();
    state.dialog = Some(Dialog::WhatsNew);
    let mut harness = harness(state);
    let version = newest();
    select(&mut harness, &version);
    harness.event(egui::Event::Copy);
    harness.step();
    assert_eq!(copied(&harness).as_deref(), Some(version.as_str()));
}

#[test]
fn selecting_in_the_release_notes_leaves_the_dialog_to_be_closed_as_before() {
    let mut state = state();
    state.dialog = Some(Dialog::WhatsNew);
    let mut harness = harness(state);
    select(&mut harness, &newest());
    // A selection asks nothing of the app.
    assert!(harness.state().actions.is_empty());
    harness.get_by_label("Close release notes").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseDialog
    )));
}

#[test]
fn the_release_notes_page_can_be_selected_and_copied() {
    let mut state = state();
    state.nav.open(Page::Changelog);
    let mut harness = harness(state);
    let version = newest();
    select(&mut harness, &version);
    harness.event(egui::Event::Copy);
    harness.step();
    assert_eq!(copied(&harness).as_deref(), Some(version.as_str()));
}

#[test]
fn a_rows_title_is_still_a_row_and_not_text_to_select() {
    // Elsewhere a drag carries songs: nothing is selected, nothing copied.
    let mut harness = harness(on_playlist());
    let row = harness.get_by_label("First song").rect();
    harness.drag_at(row.left_center() + egui::vec2(80.0, 0.0));
    harness.step();
    harness.hover_at(row.center());
    harness.step();
    harness.drop_at(row.center());
    harness.run();
    harness.event(egui::Event::Copy);
    harness.step();
    assert_eq!(copied(&harness), None);
}
