//! The frame loop: take in what happened, apply what was asked, draw.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, ColorImage};
use spotified_client::session::Command;

use crate::actions::{self, Action};
use crate::backend::{Backend, Response};
use crate::images::ImageLoader;
use crate::paths::Paths;
use crate::platform::autostart;
use crate::platform::media_keys::MediaKeys;
use crate::platform::tray::{Tray, TrayAction};
use crate::screenshot::Screenshot;
use crate::session::{self, Session};
use crate::settings::{self, Settings};
use crate::sidecar::{self, CoreStatus, Sidecar, SidecarConfig};
use crate::single_instance::InstanceGuard;
use crate::state::{Playback, State};
use crate::{changelog, channel, import, theme, themes, together, update, views};

/// Recorded responses for `--demo`, relative to the repository root.
const FIXTURES: &str = "core/testdata/fixtures";
/// A redraw while a track plays, should nothing else prompt one. The core's
/// position reports already redraw the progress bar four times a second;
/// this only keeps it moving if they stop.
const PLAYING_REPAINT: Duration = Duration::from_secs(1);
/// How long after starting the first look for a newer version waits, and
/// how long between looks after that.
const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(30);
const UPDATE_CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often, in a room, the player is checked against it.
const TOGETHER_TICK: Duration = Duration::from_secs(1);
/// How long typing must pause before a search is sent.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(280);

/// What background threads tell the UI thread.
pub enum Event {
    Core(CoreStatus),
    /// An answer from the backend started for this run of the core. The
    /// number tells it from a late answer by an earlier run.
    Api(u64, Box<Response>),
    /// The old core has stopped; a new one may start.
    CoreStopped,
    Session(session::Update),
    /// A media key, or a button on the system's now-playing card.
    Media(Action),
    Tray(TrayAction),
    /// The browser sign-in ended: well, or with a sentence saying why not.
    SignedIn(Result<(), String>),
    /// yt-dlp's updater ended, with what it said.
    ResolverUpdated(Result<String, String>),
    /// Where looking for a newer version of the app has got to.
    Update(update::Status),
    /// The line to a Listen Together relay has something to say.
    Together(together::Event),
    /// Artwork for a URL, or `None` if it could not be had.
    Image(String, Option<ColorImage>),
}

pub struct Launch {
    pub paths: Paths,
    /// This copy's hold on the profile.
    pub instance: InstanceGuard,
    pub settings: Settings,
    pub demo: bool,
    pub screenshot: Option<PathBuf>,
    /// Start without showing the window.
    pub hidden: bool,
    /// What to do at once, as `--open` gives it: pages to open, a track
    /// to play.
    pub open: Vec<String>,
    /// When the process began, for the time-to-first-frame log line.
    pub started: Instant,
}

mod effects;
mod open;

use open::opening_action;

pub struct App {
    state: State,
    /// Asked for by the views this frame; applied at the start of the next.
    actions: Vec<Action>,
    events: Receiver<Event>,
    events_tx: Sender<Event>,
    paths: Paths,
    demo: bool,
    started: Instant,
    frames: u64,
    /// `None` until the first frame is on screen, and again if it failed
    /// to start. Dropping it stops the core.
    sidecar: Option<Sidecar>,
    /// `None` until the core says where it is listening.
    backend: Option<Backend>,
    /// Counts runs of the core, so answers from an earlier run are dropped.
    core_run: u64,
    /// Playback: the session with the core, and the audio engine.
    session: Option<Session>,
    /// `None` where the system offers no media controls.
    media_keys: Option<MediaKeys>,
    /// `None` where the system has no notification area.
    tray: Option<Tray>,
    /// Quit was chosen: the next close is a real one.
    quitting: bool,
    /// Held so no second copy opens this profile.
    _instance: InstanceGuard,
    /// Commands waiting for the session to start.
    held_commands: Vec<Command>,
    images: ImageLoader,
    /// When the search on screen is to be sent, if typing has not resumed.
    search_due: Option<Instant>,
    /// Requests sent to the backend and not yet answered.
    requests_in_flight: usize,
    /// Asked for by `--open` and waiting for the player to be up.
    deferred: Vec<Action>,
    /// The line to a Listen Together relay, while there is one.
    together: Option<together::Connection>,
    /// When the player was last checked against the room.
    together_ticked: Instant,
    /// When to look for a newer version next; `None` for a copy that does
    /// not update itself.
    update_due: Option<Instant>,
    screenshot: Option<Screenshot>,
}

