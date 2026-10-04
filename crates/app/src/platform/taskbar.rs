//! The buttons on the window's taskbar thumbnail: like, previous, play or
//! pause, next, as the Electron app has them.
//!
//! Windows keeps the buttons with the window's taskbar button, which comes
//! and goes with the window: hidden to the tray and shown again, the
//! buttons have to be added afresh. Windows says when by a message to the
//! window, so the window is subclassed to hear it, and to hear the clicks.
//!
//! Everything here is the system declining or agreeing; nothing in it can
//! stop the app. A refusal is logged and the buttons are simply not there.

use raw_window_handle::HasWindowHandle;

/// What the buttons are told, so they are only told again on a change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Buttons {
    /// Something is in the player.
    pub has_track: bool,
    pub playing: bool,
    /// Signed in, so a like has somewhere to go.
    pub can_like: bool,
    pub liked: bool,
}

/// What a click on a button asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbAction {
    Like,
    Previous,
    TogglePlay,
    Next,
}

impl ThumbAction {
    /// In the order they stand on the thumbnail. A button's place here is
    /// also the id Windows hands back when it is clicked.
    const EVERY: [ThumbAction; 4] = [
        ThumbAction::Like,
        ThumbAction::Previous,
        ThumbAction::TogglePlay,
        ThumbAction::Next,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Glyph {
    Like,
    Liked,
    Previous,
    Play,
    Pause,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Enabled,
    /// Dimmed: there, with nothing to act on.
    Disabled,
    Hidden,
}

/// One button as it should be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Button {
    action: ThumbAction,
    glyph: Glyph,
    tooltip: &'static str,
    shown: Shown,
}

/// The four buttons for what is playing.
fn layout(buttons: Buttons) -> [Button; 4] {
    let transport = if buttons.has_track {
        Shown::Enabled
    } else {
        Shown::Disabled
    };
    // Hidden while there is nothing to like: Windows dims a disabled
    // button's frame and glyph together, which on a thin ring reads as
    // broken.
    let like = if buttons.has_track && buttons.can_like {
        Shown::Enabled
    } else {
        Shown::Hidden
    };
    ThumbAction::EVERY.map(|action| {
        let (glyph, tooltip, shown) = match action {
            ThumbAction::Like if buttons.liked => (Glyph::Liked, "Remove from Liked Music", like),
            ThumbAction::Like => (Glyph::Like, "Add to Liked Music", like),
            ThumbAction::Previous => (Glyph::Previous, "Previous", transport),
            ThumbAction::TogglePlay if buttons.playing => (Glyph::Pause, "Pause", transport),
            ThumbAction::TogglePlay => (Glyph::Play, "Play", transport),
            ThumbAction::Next => (Glyph::Next, "Next", transport),
        };
        Button {
            action,
            glyph,
            tooltip,
            shown,
        }
    })
}

/// The scalings the icons were drawn for. Windows draws a button's icon at
/// sixteen pixels times the display's scaling.
const SCALES: [f32; 8] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0];

/// The smallest drawn scale that covers the display's, so Windows only
/// ever shrinks an icon, which stays sharp; stretching one does not.
fn drawn_scale(display: f32) -> usize {
    SCALES
        .iter()
        .position(|scale| *scale >= display - 0.01)
        .unwrap_or(SCALES.len() - 1)
}

/// The PNG of a glyph at one of [`SCALES`]. They are the Electron app's
/// own: white, drawn for these sizes, each vertical edge on a whole pixel.
fn png(glyph: Glyph, scale: usize) -> &'static [u8] {
    macro_rules! drawn {
        ($name:literal) => {
            [
                include_bytes!(concat!("../../assets/thumbar/", $name, ".png")).as_slice(),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@1.25x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@1.5x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@1.75x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@2x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@2.25x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@2.5x.png")),
                include_bytes!(concat!("../../assets/thumbar/", $name, "@3x.png")),
            ]
        };
    }
    let drawn = match glyph {
        Glyph::Like => drawn!("like"),
        Glyph::Liked => drawn!("liked"),
        Glyph::Previous => drawn!("prev"),
        Glyph::Play => drawn!("play"),
        Glyph::Pause => drawn!("pause"),
        Glyph::Next => drawn!("next"),
    };
    drawn[scale.min(SCALES.len() - 1)]
}

