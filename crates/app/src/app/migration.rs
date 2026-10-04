//! Moving in from the Electron app: the threads that look at its profile
//! and bring things across, and what the window's thread does with what
//! they report.

use std::path::{Path, PathBuf};

use eframe::egui;
use spotified_client::Client;
use spotified_client::migrate::{ImportedSongs, Merged};

use super::{App, Event, accounts::Change, waking};
use crate::actions::Action;
use crate::migrate::run::{self, Core, Job};
use crate::migrate::{self, History, Kinds, Record, Report};
use crate::sidecar::{self, CoreStatus};

/// Where the Electron app's profile is, and what has been brought from it.
pub(super) struct Mover {
    /// `None` when there is nowhere to look.
    pub old_profile: Option<PathBuf>,
    pub record: Record,
}

/// The core as a run reaches it: the one that is running, for what is its
/// while it runs, and a core started for the one job for everything else.
struct Reached {
    running: Option<Client>,
}

impl Core for Reached {
    fn merge(&self, from: &Path, into: &Path, live: bool) -> Result<Merged, String> {
        match &self.running {
            Some(client) if live => client
                .migrate_history(from)
                .map_err(|error| error.to_string()),
            // No core has this database open: one is run to merge into it
            // and stops again.
            _ => {
                let answer = sidecar::one_shot(&[("-db", into), ("-migrate-from", from)])?;
                serde_json::from_str(&answer).map_err(|error| error.to_string())
            }
        }
    }

    fn songs(
        &self,
        dir: &Path,
        ids: &[String],
        limit_mb: Option<u32>,
    ) -> Result<ImportedSongs, String> {
        let client = self.running.as_ref().ok_or("no playback service")?;
        client
            .migrate_songs(dir, ids, limit_mb)
            .map_err(|error| error.to_string())
    }

    fn running(&self) -> bool {
        self.running.is_some()
    }
}

/// Counts what one of the Electron app's databases holds, by a core run
/// for that one job: the app itself cannot read a database.
fn inspect(database: &Path) -> Result<History, String> {
    let answer = sidecar::one_shot(&[("-migrate-inspect", database)])?;
    serde_json::from_str(&answer).map_err(|error| error.to_string())
}

impl App {
    /// Looks at the Electron app's profile, on a thread: it reads a few
    /// hundred small files and runs the core once for each database.
    pub(super) fn look_for_old_app(&mut self, ctx: &egui::Context) {
        let Some(root) = self.mover.old_profile.clone() else {
            return;
        };
        let accounts = self.accounts.list.clone();
        let record = self.mover.record.clone();
        let report = waking(ctx, &self.events_tx, Event::Migration);
        let spawned = std::thread::Builder::new()
            .name("old-app-look".into())
            .spawn(move || {
                let found = migrate::discover::discover(&root, &accounts, &record, &inspect);
                report(Report::Found(found.map(Box::new)));
            });
        if let Err(error) = spawned {
            log::warn!("the old app's profile could not be looked at: {error}");
        }
    }

    /// Brings over what was chosen, on a thread.
    pub(super) fn migrate(&mut self, ctx: &egui::Context, kinds: Kinds) {
        let Some(found) = self.state.migration.found.clone() else {
            return;
        };
        let running = match &self.state.core {
            CoreStatus::Ready { origin } => Some(Client::new(origin.clone())),
            _ => None,
        };
        // The cache will be held to the Electron app's size once its
        // preferences are in; the songs are let in up to that.
        let settings = &self.state.settings;
        let cache_max_mb = kinds
            .preferences
            .then(|| found.prefs.cache_max_mb())
            .flatten()
            .unwrap_or(settings.cache_max_mb);
        let job = Job {
            found,
            kinds,
            root: self.paths.config.clone(),
            accounts: self.accounts.list.clone(),
            live_database: self.accounts.database(),
            cache_max_mb,
        };
        log::info!("bringing things over from the old app: {kinds:?}");
        let report = waking(ctx, &self.events_tx, Event::Migration);
        let spawned = std::thread::Builder::new()
            .name("old-app-bring".into())
            .spawn(move || {
                let progress = |progress| report(Report::Progress(progress));
                let outcome = run::run(&job, &Reached { running }, &progress);
                report(Report::Done(Box::new(outcome)));
            });
        if let Err(error) = spawned {
            let failed = migrate::Outcome {
                lines: vec![migrate::Line::failed(error.to_string())],
                ..migrate::Outcome::default()
            };
            self.actions.push(Action::MigrationDone(Box::new(failed)));
        }
    }

    pub(super) fn migration_reported(&mut self, ctx: &egui::Context, report: Report) {
        match report {
            Report::Found(found) => {
                // A run that is only being looked at is not interrupted by
                // an offer nobody asked for.
                let offer = self.mover.record.unasked() && self.screenshot.is_none();
                self.actions.push(Action::MigrationFound {
                    found,
                    brought: self.mover.record.brought,
                    offer,
                });
            }
            Report::Progress(progress) => self.actions.push(Action::MigrationProgress(progress)),
            Report::Done(outcome) => {
                for line in &outcome.lines {
                    log::info!("old app: {:?}: {}", line.mark, line.text);
                }
                // The accounts' files are in place; now they are listed.
                for account in &outcome.new_accounts {
                    self.accounts.list.keep(account.clone());
                }
                // And each channel keeps the folder its history went to.
                for (account, channel) in &outcome.channels {
                    self.accounts.list.adopt_channel(account, channel.clone());
                }
                if !outcome.new_accounts.is_empty() || !outcome.channels.is_empty() {
                    self.accounts.save();
                    self.publish_accounts();
                }
                let record = &mut self.mover.record;
                record.accounts.extend(outcome.pairs.iter().cloned());
                record.brought = record.brought.with(outcome.done);
                record.seen = true;
                record.save(&self.paths.config);
                let activate = outcome.activate.clone();
                self.actions.push(Action::MigrationDone(outcome));
                if let Some(id) = activate
                    && self.accounts.list.active.is_none()
                {
                    self.change_account(ctx, Change::Switch(id));
                }
                // What is there to bring has changed: some of it is here.
                self.look_for_old_app(ctx);
            }
        }
    }

    /// The choice of what to bring was closed: the offer is not made again
    /// unasked.
    pub(super) fn migration_seen(&mut self) {
        if !self.mover.record.seen {
            self.mover.record.seen = true;
            self.mover.record.save(&self.paths.config);
        }
    }
}
