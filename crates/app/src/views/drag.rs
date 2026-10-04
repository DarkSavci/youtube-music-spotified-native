//! Songs carried by the pointer.
//!
//! A row, or a whole selection, can be dragged onto a playlist in the
//! sidebar, onto Liked Music, or onto the queue. While it is carried, a
//! small label follows the pointer saying what it is.

use eframe::egui::{self, Align2, Color32, Response, Ui, vec2};
use spotified_client::models::Track;

use crate::state::State;
use crate::theme;

/// What a drag carries: the songs, in the order they were shown.
pub struct Dragged {
    pub tracks: Vec<Track>,
}

/// Starts carrying `tracks` if `response` has just begun to be dragged.
/// The songs are only gathered then, not on every frame.
pub fn start(response: &Response, tracks: impl FnOnce() -> Vec<Track>) {
    if response.drag_started() {
        response.dnd_set_drag_payload(Dragged { tracks: tracks() });
    }
}

/// Marks `response` as somewhere songs can be dropped: lit while they are
/// held over it. Returns the songs on the frame they are let go there.
pub fn target(state: &State, ui: &Ui, response: &Response) -> Option<Vec<Track>> {
    if response.dnd_hover_payload::<Dragged>().is_some() {
        let palette = &state.palette;
        ui.painter().rect(
            response.rect,
            theme::RADIUS_ROW,
            palette.accent.gamma_multiply(0.18),
            (1.5, palette.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
        .dnd_release_payload::<Dragged>()
        .map(|dragged| dragged.tracks.clone())
}

/// The label that follows the pointer while songs are carried.
pub fn ghost(state: &State, ui: &Ui) {
    let ctx = ui.ctx();
    let (Some(dragged), Some(pointer)) = (
        egui::DragAndDrop::payload::<Dragged>(ctx),
        ctx.pointer_interact_pos(),
    ) else {
        return;
    };
    let Some(first) = dragged.tracks.first() else {
        return;
    };
    let label = match dragged.tracks.len() {
        1 => first.title.clone(),
        count => format!("{} + {} more", first.title, count - 1),
    };
    let palette = &state.palette;
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("drag-ghost"),
    ));
    let galley = painter.layout_no_wrap(label, theme::medium(13.0), palette.text);
    let padding = vec2(10.0, 6.0);
    let rect =
        Align2::LEFT_TOP.anchor_size(pointer + vec2(16.0, 6.0), galley.size() + padding * 2.0);
    painter.rect_filled(rect, theme::RADIUS_ROW, palette.overlay.gamma_multiply(0.9));
    painter.galley(rect.min + padding, galley, Color32::PLACEHOLDER);
}
