//! The window's own frame, drawn by the app: the top bar is the title bar,
//! with the minimise, maximise and close buttons at its right, and the
//! window's edges resize it.
//!
//! Only on Windows, and only unless the system's title bar was asked for in
//! Settings. Elsewhere the system draws the frame and none of this shows.

use eframe::egui::{
    self, Color32, CursorIcon, Rect, ResizeDirection, Sense, Stroke, Ui, ViewportCommand, pos2,
    vec2,
};

use super::widgets;
use crate::state::State;

/// How wide each of the three buttons is; Windows' own are this wide.
const BUTTON_WIDTH: f32 = 46.0;
/// The glyphs are drawn, not set in a font: thin strokes on a small grid.
const GLYPH: f32 = 10.0;
/// Windows' own red under the pointer on the close button.
const CLOSE_HOVER: Color32 = Color32::from_rgb(0xe8, 0x11, 0x23);
/// How close to an edge, and to a corner along an edge, the pointer
/// resizes rather than clicks.
const RESIZE_EDGE: f32 = 5.0;
const RESIZE_CORNER: f32 = 12.0;

/// Whether the app draws the window's frame itself.
pub fn custom(state: &State) -> bool {
    cfg!(windows) && !state.settings.system_title_bar
}

/// The room the three buttons take at the right of the top bar.
pub fn buttons_width(state: &State) -> f32 {
    if custom(state) {
        BUTTON_WIDTH * 3.0
    } else {
        0.0
    }
}

fn maximised(ui: &Ui) -> bool {
    ui.input(|input| input.viewport().maximized.unwrap_or(false))
}

/// Makes `rect` move the window when dragged and maximise it when
/// double-clicked, as a title bar does. Called before the controls that sit
/// in `rect` are added, so they keep their own clicks.
pub fn drag(state: &State, ui: &Ui, rect: Rect) {
    if !custom(state) {
        return;
    }
    let response = ui.interact(rect, ui.id().with("title-bar"), Sense::click_and_drag());
    if response.double_clicked() {
        let maximise = !maximised(ui);
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(maximise));
    } else if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

#[derive(Clone, Copy)]
enum Button {
    Minimise,
    Maximise,
    Restore,
    Close,
}

impl Button {
    fn label(self) -> &'static str {
        match self {
            Button::Minimise => "Minimise",
            Button::Maximise => "Maximise",
            Button::Restore => "Restore",
            Button::Close => "Close window",
        }
    }
}

/// Minimise, maximise and close, filling `area`, which is as tall as the
/// top bar and [`buttons_width`] wide.
pub fn buttons(state: &State, ui: &mut Ui, area: Rect) {
    if !custom(state) {
        return;
    }
    let palette = &state.palette;
    let middle = if maximised(ui) {
        Button::Restore
    } else {
        Button::Maximise
    };
    for (index, button) in [Button::Minimise, middle, Button::Close]
        .into_iter()
        .enumerate()
    {
        let rect = Rect::from_min_size(
            area.left_top() + vec2(BUTTON_WIDTH * index as f32, 0.0),
            vec2(BUTTON_WIDTH, area.height()),
        );
        let response = ui.interact(rect, ui.id().with(button.label()), Sense::click());
        widgets::name(ui, &response, button.label());
        let closing = matches!(button, Button::Close);
        let lift = widgets::hover(ui, &response);
        let (fill, lit) = if closing {
            (CLOSE_HOVER, Color32::WHITE)
        } else {
            (palette.surface_hover, palette.text)
        };
        ui.painter()
            .rect_filled(rect, 0.0, fill.gamma_multiply(lift));
        let color = crate::tint::blend(palette.secondary, lit, lift);
        glyph(ui, button, rect, Stroke::new(1.0, color));
        if response.on_hover_text(button.label()).clicked() {
            let maximise = !maximised(ui);
            ui.ctx().send_viewport_cmd(match button {
                Button::Minimise => ViewportCommand::Minimized(true),
                Button::Maximise | Button::Restore => ViewportCommand::Maximized(maximise),
                Button::Close => ViewportCommand::Close,
            });
        }
    }
}

fn glyph(ui: &Ui, button: Button, within: Rect, stroke: Stroke) {
    let painter = ui.painter();
    // On whole pixels, so the one-pixel strokes stay sharp.
    let centre = pos2(within.center().x.round(), within.center().y.round());
    let square = Rect::from_center_size(centre, vec2(GLYPH, GLYPH));
    match button {
        Button::Minimise => {
            painter.hline(square.x_range(), centre.y, stroke);
        }
        Button::Maximise => {
            painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
        }
        // Two windows, one behind the other.
        Button::Restore => {
            let front = Rect::from_min_size(square.left_top() + vec2(0.0, 2.0), vec2(8.0, 8.0));
            let back = front.translate(vec2(2.0, -2.0));
            painter.line_segment([back.left_top(), back.right_top()], stroke);
            painter.line_segment([back.right_top(), back.right_bottom()], stroke);
            painter.rect_stroke(front, 0.0, stroke, egui::StrokeKind::Inside);
        }
        Button::Close => {
            painter.line_segment([square.left_top(), square.right_bottom()], stroke);
            painter.line_segment([square.right_top(), square.left_bottom()], stroke);
        }
    }
}

