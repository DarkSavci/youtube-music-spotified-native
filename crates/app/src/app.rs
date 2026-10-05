//! The frame loop: take in what happened, apply what was asked, draw.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, ColorImage};
use spotified_client::session::Command;

use crate::accounts::{AccountStore, SavedAccount};
use crate::actions::{self, Action, VideoAsk};
use crate::backend::{Backend, Response};
use crate::images::ImageLoader;
use crate::paths::Paths;
use crate::platform::media_keys::MediaKeys;
use crate::platform::taskbar::{Taskbar, ThumbAction};
use crate::platform::tray::{Tray, TrayAction};
use crate::screenshot::Screenshot;
use crate::session::{self, Session};
use crate::settings::{self, Settings};
use crate::sidecar::{self, CoreStatus, Sidecar};
use crate::single_instance::InstanceGuard;
use crate::state::{Notice, Page, Playback, State};
use crate::{
    changelog, migrate, milkdrop, resolver, skins, theme, themes, together, update, views,
};

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
/// As `SEARCH_DEBOUNCE`, for the search inside a Listen Together room.
const ROOM_SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// What background threads tell the UI thread.
pub enum Event {
    /// What the core started for this run says of itself. The number
    /// tells it from the last words of a core that was stopped on purpose.
    Core(u64, CoreStatus),
    /// An answer from the backend started for this run of the core. The
    /// number tells it from a late answer by an earlier run.
    Api(u64, Box<Response>),
    /// No core is running, and one may start: the old one has stopped,
    /// or what had to be done before the first has been.
    CoreStopped,
    Session(session::Update),
    /// A media key, or a button on the system's now-playing card.
    Media(Action),
    Tray(TrayAction),
    /// A button on the taskbar thumbnail.
    Thumb(ThumbAction),
    /// The browser sign-in ended: with the account it made, or with a
    /// sentence saying why not.
    SignedIn(Result<SavedAccount, String>),
    /// A look for a newer yt-dlp that was asked for ended, with what there
    /// is to say of it.
    ResolverUpdated(Result<String, String>),
    /// The problem report is made, or could not be.
    ReportSaved(Result<PathBuf, String>),
    /// The system has said whether the app starts with it.
    StartsAtLogin(bool),
    /// Where looking for a newer version of the app has got to.
    Update(update::Status),
    /// The sound devices there are to play through.
    OutputDevices(Vec<crate::settings::OutputDevice>),
    /// The line to a Listen Together relay has something to say.
    Together(together::Event),
    /// A test of a relay's address ended.
    TogetherProbed(Result<(), String>),
    /// The Electron app's profile was looked at, or brought from.
    Migration(migrate::Report),
    /// A pack of MilkDrop presets was fetched, or could not be.
    PresetsFetched(Result<usize, String>),
    /// The skin files chosen in the file dialog; none if it was cancelled.
    SkinsPicked(Vec<PathBuf>),
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
    /// Where the Electron app's profile is, when not in the usual place.
    pub old_profile: Option<PathBuf>,
    /// What to do at once, as `--open` gives it: pages to open, a track
    /// to play.
    pub open: Vec<String>,
    /// When the process began, for the time-to-first-frame log line.
    pub started: Instant,
}

mod accounts;
mod closing;
mod desktop;
mod effects;
mod migration;
mod open;
mod script;
mod video;