/// Kept for as long as the buttons should work; dropping it lets go of
/// the window.
pub struct Taskbar {
    #[cfg(windows)]
    inner: Box<win::Inner>,
}

impl Taskbar {
    /// Attaches to the window. `on_action` is called on the window's own
    /// thread when a button is clicked. `None` where the system has no
    /// such buttons, or will not give them.
    pub fn attach(
        window: &impl HasWindowHandle,
        on_action: impl Fn(ThumbAction) + 'static,
    ) -> Option<Self> {
        #[cfg(windows)]
        {
            win::Inner::attach(window, Box::new(on_action)).map(|inner| Self { inner })
        }
        #[cfg(not(windows))]
        {
            let _ = (window, on_action);
            None
        }
    }

    /// Shows the buttons for what is playing, on a display of this scaling.
    /// Cheap to call every frame: nothing crosses into the system unless
    /// something changed.
    pub fn show(&mut self, buttons: Buttons, scale: f32) {
        #[cfg(windows)]
        self.inner.show(buttons, drawn_scale(scale));
        #[cfg(not(windows))]
        let _ = (buttons, scale);
    }
}

#[cfg(windows)]
mod win {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::UI::Shell::{
        DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass, THB_FLAGS,
        THB_ICON, THB_TOOLTIP, THBF_DISABLED, THBF_ENABLED, THBF_HIDDEN, THUMBBUTTON, TaskbarList,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateIcon, DestroyIcon, HICON, RegisterWindowMessageW, WM_COMMAND, WM_NCDESTROY,
    };
    use windows::core::w;

    use super::{Button, Buttons, Glyph, Shown, ThumbAction, layout, png};

    /// What the high word of a `WM_COMMAND` says when a thumbnail button
    /// was clicked.
    const THBN_CLICKED: usize = 0x1800;
    /// This subclass among any others on the window.
    const SUBCLASS: usize = 0x5350_4f54;
    /// A refusal to add the buttons is not asked about again sooner.
    const RETRY_AFTER: Duration = Duration::from_secs(2);

    pub(super) struct Inner {
        hwnd: HWND,
        list: ITaskbarList3,
        on_action: Box<dyn Fn(ThumbAction)>,
        /// The message Windows sends when the window's taskbar button has
        /// been made, or made again.
        button_created: u32,
        /// The buttons are on the thumbnail; later changes update them.
        added: Cell<bool>,
        /// What was last asked for, and at which drawn scale.
        wanted: Cell<Option<(Buttons, usize)>>,
        /// What the system last took.
        shown: Cell<Option<(Buttons, usize)>>,
        tried: Cell<Option<Instant>>,
        /// Whether a refusal has been written down already.
        refused: Cell<bool>,
        icons: RefCell<HashMap<(Glyph, usize), HICON>>,
    }

