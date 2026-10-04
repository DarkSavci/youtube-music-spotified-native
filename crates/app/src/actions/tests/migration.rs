//! Moving in from the Electron app: the choice, the run, and what a run
//! leaves behind it.

use super::*;
use crate::migrate::{
    Found, History, Kind, Kinds, Line, Mark, OldAccount, OldPrefs, Outcome, Progress,
};
use crate::state::{Dialog, Loadable, Migration};

/// A profile with an account, some plays and a preference in it.
pub(crate) fn found() -> Found {
    Found {
        accounts: vec![OldAccount {
            id: "theirs".into(),
            name: "Ada".into(),
            signed_in: true,
            history: History {
                plays: 590,
                first_play: "2026-09-22T10:26:25Z".into(),
                last_play: "2026-10-04T07:32:15Z".into(),
                ..History::default()
            },
            ..OldAccount::default()
        }],
        prefs: OldPrefs {
            sidebar_width: Some(72.0),
            searches: vec!["duman".into()],
            ..OldPrefs::default()
        },
        ..Found::default()
    }
}

fn looked(state: &mut State, offer: bool) -> Vec<Effect> {
    apply(
        state,
        Action::MigrationFound {
            found: Some(Box::new(found())),
            brought: Kinds::default(),
            offer,
        },
    )
}

const NO_SONGS: Kinds = Kinds {
    sign_in: true,
    history: true,
    songs: false,
    preferences: true,
};

#[test]
fn a_profile_never_asked_is_offered_the_move_once_it_is_found() {
    let mut state = ready();
    assert!(looked(&mut state, true).is_empty());
    assert_eq!(state.dialog, Some(Dialog::Migration));
    // Everything there is, ticked; there are no songs to tick.
    assert_eq!(state.migration.choice, NO_SONGS);
}

#[test]
fn a_profile_that_was_asked_before_is_left_in_peace() {
    let mut state = ready();
    looked(&mut state, false);
    assert_eq!(state.dialog, None);
    assert!(state.migration.found.is_some());
    // It is still there to be asked for.
    apply(&mut state, Action::OpenMigration);
    assert_eq!(state.dialog, Some(Dialog::Migration));
}

#[test]
fn the_offer_does_not_push_aside_a_dialog_that_is_open() {
    let mut state = ready();
    state.dialog = Some(Dialog::WhatsNew);
    looked(&mut state, true);
    assert_eq!(state.dialog, Some(Dialog::WhatsNew));
}

#[test]
fn closing_the_offer_is_remembered() {
    let mut state = ready();
    looked(&mut state, true);
    assert_eq!(
        apply(&mut state, Action::CloseDialog),
        [Effect::MigrationSeen]
    );
    assert_eq!(state.dialog, None);
    // Any other dialog closes without a word.
    state.dialog = Some(Dialog::WhatsNew);
    assert!(apply(&mut state, Action::CloseDialog).is_empty());
}

#[test]
fn there_is_nothing_to_open_without_an_old_app() {
    let mut state = ready();
    apply(&mut state, Action::OpenMigration);
    assert_eq!(state.dialog, None);
    assert!(apply(&mut state, Action::StartMigration).is_empty());
}

#[test]
fn only_a_kind_there_is_something_of_can_be_ticked() {
    let mut state = ready();
    looked(&mut state, true);
    apply(&mut state, Action::SetMigrationKind(Kind::Songs, true));
    assert!(!state.migration.choice.songs);
    apply(&mut state, Action::SetMigrationKind(Kind::History, false));
    assert!(!state.migration.choice.history);
}

#[test]
fn going_ahead_brings_what_is_ticked_and_only_once_at_a_time() {
    let mut state = ready();
    looked(&mut state, true);
    apply(
        &mut state,
        Action::SetMigrationKind(Kind::Preferences, false),
    );
    let ticked = Kinds {
        preferences: false,
        ..NO_SONGS
    };
    assert_eq!(
        apply(&mut state, Action::StartMigration),
        [Effect::Migrate(ticked)]
    );
    assert!(state.migration.running.is_some());
    // A second press, or a tick, while it runs does nothing.
    assert!(apply(&mut state, Action::StartMigration).is_empty());
    apply(&mut state, Action::SetMigrationKind(Kind::History, false));
    assert!(state.migration.choice.history);
    // And the accounts are left alone until it is done.
    assert!(account_busy(&state));

    let progress = Progress {
        step: "Copying downloaded songs".into(),
        done: 8,
        total: 302,
    };
    apply(&mut state, Action::MigrationProgress(progress.clone()));
    assert_eq!(state.migration.running, Some(progress));
}

