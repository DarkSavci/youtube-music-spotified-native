//! The MilkDrop window, as the process it runs in: this program started
//! with `--milkdrop-child`, with a window, an OpenGL context and an event
//! loop of its own.
//!
//! The app starts it and tells it where the sound, the presets and the
//! library are. It ends when its window is closed, when the app says
//! `quit` on its standard input, or when that input closes because the
//! app has gone. What goes wrong is said on standard error, which the app
//! writes to its log.
//!
//! Keys: Space, N or Right for the next preset, Backspace, P or Left for
//! the one before, L to stay on this one, F or F11 (or a double click)
//! for the whole screen, Escape to leave it or to close.

use std::ffi::{CStr, CString, c_char, c_void};
use std::io::BufRead;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentContext};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::surface::{GlSurface, Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

use super::library::{self, Api, Handle, STEREO};
use super::ring::Ring;

/// The flag that starts this program as the MilkDrop window.
pub const FLAG: &str = "--milkdrop-child";
const TITLE: &str = "MilkDrop";
/// The window's size when it opens, and the least it is dragged to.
const SIZE: LogicalSize<f64> = LogicalSize::new(640.0, 480.0);
const LEAST: LogicalSize<f64> = LogicalSize::new(320.0, 240.0);
/// How long one preset takes to fade into the next, as MilkDrop had it.
const FADE_SECONDS: f64 = 2.7;
/// Two clicks this close together are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// What projectM asked for, as its callback left it.
const NOTHING: u8 = 0;
const FADE: u8 = 1;
const CUT: u8 = 2;

/// What the app tells the window on starting it.
pub struct Args {
    /// The file the sound is shared through.
    ring: PathBuf,
    /// The folder of presets.
    presets: PathBuf,
    /// libprojectM.
    library: PathBuf,
    /// How long a preset plays.
    seconds: u32,
}

impl Args {
    /// Reads what follows [`FLAG`] on the command line.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Option<Self> {
        let (mut ring, mut presets, mut library) = (None, None, None);
        let mut seconds = super::PRESET_SECONDS;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let value = args.next()?;
            match arg.as_str() {
                "--ring" => ring = Some(PathBuf::from(value)),
                "--presets" => presets = Some(PathBuf::from(value)),
                "--library" => library = Some(PathBuf::from(value)),
                "--seconds" => seconds = value.parse().ok()?,
                _ => return None,
            }
        }
        Some(Self {
            ring: ring?,
            presets: presets?,
            library: library?,
            seconds: seconds.clamp(2, 3600),
        })
    }

    /// The same, as the command line the app starts the window with.
    pub fn command_line(
        ring: &std::path::Path,
        presets: &std::path::Path,
        library: &std::path::Path,
    ) -> Vec<std::ffi::OsString> {
        let mut line: Vec<std::ffi::OsString> = vec![FLAG.into()];
        for (flag, path) in [
            ("--ring", ring),
            ("--presets", presets),
            ("--library", library),
        ] {
            line.push(flag.into());
            line.push(path.into());
        }
        line
    }
}

/// What reaches the window from outside it.
enum Control {
    Quit,
}

/// Runs the window until it is closed. Returns the process's exit code.
pub fn run(args: &Args) -> i32 {
    let api = match library::load(&args.library) {
        Ok(api) => api,
        Err(error) => return failed(&error),
    };
    let ring = match Ring::open(&args.ring) {
        Ok(ring) => ring,
        Err(error) => return failed(&format!("the sound could not be reached: {error}")),
    };
    let event_loop = match EventLoop::<Control>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => return failed(&format!("no event loop: {error}")),
    };
    // The app's word, or its going, ends the window.
    let proxy = event_loop.create_proxy();
    let listening = std::thread::Builder::new()
        .name("milkdrop-control".into())
        .spawn(move || {
            // Until the word is given, or there is nobody left to give it.
            let lines = std::io::stdin().lock().lines();
            let _ = lines.map_while(Result::ok).find(|line| line == "quit");
            let _ = proxy.send_event(Control::Quit);
        });
    if let Err(error) = listening {
        return failed(&format!("the app cannot be listened to: {error}"));
    }
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut child = Child {
        presets: super::list_presets(&args.presets),
        args,
        api,
        ring,
        cursor: 0,
        live: None,
        history: Vec::new(),
        switch: Box::new(AtomicU8::new(NOTHING)),
        locked: false,
        clicked: None,
        random: seed(),
        failure: None,
    };
    if let Err(error) = event_loop.run_app(&mut child) {
        return failed(&format!("the window stopped: {error}"));
    }
    match &child.failure {
        Some(error) => failed(error),
        None => 0,
    }
}

