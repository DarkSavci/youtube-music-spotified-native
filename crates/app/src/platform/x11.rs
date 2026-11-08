//! What the app asks of an X server: cutting a window to a skin's shape.
//!
//! The window is found by its title, as on Windows: the toolkit does not
//! hand over the handle of a window other than the main one.

use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{
    AtomEnum, ClipOrdering, ConnectionExt as _, Rectangle, Window as XWindow,
};

/// How deep the tree of windows is looked into: a window manager puts each
/// window inside a frame or two.
const MOST_DEEP: usize = 4;

/// The library the toolkit reads the keyboard through on X11. It is loaded
/// when the window opens, and without it the toolkit panics, so it is looked
/// for first. Many systems have `libxkbcommon` without this part of it.
const KEYBOARD_LIBRARY: &str = "libxkbcommon-x11.so.0";

/// Whether the app's windows are X11's: the toolkit is asked for X11 wherever
/// there is an X server to ask (`main.rs`) and it can read the keyboard there.
pub fn in_use() -> bool {
    static IN_USE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *IN_USE.get_or_init(|| has(DISPLAY) && keyboard_library())
}

const DISPLAY: &str = "DISPLAY";
const WAYLAND_DISPLAY: &str = "WAYLAND_DISPLAY";

fn has(variable: &str) -> bool {
    std::env::var_os(variable).is_some_and(|value| !value.is_empty())
}

fn keyboard_library() -> bool {
    // SAFETY: loading the library runs its initialisers, which for this one
    // set nothing up; it is unloaded again at once.
    let loaded = unsafe { libloading::Library::new(KEYBOARD_LIBRARY) };
    if let Err(error) = &loaded {
        log::warn!("{KEYBOARD_LIBRARY} could not be loaded: {error}");
    }
    loaded.is_ok()
}

/// Why no window can be opened, if none can: an X server whose keyboard
/// library is missing, and no Wayland to use instead. Said before the
/// toolkit would panic over it with a message nobody sees.
pub fn cannot_open() -> Option<String> {
    if in_use() || has(WAYLAND_DISPLAY) || !has(DISPLAY) {
        return None;
    }
    Some(format!(
        "{KEYBOARD_LIBRARY} is missing, and the window needs it on X11. Install it:          `sudo apt install libxkbcommon-x11-0` on Debian or Ubuntu,          `sudo dnf install libxkbcommon-x11` on Fedora,          `sudo pacman -S libxkbcommon-x11` on Arch."
    ))
}

/// Asks for X11 where there is an X server, which is what a desktop on Wayland
/// gives its older programs too. Wayland can neither hide a window nor place
/// one, and the tray, the mini player and its skins want both. Without the
/// keyboard library X11 needs, Wayland is used where there is one.
pub fn prefer<T>(builder: &mut winit::event_loop::EventLoopBuilder<T>) {
    use winit::platform::x11::EventLoopBuilderExtX11;
    if in_use() {
        builder.with_x11();
    } else if has(DISPLAY) && has(WAYLAND_DISPLAY) {
        log::warn!("using Wayland, as X11 cannot read the keyboard without {KEYBOARD_LIBRARY}");
    }
}

/// Cuts this app's window of that title to the boxes given, or gives it its
/// rectangle back with `None`. Returns whether the window was there to be
/// shaped; on Wayland it is left as it is, and that is the answer too.
pub fn shape_window(title: &str, boxes: Option<&[[i32; 4]]>) -> bool {
    if !in_use() {
        return true;
    }
    match shaped(title, boxes) {
        Ok(found) => found,
        Err(error) => {
            log::debug!("the window was not shaped: {error}");
            // Not worth asking again on every frame.
            true
        }
    }
}

fn shaped(title: &str, boxes: Option<&[[i32; 4]]>) -> Result<bool, Box<dyn std::error::Error>> {
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    let Some(window) = find(&connection, root, title, 0)? else {
        return Ok(false);
    };
    for kind in [SK::BOUNDING, SK::INPUT] {
        match boxes {
            Some(boxes) => {
                let rectangles = rectangles(boxes);
                connection.shape_rectangles(
                    SO::SET,
                    kind,
                    ClipOrdering::UNSORTED,
                    window,
                    0,
                    0,
                    &rectangles,
                )?;
            }
            None => {
                shape::mask(&connection, SO::SET, kind, window, 0, 0, x11rb::NONE)?;
            }
        }
    }
    // Waited for, so an error comes back here and not to nobody.
    connection.get_input_focus()?.reply()?;
    Ok(true)
}

fn rectangles(boxes: &[[i32; 4]]) -> Vec<Rectangle> {
    boxes
        .iter()
        .map(|&[left, top, right, bottom]| Rectangle {
            x: left as i16,
            y: top as i16,
            width: (right - left).max(0) as u16,
            height: (bottom - top).max(0) as u16,
        })
        .collect()
}

fn find(
    connection: &impl Connection,
    window: XWindow,
    title: &str,
    depth: usize,
) -> Result<Option<XWindow>, Box<dyn std::error::Error>> {
    if name_of(connection, window)?.as_deref() == Some(title) {
        return Ok(Some(window));
    }
    if depth >= MOST_DEEP {
        return Ok(None);
    }
    for child in connection.query_tree(window)?.reply()?.children {
        if let Some(found) = find(connection, child, title, depth + 1)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn name_of(
    connection: &impl Connection,
    window: XWindow,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let reply = connection
        .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 256)?
        .reply();
    // A window can be gone by the time it is asked about.
    Ok(reply
        .ok()
        .filter(|reply| !reply.value.is_empty())
        .map(|reply| String::from_utf8_lossy(&reply.value).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_becomes_a_rectangle() {
        let made = rectangles(&[[2, 3, 12, 8]]);
        assert_eq!((made[0].x, made[0].y), (2, 3));
        assert_eq!((made[0].width, made[0].height), (10, 5));
    }

    #[test]
    fn a_box_turned_inside_out_is_empty() {
        let made = rectangles(&[[5, 5, 1, 1]]);
        assert_eq!((made[0].width, made[0].height), (0, 0));
    }
}
