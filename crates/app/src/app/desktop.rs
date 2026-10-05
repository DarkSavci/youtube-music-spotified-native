//! The app's place on the desktop outside its window: the tray icon and
//! its flyout, the buttons on the taskbar thumbnail, and the upkeep that
//! goes on behind them.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Instant, SystemTime};

use eframe::egui;

use super::{App, Event, waking};
use crate::actions::Action;
use crate::platform::shell::{self, Area};
use crate::platform::taskbar::{Buttons, ThumbAction};
use crate::platform::tray::{Snapshot, TrayAction};
use crate::platform::{autostart, tray};
use crate::resolver::{self, Resolver};
use crate::state::State;
use crate::{report, sidecar, update, views};

/// The gap between the flyout and the taskbar, the one Windows' own
/// flyouts keep. In points.
const FLYOUT_MARGIN: f32 = 12.0;

impl App {
    pub(super) fn tray_asked(&mut self, ctx: &egui::Context, action: TrayAction) {
        match action {
            TrayAction::Show => self.actions.push(Action::ShowMainWindow),
            TrayAction::Hide => {
                self.window_shown = false;
                ctx.send_viewport_cmd_to(
                    egui::ViewportId::ROOT,
                    egui::ViewportCommand::Visible(false),
                );
            }
            TrayAction::Flyout(icon) => self.toggle_flyout(ctx, icon),
            TrayAction::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
            TrayAction::TogglePlay => self.actions.push(Action::TogglePlay),
            TrayAction::Next => self.actions.push(Action::Next),
            TrayAction::Previous => self.actions.push(Action::Previous),
            TrayAction::Like => self.actions.push(Action::SaveCurrent),
            TrayAction::Mini => self.actions.push(Action::ToggleMiniPlayer),
            TrayAction::InstallUpdate => self.actions.push(Action::InstallUpdate),
        }
    }

    pub(super) fn thumb_asked(&mut self, action: ThumbAction) {
        self.actions.push(match action {
            ThumbAction::Like => Action::SaveCurrent,
            ThumbAction::Previous => Action::Previous,
            ThumbAction::TogglePlay => Action::TogglePlay,
            ThumbAction::Next => Action::Next,
        });
    }

    /// Opens the flyout against the taskbar by the icon, or closes it.
    fn toggle_flyout(&mut self, ctx: &egui::Context, icon: Area) {
        // The system speaks of the screen in its own pixels; the window is
        // placed in points.
        let scale = ctx.pixels_per_point();
        let [width, height] = views::flyout::SIZE;
        let size = (width * scale, height * scale);
        let (centre_x, centre_y) = (
            (icon.left + icon.right) / 2.0,
            (icon.top + icon.bottom) / 2.0,
        );
        let Some(display) = shell::display_at(centre_x, centre_y) else {
            return;
        };
        let (x, y) = shell::flyout_position(icon, display, size, FLYOUT_MARGIN * scale);
        self.actions.push(Action::ToggleFlyout {
            position: [x / scale, y / scale],
            now: Instant::now(),
        });
    }

    /// Opens the flyout as a click on the tray icon would, for `--open`.
    pub(super) fn open_flyout(&mut self, ctx: &egui::Context) {
        let icon = self.tray.as_ref().and_then(tray::Tray::place);
        match icon {
            Some(icon) => self.toggle_flyout(ctx, icon),
            None => log::warn!("there is no tray icon to open the flyout by"),
        }
    }

    /// Draws the mini player, in a window of its own, while it is open.
    /// From here so that it reads the same state and asks through the same
    /// actions as the main window.
    pub(super) fn show_mini(&mut self, ui: &mut egui::Ui) {
        let builder = views::mini::builder(&self.state, ui.ctx().pixels_per_point());
        let id = views::mini::viewport();
        let open = self.state.mini_player;
        if !window_made(ui.ctx(), &mut self.mini_made, open, id, &builder) {
            // A window made afresh has no shape yet.
            self.mini_shape = None;
            return;
        }
        let (state, actions) = (&self.state, &mut self.actions);
        ui.ctx().show_viewport_immediate(id, builder, |ui, _| {
            views::mini::show(state, ui, actions);
        });
        self.shape_mini(ui.ctx());
    }

    /// Cuts the mini player's window to the shape its skin gives it, when
    /// that is not the shape it has. Most skins are rectangles, and the
    /// app's own mini player always is.
    fn shape_mini(&mut self, ctx: &egui::Context) {
        use views::mini::skinned;
        let settings = &self.state.settings;
        let scale = ctx.pixels_per_point();
        let boxes = self
            .state
            .worn_skin()
            .and_then(|worn| skinned::window_shape(worn, settings, scale));
        let mut hasher = DefaultHasher::new();
        boxes.hash(&mut hasher);
        let key = hasher.finish();
        if self.mini_shape == Some(key) {
            return;
        }
        // Not there yet on the frame it is first asked for: tried again.
        if shell::shape_window(views::mini::TITLE, boxes.as_deref()) {
            self.mini_shape = Some(key);
        }
    }

