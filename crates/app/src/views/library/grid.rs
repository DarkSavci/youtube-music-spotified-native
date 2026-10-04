//! The library as covers: a grid of them with their names beneath, and
//! the collapsed sidebar's rail of covers alone.

use eframe::egui::{self, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{Folder, LibraryItem};

use super::arrange::Row;
use super::{folder_interact, highlight, holding, item_interact, item_kind, play_over};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::views::format::middle_dotted;
use crate::views::widgets;

const GAP: f32 = 8.0;
const CELL_PADDING: f32 = 10.0;
/// The room under a cover for its title and what it is.
const CELL_TEXT: f32 = 46.0;
/// A folder takes a line of its own across the grid.
const FOLDER_HEIGHT: f32 = 44.0;
const RAIL_CELL: f32 = 64.0;
const RAIL_THUMB: f32 = 48.0;

/// How many columns fit in `width` when none may be narrower than `least`,
/// and how wide each then is.
fn columns(width: f32, least: f32) -> (usize, f32) {
    let count = ((width + GAP) / (least + GAP)).floor().max(1.0);
    let cell = ((width - GAP * (count - 1.0)) / count).floor();
    (count as usize, cell)
}

/// Where each row goes: items fill the columns in turn, and a folder
/// breaks the line to take one of its own. Returns the places, relative to
/// the grid's top left, and the grid's height.
fn places(rows: &[Row], width: f32, least: f32) -> (Vec<Rect>, f32) {
    let (count, cell) = columns(width, least);
    let cell_height = cell + CELL_TEXT;
    let mut places = Vec::with_capacity(rows.len());
    let (mut column, mut top) = (0, 0.0);
    for row in rows {
        match row {
            Row::Folder { .. } => {
                if column > 0 {
                    top += cell_height + GAP;
                    column = 0;
                }
                places.push(Rect::from_min_size(
                    pos2(0.0, top),
                    vec2(width, FOLDER_HEIGHT),
                ));
                top += FOLDER_HEIGHT + GAP;
            }
            Row::Item { .. } => {
                let left = column as f32 * (cell + GAP);
                places.push(Rect::from_min_size(
                    pos2(left, top),
                    vec2(cell, cell_height),
                ));
                column += 1;
                if column == count {
                    column = 0;
                    top += cell_height + GAP;
                }
            }
        }
    }
    if column > 0 {
        top += cell_height + GAP;
    }
    (places, (top - GAP).max(0.0))
}

pub fn grid(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, rows: &[Row], least: f32) {
    egui::ScrollArea::vertical()
        .id_salt("library-grid")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (places, height) = places(rows, ui.available_width(), least);
            let (area, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
            for (index, (row, place)) in rows.iter().zip(places).enumerate() {
                let rect = place.translate(area.min.to_vec2());
                // Nothing is laid out, or asked of the network, for what
                // is out of sight.
                if !ui.is_rect_visible(rect) {
                    continue;
                }
                let id = ui.id().with(("cell", index));
                match row {
                    Row::Folder {
                        folder,
                        holds,
                        open,
                    } => folder_line(state, ui, actions, (id, rect), folder, (*holds, *open)),
                    Row::Item { item, .. } => cell(state, ui, actions, (id, rect), item),
                }
            }
        });
}

/// A folder in the grid: a line across it, with its name and how much it
/// holds.
fn folder_line(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (id, rect): (egui::Id, Rect),
    folder: &Folder,
    (holds, open): (usize, bool),
) {
    let palette = &state.palette;
    let response = ui.interact(rect, id, Sense::click());
    widgets::name(ui, &response, &folder.name);
    folder_interact(&state.palette, actions, &response, folder);
    highlight(state, ui, rect, false, &response);
    let icon = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(18.0, 18.0));
    widgets::paint_icon(ui, Icon::Folder, icon, 18.0, palette.secondary);
    let chevron =
        Rect::from_center_size(pos2(rect.right() - 16.0, rect.center().y), vec2(16.0, 16.0));
    let glyph = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    widgets::paint_icon(ui, glyph, chevron, 16.0, palette.secondary);
    let text = middle_dotted([folder.name.as_str(), &holding(holds)]);
    let width = (chevron.left() - icon.right() - 16.0).max(20.0);
    let galley = widgets::elided(ui, &text, theme::medium(13.0), palette.text, width, 1);
    let at = pos2(icon.right() + 10.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, palette.text);
}

