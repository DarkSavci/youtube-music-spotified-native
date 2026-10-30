//! Small things asked of the desktop itself: where the taskbar is, how a
//! window with no frame is cornered, how much memory there is, showing a
//! file in its folder, and what this copy is started by.
//!
//! Each is a question the system may decline; the answer is then `None`
//! and the app does without.

use std::io;
use std::path::Path;
use std::process::Command;

/// A rectangle of the screen, in the system's own pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Area {
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    fn centre(&self) -> (f32, f32) {
        (
            (self.left + self.right) / 2.0,
            (self.top + self.bottom) / 2.0,
        )
    }
}

/// A display: all of it, and the part the taskbar leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub bounds: Area,
    pub work: Area,
}

/// Where a flyout of `size` goes for a tray icon at `icon`: against the
/// taskbar, centred on the icon, `margin` clear of the taskbar and of the
/// screen's edges. Returns its top-left corner.
///
/// The taskbar can sit on any edge, and the work area says which: it is
/// the display less the taskbar. When the two are equal the taskbar hides
/// itself, and the icon's place on the display decides instead.
pub fn flyout_position(icon: Area, display: Display, size: (f32, f32), margin: f32) -> (f32, f32) {
    #[derive(Clone, Copy)]
    enum Edge {
        Top,
        Left,
        Right,
        Bottom,
    }
    let (bounds, work) = (display.bounds, display.work);
    let (centre_x, centre_y) = icon.centre();
    let (width, height) = size;
    let edge = if work.top > bounds.top {
        Edge::Top
    } else if work.left > bounds.left {
        Edge::Left
    } else if work.width() < bounds.width() {
        Edge::Right
    } else if work.height() < bounds.height() {
        Edge::Bottom
    } else if centre_y < bounds.top + bounds.height() / 2.0 {
        Edge::Top
    } else {
        Edge::Bottom
    };
    // `max` after `min`, so a flyout wider than the work area still starts
    // inside it rather than panicking a clamp.
    let hold_x = |x: f32| x.min(work.right - width - margin).max(work.left + margin);
    let hold_y = |y: f32| y.min(work.bottom - height - margin).max(work.top + margin);
    match edge {
        Edge::Top => (hold_x(centre_x - width / 2.0), work.top + margin),
        Edge::Left => (work.left + margin, hold_y(centre_y - height / 2.0)),
        Edge::Right => (work.right - width - margin, hold_y(centre_y - height / 2.0)),
        Edge::Bottom => (
            hold_x(centre_x - width / 2.0),
            work.bottom - height - margin,
        ),
    }
}

/// The display nearest this point of the screen.
pub fn display_at(x: f32, y: f32) -> Option<Display> {
    #[cfg(windows)]
    {
        win::display_at(x, y)
    }
    #[cfg(target_os = "macos")]
    {
        mac::display_at(x, y)
    }
    // A Linux tray does not say where its icon is, so nothing is placed by
    // one.
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (x, y);
        None
    }
}

/// Rounds the corners of this app's window of that title, as Windows 11
/// rounds a framed one, and gives it the shadow that goes with them. A
/// window made without a frame gets neither by itself.
pub fn round_corners(title: &str) {
    #[cfg(windows)]
    win::round_corners(title);
    #[cfg(not(windows))]
    let _ = title;
}

/// Cuts this app's window of that title to a shape: the boxes given, as
/// left, top, right and bottom in the window's own pixels, are what shows
/// and what takes a click. `None` gives the window back its rectangle.
/// Returns whether the window was there to be shaped.
pub fn shape_window(title: &str, boxes: Option<&[[i32; 4]]>) -> bool {
    #[cfg(windows)]
    {
        win::shape_window(title, boxes)
    }
    #[cfg(target_os = "linux")]
    {
        super::x11::shape_window(title, boxes)
    }
    // A Mac's window stays the rectangle the skin is drawn in.
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (title, boxes);
        true
    }
}

