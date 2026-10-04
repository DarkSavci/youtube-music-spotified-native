//! What `--open` can do with the pointer and the keys, for what only they
//! can reach: a menu is opened by a right click, and lit by the pointer
//! lying on it. Places are in points from the window's top left.
//!
//! `hover:x,y`, `click:x,y`, `rclick:x,y`, `dclick:x,y`, `drag:x,y`
//! (the button goes down and stays down), `drop:x,y`, `key:ArrowDown`
//! (any of egui's key names, with `ctrl+` before it for the Ctrl key).

use eframe::egui::{Event, Key, Modifiers, PointerButton, Pos2, pos2};

/// Whether `kind` is one of this file's.
pub(super) fn knows(kind: &str) -> bool {
    matches!(
        kind,
        "hover" | "click" | "rclick" | "dclick" | "drag" | "drop" | "key"
    )
}

fn place(value: &str) -> Option<Pos2> {
    let (x, y) = value.split_once(',')?;
    Some(pos2(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

fn button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// The events a step comes to. Nothing for one that cannot be read.
pub(super) fn events(kind: &str, value: &str) -> Vec<Event> {
    if kind == "key" {
        let (modifiers, name) = match value.strip_prefix("ctrl+") {
            Some(name) => (Modifiers::COMMAND, name),
            None => (Modifiers::NONE, value),
        };
        return Key::from_name(name)
            .map(|key| Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            })
            .into_iter()
            .collect();
    }
    let Some(at) = place(value) else {
        return Vec::new();
    };
    let moved = Event::PointerMoved(at);
    let click = |which| {
        vec![
            moved.clone(),
            button(at, which, true),
            button(at, which, false),
        ]
    };
    match kind {
        "click" => click(PointerButton::Primary),
        "rclick" => click(PointerButton::Secondary),
        "dclick" => [click(PointerButton::Primary), click(PointerButton::Primary)].concat(),
        "drag" => vec![moved, button(at, PointerButton::Primary, true)],
        "drop" => vec![moved, button(at, PointerButton::Primary, false)],
        _ => vec![moved],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_right_click_moves_there_and_presses_and_lets_go() {
        let events = events("rclick", "40, 60.5");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], Event::PointerMoved(pos2(40.0, 60.5)));
        assert!(matches!(
            events[2],
            Event::PointerButton {
                button: PointerButton::Secondary,
                pressed: false,
                ..
            }
        ));
    }

    #[test]
    fn a_step_that_cannot_be_read_does_nothing() {
        assert!(events("click", "here").is_empty());
        assert!(events("key", "NoSuchKey").is_empty());
        assert_eq!(events("key", "ctrl+ArrowDown").len(), 1);
    }
}