    impl Inner {
        pub(super) fn attach(
            window: &impl HasWindowHandle,
            on_action: Box<dyn Fn(ThumbAction)>,
        ) -> Option<Box<Self>> {
            let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
                return None;
            };
            let hwnd = HWND(handle.hwnd.get() as *mut _);
            // SAFETY: plain calls into COM on the thread that owns the
            // window, which is where the taskbar list must be used. The
            // window system has usually initialised COM here already, and
            // being told so is not a failure.
            let list: ITaskbarList3 = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)
                    .and_then(|list: ITaskbarList3| list.HrInit().map(|()| list))
                    .inspect_err(|error| log::warn!("no taskbar buttons: {error}"))
                    .ok()?
            };
            // SAFETY: registering a message by name has no preconditions.
            let button_created = unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) };
            let inner = Box::new(Self {
                hwnd,
                list,
                on_action,
                button_created,
                added: Cell::new(false),
                wanted: Cell::new(None),
                shown: Cell::new(None),
                tried: Cell::new(None),
                refused: Cell::new(false),
                icons: RefCell::new(HashMap::new()),
            });
            // SAFETY: the pointer handed over is to the boxed value, which
            // does not move and outlives the subclass: `Drop` removes it.
            let subclassed = unsafe {
                let data = &raw const *inner as usize;
                SetWindowSubclass(hwnd, Some(subclass), SUBCLASS, data).as_bool()
            };
            if !subclassed {
                log::warn!("no taskbar buttons: the window could not be listened to");
                return None;
            }
            Some(inner)
        }

        pub(super) fn show(&self, buttons: Buttons, scale: usize) {
            self.wanted.set(Some((buttons, scale)));
            self.apply();
        }

        /// Brings the thumbnail to what is wanted, if it is not there.
        fn apply(&self) {
            let Some(wanted) = self.wanted.get() else {
                return;
            };
            if self.added.get() && self.shown.get() == Some(wanted) {
                return;
            }
            let now = Instant::now();
            let too_soon = |tried: Instant| now.duration_since(tried) < RETRY_AFTER;
            if !self.added.get() && self.tried.get().is_some_and(too_soon) {
                return;
            }
            self.tried.set(Some(now));
            let (buttons, scale) = wanted;
            let described = layout(buttons).map(|button| self.describe(button, scale));
            // SAFETY: the list was made on this thread and the buttons are
            // plain data that lives to the end of the call.
            let result = unsafe {
                if self.added.get() {
                    self.list.ThumbBarUpdateButtons(self.hwnd, &described)
                } else {
                    self.list.ThumbBarAddButtons(self.hwnd, &described)
                }
            };
            match result {
                Ok(()) => {
                    if !self.added.replace(true) {
                        log::info!("taskbar buttons added (HRESULT 0x00000000)");
                    }
                    self.refused.set(false);
                    self.shown.set(Some(wanted));
                }
                // Before the window has a taskbar button there is nothing
                // to add them to; the message that it has one tries again.
                Err(error) => {
                    if !self.refused.replace(true) {
                        log::warn!(
                            "taskbar buttons refused (HRESULT {:#010x}): {}",
                            error.code().0,
                            error.message()
                        );
                    }
                }
            }
        }

        fn describe(&self, button: Button, scale: usize) -> THUMBBUTTON {
            let mut tip = [0u16; 260];
            for (slot, unit) in tip.iter_mut().zip(button.tooltip.encode_utf16().take(259)) {
                *slot = unit;
            }
            // No "no background": the button's background is also where
            // Windows draws hover and press, and without it the buttons
            // give no feedback.
            let flags = match button.shown {
                Shown::Enabled => THBF_ENABLED,
                Shown::Disabled => THBF_DISABLED,
                Shown::Hidden => THBF_HIDDEN,
            };
            let place = ThumbAction::EVERY
                .iter()
                .position(|action| *action == button.action);
            THUMBBUTTON {
                dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                iId: place.unwrap_or_default() as u32,
                iBitmap: 0,
                hIcon: self.icon(button.glyph, scale).unwrap_or_default(),
                szTip: tip,
                dwFlags: flags,
            }
        }

        /// The glyph as an icon, made once for each scale it is asked at.
        fn icon(&self, glyph: Glyph, scale: usize) -> Option<HICON> {
            if let Some(icon) = self.icons.borrow().get(&(glyph, scale)) {
                return Some(*icon);
            }
            let image = image::load_from_memory(png(glyph, scale))
                .inspect_err(|error| log::warn!("a taskbar icon could not be read: {error}"))
                .ok()?
                .into_rgba8();
            let (width, height) = image.dimensions();
            // Windows wants blue first.
            let mut pixels = image.into_raw();
            for pixel in pixels.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            // The mask is a bit a pixel, in rows padded to sixteen bits.
            // All clear: the picture's own alpha says what shows.
            let mask = vec![0u8; (width.div_ceil(16) * 2 * height) as usize];
            // SAFETY: both buffers are as large as the size given says and
            // are copied by the call.
            let icon = unsafe {
                CreateIcon(
                    None,
                    width as i32,
                    height as i32,
                    1,
                    32,
                    mask.as_ptr(),
                    pixels.as_ptr(),
                )
            }
            .inspect_err(|error| log::warn!("a taskbar icon could not be made: {error}"))
            .ok()?;
            self.icons.borrow_mut().insert((glyph, scale), icon);
            Some(icon)
        }

        fn clicked(&self, id: usize) {
            if let Some(action) = ThumbAction::EVERY.get(id) {
                (self.on_action)(*action);
            }
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            // SAFETY: undoes what `attach` did, on the same thread. A
            // window that has gone already makes both a harmless failure.
            unsafe {
                let _ = RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS);
                for (_, icon) in self.icons.borrow_mut().drain() {
                    let _ = DestroyIcon(icon);
                }
            }
        }
    }

    /// Hears the window's messages ahead of the window itself.
    unsafe extern "system" fn subclass(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        // SAFETY: `data` is the pointer `attach` gave, to an `Inner` that
        // lives until the subclass is removed; the system only calls this
        // on the window's thread, so nothing else is using it.
        let inner = unsafe { &*(data as *const Inner) };
        if message == WM_COMMAND && (wparam.0 >> 16) & 0xffff == THBN_CLICKED {
            inner.clicked(wparam.0 & 0xffff);
            return LRESULT(0);
        }
        if message == inner.button_created {
            // A new taskbar button has no thumbnail buttons yet.
            inner.added.set(false);
            inner.tried.set(None);
            inner.apply();
        }
        if message == WM_NCDESTROY {
            // SAFETY: removing this subclass from its own window.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS);
            }
        }
        // SAFETY: passing the message on as it came.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(buttons: Buttons) -> [Shown; 4] {
        layout(buttons).map(|button| button.shown)
    }

    #[test]
    fn with_nothing_playing_the_transport_is_dimmed_and_the_heart_hidden() {
        assert_eq!(
            shown(Buttons::default()),
            [
                Shown::Hidden,
                Shown::Disabled,
                Shown::Disabled,
                Shown::Disabled
            ]
        );
    }

    #[test]
    fn the_heart_shows_only_when_there_is_an_account_to_like_for() {
        let signed_out = Buttons {
            has_track: true,
            ..Buttons::default()
        };
        assert_eq!(shown(signed_out)[0], Shown::Hidden);
        assert_eq!(shown(signed_out)[1..], [Shown::Enabled; 3]);
        let signed_in = Buttons {
            can_like: true,
            ..signed_out
        };
        assert_eq!(shown(signed_in), [Shown::Enabled; 4]);
    }

    #[test]
    fn the_middle_button_says_what_pressing_it_would_do() {
        let paused = Buttons {
            has_track: true,
            ..Buttons::default()
        };
        assert_eq!(layout(paused)[2].tooltip, "Play");
        assert_eq!(layout(paused)[2].glyph, Glyph::Play);
        let playing = Buttons {
            playing: true,
            ..paused
        };
        assert_eq!(layout(playing)[2].tooltip, "Pause");
        assert_eq!(layout(playing)[2].glyph, Glyph::Pause);
    }

    #[test]
    fn the_heart_says_whether_the_song_is_liked() {
        let liked = Buttons {
            has_track: true,
            can_like: true,
            liked: true,
            playing: false,
        };
        assert_eq!(layout(liked)[0].tooltip, "Remove from Liked Music");
        let not = Buttons {
            liked: false,
            ..liked
        };
        assert_eq!(layout(not)[0].tooltip, "Add to Liked Music");
    }

    #[test]
    fn an_icon_is_never_stretched_only_shrunk() {
        assert_eq!(SCALES[drawn_scale(1.0)], 1.0);
        assert_eq!(SCALES[drawn_scale(1.1)], 1.25);
        assert_eq!(SCALES[drawn_scale(1.5)], 1.5);
        assert_eq!(SCALES[drawn_scale(2.6)], 3.0);
        // Past the largest drawn, the largest is the best there is.
        assert_eq!(SCALES[drawn_scale(4.0)], 3.0);
    }

    #[test]
    fn every_glyph_is_drawn_at_every_scale_and_at_its_size() {
        let glyphs = [
            Glyph::Like,
            Glyph::Liked,
            Glyph::Previous,
            Glyph::Play,
            Glyph::Pause,
            Glyph::Next,
        ];
        for glyph in glyphs {
            for (index, scale) in SCALES.iter().enumerate() {
                let image = image::load_from_memory(png(glyph, index)).expect("a PNG");
                assert_eq!(image.width() as f32, 16.0 * scale, "{glyph:?} at {scale}");
            }
        }
    }
}