    /// Draws the flyout, in a window of its own, while it is open.
    pub(super) fn show_flyout(&mut self, ui: &mut egui::Ui) {
        let flyout = self.state.flyout;
        let builder = flyout.as_ref().map(views::flyout::builder);
        let id = views::flyout::viewport();
        let (Some(flyout), Some(builder)) = (flyout, builder) else {
            self.flyout_made = false;
            return;
        };
        if !window_made(ui.ctx(), &mut self.flyout_made, true, id, &builder) {
            return;
        }
        let (state, actions) = (&self.state, &mut self.actions);
        ui.ctx().show_viewport_immediate(id, builder, |ui, _| {
            views::flyout::show(state, ui, actions);
        });
        // Once for each opening, when the window exists to be found.
        if self.flyout_rounded != Some(flyout.opened) {
            self.flyout_rounded = Some(flyout.opened);
            shell::round_corners(views::flyout::TITLE);
        }
    }

    /// Keeps the tray's tooltip and menu and the taskbar's buttons saying
    /// what is so. Called every frame, so it tells a change by a hash and
    /// builds nothing until there is one.
    pub(super) fn show_on_desktop(&mut self, ctx: &egui::Context) {
        if self.tray.is_none() && self.taskbar.is_none() {
            return;
        }
        let scale = ctx.native_pixels_per_point().unwrap_or(1.0);
        let key = desktop_key(&self.state, self.window_shown, scale);
        if self.desktop_shown == Some(key) {
            // The taskbar may still be waiting for its button to exist.
            if let Some(taskbar) = &mut self.taskbar {
                taskbar.show(buttons(&self.state), scale);
            }
            return;
        }
        self.desktop_shown = Some(key);
        if let Some(taskbar) = &mut self.taskbar {
            taskbar.show(buttons(&self.state), scale);
        }
        if let Some(tray) = &mut self.tray {
            tray.show(snapshot(&self.state, self.window_shown));
        }
    }

    /// Looks for a newer yt-dlp when a day has passed since the last look.
    /// Quietly: nobody asked, so nothing is said unless the log is read.
    pub(super) fn keep_resolver_current(&mut self, ctx: &egui::Context) {
        let Some(due) = self.resolver_due else {
            return;
        };
        let now = Instant::now();
        if now < due {
            // The window sleeps when nothing happens; this wakes it.
            ctx.request_repaint_after(due - now);
            return;
        }
        self.resolver_due = Some(now + resolver::CHECK_EVERY);
        let resolver = Resolver::new(self.paths.resolver_folder());
        // The stamp is a file's date, read here so that a look that is
        // not due costs no thread.
        if !resolver.due(SystemTime::now()) {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("resolver-update".into())
            .spawn(move || {
                let bundled = sidecar::bundled_resolver();
                match resolver.update(bundled.as_deref(), false) {
                    Ok(outcome) => log::info!("yt-dlp: {outcome:?}"),
                    Err(error) => log::warn!("yt-dlp: the update failed: {error}"),
                }
            });
        if let Err(error) = spawned {
            log::warn!("yt-dlp: the update could not start: {error}");
        }
    }

    /// Looks for a newer yt-dlp now, because it was asked for.
    pub(super) fn update_resolver(&mut self, ctx: &egui::Context) {
        let resolver = Resolver::new(self.paths.resolver_folder());
        let done = waking(ctx, &self.events_tx, Event::ResolverUpdated);
        let spawned = std::thread::Builder::new()
            .name("resolver-update".into())
            .spawn(move || {
                let bundled = sidecar::bundled_resolver();
                let outcome = resolver.update(bundled.as_deref(), true);
                done(outcome.map(|outcome| outcome.said().to_owned()));
            });
        if let Err(error) = spawned {
            let failed = Action::ResolverUpdated(Err(error.to_string()));
            self.actions.push(failed);
        }
    }

    /// Gathers the logs and a summary into a zip in the Downloads folder,
    /// and shows it there.
    pub(super) fn save_report(&mut self, ctx: &egui::Context) {
        let downloads = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(std::path::Path::to_path_buf));
        let Some(into) = self.report_folder.clone().or(downloads) else {
            let failed = Err("there is no Downloads folder to save it in".to_owned());
            return self.actions.push(Action::ReportSaved(failed));
        };
        let resolver = Resolver::new(self.paths.resolver_folder());
        let core = match &self.state.core {
            sidecar::CoreStatus::Ready { origin } => Some(origin.clone()),
            _ => None,
        };
        let request = report::Request {
            logs: self.paths.logs.clone(),
            into,
            core,
            resolver: resolver.updated_exe().or_else(sidecar::bundled_resolver),
            credentials: self.accounts.credentials(),
            audio_cache: self.paths.audio_cache(),
            page: report::page_state(&self.state),
        };
        // A run that is only being looked at opens no Explorer window.
        let reveal = self.report_folder.is_none();
        let done = waking(ctx, &self.events_tx, Event::ReportSaved);
        let spawned = std::thread::Builder::new()
            .name("report".into())
            .spawn(move || {
                let saved = report::save(&request);
                if let (Ok(zip), true) = (&saved, reveal)
                    && let Err(error) = shell::reveal(zip)
                {
                    log::warn!("the report could not be shown in its folder: {error}");
                }
                done(saved);
            });
        if let Err(error) = spawned {
            self.actions
                .push(Action::ReportSaved(Err(error.to_string())));
        }
    }