#[test]
fn nothing_ticked_is_nothing_to_bring() {
    let mut state = ready();
    looked(&mut state, true);
    for kind in Kind::EVERY {
        apply(&mut state, Action::SetMigrationKind(kind, false));
    }
    assert!(apply(&mut state, Action::StartMigration).is_empty());
}

#[test]
fn the_move_waits_for_an_account_change_under_way() {
    let mut state = ready();
    looked(&mut state, true);
    state.signing_in = true;
    assert!(apply(&mut state, Action::StartMigration).is_empty());
    assert_eq!(state.migration.running, None);
}

#[test]
fn a_run_that_ends_says_what_it_brought_and_takes_in_the_preferences() {
    let mut state = ready();
    state.nav.open(Page::Stats);
    state.stats = Loadable::Loading;
    looked(&mut state, true);
    apply(&mut state, Action::StartMigration);
    let outcome = Outcome {
        lines: vec![Line::brought("Ada: 590 plays brought.")],
        prefs: Some(found().prefs),
        done: NO_SONGS,
        history_changed: true,
        songs_changed: true,
        ..Outcome::default()
    };
    let effects = apply(&mut state, Action::MigrationDone(Box::new(outcome)));

    assert_eq!(state.migration.running, None);
    assert_eq!(state.migration.brought, NO_SONGS);
    assert!(state.settings.sidebar_collapsed);
    assert_eq!(state.settings.recent_searches, ["duman"]);
    assert_eq!(state.search.recent[0].query, "duman");
    let lines = state.migration.outcome.as_deref().unwrap_or_default();
    assert_eq!(lines[0].text, "Ada: 590 plays brought.");
    assert_eq!(
        lines[1].text,
        "Preferences brought: sidebar, recent searches."
    );
    // The statistics on screen are from before the plays arrived.
    assert!(effects.contains(&Effect::SaveSettings));
    assert!(effects.contains(&Effect::ApplyAudioSettings));
    assert!(effects.contains(&Effect::Fetch(Request::Stats(state.stats_days))));
    assert!(effects.contains(&Effect::Fetch(Request::CacheUsage)));
    // The dialog is open on the outcome, so no toast repeats it.
    assert!(state.toasts.is_empty());

    // Preferences are brought once: the next choice leaves them unticked.
    apply(&mut state, Action::OpenMigration);
    assert_eq!(state.migration.outcome, None);
    assert!(!state.migration.choice.preferences);
    assert!(state.migration.choice.history);
}

#[test]
fn a_run_that_ends_behind_a_closed_dialog_says_so_in_a_toast() {
    let mut state = ready();
    looked(&mut state, true);
    apply(&mut state, Action::StartMigration);
    apply(&mut state, Action::CloseDialog);
    let effects = apply(&mut state, Action::MigrationDone(Box::default()));
    assert!(effects.is_empty());
    assert_eq!(state.toasts.len(), 1);
    let lines = state.migration.outcome.as_deref().unwrap_or_default();
    assert_eq!(lines[0].mark, Mark::Skipped);
    assert_eq!(lines[0].text, "There was nothing to bring.");
}

#[test]
fn preferences_that_are_the_same_here_are_said_to_be() {
    let mut state = ready();
    looked(&mut state, true);
    state.settings.sidebar_collapsed = true;
    state.settings.recent_searches = vec!["Duman".into()];
    let outcome = Outcome {
        prefs: Some(found().prefs),
        ..Outcome::default()
    };
    let effects = apply(&mut state, Action::MigrationDone(Box::new(outcome)));
    assert!(effects.is_empty());
    let lines = state.migration.outcome.as_deref().unwrap_or_default();
    assert!(lines[0].text.starts_with("Preferences: nothing to change."));
}

#[test]
fn ticks_being_looked_at_are_not_moved_by_a_fresh_look() {
    let mut state = ready();
    looked(&mut state, true);
    apply(&mut state, Action::SetMigrationKind(Kind::History, false));
    looked(&mut state, false);
    assert!(!state.migration.choice.history);
    // With the dialog closed, the ticks follow what is there.
    apply(&mut state, Action::CloseDialog);
    looked(&mut state, false);
    assert!(state.migration.choice.history);
    assert_eq!(
        state.migration,
        Migration {
            found: Some(found()),
            choice: NO_SONGS,
            ..Migration::default()
        }
    );
}