impl App {
    pub fn new(context: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        let mut settings = launch.settings;
        if settings.device_id.is_empty() {
            settings.device_id = settings::new_device_id();
            if let Err(error) = settings::save(&launch.paths.settings_file(), &settings) {
                log::warn!("settings could not be saved: {error}");
            }
        }
        let mut state = State::new(settings);
        let themes_folder = launch.paths.themes_folder();
        themes::write_presets(&themes_folder);
        state.themes = themes::list(&themes_folder);
        // What the system is set to is not known until a frame has run;
        // until then, dark, which `wear_theme` puts right.
        let theme = &state.settings;
        let custom = theme.custom_theme.as_deref();
        state.palette = themes::palette(theme.theme, custom, &state.themes, true);
        // A demo shows no account, so it offers none to copy.
        if !launch.demo {
            state.import_source = import::electron_credentials();
        }
        state.channel_id = channel::active(&launch.paths.credentials_file());
        // A version that has not run here before, on a profile that has
        // run another, is an update: say so once.
        if state.settings.last_seen_version != changelog::VERSION {
            if !state.settings.last_seen_version.is_empty() {
                let version = changelog::VERSION;
                state.toast(format!(
                    "Updated to {version}. What's new is in the account menu."
                ));
            }
            state.settings.last_seen_version = changelog::VERSION.to_owned();
            if let Err(error) = settings::save(&launch.paths.settings_file(), &state.settings) {
                log::warn!("settings could not be saved: {error}");
            }
        }
        let update_due = if !launch.demo && update::installed() {
            Some(launch.started + FIRST_UPDATE_CHECK)
        } else {
            state.update = update::Status::Unavailable;
            None
        };
        theme::install(&context.egui_ctx, &state.palette);
        let (events_tx, events) = crossbeam_channel::unbounded();
        let deliver = waking(&context.egui_ctx, &events_tx, |(url, image)| {
            Event::Image(url, image)
        });
        let images = ImageLoader::start(launch.paths.cache.join("art"), move |url, image| {
            deliver((url, image));
        });
        let media_keys =
            MediaKeys::attach(context, waking(&context.egui_ctx, &events_tx, Event::Media));
        // A later launch of the app means "show me the window".
        let show = waking(&context.egui_ctx, &events_tx, |()| {
            Event::Tray(TrayAction::Show)
        });
        if let Err(error) = launch.instance.listen(move || show(())) {
            log::warn!("later launches cannot reach this one: {error}");
        }
        // A screenshot run closes for real when it is done.
        let tray = launch
            .screenshot
            .is_none()
            .then(|| Tray::new(waking(&context.egui_ctx, &events_tx, Event::Tray)))
            .flatten();
        // Started at sign-in: stay out of the way, in the tray. Without a
        // tray there would be no way back to the window, so it shows.
        if launch.hidden && tray.is_some() {
            context
                .egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        state.starts_at_login = !launch.demo && autostart::is_enabled();
        // What `--open` asks of a room waits for the player: a room is
        // entered with the music that is playing, or with none.
        let (deferred, actions) = launch
            .open
            .iter()
            .filter_map(|spec| Some((spec.starts_with("together-"), opening_action(spec)?)))
            .partition::<Vec<_>, _>(|(for_a_room, _)| *for_a_room);
        let strip = |list: Vec<(bool, Action)>| list.into_iter().map(|(_, action)| action);
        let (deferred, actions) = (strip(deferred).collect(), strip(actions).collect());
        Self {
            state,
            actions,
            events,
            events_tx,
            paths: launch.paths,
            demo: launch.demo,
            started: launch.started,
            frames: 0,
            sidecar: None,
            backend: None,
            core_run: 0,
            session: None,
            held_commands: Vec::new(),
            media_keys,
            tray,
            quitting: false,
            _instance: launch.instance,
            images,
            search_due: None,
            requests_in_flight: 0,
            update_due,
            deferred,
            together: None,
            together_ticked: launch.started,
            screenshot: launch.screenshot.map(Screenshot::new),
        }
    }

    fn start_core(&mut self, ctx: &egui::Context) {
        let config = SidecarConfig {
            credentials: self.paths.credentials_file(),
            database: self.paths.database_file(),
            fixtures: self.demo.then(|| sidecar::locate(FIXTURES)).flatten(),
        };
        let report = waking(ctx, &self.events_tx, Event::Core);
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

    /// Wears the theme the settings name, when it is not the one worn:
    /// after a choice in Settings, or when the system goes light or dark.
    fn wear_theme(&mut self, ctx: &egui::Context) {
        let system_dark = ctx.system_theme() != Some(egui::Theme::Light);
        let settings = &self.state.settings;
        let custom = settings.custom_theme.as_deref();
        let palette = themes::palette(settings.theme, custom, &self.state.themes, system_dark);
        if palette != self.state.palette {
            self.state.palette = palette;
            theme::apply(ctx, &palette);
        }
    }

    /// Closing the window hides it instead, when there is a tray to live in
    /// and the setting asks for that. Quit, from the tray, closes for real.
    fn close_to_tray(&self, ctx: &egui::Context) {
        let closing = ctx.input(|input| input.viewport().close_requested());
        if closing && !self.quitting && self.tray.is_some() && self.state.settings.close_to_tray {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    /// Stops the core and starts it again, to pick up new credentials. The
    /// stop can take a moment (the core saves its state), so it happens on
    /// another thread, and the new core starts only once the old has gone:
    /// the two must never share the database.
    fn restart_core(&mut self, ctx: &egui::Context) {
        self.backend = None;
        self.session = None;
        self.core_run += 1;
        self.requests_in_flight = 0;
        let Some(old) = self.sidecar.take() else {
            self.forget_cached_answers();
            self.start_core(ctx);
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

    /// Deletes the answers the core kept from YouTube. They belong to the
    /// account and channel that asked; another must not be shown them. Only
    /// done while no core is running, since it holds the file open.
    fn forget_cached_answers(&self) {
        for suffix in ["", "-wal", "-shm"] {
            let file = self.paths.config.join(format!("responses.db{suffix}"));
            if let Err(error) = std::fs::remove_file(&file)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                log::warn!("{} could not be deleted: {error}", file.display());
            }
        }
    }

    fn take_in(&mut self, ctx: &egui::Context, event: Event) {
        match event {
            Event::Core(status) => {
                log::info!("playback service: {status:?}");
                if let CoreStatus::Ready { origin } = &status {
                    let run = self.core_run;
                    let deliver = waking(ctx, &self.events_tx, move |response| {
                        Event::Api(run, Box::new(response))
                    });
                    self.backend = Some(Backend::start(origin, deliver));
                    let updates = waking(ctx, &self.events_tx, Event::Session);
                    let device_id = self.state.settings.device_id.clone();
                    let normalise = self.state.settings.normalise_volume;
                    match Session::start(origin, device_id, normalise, updates) {
                        Ok(session) => {
                            let settings = &self.state.settings;
                            session.set_equalizer(settings.equalizer_on, settings.equalizer);
                            session.set_crossfade(settings.crossfade_seconds);
                            session.tap().set_watching(settings.visualizer);
                            for command in self.held_commands.drain(..) {
                                session.send(command);
                            }
                            self.state.audio_tap = Some(session.tap());
                            self.session = Some(session);
                        }
                        Err(error) => log::error!("playback could not start: {error}"),
                    }
                }
                self.actions.push(Action::CoreChanged(status));
            }
            Event::Api(run, response) => {
                if run == self.core_run {
                    self.requests_in_flight = self.requests_in_flight.saturating_sub(1);
                    self.actions.push(Action::Loaded(response));
                }
            }
            Event::CoreStopped => {
                self.forget_cached_answers();
                self.start_core(ctx);
            }
            Event::ResolverUpdated(result) => self.actions.push(Action::ResolverUpdated(result)),
            Event::Update(status) => self.actions.push(Action::UpdateChanged(status)),
            Event::Together(event) => {
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
            Event::Media(action) => self.actions.push(action),
            Event::SignedIn(Ok(())) => {
                log::info!("signed in; restarting the playback service");
                self.restart_core(ctx);
                self.actions.push(Action::AccountChanged);
            }
            Event::SignedIn(Err(reason)) => {
                log::warn!("sign-in failed: {reason}");
                self.actions.push(Action::SignInFailed(reason));
            }
            Event::Tray(TrayAction::Show) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Event::Tray(TrayAction::Quit) => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Event::Tray(TrayAction::TogglePlay) => self.actions.push(Action::TogglePlay),
            Event::Tray(TrayAction::Next) => self.actions.push(Action::Next),
            Event::Tray(TrayAction::Previous) => self.actions.push(Action::Previous),
            Event::Session(session::Update::Projection(projection)) => {
                self.actions.push(Action::SessionChanged(projection));
            }
            // Shown to the person once there are toasts to show it in.
            Event::Session(session::Update::Refused(reason)) => log::warn!("{reason}"),
            Event::Image(url, image) => self.state.images.loaded(ctx, url, image),
        }
    }

    /// When something has been asked to play, whether it has been playing
    /// for a moment.
    fn heard_if_playing(&self) -> bool {
        match &self.state.playback {
            Some(playback) if playback.wants_to_play() => {
                playback.is_playing() && playback.position_ms(Instant::now()) >= SHOT_PLAYED_MS
            }
            _ => true,
        }
    }

    /// Whether the window shows what it is going to show, for `--screenshot`.
    fn settled(&self) -> bool {
        self.state.core != CoreStatus::Starting
            && self.requests_in_flight == 0
            && self.search_due.is_none()
            && !self.state.images.loading()
            && self.heard_if_playing()
    }
}

/// How far into a track a screenshot waits, so it shows playback under way.
const SHOT_PLAYED_MS: u64 = 3000;

/// A callback for a background thread: wraps what it reports as an event,
/// queues it, and wakes the window, which would otherwise sleep until the
/// mouse moved.
fn waking<T>(
    ctx: &egui::Context,
    events: &Sender<Event>,
    wrap: impl Fn(T) -> Event + Send + Clone + 'static,
) -> impl Fn(T) + Send + Clone + 'static {
    let ctx = ctx.clone();
    let events = events.clone();
    move |value| {
        let _ = events.send(wrap(value));
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The core is started once a frame has been drawn, so the window
        // never waits for a child process.
        if self.frames == 1 {
            log::info!(
                "first frame after {} ms",
                self.started.elapsed().as_millis()
            );
            self.start_core(ctx);
        }
        self.frames += 1;

        self.close_to_tray(ctx);
        self.wear_theme(ctx);
        while let Ok(event) = self.events.try_recv() {
            self.take_in(ctx, event);
        }
        if let Some(due) = self.update_due {
            let now = Instant::now();
            if now >= due {
                self.update_due = Some(now + UPDATE_CHECK_EVERY);
                self.actions.push(Action::CheckForUpdate);
            } else {
                // The window sleeps when nothing happens; this wakes it.
                ctx.request_repaint_after(due - now);
            }
        }
        let settled = self.state.playback.as_ref().is_some_and(|playback| {
            self.held_commands.is_empty() && (playback.current().is_none() || playback.is_playing())
        });
        if settled && !self.deferred.is_empty() {
            self.actions.append(&mut self.deferred);
        }
        if self.state.together.in_room() {
            if self.together_ticked.elapsed() >= TOGETHER_TICK {
                self.together_ticked = Instant::now();
                self.actions.push(Action::TogetherTick);
            }
            ctx.request_repaint_after(TOGETHER_TICK);
        }
        if self.search_due.is_some_and(|due| Instant::now() >= due) {
            self.search_due = None;
            self.actions.push(Action::RunSearch);
        }
        for action in std::mem::take(&mut self.actions) {
            for effect in actions::apply(&mut self.state, action) {
                self.run(ctx, effect);
            }
        }
        if let Some(media_keys) = &mut self.media_keys {
            media_keys.show(self.state.playback.as_ref());
        }
        // A toast leaves by the clock, so the window must wake to see it go.
        if let Some(next) = self.state.expire_toasts(Instant::now()) {
            ctx.request_repaint_after(next);
        }

        if let Some(mut screenshot) = self.screenshot.take() {
            screenshot.step(ctx, self.settled());
            self.screenshot = Some(screenshot);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        views::show(&self.state, ui, &mut self.actions);
        // The mini player is a window of its own, drawn from here so it
        // reads the same state and asks through the same actions.
        if self.state.mini_player {
            let (state, actions) = (&self.state, &mut self.actions);
            let builder = views::mini::builder(state);
            ui.ctx()
                .show_viewport_immediate(views::mini::viewport(), builder, |ui, _| {
                    views::mini::show(state, ui, actions);
                });
        }
        self.state.images.end_frame(&self.images);
        if self
            .state
            .playback
            .as_ref()
            .is_some_and(Playback::is_playing)
        {
            ui.ctx().request_repaint_after(PLAYING_REPAINT);
        }
        if !self.actions.is_empty() {
            // What was just asked for is applied on the next frame; without
            // this the window would sit unchanged until the mouse moved.
            ui.ctx().request_repaint();
        } else if self.frames <= 1 {
            // The first frame has no input to prompt a second.
            ui.ctx().request_repaint_after(Duration::from_millis(1));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.state.palette.window.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // What moved since the last save, such as the mini player's place.
        if let Err(error) = settings::save(&self.paths.settings_file(), &self.state.settings) {
            log::warn!("settings could not be saved: {error}");
        }
        // Stop the core here, while the log is still open to say so.
        self.session = None;
        self.backend = None;
        self.sidecar = None;
        log::info!("closed");
        log::logger().flush();
    }
}