use open::{opening_action, opening_commands, waits};
use script::Script;

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
    /// `None` where the taskbar offers no buttons on a thumbnail.
    taskbar: Option<Taskbar>,
    /// What the tray and the taskbar were last told, as a hash of it.
    desktop_shown: Option<u64>,
    /// The main window is on screen rather than away in the tray.
    window_shown: bool,
    /// The opening of the flyout whose window has had its corners rounded.
    flyout_rounded: Option<Instant>,
    /// The windows of the flyout and of the mini player exist.
    flyout_made: bool,
    mini_made: bool,
    /// The saved accounts, and the folders they keep their files in.
    accounts: AccountStore,
    /// What is to change about them once the core has stopped.
    pending_change: Option<accounts::Change>,
    /// The Electron app's profile, and what has been brought from it.
    mover: migration::Mover,
    /// When to look for a newer yt-dlp next; `None` for a copy that leaves
    /// the one it came with alone.
    resolver_due: Option<Instant>,
    /// Where a problem report goes instead of the Downloads folder, for a
    /// run that is only being looked at.
    report_folder: Option<PathBuf>,
    /// Quit was chosen: the next close is a real one.
    quitting: bool,
    /// Held so no second copy opens this profile.
    _instance: InstanceGuard,
    /// Commands waiting for the session to start.
    held_commands: Vec<Command>,
    /// The speed the engine was last told to play at.
    speed_applied: f32,
    /// Whether the engine was last told that the sound is being drawn.
    sound_watched: bool,
    /// MilkDrop's window, which is a process of its own.
    milkdrop: milkdrop::host::Host,
    /// The shape the mini player's window was last cut to, as a hash of
    /// it: a skin that is not a rectangle has one.
    mini_shape: Option<u64>,
    images: ImageLoader,
    /// When the search on screen is to be sent, if typing has not resumed.
    search_due: Option<Instant>,
    /// As `search_due`, for the search inside a Listen Together room.
    room_search_due: Option<Instant>,
    /// The installer is already running: the update was asked for now.
    installing: bool,
    /// As `search_due`, for the lookup in Your listening.
    lookup_due: Option<Instant>,
    /// Requests sent to the backend and not yet answered.
    requests_in_flight: usize,
    /// Asked for by `--open` and waiting for the player to be up.
    script: Script,
    /// The line to a Listen Together relay, while there is one.
    together: Option<together::Connection>,
    /// When the player was last checked against the room.
    together_ticked: Instant,
    /// When to look for a newer version next; `None` for a copy that does
    /// not update itself.
    update_due: Option<Instant>,
    screenshot: Option<Screenshot>,
    /// The music video's decoder and the texture its pictures go into.
    video: video::Screen,
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
        state.skins = skins::list(&launch.paths.skins_folder());
        state.milkdrop.presets = milkdrop::list_presets(&launch.paths.milkdrop_folder()).len();
        // What the system is set to is not known until a frame has run;
        // until then, dark, which `wear_theme` puts right.
        let theme = &state.settings;
        let custom = theme.custom_theme.as_deref();
        state.palette = themes::palette(theme.theme, custom, &state.themes, true);
        let mover = migration::Mover {
            old_profile: migrate::old_profile(launch.old_profile.as_deref(), launch.demo),
            record: migrate::Record::load(&launch.paths.config),
        };
        let accounts = AccountStore::load(&launch.paths.config);
        state.accounts = accounts.list.clone();
        state.channel_id = accounts
            .list
            .active()
            .map(|account| account.channel.clone())
            .unwrap_or_default();
        // A version that has not run here before, on a profile that has
        // run another, is an update: say so once.
        if state.settings.last_seen_version != changelog::VERSION {
            if !state.settings.last_seen_version.is_empty() {
                let version = changelog::VERSION;
                let said = format!("Updated to {version}");
                state.toast_with_link(said, "See what's new", Page::Changelog);
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
        crate::platform::identity::dress_menus(state.palette.dark);
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
        let on_thumb = waking(&context.egui_ctx, &events_tx, Event::Thumb);
        let taskbar = Taskbar::attach(context, on_thumb);
        // A screenshot run closes for real when it is done.
        let tray = launch
            .screenshot
            .is_none()
            .then(|| Tray::new(waking(&context.egui_ctx, &events_tx, Event::Tray)))
            .flatten();
        // Started at sign-in: stay out of the way, in the tray. Without a
        // tray there would be no way back to the window, so it shows.
        let hidden = launch.hidden && tray.is_some();
        if hidden {
            context
                .egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        // Only a copy put together to be run keeps yt-dlp current: a build
        // in a working tree uses the one fetched for it.
        let resolver_due = (!launch.demo && sidecar::packaged())
            .then(|| launch.started + resolver::FIRST_CHECK_AFTER);
        let report_folder = launch
            .demo
            .then(|| launch.paths.cache.with_file_name("reports"));
        // The times a room shows are this computer's, not Greenwich's. Where
        // the system will not say how far apart they are, Greenwich's it is.
        state.together.zone_minutes = time::UtcOffset::current_local_offset()
            .map_or(0, |offset| i32::from(offset.whole_minutes()));
        // Some of what `--open` asks for waits until the app has settled.
        let (deferred, at_once) = launch
            .open
            .iter()
            .partition::<Vec<_>, _>(|spec| waits(spec));
        let actions = at_once.into_iter().filter_map(|spec| opening_action(spec));
        let actions = actions.collect();
        let script = Script::new(deferred.into_iter().cloned());
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
            held_commands: opening_commands(&launch.open),
            speed_applied: 1.0,
            sound_watched: false,
            milkdrop: milkdrop::host::Host::default(),
            mini_shape: None,
            media_keys,
            tray,
            taskbar,
            desktop_shown: None,
            window_shown: !hidden,
            flyout_rounded: None,
            flyout_made: false,
            mini_made: false,
            accounts,
            pending_change: None,
            mover,
            resolver_due,
            report_folder,
            quitting: false,
            _instance: launch.instance,
            images,
            search_due: None,
            room_search_due: None,
            installing: false,
            lookup_due: None,
            requests_in_flight: 0,
            update_due,
            script,
            together: None,
            together_ticked: launch.started,
            screenshot: launch.screenshot.map(Screenshot::new),
            video: video::Screen::default(),
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
            crate::platform::identity::dress_menus(palette.dark);
        }
    }

    /// Reads the skin the settings name, when the mini player is open and
    /// is not wearing it yet: after one is chosen, and the first time the
    /// mini player opens. A skin that cannot be read is given up, and said
    /// to be, so the mini player goes back to being the app's own.
    fn wear_skin(&mut self, ctx: &egui::Context) {
        let Some(wanted) = self.state.settings.mini_skin.as_deref() else {
            self.state.skin = None;
            return;
        };
        if !self.state.mini_player || self.state.worn_skin().is_some() {
            return;
        }
        match skins::wear(ctx, wanted, &self.paths.skins_folder()) {
            Ok(worn) => self.state.skin = Some(worn),
            Err(error) => {
                log::warn!("the skin {wanted} could not be read: {error}");
                let failed = skins::Ask::Failed(error.to_string());
                self.actions.push(Action::Skin(failed));
            }
        }
    }

    /// Tells the engine whether anything draws the sound, when that has
    /// changed: the visualizer behind the player bar, or a skin's analyser
    /// while the mini player is open. Copying the sound aside costs a
    /// little, and is not done for nobody.
    fn watch_sound(&mut self) {
        let watched = self.state.watches_sound();
        if watched != self.sound_watched
            && let Some(session) = &self.session
        {
            session.tap().set_watching(watched);
            self.sound_watched = watched;
        }
    }

    /// Keeps MilkDrop's window in sound while it is open, and notices when
    /// it has been closed from inside it.
    fn keep_milkdrop(&mut self, ctx: &egui::Context) {
        if !self.state.milkdrop.open {
            return;
        }
        if !self.milkdrop.is_running() {
            self.actions.push(Action::MilkDrop(milkdrop::Ask::Closed));
            return;
        }
        if let Some(tap) = &self.state.audio_tap {
            self.milkdrop.feed(tap);
        }
        // Nothing else says that the window has gone.
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// Stills the animations when the settings ask for that, and lets them
    /// move again when they do not. egui's own (a switch's knob, a scroll
    /// brought to a line) follow its style; the app's read the same figure.
    fn keep_motion(&self, ctx: &egui::Context) {
        let still = self.state.settings.reduce_motion;
        if still == views::widgets::still(ctx) {
            return;
        }
        ctx.all_styles_mut(|style| {
            style.animation_time = if still { 0.0 } else { theme::ANIMATION_TIME };
            style.scroll_animation = if still {
                egui::style::ScrollAnimation::none()
            } else {
                egui::style::ScrollAnimation::default()
            };
        });
    }

    /// Closing the window hides it instead, when there is a tray to live in
    /// and the setting asks for that. Quit, from the tray, closes for real.
    fn close_to_tray(&mut self, ctx: &egui::Context) {
        let closing = ctx.input(|input| input.viewport().close_requested());
        if closing && !self.quitting && self.tray.is_some() && self.state.settings.close_to_tray {
            self.window_shown = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn take_in(&mut self, ctx: &egui::Context, event: Event) {
        match event {
            // The last words of a core that was stopped on purpose.
            Event::Core(run, _) if run != self.core_run => {}
            Event::Core(_, status) => {
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
                            // Before anything is asked to play, so the first
                            // track is loaded as the settings have it.
                            effects::apply_audio_settings(&session, &self.state);
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
            Event::CoreStopped => self.core_stopped(ctx),
            Event::ReportSaved(result) => self.actions.push(Action::ReportSaved(result)),
            Event::StartsAtLogin(on) => self.actions.push(Action::StartAtLoginKnown(on)),
            Event::Thumb(action) => self.thumb_asked(action),
            Event::ResolverUpdated(result) => self.actions.push(Action::ResolverUpdated(result)),
            Event::Update(status) => self.actions.push(Action::UpdateChanged(status)),
            Event::OutputDevices(devices) => {
                self.actions.push(Action::OutputDevicesListed(devices));
            }
            Event::TogetherProbed(result) => {
                self.actions
                    .push(Action::Room(together::Ask::Tested(result)));
            }
            Event::Together(event) => self.heard_from_relay(event),
            Event::Media(action) => self.actions.push(action),
            Event::SignedIn(Ok(account)) => {
                log::info!("signed in; restarting the playback service");
                self.change_account(ctx, accounts::Change::Add(account));
            }
            Event::SignedIn(Err(reason)) => {
                log::warn!("sign-in failed: {reason}");
                self.actions.push(Action::SignInFailed(reason));
            }
            Event::Tray(action) => self.tray_asked(ctx, action),
            Event::Session(session::Update::Projection(projection)) => {
                self.actions.push(Action::SessionChanged(projection));
            }
            // Shown to the person once there are toasts to show it in.
            Event::Session(session::Update::Refused(reason)) => {
                log::warn!("{reason}");
                self.actions.push(Action::Video(VideoAsk::Refused));
            }
            Event::Session(session::Update::RateLimited) => {
                self.actions.push(Action::Notify(Notice::RateLimited));
            }
            Event::Migration(report) => self.migration_reported(ctx, report),
            Event::PresetsFetched(result) => {
                let folder = self.paths.milkdrop_folder();
                self.state.milkdrop.presets = milkdrop::list_presets(&folder).len();
                let fetched = milkdrop::Ask::Fetched(result);
                self.actions.push(Action::MilkDrop(fetched));
            }
            Event::SkinsPicked(files) => {
                self.actions.push(Action::Skin(skins::Ask::Install(files)));
            }
            Event::Image(url, image) => self.state.images.loaded(ctx, url, image),
        }
    }
}

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
            self.first_start(ctx);
            self.look_for_old_app(ctx);
            if !self.demo {
                self.ask_start_at_login(ctx);
            }
        }
        self.frames += 1;

        self.close_to_tray(ctx);
        self.wear_theme(ctx);
        self.wear_skin(ctx);
        self.keep_motion(ctx);
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
        self.keep_resolver_current(ctx);
        let settled = self.state.playback.as_ref().is_some_and(|playback| {
            self.held_commands.is_empty() && (playback.current().is_none() || playback.is_playing())
        });
        self.run_script(ctx, settled);
        if self.state.together.in_room() {
            if self.together_ticked.elapsed() >= TOGETHER_TICK {
                self.together_ticked = Instant::now();
                self.actions.push(Action::TogetherTick);
            }
            ctx.request_repaint_after(TOGETHER_TICK);
        }
        self.send_due_searches();
        for action in std::mem::take(&mut self.actions) {
            for effect in actions::apply(&mut self.state, action) {
                self.run(ctx, effect);
            }
        }
        // Entering a room holds the speed to normal and leaving one lets
        // it go, neither of which is a change to the settings.
        let speed = self.state.speed();
        if speed != self.speed_applied
            && let Some(session) = &self.session
        {
            session.set_speed(speed);
            self.speed_applied = speed;
        }
        self.watch_sound();
        self.keep_milkdrop(ctx);
        if let Some(media_keys) = &mut self.media_keys {
            media_keys.show(self.state.playback.as_ref());
        }
        self.show_on_desktop(ctx);
        // A toast leaves by the clock, so the window must wake to see it go.
        if let Some(next) = self.state.expire_toasts(Instant::now()) {
            ctx.request_repaint_after(next);
        }

        if let Some(mut screenshot) = self.screenshot.take() {
            screenshot.step(ctx, self.settled(), !self.script.is_done());
            self.screenshot = Some(screenshot);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        views::show(&self.state, ui, &mut self.actions);
        self.show_mini(ui);
        self.show_flyout(ui);
        self.show_video(ui.ctx());
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

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.script.feed(raw_input);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.state.palette.window.to_normalized_gamma_f32()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shut_down();
    }
}