/// The computer's memory in bytes: all of it, and what is free.
pub fn memory() -> Option<(u64, u64)> {
    #[cfg(windows)]
    {
        win::memory()
    }
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        meminfo(&text)
    }
    #[cfg(target_os = "macos")]
    {
        mac::memory()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// The total and the available memory in `/proc/meminfo`, in bytes.
#[cfg(any(target_os = "linux", test))]
fn meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| -> Option<u64> {
        let line = text.lines().find_map(|line| line.strip_prefix(name))?;
        let kilobytes = line.trim_start_matches(':').split_whitespace().next()?;
        Some(kilobytes.parse::<u64>().ok()? * 1024)
    };
    Some((field("MemTotal")?, field("MemAvailable")?))
}

/// What starts this copy again: its own file, or on Linux the AppImage it
/// runs from, as the executable itself is under a temporary mount.
#[cfg(unix)]
pub fn launcher() -> io::Result<std::path::PathBuf> {
    if cfg!(target_os = "linux")
        && let Some(image) = std::env::var_os("APPIMAGE")
    {
        return Ok(image.into());
    }
    std::env::current_exe()
}

/// Opens an address in the browser the system is set to.
pub fn open_url(url: &str) -> io::Result<()> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(program).arg(url).spawn().map(drop)
}

/// Opens the file's folder in the file manager with the file picked out.
pub fn reveal(file: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        // Explorer takes the path as part of the switch, quotes and all.
        Command::new("explorer")
            .arg(format!("/select,{}", file.display()))
            .spawn()
            .map(drop)
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg("-R").arg(file).spawn().map(drop)
    }
    #[cfg(target_os = "linux")]
    {
        if let Err(error) = super::linux::show_in_file_manager(&file_uri(file)) {
            log::debug!("no file manager to ask: {error}");
            let folder = file.parent().unwrap_or(file);
            return Command::new("xdg-open").arg(folder).spawn().map(drop);
        }
        Ok(())
    }
}