/// One thing in the grid: its cover, its title, and what it is.
fn cell(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (id, rect): (egui::Id, Rect),
    item: &LibraryItem,
) {
    let palette = &state.palette;
    let response = ui.interact(rect, id, Sense::click());
    widgets::name(ui, &response, &item.title);
    let open = item_interact(state, ui, actions, &response, item);
    let (_, kind, shape, placeholder) = item_kind(item);
    highlight(state, ui, rect, open, &response);
    let side = rect.width() - CELL_PADDING * 2.0;
    let cover = Rect::from_min_size(
        rect.min + vec2(CELL_PADDING, CELL_PADDING),
        vec2(side, side),
    );
    widgets::artwork(ui, state, &item.artwork, cover, shape, placeholder);
    play_over(ui, actions, (&response, rect), (cover, shape), item);
    let title = widgets::elided(ui, &item.title, theme::medium(14.0), palette.text, side, 1);
    let top = cover.bottom() + 8.0;
    ui.painter()
        .galley(pos2(cover.left(), top), title, palette.text);
    let second = middle_dotted([kind, &item.subtitle]);
    let font = theme::regular(12.0);
    let second = widgets::elided(ui, &second, font, palette.secondary, side, 1);
    ui.painter()
        .galley(pos2(cover.left(), top + 19.0), second, palette.secondary);
    if item.pinned {
        let pin = Rect::from_min_size(cover.min + vec2(6.0, 6.0), vec2(20.0, 20.0));
        ui.painter()
            .circle_filled(pin.center(), 10.0, palette.panel.gamma_multiply(0.85));
        widgets::paint_icon(ui, Icon::Pin, pin, 12.0, palette.accent);
    }
}

/// The collapsed sidebar: covers alone, each named by a tooltip.
pub fn rail(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, rows: &[Row]) {
    let palette = &state.palette;
    egui::ScrollArea::vertical()
        .id_salt("library-rail")
        .auto_shrink([false, false])
        .show_rows(ui, RAIL_CELL, rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in &rows[range] {
                let size = vec2(ui.available_width(), RAIL_CELL);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let thumb = Rect::from_center_size(rect.center(), vec2(RAIL_THUMB, RAIL_THUMB));
                match row {
                    Row::Folder { folder, holds, .. } => {
                        widgets::name(ui, &response, &folder.name);
                        folder_interact(&state.palette, actions, &response, folder);
                        highlight(state, ui, rect, false, &response);
                        ui.painter().rect_filled(thumb, 4.0, palette.surface);
                        widgets::paint_icon(ui, Icon::Folder, thumb, 20.0, palette.secondary);
                        let tip = middle_dotted([folder.name.as_str(), &holding(*holds)]);
                        response.on_hover_text(tip);
                    }
                    Row::Item { item, nested } => {
                        widgets::name(ui, &response, &item.title);
                        let open = item_interact(state, ui, actions, &response, item);
                        let (_, kind, shape, placeholder) = item_kind(item);
                        highlight(state, ui, rect, open, &response);
                        // What is in a folder is drawn a little smaller, to
                        // be told from what is not.
                        let thumb = if *nested { thumb.shrink(6.0) } else { thumb };
                        widgets::artwork(ui, state, &item.artwork, thumb, shape, placeholder);
                        play_over(ui, actions, (&response, rect), (thumb, shape), item);
                        let what = middle_dotted([kind, &item.subtitle]);
                        response.on_hover_text(format!("{}\n{what}", item.title));
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_many_columns_as_fit_share_the_width() {
        // Two columns of at least 100 fit in 248 with a gap between.
        assert_eq!(columns(248.0, 100.0), (2, 120.0));
        // Too narrow for even one: one column, as wide as there is.
        assert_eq!(columns(90.0, 100.0), (1, 90.0));
    }

    #[test]
    fn a_folder_breaks_the_line_and_takes_one_of_its_own() {
        let item = LibraryItem::default();
        let folder = Folder::default();
        let rows = [
            Row::Item {
                item: &item,
                nested: false,
            },
            Row::Folder {
                folder: &folder,
                holds: 0,
                open: false,
            },
            Row::Item {
                item: &item,
                nested: false,
            },
            Row::Item {
                item: &item,
                nested: false,
            },
        ];
        let (places, height) = places(&rows, 248.0, 100.0);
        let cell = 120.0 + CELL_TEXT;
        assert_eq!(places[0].min, pos2(0.0, 0.0));
        // The folder starts a new line though the first was not full.
        assert_eq!(places[1].min, pos2(0.0, cell + GAP));
        assert_eq!(places[1].width(), 248.0);
        let below = cell + GAP + FOLDER_HEIGHT + GAP;
        assert_eq!(places[2].min, pos2(0.0, below));
        assert_eq!(places[3].min, pos2(128.0, below));
        assert_eq!(height, below + cell);
    }
}
