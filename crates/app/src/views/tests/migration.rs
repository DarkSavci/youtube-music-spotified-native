//! The move from the Electron app: the row in Settings, the choice, the
//! progress and the outcome.

use super::*;
use crate::migrate::{Found, History, Kind, Kinds, Line, OldAccount, OldPrefs, Progress, Songs};
use crate::state::{Dialog, Migration};

/// A signed-out app that has found the old one: two accounts, one of them
/// here already, with plays, songs and preferences.
fn found_old_app() -> State {
    let found = Found {
        accounts: vec![
            OldAccount {
                name: "Ada".into(),
                signed_in: true,
                history: History {
                    plays: 590,
                    first_play: "2026-09-22T10:26:25Z".into(),
                    last_play: "2026-10-04T07:32:15Z".into(),
                    pins: 2,
                    ..History::default()
                },
                ..OldAccount::default()
            },
            OldAccount {
                name: "Grace".into(),
                signed_in: true,
                here: Some("ours".into()),
                ..OldAccount::default()
            },
        ],
        songs: Songs {
            count: 302,
            bytes: 2_254_857_830,
            folders: Vec::new(),
        },
        prefs: OldPrefs {
            searches: vec!["duman".into(), "adamlar".into()],
            ..OldPrefs::default()
        },
        ..Found::default()
    };
    let mut state = state();
    state.migration = Migration {
        found: Some(found),
        ..Migration::default()
    };
    state.migration.suggest();
    state
}

fn choosing() -> State {
    let mut state = found_old_app();
    state.dialog = Some(Dialog::Migration);
    state
}

#[test]
fn settings_offers_the_move_when_the_old_app_is_there() {
    let mut on_settings = found_old_app();
    on_settings.nav.open(Page::Settings);
    let mut harness = harness(on_settings);
    harness.get_by_label_contains("2 accounts · 590 plays · 302 songs downloaded");
    harness.get_by_label("Bring over…").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::OpenMigration
    )));
}

#[test]
fn settings_says_nothing_of_an_old_app_that_is_not_there() {
    let mut on_settings = state();
    on_settings.nav.open(Page::Settings);
    let harness = harness(on_settings);
    assert!(harness.query_by_label("Bring over…").is_none());
    assert!(harness.query_by_label("Move from the old app").is_none());
}

#[test]
fn the_signed_out_library_points_to_the_old_app() {
    let mut harness = harness(found_old_app());
    harness.get_by_label("Move from the old app").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::OpenMigration
    )));
}

#[test]
fn the_choice_says_what_was_found_and_each_kind_can_be_unticked() {
    let mut harness = harness(choosing());
    harness.get_by_label_contains("Stay signed in as Ada, Grace (already here).");
    harness.get_by_label_contains("590 plays from 22 Sep 2026 to 4 Oct 2026 · 2 pins");
    harness.get_by_label_contains("302 songs (2.1 GB)");
    harness.get_by_label_contains("2 recent searches.");
    harness.get_by_label("Listening history").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetMigrationKind(Kind::History, false)
    )));
}

#[test]
fn an_unticked_kind_can_be_ticked_again() {
    let mut state = choosing();
    state.migration.choice.songs = false;
    let mut harness = harness(state);
    harness.get_by_label("Downloaded songs").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetMigrationKind(Kind::Songs, true)
    )));
}

#[test]
fn bring_it_over_goes_ahead_and_not_now_closes() {
    let mut harness = harness(choosing());
    harness.get_by_label("Bring it over").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartMigration
    )));
    harness.get_by_label("Not now").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseDialog
    )));
}

#[test]
fn with_nothing_ticked_there_is_nothing_to_go_ahead_with() {
    let mut state = choosing();
    state.migration.choice = Kinds::default();
    let mut harness = harness(state);
    harness.get_by_label("Bring it over").click();
    harness.run();
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::StartMigration
    )));
}

#[test]
fn a_run_under_way_shows_how_far_it_has_got_and_can_be_hidden() {
    let mut state = choosing();
    state.migration.running = Some(Progress {
        step: "Copying downloaded songs".into(),
        done: 120,
        total: 302,
    });
    let mut harness = harness(state);
    harness.get_by_label("Copying downloaded songs: 120 of 302");
    assert!(harness.query_by_label("Bring it over").is_none());
    harness.get_by_label("Hide").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseDialog
    )));
}

#[test]
fn the_outcome_lists_what_was_brought_and_what_was_left_and_why() {
    let mut state = choosing();
    state.migration.outcome = Some(vec![
        Line::brought("Ada: 590 plays brought."),
        Line::skipped("Grace is already signed in here."),
        Line::failed("3 songs could not be read."),
    ]);
    let mut harness = harness(state);
    harness.get_by_label("Brought over, in part");
    harness.get_by_label("Ada: 590 plays brought.");
    harness.get_by_label("Grace is already signed in here.");
    harness.get_by_label("3 songs could not be read.");
    harness.get_by_label("Bring more").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::OpenMigration
    )));
    harness.get_by_label("Done").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseDialog
    )));
}