/// The `file:` address of a path, with what an address cannot hold escaped.
#[cfg(any(target_os = "linux", test))]
fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.display().to_string().bytes() {
        match byte {
            b'/' | b'-' | b'_' | b'.' | b'~' => uri.push(byte as char),
            _ if byte.is_ascii_alphanumeric() => uri.push(byte as char),
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

#[cfg(target_os = "macos")]
mod mac {
    use std::process::Command;

    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    use objc2_foundation::NSRect;

    use super::{Area, Display};

    /// The display nearest a point, in the flipped, physical pixels the tray
    /// gives its icon's place in. A Mac measures upward from the bottom of
    /// the first screen, in points.
    pub(super) fn display_at(x: f32, y: f32) -> Option<Display> {
        let main = MainThreadMarker::new()?;
        let screens = NSScreen::screens(main);
        let first_height = screens.firstObject()?.frame().size.height;
        let mut best: Option<(f32, Display)> = None;
        for screen in screens.iter() {
            let scale = screen.backingScaleFactor();
            let area = |rect: NSRect| Area {
                left: (rect.origin.x * scale) as f32,
                top: ((first_height - rect.origin.y - rect.size.height) * scale) as f32,
                right: ((rect.origin.x + rect.size.width) * scale) as f32,
                bottom: ((first_height - rect.origin.y) * scale) as f32,
            };
            let display = Display {
                bounds: area(screen.frame()),
                work: area(screen.visibleFrame()),
            };
            let (centre_x, centre_y) = display.bounds.centre();
            let inside = (display.bounds.left..display.bounds.right).contains(&x)
                && (display.bounds.top..display.bounds.bottom).contains(&y);
            let distance = if inside {
                0.0
            } else {
                (centre_x - x).powi(2) + (centre_y - y).powi(2)
            };
            if best.is_none_or(|(nearest, _)| distance < nearest) {
                best = Some((distance, display));
            }
        }
        best.map(|(_, display)| display)
    }

    fn said(program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// All the memory, and what `vm_stat` calls free and inactive.
    pub(super) fn memory() -> Option<(u64, u64)> {
        let total = said("sysctl", &["-n", "hw.memsize"])?.trim().parse().ok()?;
        let page = said("sysctl", &["-n", "hw.pagesize"])?
            .trim()
            .parse::<u64>()
            .ok()?;
        let stat = said("vm_stat", &[])?;
        let pages = |name: &str| -> Option<u64> {
            let line = stat.lines().find_map(|line| line.strip_prefix(name))?;
            line.trim_matches([':', ' ', '.']).parse().ok()
        };
        let free = pages("Pages free")?.saturating_add(pages("Pages inactive")?);
        Some((total, free.saturating_mul(page)))
    }
}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{HWND, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    use windows::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, DeleteObject, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST,
        MONITORINFO, MonitorFromPoint, RGN_OR, SetWindowRgn,
    };
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};
    use windows::core::{HSTRING, PCWSTR};

    use super::{Area, Display};

    fn area(rect: RECT) -> Area {
        Area {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
        }
    }

    pub(super) fn display_at(x: f32, y: f32) -> Option<Display> {
        let point = POINT {
            x: x.round() as i32,
            y: y.round() as i32,
        };
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..MONITORINFO::default()
        };
        // SAFETY: `info` is a whole MONITORINFO with its size filled in,
        // which is all the call asks of it.
        let found = unsafe {
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            GetMonitorInfoW(monitor, &raw mut info).as_bool()
        };
        if !found {
            log::warn!("the display could not be asked about");
        }
        found.then(|| Display {
            bounds: area(info.rcMonitor),
            work: area(info.rcWork),
        })
    }

    /// This app's window of that title, if it has one.
    fn own_window(title: &str) -> Option<HWND> {
        let title = HSTRING::from(title);
        // SAFETY: the title is a terminated string that outlives the call,
        // and the process id is written to a value of its own size.
        unsafe {
            let hwnd = FindWindowW(PCWSTR::null(), &title).ok()?;
            // Another program's window may carry the same title.
            let mut owner = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&raw mut owner));
            (owner == GetCurrentProcessId()).then_some(hwnd)
        }
    }

    pub(super) fn shape_window(title: &str, boxes: Option<&[[i32; 4]]>) -> bool {
        let Some(hwnd) = own_window(title) else {
            return false;
        };
        // SAFETY: plain calls with regions made here. Each box's region is
        // deleted once it has been added to the whole; the whole becomes
        // the system's own when the window takes it, and is deleted here
        // only if the window does not.
        unsafe {
            let Some(boxes) = boxes else {
                SetWindowRgn(hwnd, None, true);
                return true;
            };
            let whole = CreateRectRgn(0, 0, 0, 0);
            for [left, top, right, bottom] in boxes {
                let part = CreateRectRgn(*left, *top, *right, *bottom);
                CombineRgn(Some(whole), Some(whole), Some(part), RGN_OR);
                let _ = DeleteObject(part.into());
            }
            if SetWindowRgn(hwnd, Some(whole), true) == 0 {
                let _ = DeleteObject(whole.into());
            }
        }
        true
    }

    pub(super) fn round_corners(title: &str) {
        let Some(hwnd) = own_window(title) else {
            return;
        };
        // SAFETY: the attribute is read from a value of the size stated.
        unsafe {
            let preference = DWMWCP_ROUND;
            let set = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const preference).cast(),
                size_of_val(&preference) as u32,
            );
            // Windows 10 has no such attribute; its corners stay square.
            if let Err(error) = set {
                log::debug!("corners not rounded (HRESULT {:#010x})", error.code().0);
            }
        }
    }

    pub(super) fn memory() -> Option<(u64, u64)> {
        let mut status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..MEMORYSTATUSEX::default()
        };
        // SAFETY: `status` is a whole MEMORYSTATUSEX with its length set.
        unsafe { GlobalMemoryStatusEx(&raw mut status) }.ok()?;
        Some((status.ullTotalPhys, status.ullAvailPhys))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_is_read_from_proc_meminfo() {
        let text = "MemTotal:       16000 kB\nMemFree:  1 kB\nMemAvailable:   8000 kB\n";
        assert_eq!(meminfo(text), Some((16000 * 1024, 8000 * 1024)));
        assert_eq!(meminfo("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn a_files_address_escapes_what_an_address_cannot_hold() {
        assert_eq!(
            file_uri(Path::new("/home/ada/My Logs/a&b.zip")),
            "file:///home/ada/My%20Logs/a%26b.zip"
        );
    }

    const SIZE: (f32, f32) = (360.0, 136.0);
    const MARGIN: f32 = 12.0;

    fn area(left: f32, top: f32, right: f32, bottom: f32) -> Area {
        Area {
            left,
            top,
            right,
            bottom,
        }
    }

    const SCREEN: Area = Area {
        left: 0.0,
        top: 0.0,
        right: 1920.0,
        bottom: 1080.0,
    };

    #[test]
    fn with_the_taskbar_at_the_bottom_the_flyout_sits_above_the_icon() {
        let display = Display {
            bounds: SCREEN,
            work: area(0.0, 0.0, 1920.0, 1032.0),
        };
        let icon = area(1500.0, 1040.0, 1524.0, 1064.0);
        let (x, y) = flyout_position(icon, display, SIZE, MARGIN);
        assert_eq!(x, 1512.0 - 180.0);
        assert_eq!(y, 1032.0 - 136.0 - 12.0);
    }

    #[test]
    fn an_icon_near_the_corner_keeps_the_flyout_on_the_screen() {
        let display = Display {
            bounds: SCREEN,
            work: area(0.0, 0.0, 1920.0, 1032.0),
        };
        let icon = area(1890.0, 1040.0, 1914.0, 1064.0);
        let (x, _) = flyout_position(icon, display, SIZE, MARGIN);
        assert_eq!(x, 1920.0 - 360.0 - 12.0);
    }

    #[test]
    fn a_taskbar_on_another_edge_has_the_flyout_against_that_edge() {
        let top = Display {
            bounds: SCREEN,
            work: area(0.0, 48.0, 1920.0, 1080.0),
        };
        let icon = area(1500.0, 12.0, 1524.0, 36.0);
        assert_eq!(flyout_position(icon, top, SIZE, MARGIN).1, 60.0);

        let left = Display {
            bounds: SCREEN,
            work: area(64.0, 0.0, 1920.0, 1080.0),
        };
        let icon = area(20.0, 900.0, 44.0, 924.0);
        let (x, y) = flyout_position(icon, left, SIZE, MARGIN);
        assert_eq!((x, y), (76.0, 912.0 - 68.0));

        let right = Display {
            bounds: SCREEN,
            work: area(0.0, 0.0, 1856.0, 1080.0),
        };
        let icon = area(1876.0, 1040.0, 1900.0, 1064.0);
        let (x, y) = flyout_position(icon, right, SIZE, MARGIN);
        assert_eq!((x, y), (1856.0 - 372.0, 1080.0 - 136.0 - 12.0));
    }

    #[test]
    fn a_taskbar_that_hides_itself_is_found_by_where_the_icon_is() {
        let display = Display {
            bounds: SCREEN,
            work: SCREEN,
        };
        let low = area(1500.0, 1040.0, 1524.0, 1064.0);
        assert_eq!(
            flyout_position(low, display, SIZE, MARGIN).1,
            1080.0 - 148.0
        );
        let high = area(1500.0, 10.0, 1524.0, 34.0);
        assert_eq!(flyout_position(high, display, SIZE, MARGIN).1, 12.0);
    }

    #[test]
    fn a_second_display_to_the_left_has_its_own_work_area() {
        let display = Display {
            bounds: area(-1920.0, 0.0, 0.0, 1080.0),
            work: area(-1920.0, 0.0, 0.0, 1032.0),
        };
        let icon = area(-600.0, 1040.0, -576.0, 1064.0);
        let (x, y) = flyout_position(icon, display, SIZE, MARGIN);
        assert_eq!((x, y), (-768.0, 884.0));
    }
}