    /// Asks the system, off the UI thread, whether the app starts with it;
    /// and first points an entry left by a copy that has moved at this one.
    pub(super) fn ask_start_at_login(&self, ctx: &egui::Context) {
        let known = waking(ctx, &self.events_tx, Event::StartsAtLogin);
        let refresh = sidecar::packaged();
        let spawned = std::thread::Builder::new()
            .name("autostart".into())
            .spawn(move || {
                if refresh {
                    autostart::refresh();
                }
                known(autostart::is_enabled());
            });
        if let Err(error) = spawned {
            log::warn!("start at login could not be asked about: {error}");
        }
    }
}

/// Whether a window of the app's other than the main one exists to be
/// drawn in this frame; it does from the frame after it is first wanted.
///
/// The window toolkit can only make the window for a viewport drawn from
/// inside the main window's frame while its event loop is at hand, and on
/// the frames it runs for a main window that is hidden or minimised it is
/// not: asking then stops the app. A viewport that draws itself has its
/// window made after the frame, where the loop is always at hand. So the
/// window is first asked for that way, hidden and empty, and drawn from
/// here once it is there. The mini player and the flyout are both opened
/// from the tray, which is exactly when the main window is away.
fn window_made(
    ctx: &egui::Context,
    made: &mut bool,
    open: bool,
    id: egui::ViewportId,
    builder: &egui::ViewportBuilder,
) -> bool {
    if !open {
        *made = false;
        return false;
    }
    if *made {
        return true;
    }
    *made = true;
    let hidden = builder.clone().with_visible(false);
    ctx.show_viewport_deferred(id, hidden, |_, _| {});
    ctx.request_repaint();
    false
}

/// The version of an update that is downloaded and waiting.
fn waiting_update(state: &State) -> Option<&str> {
    match &state.update {
        update::Status::Ready { version, .. } => Some(version),
        _ => None,
    }
}

/// Everything the tray and the taskbar show, boiled down to a number.
fn desktop_key(state: &State, window_shown: bool, scale: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    let playback = state.playback.as_ref();
    let track = playback.and_then(|playback| playback.current());
    track.map(|track| &track.id).hash(&mut hasher);
    playback
        .is_some_and(|playback| playback.wants_to_play())
        .hash(&mut hasher);
    track
        .is_some_and(|track| state.likes.is_liked(&track.id))
        .hash(&mut hasher);
    state.account.is_some().hash(&mut hasher);
    state.mini_player.hash(&mut hasher);
    window_shown.hash(&mut hasher);
    waiting_update(state).hash(&mut hasher);
    scale.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn buttons(state: &State) -> Buttons {
    let playback = state.playback.as_ref();
    let track = playback.and_then(|playback| playback.current());
    Buttons {
        has_track: track.is_some(),
        playing: playback.is_some_and(|playback| playback.wants_to_play()),
        can_like: state.account.is_some(),
        liked: track.is_some_and(|track| state.likes.is_liked(&track.id)),
    }
}

fn snapshot(state: &State, window_visible: bool) -> Snapshot {
    let buttons = buttons(state);
    let track = state
        .playback
        .as_ref()
        .and_then(|playback| playback.current());
    Snapshot {
        track: track.map(|track| (track.title.clone(), track.artist_names())),
        playing: buttons.playing,
        can_like: buttons.can_like,
        liked: buttons.liked,
        mini_open: state.mini_player,
        window_visible,
        update: waiting_update(state).map(str::to_owned),
    }
}
