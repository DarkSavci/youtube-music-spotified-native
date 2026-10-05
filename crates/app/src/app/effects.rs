//! Carrying out what `actions::apply` asked for: everything that reaches
//! outside the state, from the settings file to the window itself.

use std::time::Instant;

use eframe::egui;

use super::{App, Event, ROOM_SEARCH_DEBOUNCE, SEARCH_DEBOUNCE, waking};

/// How long the lookup in Your listening waits for the typing to pause.
const LOOKUP_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(200);
use crate::actions::{Action, Effect};
use crate::platform::autostart;
use crate::platform::tray::TrayAction;
use crate::session::Session;
use crate::settings::OutputDevice;
use crate::state::State;
use crate::together::sync::Routed;
use crate::update::Then;
use crate::{settings, skins, themes, together, update, views};
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
            Effect::Migrate(kinds) => self.migrate(ctx, kinds),
            Effect::MigrationSeen => self.migration_seen(),
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
            Effect::TogetherHandOver(next) => {
                if let Some(connection) = &self.together {
                    connection.leave(next);
                }
            }
            Effect::TogetherProbe(url) => {
                let done = waking(ctx, &self.events_tx, Event::TogetherProbed);
                let spawned = std::thread::Builder::new()
                    .name("listen-together-test".into())
                    .spawn(move || done(together::client::probe(&url)));
                if let Err(error) = spawned {
                    let failed = together::Ask::Tested(Err(error.to_string()));
                    self.actions.push(Action::Room(failed));
                }
            }
            Effect::DebounceRoomSearch => {
                self.room_search_due = Some(Instant::now() + ROOM_SEARCH_DEBOUNCE);
                ctx.request_repaint_after(ROOM_SEARCH_DEBOUNCE);
            }
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
                self.window_shown = true;
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
                    apply_audio_settings(session, &self.state);
                }
            }
            // On a thread of its own: the system can take a moment to say.
            Effect::ListOutputDevices => {
                let listed = waking(ctx, &self.events_tx, Event::OutputDevices);
                let spawned = std::thread::Builder::new()
                    .name("sound-devices".into())
                    .spawn(move || {
                        let devices = spotified_audio::output_devices().into_iter();
                        let device = |device: spotified_audio::OutputDevice| OutputDevice {
                            id: device.id,
                            name: device.name,
                        };
                        listed(devices.map(device).collect());
                    });
                if let Err(error) = spawned {
                    log::warn!("the sound devices could not be listed: {error}");
                }
            }
            Effect::ApplyEqualizer => {
                if let Some(session) = &self.session {
                    session.set_equalizer(crate::equalizer::for_engine(&self.state.settings));
                }
            }
            Effect::SignIn => self.sign_in(ctx),
            Effect::SwitchAccount(id) => {
                self.change_account(ctx, super::accounts::Change::Switch(id));
            }
            Effect::RemoveAccount(id) => self.remove_account(ctx, id),
            Effect::SwitchChannel(channel_id) => {
                log::info!("channel changed; restarting the playback service");
                self.change_account(ctx, super::accounts::Change::Channel(channel_id));
            }
            Effect::RememberAccount { name, avatar_url } => {
                let changed = self.accounts.list.set_name(&name, &avatar_url);
                self.remember(changed);
            }
            Effect::RememberChannels(channels) => {
                let changed = self.accounts.list.set_channels(&channels);
                self.remember(changed);
            }
            Effect::SaveReport => self.save_report(ctx),
            Effect::OpenThemesFolder => {
                if let Err(error) = open_folder(&self.paths.themes_folder()) {
                    log::warn!("the themes folder could not be opened: {error}");
                }
            }
            Effect::ReloadThemes => {
                self.state.themes = themes::list(&self.paths.themes_folder());
            }
            Effect::OpenMilkDrop => {
                let scratch = self.paths.cache.join("milkdrop");
                let presets = self.paths.milkdrop_folder();
                if let Err(why) = self.milkdrop.open(&scratch, &presets) {
                    log::warn!("MilkDrop could not open: {why}");
                    let failed = crate::milkdrop::Ask::Failed(why);
                    self.actions.push(Action::MilkDrop(failed));
                }
            }
            Effect::CloseMilkDrop => self.milkdrop.close(),
            Effect::FetchPresets(pack) => {
                let fetched = waking(ctx, &self.events_tx, Event::PresetsFetched);
                let folder = self.paths.milkdrop_folder();
                let scratch = self.paths.cache.join("milkdrop");
                let spawned = std::thread::Builder::new()
                    .name("milkdrop-presets".into())
                    .spawn(move || {
                        let pack = &crate::milkdrop::PACKS[pack];
                        fetched(crate::milkdrop::fetch_pack(pack, &folder, &scratch));
                    });
                if let Err(error) = spawned {
                    let failed = crate::milkdrop::Ask::Fetched(Err(error.to_string()));
                    self.actions.push(Action::MilkDrop(failed));
                }
            }
            Effect::OpenMilkDropFolder => {
                let folder = self.paths.milkdrop_folder();
                let opened = std::fs::create_dir_all(&folder).and_then(|()| open_folder(&folder));
                if let Err(error) = opened {
                    log::warn!("the MilkDrop folder could not be opened: {error}");
                }
            }
            Effect::InstallSkins(files) => self.install_skins(files),
            Effect::PickSkins => self.pick_skins(ctx),
            Effect::OpenSkinsFolder => {
                let folder = self.paths.skins_folder();
                // Made here: a profile that has never had a skin has none.
                let opened = std::fs::create_dir_all(&folder).and_then(|()| open_folder(&folder));
                if let Err(error) = opened {
                    log::warn!("the skins folder could not be opened: {error}");
                }
            }
            Effect::ReloadSkins => {
                self.state.skins = skins::list(&self.paths.skins_folder());
            }
            Effect::ApplyBalance => {
                if let Some(session) = &self.session {
                    session.set_balance(self.state.settings.balance);
                }
            }
            Effect::OpenUrl(url) => {
                if let Err(error) = crate::platform::shell::open_url(url) {
                    log::warn!("{url} could not be opened: {error}");
                }
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
            Effect::NotifyUpdate(version) => {
                let install = TrayAction::InstallUpdate;
                let asked = waking(ctx, &self.events_tx, move |()| Event::Tray(install));
                crate::platform::notify::update_ready(&version, move || asked(()));
            }
            Effect::InstallUpdate(installer) => match update::install(&installer, Then::Relaunch) {
                // The installer replaces the files this copy holds open, so
                // it goes: for real, not to the tray.
                Ok(()) => {
                    log::info!("installing {}", installer.display());
                    self.installing = true;
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(error) => {
                    let failed = update::Status::Failed(error.to_string());
                    self.actions.push(Action::UpdateChanged(failed));
                }
            },
            Effect::UpdateResolver => self.update_resolver(ctx),
            Effect::DebounceSearch => {
                self.search_due = Some(Instant::now() + SEARCH_DEBOUNCE);
                ctx.request_repaint_after(SEARCH_DEBOUNCE);
            }
            Effect::DebounceStatsLookup => {
                self.lookup_due = Some(Instant::now() + LOOKUP_DEBOUNCE);
                ctx.request_repaint_after(LOOKUP_DEBOUNCE);
            }
        }
    }

    /// Puts skin files in the skins folder and wears the last of them that
    /// is a skin, saying what came of each that is not.
    fn install_skins(&mut self, files: Vec<std::path::PathBuf>) {
        let folder = self.paths.skins_folder();
        let mut installed = None;
        for file in &files {
            match skins::install(file, &folder) {
                Ok(name) => installed = Some(name),
                Err(why) => {
                    log::warn!("a skin was not installed: {why}");
                    self.state.toast_error(why);
                }
            }
        }
        let Some(name) = installed else {
            return;
        };
        self.state.skins = skins::list(&folder);
        // Read again even if a skin of this name was being worn: the file
        // behind the name is a new one.
        self.state.skin = None;
        let said = format!("{} is the mini player's skin", skins::label(&name));
        self.state.toast(said);
        self.actions
            .push(Action::Skin(skins::Ask::Wear(Some(name))));
    }

    /// Asks the person for skin files, off the UI thread: the dialog is
    /// the system's, and stays for as long as they look.
    fn pick_skins(&mut self, ctx: &egui::Context) {
        let picked = waking(ctx, &self.events_tx, Event::SkinsPicked);
        let spawned = std::thread::Builder::new()
            .name("skin-picker".into())
            .spawn(move || picked(crate::platform::pick::skins()));
        if let Err(error) = spawned {
            log::warn!("the file dialog could not be opened: {error}");
        }
    }

    /// Passes on what the line to a Listen Together relay said.
    pub(super) fn heard_from_relay(&mut self, event: together::Event) {
        match &event {
            together::Event::Room { room, .. } => log::debug!(
                "listen together: room at revision {}, {} in it, {} queued, playing: {}",
                room.revision,
                room.members.len(),
                room.queue.len(),
                room.playing
            ),
            other => log::debug!("listen together: {other:?}"),
        }
        self.actions.push(Action::TogetherEvent(Box::new(event)));
    }

    /// Sends the searches whose typing has paused for long enough.
    pub(super) fn send_due_searches(&mut self) {
        let now = Instant::now();
        if self.search_due.is_some_and(|due| now >= due) {
            self.search_due = None;
            self.actions.push(Action::RunSearch);
        }
        if self.room_search_due.is_some_and(|due| now >= due) {
            self.room_search_due = None;
            self.actions.push(Action::Room(together::Ask::RunSearch));
        }
        if self.lookup_due.is_some_and(|due| now >= due) {
            self.lookup_due = None;
            self.actions.push(Action::RunStatsLookup);
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
                Routed::Room(kind, fields) => {
                    connection.command(kind, fields);
                    if together::sync::trimmed(&command) {
                        self.state.toast(together::sync::FIRST_HUNDRED);
                    }
                    return;
                }
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

/// Tells the engine and the core what the settings now are. Some are this
/// device's (the equalizer, the speed, how loud); the rest are the core's,
/// which decides each transition and writes the queue down.
pub(super) fn apply_audio_settings(session: &Session, state: &State) {
    let settings = &state.settings;
    session.set_normalise_volume(settings.normalise_volume);
    session.set_loudness_target(settings.volume_level.lufs());
    session.set_equalizer(crate::equalizer::for_engine(settings));
    session.set_speed(state.speed());
    session.set_balance(settings.balance);
    let device = settings.output_device.as_ref();
    session.set_output_device(device.map(|device| device.id.clone()));
    session.tap().set_watching(state.watches_sound());
    session.set_settings(spotified_client::session::Settings {
        crossfade_ms: u64::from(settings.crossfade_seconds) * 1000,
        gapless: settings.gapless,
        resume_on_launch: settings.resume_on_launch,
        report_to_youtube: settings.report_to_youtube,
        cache_max_mb: u64::from(settings.cache_max_mb),
        autoplay: settings.autoplay,
        blocked: settings.blocked.for_core(),
    });
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
