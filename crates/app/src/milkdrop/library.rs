//! libprojectM, loaded from its file when MilkDrop is first wanted.
//!
//! Only the handful of its functions the window uses are looked up, by
//! the names its C interface gives them. The library is never unloaded:
//! the process that loads it lives exactly as long as the window does.

use std::ffi::{c_char, c_void};
use std::path::Path;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{HSTRING, PCSTR};

/// One running projectM, as the library hands it out.
pub type Handle = *mut c_void;
/// Called when projectM wants the next preset: on a beat, or in its time.
pub type SwitchRequested = unsafe extern "C" fn(hard_cut: bool, user_data: *mut c_void);
/// Called when a preset could not be loaded.
pub type SwitchFailed =
    unsafe extern "C" fn(file: *const c_char, message: *const c_char, user_data: *mut c_void);

/// `projectm_channels`: the samples come as left and right.
pub const STEREO: i32 = 2;

macro_rules! api {
    ($($name:ident: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        /// The functions of the library this app calls, each under the
        /// name the library exports it by.
        pub struct Api {
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }

        impl Api {
            /// Looks every function up; `None` names the first not found.
            fn load(module: HMODULE) -> Result<Self, &'static str> {
                Ok(Self {
                    $($name: {
                        let name = concat!(stringify!($name), "\0");
                        // SAFETY: the module is loaded and the name is a
                        // terminated string.
                        let found = unsafe { GetProcAddress(module, PCSTR(name.as_ptr())) };
                        let found = found.ok_or(stringify!($name))?;
                        // SAFETY: the library documents this function with
                        // exactly this signature, and on 64-bit Windows
                        // there is one calling convention.
                        unsafe {
                            std::mem::transmute::<
                                unsafe extern "system" fn() -> isize,
                                unsafe extern "C" fn($($arg),*) $(-> $ret)?,
                            >(found)
                        }
                    },)*
                })
            }
        }
    };
}

api! {
    projectm_create: fn() -> Handle;
    projectm_destroy: fn(Handle);
    projectm_load_preset_file: fn(Handle, *const c_char, bool);
    projectm_set_preset_switch_requested_event_callback: fn(Handle, Option<SwitchRequested>, *mut c_void);
    projectm_set_preset_switch_failed_event_callback: fn(Handle, Option<SwitchFailed>, *mut c_void);
    projectm_set_texture_search_paths: fn(Handle, *mut *const c_char, usize);
    projectm_set_preset_duration: fn(Handle, f64);
    projectm_set_soft_cut_duration: fn(Handle, f64);
    projectm_set_hard_cut_enabled: fn(Handle, bool);
    projectm_set_preset_locked: fn(Handle, bool);
    projectm_set_aspect_correction: fn(Handle, bool);
    projectm_set_mesh_size: fn(Handle, usize, usize);
    projectm_set_fps: fn(Handle, i32);
    projectm_set_window_size: fn(Handle, usize, usize);
    projectm_pcm_add_float: fn(Handle, *const f32, u32, i32);
    projectm_opengl_render_frame: fn(Handle);
}

/// Loads the library from `file` and finds its functions.
pub fn load(file: &Path) -> Result<Api, String> {
    let name = HSTRING::from(file.as_os_str());
    // SAFETY: the name is a terminated string that outlives the call. The
    // library's own start-up code runs here; it is the app's own build of
    // libprojectM, from beside the program.
    let module = unsafe { LoadLibraryW(&name) }
        .map_err(|error| format!("{} could not be loaded: {error}", file.display()))?;
    Api::load(module).map_err(|missing| format!("{} has no {missing}", file.display()))
}
