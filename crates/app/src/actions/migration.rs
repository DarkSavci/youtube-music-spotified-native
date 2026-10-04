//! Moving in from the Electron app: choosing what to bring, and taking in
//! what a run came to.

use spotified_client::session::Command;

use super::loading::load_current_page;
use super::{Action, Effect, account_busy};
use crate::backend::Request;
use crate::migrate::{Line, Outcome, Progress, prefs};
use crate::state::{Dialog, Loadable, State};

pub(super) fn migration(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::OpenMigration => {
            if state.migration.found.is_none() {
                return Vec::new();
            }
            // A run that has ended makes way for the next choice; one under
            // way is shown as it is.
            if state.migration.running.is_none() {
                state.migration.outcome = None;
                state.migration.suggest();
            }
            state.dialog = Some(Dialog::Migration);
            Vec::new()
        }
        Action::SetMigrationKind(kind, on) => {
            let there = state
                .migration
                .found
                .as_ref()
                .is_some_and(|found| found.available().get(kind));
            if there && state.migration.running.is_none() {
                state.migration.choice.set(kind, on);
            }
            Vec::new()
        }
        Action::StartMigration => {
            let migration = &state.migration;
            let ready = migration.found.is_some()
                && migration.choice.any()
                && migration.running.is_none()
                // An account being added or changed has the core stopped
                // and the list in flux; both are needed whole.
                && !account_busy(state);
            if !ready {
                return Vec::new();
            }
            state.migration.outcome = None;
            state.migration.running = Some(Progress {
                step: "Getting ready".to_owned(),
                ..Progress::default()
            });
            vec![Effect::Migrate(state.migration.choice)]
        }
        Action::MigrationFound {
            found,
            brought,
            offer,
        } => {
            state.migration.brought = brought;
            state.migration.found = found.map(|found| *found);
            let open = state.dialog == Some(Dialog::Migration);
            // Ticks a person is looking at are theirs to change, not ours.
            if !open && state.migration.running.is_none() {
                state.migration.suggest();
            }
            if offer && state.migration.found.is_some() && state.dialog.is_none() {
                state.dialog = Some(Dialog::Migration);
            }
            Vec::new()
        }
        Action::MigrationProgress(progress) => {
            if state.migration.running.is_some() {
                state.migration.running = Some(progress);
            }
            Vec::new()
        }
        Action::MigrationDone(outcome) => done(state, *outcome),
        _ => Vec::new(),
    }
}

/// A run ended: its preferences are taken in, and what it changed under
/// the pages on screen is asked for again.
fn done(state: &mut State, outcome: Outcome) -> Vec<Effect> {
    let mut effects = Vec::new();
    let mut lines = outcome.lines;
    if let Some(old) = &outcome.prefs {
        let scope = state.search_scope();
        let changed = prefs::apply(&mut state.settings, old, &scope);
        if changed.is_empty() {
            lines.push(Line::skipped(if old.is_empty() {
                "No preferences to bring."
            } else {
                "Preferences: nothing to change. They were the same here already."
            }));
        } else {
            lines.push(Line::brought(format!(
                "Preferences brought: {}.",
                changed.join(", ")
            )));
            effects.extend([Effect::SaveSettings, Effect::ApplyAudioSettings]);
            state.refresh_recent_searches();
            // A boosted level has no boost to stand on if that went.
            if let Some(playback) = &mut state.playback
                && !state.settings.volume_boost
                && playback.session.volume > 1.0
            {
                playback.session.volume = 1.0;
                effects.push(Effect::Command(Command::SetVolume(1.0)));
            }
        }
        if old.settings.is_some() {
            lines.push(Line::skipped(prefs::NOT_BROUGHT));
        }
    }
    if lines.is_empty() {
        lines.push(Line::skipped("There was nothing to bring."));
    }
    state.migration.running = None;
    state.migration.brought = state.migration.brought.with(outcome.done);
    state.migration.outcome = Some(lines);
    if state.dialog != Some(Dialog::Migration) {
        state.toast("Finished bringing things over from the old app");
    }

    if outcome.history_changed {
        // The figures, the pins and the folders on screen are from before.
        state.stats = Loadable::NotLoaded;
        effects.extend(load_current_page(state));
        if state.core_ready() && state.account.is_some() {
            effects.push(Effect::Fetch(Request::Folders));
            effects.push(Effect::Fetch(Request::Library));
        }
    }
    if outcome.songs_changed && state.core_ready() {
        effects.push(Effect::Fetch(Request::CacheUsage));
    }
    effects
}