fn failed(error: &str) -> i32 {
    eprintln!("MilkDrop: {error}");
    1
}

/// A number to start choosing presets from, different on every run.
fn seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let random = std::collections::hash_map::RandomState::new();
    random.build_hasher().finish() | 1
}

/// The window while it exists, with the context its picture is drawn in.
struct Live {
    window: Window,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    handle: Handle,
}

struct Child<'a> {
    args: &'a Args,
    api: Api,
    ring: Ring,
    /// How much of the sound has been handed to projectM.
    cursor: u64,
    live: Option<Live>,
    presets: Vec<PathBuf>,
    /// The presets shown, by their place in the list, newest last.
    history: Vec<usize>,
    /// What projectM asked for from its callback. Boxed so its address
    /// holds for as long as projectM has it.
    switch: Box<AtomicU8>,
    /// Stay on this preset.
    locked: bool,
    /// When the picture was last clicked, to tell a double click by.
    clicked: Option<Instant>,
    random: u64,
    /// Why the window could not be made, if it could not.
    failure: Option<String>,
}

unsafe extern "C" fn switch_requested(hard_cut: bool, user_data: *mut c_void) {
    // SAFETY: `user_data` is the window's own flag, boxed so that it
    // outlives projectM, which only calls this while it exists.
    let switch = unsafe { &*user_data.cast::<AtomicU8>() };
    switch.store(if hard_cut { CUT } else { FADE }, Ordering::Relaxed);
}

unsafe extern "C" fn switch_failed(
    file: *const c_char,
    message: *const c_char,
    _user_data: *mut c_void,
) {
    let text = |pointer: *const c_char| {
        if pointer.is_null() {
            return String::new();
        }
        // SAFETY: projectM hands over terminated strings that live for
        // the call.
        let text = unsafe { CStr::from_ptr(pointer) };
        text.to_string_lossy().into_owned()
    };
    eprintln!("MilkDrop: {} would not load: {}", text(file), text(message));
}

