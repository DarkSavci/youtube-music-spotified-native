//! What a page shows while it loads: grey shapes where its shelves or its
//! songs will be, so that their arrival moves nothing.

use std::f32::consts::TAU;
use std::time::Duration;

use eframe::egui::{Rect, Sense, Ui, pos2, vec2};

use crate::state::State;
use crate::theme;

/// How long one swell and fade of the shapes takes.
const PERIOD: f64 = 1.4;
/// How often they are redrawn: enough for the swell to look smooth, and
/// far less than every frame.
const REDRAW: Duration = Duration::from_millis(50);
/// A shelf's cards: as wide as the narrowest real ones, with their gap.
const CARD: f32 = 168.0;
const GAP: f32 = 16.0;
const ROW_HEIGHT: f32 = 56.0;

/// One grey shape. All of them swell and fade together.
fn block(state: &State, ui: &Ui, rect: Rect, radius: u8) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let palette = &state.palette;
    // With motion reduced the shapes hold still, half lit, and ask for no
    // redraws.
    let lit = if state.settings.reduce_motion {
        0.5
    } else {
        ui.ctx().request_repaint_after(REDRAW);
        let phase = (ui.input(|input| input.time) / PERIOD).fract() as f32;
        0.5 - 0.5 * (phase * TAU).cos()
    };
    let fill = crate::tint::blend(palette.surface, palette.surface_active, lit);
    ui.painter().rect_filled(rect, radius, fill);
}

/// A shelf: a title, and a row of as many cards as fit.
pub fn shelf(state: &State, ui: &mut Ui) {
    ui.add_space(32.0);
    let (title, _) = ui.allocate_exact_size(vec2(180.0, 24.0), Sense::hover());
    block(state, ui, title, 4);
    ui.add_space(12.0);
    let room = ui.available_width();
    let count = ((room + GAP) / (CARD + GAP)).floor().clamp(2.0, 8.0);
    let width = ((room - GAP * (count - 1.0)) / count).floor();
    let cover = width - 24.0;
    let (area, _) = ui.allocate_exact_size(vec2(room, cover + 80.0), Sense::hover());
    for index in 0..count as usize {
        let left = area.left() + index as f32 * (width + GAP) + 12.0;
        let top = area.top() + 12.0;
        block(
            state,
            ui,
            Rect::from_min_size(pos2(left, top), vec2(cover, cover)),
            4,
        );
        let line = |down: f32, part: f32| {
            Rect::from_min_size(pos2(left, top + cover + down), vec2(cover * part, 10.0))
        };
        block(state, ui, line(14.0, 0.8), 4);
        block(state, ui, line(32.0, 0.55), 4);
    }
}

pub fn shelves(state: &State, ui: &mut Ui, count: usize) {
    ui.spacing_mut().item_spacing.y = 0.0;
    for _ in 0..count {
        shelf(state, ui);
    }
}

/// A track list: a number, a cover, two lines and a length to each row.
pub fn tracks(state: &State, ui: &mut Ui, rows: usize) {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.add_space(theme::PAGE_PADDING);
    for _ in 0..rows {
        let size = vec2(ui.available_width(), ROW_HEIGHT);
        let (row, _) = ui.allocate_exact_size(size, Sense::hover());
        let middle = row.center().y;
        let at = |left: f32, width: f32, up: f32, height: f32| {
            Rect::from_min_size(pos2(row.left() + left, middle + up), vec2(width, height))
        };
        block(state, ui, at(24.0, 16.0, -8.0, 16.0), 4);
        block(state, ui, at(64.0, 40.0, -20.0, 40.0), 4);
        let text = (row.width() - 116.0 - 96.0).max(40.0);
        block(state, ui, at(116.0, text * 0.4, -13.0, 10.0), 4);
        block(state, ui, at(116.0, text * 0.25, 5.0, 10.0), 4);
        block(state, ui, at(row.width() - 56.0, 40.0, -5.0, 10.0), 4);
    }
}

/// A block of the page's own choosing, for a panel that is loading.
pub fn panel(state: &State, ui: &mut Ui, height: f32) {
    let size = vec2(ui.available_width(), height);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    block(state, ui, rect, theme::RADIUS);
}
