//! The core, and whose account it runs on.
//!
//! The core serves one account at a time and reads its credentials once,
//! as it starts. So every change of account is the same three steps: stop
//! the core, change what is on disk, start it again. The middle step waits
//! for the first to finish, since a running core holds its files open.

use eframe::egui;

use super::{App, Event, FIXTURES, waking};
use crate::accounts::SavedAccount;
use crate::actions::Action;
use crate::resolver::Resolver;
use crate::sidecar::{self, CoreStatus, SidecarConfig};
use crate::{accounts, channel, signin};

/// What is to change once the core has stopped.
pub(super) enum Change {
    /// A sign-in wrote this account's credentials: it joins the list and
    /// is the one in use.
    Add(SavedAccount),
    /// Use this saved account.
    Switch(String),
    /// Sign this account out and forget it.
    Remove(String),
    /// Act as this channel of the account in use; empty for its own.
    Channel(String),
}

impl App {
    pub(super) fn start_core(&mut self, ctx: &egui::Context) {
        let resolver = Resolver::new(self.paths.resolver_folder());
        self.accounts.prepare();
        let config = SidecarConfig {
            credentials: self.accounts.credentials(),
            database: self.accounts.database(),
            cache: self.paths.audio_cache(),
            resolver: resolver.updated_exe(),
            fixtures: self.demo.then(|| sidecar::locate(FIXTURES)).flatten(),
        };
        let run = self.core_run;
        let report = waking(ctx, &self.events_tx, move |status| Event::Core(run, status));
        match sidecar::spawn(&config, report) {
            Ok(sidecar) => self.sidecar = Some(sidecar),
            Err(error) => {
                log::error!("the playback service could not start: {error}");
                self.actions
                    .push(Action::CoreChanged(CoreStatus::Failed(format!(
                        "The playback service could not start: {error}."
                    ))));
            }
        }
    }

    /// Starts the core for the first time. A yt-dlp update staged on an
    /// earlier run goes into use first, on another thread: it moves a
    /// folder of some hundred files, which the window must not wait for.
    pub(super) fn first_start(&mut self, ctx: &egui::Context) {
        let resolver = Resolver::new(self.paths.resolver_folder());
        if !resolver.staged() {
            return self.start_core(ctx);
        }
        let promoted = waking(ctx, &self.events_tx, |()| Event::CoreStopped);
        let spawned = std::thread::Builder::new()
            .name("resolver-promote".into())
            .spawn(move || {
                resolver.promote();
                promoted(());
            });
        if spawned.is_err() {
            self.start_core(ctx);
        }
    }

    /// Stops the core, to start it again once it has gone. The stop can
    /// take a moment (the core saves its state), so it happens on another
    /// thread: the two must never share the database.
    fn restart_core(&mut self, ctx: &egui::Context) {
        self.backend = None;
        self.session = None;
        self.core_run += 1;
        self.requests_in_flight = 0;
        let Some(old) = self.sidecar.take() else {
            self.core_stopped(ctx);
            return;
        };
        let stopped = waking(ctx, &self.events_tx, |()| Event::CoreStopped);
        let spawned = std::thread::Builder::new()
            .name("core-stop".into())
            .spawn(move || {
                drop(old);
                stopped(());
            });
        if let Err(error) = spawned {
            log::error!("the playback service could not be restarted: {error}");
        }
    }

    /// No core is running: what was waiting for that is done, and a core
    /// is started on what is then on disk.
    pub(super) fn core_stopped(&mut self, ctx: &egui::Context) {
        if let Some(change) = self.pending_change.take() {
            self.make(change);
        }
        self.start_core(ctx);
    }

    /// Restarts the core around a change of account. Everything fetched so
    /// far was fetched as whoever was signed in before, and is forgotten.
    pub(super) fn change_account(&mut self, ctx: &egui::Context, change: Change) {
        self.pending_change = Some(change);
        self.restart_core(ctx);
        self.actions.push(Action::AccountChanged);
    }

    fn make(&mut self, change: Change) {
        match change {
            Change::Add(mut account) => {
                // A copied sign-in may name a channel already.
                if let Ok(credentials) = self.accounts.credentials_for(&account) {
                    account.channel = channel::active(&credentials);
                }
                log::info!("signed in; the new account is in use");
                self.accounts.list.add(account);
                self.accounts.save();
            }
            Change::Switch(id) => {
                if self.accounts.list.activate(&id) {
                    log::info!("another saved account is in use");
                    self.accounts.save();
                }
            }
            Change::Remove(id) => {
                log::info!("signed out of an account");
                self.accounts.remove(&id);
            }
            Change::Channel(channel_id) => {
                match channel::select(&self.accounts.credentials(), &channel_id) {
                    Ok(()) => {
                        self.accounts.list.select_channel(&channel_id);
                        self.accounts.save();
                        // What YouTube answered the other channel is not
                        // this one's to be shown.
                        self.accounts.forget_answers();
                    }
                    Err(error) => {
                        log::warn!("the channel could not be changed: {error}");
                        self.state.toast_error("The channel could not be changed");
                    }
                }
            }
        }
        self.publish_accounts();
    }

    /// Tells the views what the list of accounts now is.
    pub(super) fn publish_accounts(&mut self) {
        let list = Box::new(self.accounts.list.clone());
        self.actions.push(Action::AccountsChanged(list));
    }

    /// Opens the browser for a sign-in, which becomes a new saved account.
    pub(super) fn sign_in(&mut self, ctx: &egui::Context) {
        let account = accounts::new_account("New account");
        let credentials = match self.accounts.credentials_for(&account) {
            Ok(credentials) => credentials,
            Err(error) => return self.actions.push(Action::SignInFailed(error.to_string())),
        };
        // The browser stays open for as long as the person takes.
        let scratch = self.paths.config.clone();
        let folder = self.accounts.directory(Some(&account));
        let done = waking(ctx, &self.events_tx, Event::SignedIn);
        let spawned = std::thread::Builder::new()
            .name("sign-in".into())
            .spawn(move || match signin::sign_in(&scratch, &credentials) {
                Ok(()) => done(Ok(account)),
                Err(error) => {
                    // The folder was made for this sign-in and holds
                    // nothing else.
                    let _ = std::fs::remove_dir_all(folder);
                    done(Err(error.to_string()));
                }
            });
        if let Err(error) = spawned {
            self.actions.push(Action::SignInFailed(error.to_string()));
        }
    }

    /// Signs a saved account out. Only the one in use needs the core
    /// stopped: nothing has the others' files open.
    pub(super) fn remove_account(&mut self, ctx: &egui::Context, id: String) {
        if self.accounts.list.is_active(&id) {
            self.change_account(ctx, Change::Remove(id));
        } else {
            self.accounts.remove(&id);
            self.publish_accounts();
        }
    }

    /// Notes what the core said of the account in use, for the list.
    pub(super) fn remember(&mut self, changed: bool) {
        if changed {
            self.accounts.save();
            self.publish_accounts();
        }
    }
}
