//! Carrying out what `actions::apply` asked for: everything that reaches
//! outside the state, from the settings file to the window itself.

use std::time::Instant;

use eframe::egui;

use super::{App, Event, SEARCH_DEBOUNCE, waking};
use crate::actions::{Action, Effect};
use crate::platform::autostart;
use crate::together::sync::Routed;
use crate::{channel, import, settings, sidecar, signin, themes, together, update, views};
use spotified_client::session::Command;

impl App {
    pub(super) fn run(&mut self, ctx: &egui::Context, effect: Effect) {
        match effect {
            Effect::SaveSettings => {
                if let Err(error) =
                    settings::save(&self.paths.settings_file(), &self.state.settings)
                {
                    log::warn!("settings could not be saved: {error}");
                }
            }
            Effect::Fetch(request) => {
                // `apply` only asks once the core is ready, and the backend
                // is started in the same breath as that status is handled.
                if let Some(backend) = &self.backend {
                    backend.send(request);
                    self.requests_in_flight += 1;
                }
            }
            Effect::ImportSignIn(source) => {
                match import::import(&source, &self.paths.credentials_file()) {
                    Ok(()) => {
                        log::info!("sign-in imported; restarting the playback service");
                        self.restart_core(ctx);
                        self.actions.push(Action::AccountChanged);
                    }
                    Err(error) => {
                        log::warn!("sign-in import failed: {error}");
                        self.actions.push(Action::SignInFailed(error.to_string()));
                    }
                }
            }
            Effect::Command(command) => self.send_command(command),
            Effect::TogetherConnect(options) => {
                let deliver = waking(ctx, &self.events_tx, Event::Together);
                match together::Connection::start(options, deliver) {
                    Ok(connection) => self.together = Some(connection),
                    Err(error) => {
                        let failed = together::Event::Failed(error.to_string());
                        self.actions.push(Action::TogetherEvent(Box::new(failed)));
                    }
                }
            }
            // Dropping the line says goodbye to the room on its way out.
            Effect::TogetherDisconnect => self.together = None,
            Effect::TogetherCommand(kind, fields) => {
                if let Some(connection) = &self.together {
                    connection.command(kind, fields);
                }
            }
            Effect::TogetherStatus(status, entry) => {
                if let Some(connection) = &self.together {
                    connection.status(status, entry);
                }
            }
            Effect::CopyToClipboard(text) => ctx.copy_text(text),
            Effect::SetFullscreen(on) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
            }
            Effect::GrowMini(least) => {
                let [width, height] = self.state.settings.mini_size;
                let size = egui::vec2(width.max(least[0]), height.max(least[1]));
                let grow = egui::ViewportCommand::InnerSize(size);
                ctx.send_viewport_cmd_to(views::mini::viewport(), grow);
            }
            Effect::ShowMainWindow => {
                let root = egui::ViewportId::ROOT;
                for command in [
                    egui::ViewportCommand::Visible(true),
                    egui::ViewportCommand::Minimized(false),
                    egui::ViewportCommand::Focus,
                ] {
                    ctx.send_viewport_cmd_to(root, command);
                }
            }
            Effect::SetDecorations(on) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(on));
            }
            Effect::SetStartAtLogin(on) => {
                if let Err(error) = autostart::set(on) {
                    log::warn!("start at login could not be changed: {error}");
                    // Show what is actually so.
                    self.state.starts_at_login = autostart::is_enabled();
                }
            }
            Effect::ApplyAudioSettings => {
                if let Some(session) = &self.session {
                    let settings = &self.state.settings;
                    session.set_normalise_volume(settings.normalise_volume);
                    session.set_equalizer(settings.equalizer_on, settings.equalizer);
                    session.set_crossfade(settings.crossfade_seconds);
                    session.tap().set_watching(settings.visualizer);
                }
            }
            Effect::SignIn => {
                // The browser stays open for as long as the person takes.
                let scratch = self.paths.config.clone();
                let credentials = self.paths.credentials_file();
                let done = waking(ctx, &self.events_tx, Event::SignedIn);
                let spawned = std::thread::Builder::new()
                    .name("sign-in".into())
                    .spawn(move || {
                        let result = signin::sign_in(&scratch, &credentials);
                        done(result.map_err(|error| error.to_string()));
                    });
                if let Err(error) = spawned {
                    self.actions.push(Action::SignInFailed(error.to_string()));
                }
            }
            Effect::SwitchChannel(channel_id) => {
                match channel::select(&self.paths.credentials_file(), &channel_id) {
                    Ok(()) => {
                        log::info!("channel changed; restarting the playback service");
                        self.restart_core(ctx);
                        self.actions.push(Action::AccountChanged);
                    }
                    Err(error) => {
                        log::warn!("the channel could not be changed: {error}");
                        self.state.channel_id = channel::active(&self.paths.credentials_file());
                        self.state.toast_error("The channel could not be changed");
                    }
                }
            }
            Effect::OpenThemesFolder => {
                if let Err(error) = open_folder(&self.paths.themes_folder()) {
                    log::warn!("the themes folder could not be opened: {error}");
                }
            }
            Effect::ReloadThemes => {
                self.state.themes = themes::list(&self.paths.themes_folder());
            }
            Effect::OpenLogs => {
                if let Err(error) = open_folder(&self.paths.logs) {
                    log::warn!("the logs folder could not be opened: {error}");
                }
            }
            Effect::CheckForUpdate => {
                let report = waking(ctx, &self.events_tx, Event::Update);
                let folder = self.paths.cache.join("updates");
                let spawned = std::thread::Builder::new()
                    .name("update".into())
                    .spawn(move || {
                        update::check(&folder, report);
                    });
                if let Err(error) = spawned {
                    let failed = update::Status::Failed(error.to_string());
                    self.actions.push(Action::UpdateChanged(failed));
                }
            }
            Effect::InstallUpdate(installer) => match update::install(&installer) {
                // The installer replaces the files this copy holds open, so
                // it goes: for real, not to the tray.
                Ok(()) => {
                    log::info!("installing {}", installer.display());
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(error) => {
                    let failed = update::Status::Failed(error.to_string());
                    self.actions.push(Action::UpdateChanged(failed));
                }
            },
            Effect::UpdateResolver => {
                let done = waking(ctx, &self.events_tx, Event::ResolverUpdated);
                let spawned = std::thread::Builder::new()
                    .name("resolver-update".into())
                    .spawn(move || done(sidecar::update_resolver()));
                if let Err(error) = spawned {
                    let failed = Action::ResolverUpdated(Err(error.to_string()));
                    self.actions.push(failed);
                }
            }
            Effect::SignOut => {
                // The cookie file beside it is the same session, for yt-dlp.
                let credentials = self.paths.credentials_file();
                let _ = std::fs::remove_file(credentials.with_file_name("yt-dlp-cookies.txt"));
                match std::fs::remove_file(&credentials) {
                    Ok(()) => {
                        log::info!("signed out; restarting the playback service");
                        self.restart_core(ctx);
                        self.actions.push(Action::AccountChanged);
                    }
                    Err(error) => log::warn!("sign-out failed: {error}"),
                }
            }
            Effect::DebounceSearch => {
                self.search_due = Some(Instant::now() + SEARCH_DEBOUNCE);
                ctx.request_repaint_after(SEARCH_DEBOUNCE);
            }
        }
    }

    /// Sends a player command where it belongs: to the core, or, in a
    /// Listen Together room, to the room, which then tells every player.
    fn send_command(&mut self, command: Command) {
        let room = self.state.together.room.as_ref();
        if let (Some(room), Some(connection), true) =
            (room, &self.together, self.state.together.in_room())
        {
            let position_ms = self
                .state
                .playback
                .as_ref()
                .map_or(0, |playback| playback.position_ms(Instant::now()));
            let me = &self.state.together.me;
            match together::sync::route(&command, room, me, position_ms) {
                Routed::Room(kind, fields) => return connection.command(kind, fields),
                Routed::Refused(why) => return self.state.toast_error(why),
                Routed::Core => {}
            }
        }
        match &self.session {
            Some(session) => session.send(command),
            // Asked for before the core was up (`--open track:`): sent as
            // soon as there is a session to send it to.
            None => self.held_commands.push(command),
        }
    }
}

/// Shows a folder in the system's file manager.
fn open_folder(folder: &std::path::Path) -> std::io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(drop)
}