/// Lets the window's edges and corners resize it, which the system no
/// longer does for a window without its frame. Called last, so the cursor
/// it sets is the one that shows.
pub fn resize(state: &State, ui: &Ui) {
    let fullscreen = ui.input(|input| input.viewport().fullscreen.unwrap_or(false));
    if !custom(state) || fullscreen || maximised(ui) {
        return;
    }
    resize_edges(ui);
}

/// The same for any window drawn without the system's frame, such as the
/// mini player's.
pub fn resize_edges(ui: &Ui) {
    let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) else {
        return;
    };
    let Some((direction, cursor)) = resize_direction(ui.ctx().content_rect(), pointer) else {
        return;
    };
    ui.ctx().set_cursor_icon(cursor);
    if ui.input(|input| input.pointer.primary_pressed()) {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

/// Whether the pointer is where a press would resize the window. What
/// moves the window must stand down there, or one press asks for both and
/// the move wins.
pub fn at_resize_edge(ui: &Ui) -> bool {
    ui.input(|input| input.pointer.hover_pos())
        .is_some_and(|pointer| resize_direction(ui.ctx().content_rect(), pointer).is_some())
}

/// Which way a press at `pointer` would resize `window`, if it is on an
/// edge. Near a corner an edge gives way to the corner.
fn resize_direction(window: Rect, pointer: egui::Pos2) -> Option<(ResizeDirection, CursorIcon)> {
    if !window.contains(pointer) {
        return None;
    }
    let from = |near: f32, far: f32, reach: f32| i8::from(far <= reach) - i8::from(near <= reach);
    let (left, right) = (pointer.x - window.left(), window.right() - pointer.x);
    let (top, bottom) = (pointer.y - window.top(), window.bottom() - pointer.y);
    let mut across = from(left, right, RESIZE_EDGE);
    let mut down = from(top, bottom, RESIZE_EDGE);
    if down != 0 {
        across = from(left, right, RESIZE_CORNER);
    }
    if across != 0 {
        down = from(top, bottom, RESIZE_CORNER);
    }
    use {CursorIcon as Cursor, ResizeDirection as Direction};
    Some(match (across, down) {
        (-1, -1) => (Direction::NorthWest, Cursor::ResizeNwSe),
        (1, -1) => (Direction::NorthEast, Cursor::ResizeNeSw),
        (-1, 1) => (Direction::SouthWest, Cursor::ResizeNeSw),
        (1, 1) => (Direction::SouthEast, Cursor::ResizeNwSe),
        (-1, 0) => (Direction::West, Cursor::ResizeHorizontal),
        (1, 0) => (Direction::East, Cursor::ResizeHorizontal),
        (0, -1) => (Direction::North, Cursor::ResizeVertical),
        (0, 1) => (Direction::South, Cursor::ResizeVertical),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Option<ResizeDirection> {
        let window = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        resize_direction(window, pos2(x, y)).map(|(direction, _)| direction)
    }

    #[test]
    fn the_middle_of_the_window_does_not_resize() {
        assert_eq!(at(400.0, 300.0), None);
        assert_eq!(at(6.0, 300.0), None);
    }

    #[test]
    fn an_edge_resizes_along_its_side() {
        assert_eq!(at(2.0, 300.0), Some(ResizeDirection::West));
        assert_eq!(at(798.0, 300.0), Some(ResizeDirection::East));
        assert_eq!(at(400.0, 1.0), Some(ResizeDirection::North));
        assert_eq!(at(400.0, 599.0), Some(ResizeDirection::South));
    }

    #[test]
    fn near_a_corner_an_edge_gives_way_to_the_corner() {
        assert_eq!(at(2.0, 2.0), Some(ResizeDirection::NorthWest));
        // On the top edge, within the corner's reach of the right side.
        assert_eq!(at(790.0, 2.0), Some(ResizeDirection::NorthEast));
        assert_eq!(at(3.0, 590.0), Some(ResizeDirection::SouthWest));
        assert_eq!(at(797.0, 597.0), Some(ResizeDirection::SouthEast));
    }

    #[test]
    fn outside_the_window_is_nobodys() {
        assert_eq!(at(-1.0, 300.0), None);
    }
}