impl Child<'_> {
    /// Makes the window, its context, and projectM in it.
    fn build(&self, event_loop: &ActiveEventLoop) -> Result<Live, String> {
        let attributes = Window::default_attributes()
            .with_title(TITLE)
            .with_inner_size(SIZE)
            .with_min_inner_size(LEAST);
        // Opaque: no alpha, so the window is never see-through.
        let template = ConfigTemplateBuilder::new().with_alpha_size(0);
        let (window, config) = DisplayBuilder::new()
            .with_window_attributes(Some(attributes))
            .build(event_loop, template, |mut configs| {
                // The builder gives at least one, or fails before this.
                #[expect(clippy::expect_used)]
                configs.next().expect("a configuration to draw with")
            })
            .map_err(|error| format!("no way to draw was found: {error}"))?;
        let window = window.ok_or("the window was not made")?;
        let raw = window
            .window_handle()
            .map_err(|error| format!("the window has no handle: {error}"))?
            .as_raw();
        let wanted = ContextAttributesBuilder::new().build(Some(raw));
        let display = config.display();
        // SAFETY: the handle is the window's, which outlives the context:
        // both are kept together and dropped together.
        let context = unsafe { display.create_context(&config, &wanted) }
            .map_err(|error| format!("no OpenGL context: {error}"))?;
        let attributes = window
            .build_surface_attributes(Default::default())
            .map_err(|error| format!("the window cannot be drawn on: {error}"))?;
        // SAFETY: as above; the surface is the window's own.
        let surface = unsafe { display.create_window_surface(&config, &attributes) }
            .map_err(|error| format!("no surface to draw on: {error}"))?;
        let context = context
            .make_current(&surface)
            .map_err(|error| format!("the OpenGL context cannot be used: {error}"))?;
        // In step with the screen: no faster than it can be seen.
        let _ = surface.set_swap_interval(&context, SwapInterval::Wait(NonZeroU32::MIN));

        // SAFETY: the context is current on this thread, which is all
        // projectM asks; a null handle is how it says it cannot work in it.
        let handle = unsafe { (self.api.projectm_create)() };
        if handle.is_null() {
            return Err("the graphics driver is short of OpenGL 3.3".into());
        }
        let folders = [
            self.args.presets.clone(),
            self.args.presets.join("textures"),
        ];
        let folders: Vec<CString> = folders
            .iter()
            .filter_map(|folder| CString::new(folder.to_string_lossy().as_bytes()).ok())
            .collect();
        let mut pointers: Vec<*const c_char> = folders.iter().map(|path| path.as_ptr()).collect();
        let flag = (&raw const *self.switch).cast_mut().cast::<c_void>();
        let size = window.inner_size();
        // SAFETY: a fresh handle; the flag outlives it; the strings live
        // for the call, which copies them.
        unsafe {
            let api = &self.api;
            (api.projectm_set_preset_switch_requested_event_callback)(
                handle,
                Some(switch_requested),
                flag,
            );
            (api.projectm_set_preset_switch_failed_event_callback)(
                handle,
                Some(switch_failed),
                std::ptr::null_mut(),
            );
            (api.projectm_set_texture_search_paths)(handle, pointers.as_mut_ptr(), pointers.len());
            (api.projectm_set_preset_duration)(handle, f64::from(self.args.seconds));
            (api.projectm_set_soft_cut_duration)(handle, FADE_SECONDS);
            // Cuts on the beat are off, as MilkDrop shipped them.
            (api.projectm_set_hard_cut_enabled)(handle, false);
            (api.projectm_set_aspect_correction)(handle, true);
            // MilkDrop's own mesh, on which a preset's equations run.
            (api.projectm_set_mesh_size)(handle, 48, 36);
            (api.projectm_set_fps)(handle, 60);
            (api.projectm_set_window_size)(handle, size.width as usize, size.height as usize);
        }
        Ok(Live {
            window,
            surface,
            context,
            handle,
        })
    }

    /// The next number of a sequence that does not repeat for a long time.
    fn roll(&mut self) -> u64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }

    /// Shows a preset by its place in the list, fading into it or cutting.
    fn show(&mut self, index: usize, smooth: bool) {
        let (Some(live), Some(preset)) = (&self.live, self.presets.get(index)) else {
            return;
        };
        let Ok(file) = CString::new(preset.to_string_lossy().as_bytes()) else {
            return;
        };
        // SAFETY: a live handle and a string that lives for the call.
        unsafe { (self.api.projectm_load_preset_file)(live.handle, file.as_ptr(), smooth) };
        self.switch.store(NOTHING, Ordering::Relaxed);
        self.name_window();
    }

    /// Says in the title which preset is showing, and whether it stays.
    fn name_window(&self) {
        let Some(live) = &self.live else {
            return;
        };
        let preset = self.history.last().and_then(|at| self.presets.get(*at));
        let name = preset.and_then(|preset| preset.file_stem());
        let mut title = match name {
            Some(name) => format!("{TITLE}: {}", name.to_string_lossy()),
            None => TITLE.to_owned(),
        };
        if self.locked {
            title.push_str(" (staying on this one)");
        }
        live.window.set_title(&title);
    }

    /// Another preset, any but the one showing.
    fn next(&mut self, smooth: bool) {
        let count = self.presets.len();
        if count == 0 {
            return;
        }
        let showing = self.history.last().copied();
        let mut index = (self.roll() % count as u64) as usize;
        if Some(index) == showing {
            index = (index + 1) % count;
        }
        self.history.push(index);
        // Enough to go back a long way, and no more.
        if self.history.len() > 256 {
            self.history.remove(0);
        }
        self.show(index, smooth);
    }

    /// The preset that was showing before this one.
    fn previous(&mut self) {
        if self.history.len() < 2 {
            return;
        }
        self.history.pop();
        if let Some(index) = self.history.last().copied() {
            self.show(index, true);
        }
    }

    fn toggle_lock(&mut self) {
        self.locked = !self.locked;
        if let Some(live) = &self.live {
            // SAFETY: a live handle.
            unsafe { (self.api.projectm_set_preset_locked)(live.handle, self.locked) };
        }
        self.name_window();
    }

    /// Gives the picture the whole screen, or takes it back. Returns
    /// whether it now has it.
    fn toggle_fullscreen(&self) -> bool {
        let Some(live) = &self.live else {
            return false;
        };
        let whole = live.window.fullscreen().is_none();
        let screen = whole.then_some(Fullscreen::Borderless(None));
        live.window.set_fullscreen(screen);
        // A pointer over a picture that fills the screen is a blemish.
        live.window.set_cursor_visible(!whole);
        whole
    }

    fn resized(&self, width: u32, height: u32) {
        let (Some(live), Some(wide), Some(tall)) =
            (&self.live, NonZeroU32::new(width), NonZeroU32::new(height))
        else {
            return;
        };
        live.surface.resize(&live.context, wide, tall);
        // SAFETY: a live handle, in its own context.
        unsafe {
            (self.api.projectm_set_window_size)(live.handle, width as usize, height as usize)
        };
    }

    /// Hands projectM the sound since the last frame and has it draw one.
    fn render(&mut self) {
        match self.switch.swap(NOTHING, Ordering::Relaxed) {
            FADE if !self.locked => self.next(true),
            CUT if !self.locked => self.next(false),
            _ => {}
        }
        let heard = self.ring.since(&mut self.cursor);
        let Some(live) = &self.live else {
            return;
        };
        // SAFETY: a live handle in its own context, current on this
        // thread; the frames are pairs of floats, left then right, which
        // is how projectM reads `len` of them.
        unsafe {
            if !heard.is_empty() {
                let samples = heard.as_ptr().cast::<f32>();
                (self.api.projectm_pcm_add_float)(live.handle, samples, heard.len() as u32, STEREO);
            }
            (self.api.projectm_opengl_render_frame)(live.handle);
        }
        if let Err(error) = live.surface.swap_buffers(&live.context) {
            eprintln!("MilkDrop: the picture could not be shown: {error}");
        }
    }

    fn close(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(live) = self.live.take() {
            // SAFETY: the handle is live and its context still current.
            unsafe { (self.api.projectm_destroy)(live.handle) };
        }
        event_loop.exit();
    }

    fn key(&mut self, key: &Key, event_loop: &ActiveEventLoop) {
        match key {
            Key::Named(NamedKey::Space | NamedKey::ArrowRight) => self.next(true),
            Key::Named(NamedKey::Backspace | NamedKey::ArrowLeft) => self.previous(),
            Key::Named(NamedKey::F11 | NamedKey::Enter) => {
                self.toggle_fullscreen();
            }
            Key::Named(NamedKey::Escape) => {
                let whole = self
                    .live
                    .as_ref()
                    .is_some_and(|live| live.window.fullscreen().is_some());
                if whole {
                    self.toggle_fullscreen();
                } else {
                    self.close(event_loop);
                }
            }
            Key::Character(letter) => match letter.to_lowercase().as_str() {
                "n" => self.next(true),
                "p" => self.previous(),
                "l" => self.toggle_lock(),
                "f" => {
                    self.toggle_fullscreen();
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl ApplicationHandler<Control> for Child<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.live.is_some() {
            return;
        }
        match self.build(event_loop) {
            Ok(live) => {
                self.live = Some(live);
                // Straight to a preset: projectM's own idle picture is
                // only for a folder with none in it.
                self.next(false);
            }
            Err(error) => {
                self.failure = Some(error);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, control: Control) {
        match control {
            Control::Quit => self.close(event_loop),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close(event_loop),
            WindowEvent::Resized(size) => self.resized(size.width, size.height),
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.key(&event.logical_key, event_loop);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let now = Instant::now();
                let again = self
                    .clicked
                    .is_some_and(|before| now.duration_since(before) < DOUBLE_CLICK);
                self.clicked = (!again).then_some(now);
                if again {
                    self.toggle_fullscreen();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(live) = &self.live {
            live.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_reads_back_what_the_app_starts_it_with() {
        let (ring, presets, library) = (
            std::path::Path::new("C:/cache/milkdrop.ring"),
            std::path::Path::new("C:/profile/milkdrop"),
            std::path::Path::new("C:/app/libprojectM-4.dll"),
        );
        let line = Args::command_line(ring, presets, library);
        assert_eq!(line[0], FLAG);
        let rest = line[1..]
            .iter()
            .map(|part| part.to_string_lossy().into_owned());
        let args = Args::parse(rest).expect("arguments");
        assert_eq!(args.ring, ring);
        assert_eq!(args.presets, presets);
        assert_eq!(args.library, library);
        assert_eq!(args.seconds, crate::milkdrop::PRESET_SECONDS);
    }

    #[test]
    fn arguments_that_are_short_or_unknown_are_refused() {
        let parse = |args: &[&str]| Args::parse(args.iter().map(|arg| (*arg).to_owned()));
        assert!(parse(&["--ring", "a", "--presets", "b"]).is_none());
        assert!(parse(&["--ring", "a", "--presets", "b", "--library"]).is_none());
        assert!(
            parse(&[
                "--ring",
                "a",
                "--presets",
                "b",
                "--library",
                "c",
                "--what",
                "d"
            ])
            .is_none()
        );
        let args = parse(&[
            "--ring",
            "a",
            "--presets",
            "b",
            "--library",
            "c",
            "--seconds",
            "1",
        ]);
        assert_eq!(args.expect("arguments").seconds, 2);
    }
}
