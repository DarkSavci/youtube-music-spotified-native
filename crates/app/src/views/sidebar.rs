//! The left panel: the library. At its usual width it has a header, chips
//! that narrow it, a search field and the order it is listed in; collapsed
//! it is a rail of covers; and expanded it takes the page's room, to be
//! looked at as a whole.

use eframe::egui::{self, Align, Align2, Frame, Layout, Margin, Sense, Ui, vec2};
use spotified_client::models::LibraryKind;

use super::library::{self, Layout as LibraryLayout};
use super::widgets;
use super::widgets::menu::{self, Entry};
use crate::actions::Action;
use crate::settings::LibrarySort;
use crate::state::State;
use crate::theme::{self, Icon};

const HEADER_HEIGHT: f32 = 56.0;
/// The room either side of everything above the list.
const PADDING: f32 = 16.0;
const GUTTERS: Margin = Margin {
    left: theme::GUTTER,
    right: theme::HALF_GUTTER,
    top: theme::HALF_GUTTER,
    bottom: theme::GUTTER,
};
/// The narrowest a cover gets in the sidebar's grid, and in the grid of
/// the library given the page's room.
const GRID_CELL: f32 = 100.0;
const GRID_CELL_EXPANDED: f32 = 170.0;
/// The widest the search field gets when the library has the page's room.
const SEARCH_MOST: f32 = 360.0;

const CHIPS: [(&str, LibraryKind); 3] = [
    ("Playlists", LibraryKind::Playlist),
    ("Artists", LibraryKind::Artist),
    ("Albums", LibraryKind::Album),
];

/// How the sidebar is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// At the width it was dragged to, which may be at most this.
    Full { most: f32 },
    /// Covers only.
    Rail,
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, shape: Shape) {
    let palette = &state.palette;
    let frame = widgets::card(palette, GUTTERS);
    match shape {
        Shape::Rail => {
            // Its own panel, so the width the full one was dragged to is
            // still there to come back to.
            egui::Panel::left("sidebar-rail")
                .resizable(false)
                .show_separator_line(false)
                .exact_size(theme::SIDEBAR_RAIL_WIDTH)
                .frame(frame)
                .show(ui, |ui| rail(state, ui, actions));
        }
        Shape::Full { most } => {
            let panel = egui::Panel::left("sidebar")
                .resizable(true)
                .show_separator_line(false)
                .default_size(state.settings.sidebar_width)
                .size_range(theme::SIDEBAR_MIN_WIDTH..=most)
                .frame(frame)
                .show(ui, |ui| contents(state, ui, actions));
            // The width that is remembered is the card's with its gutters,
            // which is what the panel is given back as its size.
            let width = panel.response.rect.width();
            // A width the window forced on the panel is not one the person
            // chose.
            let squeezed = width >= most - 1.0 && state.settings.sidebar_width > most;
            if !squeezed && (width - state.settings.sidebar_width).abs() >= 1.0 {
                actions.push(Action::ResizeSidebar(width));
            }
        }
    }
}

/// The library where the page would be, with `gutters` around its card.
pub fn expanded(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, gutters: Margin) {
    egui::CentralPanel::default()
        .frame(widgets::card(&state.palette, gutters))
        .show(ui, |ui| contents(state, ui, actions));
}

/// The collapsed sidebar: the way back to the full one, and the covers.
fn rail(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), HEADER_HEIGHT);
    let (header, _) = ui.allocate_exact_size(size, Sense::hover());
    let button = widgets::IconButton {
        icon: Icon::SquareLibrary,
        size: 22.0,
        tooltip: "Expand Your Library",
        active: false,
    };
    if button.show_at(ui, palette, header.center()).clicked() {
        actions.push(Action::ToggleSidebar);
    }
    padded(ui, 8.0, |ui| {
        library::show(state, ui, actions, LibraryLayout::Rail);
    });
}

/// Everything the sidebar holds at its usual width, and when expanded.
fn contents(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    ui.spacing_mut().item_spacing.y = 0.0;
    header(state, ui, actions);
    padded(ui, PADDING, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (label, kind) in CHIPS {
                let active = state.library_filter == Some(kind);
                if widgets::chip(ui, palette, label, active).clicked() {
                    actions.push(Action::FilterLibrary(kind));
                }
            }
        });
        ui.add_space(8.0);
        let room = ui.available_width();
        let field = widgets::TextField {
            text: &state.library_query,
            hint: "Search in your library",
            label: "Search in your library",
            icon: Some(Icon::Search),
            width: if state.library_expanded {
                room.min(SEARCH_MOST)
            } else {
                room
            },
            compact: true,
        };
        if let Some(query) = field.show(ui, palette) {
            actions.push(Action::SetLibraryQuery(query));
        }
        ui.add_space(8.0);
        tools(state, ui, actions);
        ui.add_space(8.0);
    });
    let grid = if state.library_expanded {
        state
            .settings
            .library_expanded_grid
            .then_some(GRID_CELL_EXPANDED)
    } else {
        state.settings.library_grid.then_some(GRID_CELL)
    };
    padded(ui, 8.0, |ui| {
        let layout = grid.map_or(LibraryLayout::List, LibraryLayout::Grid);
        library::show(state, ui, actions, layout);
    });
}

/// Draws `inner` with `side` points of room either side of it.
fn padded(ui: &mut Ui, side: f32, inner: impl FnOnce(&mut Ui)) {
    Frame::new()
        .inner_margin(Margin {
            left: side as i8,
            right: side as i8,
            top: 0,
            bottom: 0,
        })
        .show(ui, inner);
}

/// "Your Library", which collapses the sidebar, and beside it the ways to
/// make a playlist or a folder and to give the library the page's room.
fn header(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), HEADER_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(PADDING, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    // Expanded, there is no sidebar to collapse: the title is only that.
    let title = "Your Library";
    let font = theme::bold(14.0);
    let text = row
        .painter()
        .layout_no_wrap(title.to_owned(), font.clone(), palette.secondary);
    let size = vec2(22.0 + 12.0 + text.size().x, 32.0);
    let sense = if state.library_expanded {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (toggle, response) = row.allocate_exact_size(size, sense);
    let lift = if state.library_expanded {
        0.0
    } else {
        widgets::name(&row, &response, "Collapse Your Library");
        widgets::hover(&row, &response)
    };
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let icon = egui::Rect::from_min_size(
        egui::pos2(toggle.left(), toggle.center().y - 11.0),
        vec2(22.0, 22.0),
    );
    // Under the pointer the icon becomes the way it will go.
    let glyph = if lift > 0.5 {
        Icon::ChevronLeft
    } else {
        Icon::SquareLibrary
    };
    widgets::paint_icon(&row, glyph, icon, 22.0, color);
    let at = egui::pos2(icon.right() + 12.0, toggle.center().y);
    widgets::text_at(&row, at, Align2::LEFT_CENTER, title, font, color);
    if !state.library_expanded && response.on_hover_text("Collapse Your Library").clicked() {
        actions.push(Action::ToggleSidebar);
    }

    row.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let (icon, tooltip) = if state.library_expanded {
            (Icon::Minimize2, "Collapse library view")
        } else {
            (Icon::Maximize2, "Expand library view")
        };
        if widgets::icon_button(ui, palette, icon, 18.0, tooltip).clicked() {
            actions.push(Action::ToggleLibraryExpanded);
        }
        let create =
            widgets::icon_button(ui, palette, Icon::Plus, 18.0, "Create playlist or folder");
        menu::popup(&create, palette, |menu| {
            if menu.item("Create a playlist") {
                actions.push(Action::NewPlaylist {
                    name: "My playlist".to_owned(),
                    track_ids: Vec::new(),
                });
            }
            if menu.item("Create a folder") {
                actions.push(Action::NewFolder);
            }
        });
    });
}

/// The order the library is listed in, and whether as rows or as a grid.
fn tools(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let sort = state.settings.library_sort;
    let text = row.painter().layout_no_wrap(
        sort.label().to_owned(),
        theme::medium(12.0),
        palette.secondary,
    );
    let size = vec2(4.0 + text.size().x + 8.0 + 14.0 + 4.0, 26.0);
    let (button, response) = row.allocate_exact_size(size, Sense::click());
    widgets::name(&row, &response, "Sort library");
    let lift = widgets::hover(&row, &response);
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let at = egui::pos2(button.left() + 4.0, button.center().y - text.size().y / 2.0);
    let chevron = egui::Rect::from_center_size(
        egui::pos2(at.x + text.size().x + 8.0 + 7.0, button.center().y),
        vec2(14.0, 14.0),
    );
    row.painter().galley(at, text, color);
    widgets::paint_icon(&row, Icon::ChevronDown, chevron, 14.0, color);
    let response = response.on_hover_text("Sort library");
    menu::popup(&response, palette, |menu| {
        for choice in LibrarySort::EVERY {
            let entry = Entry::plain(choice.label()).checked(choice == sort);
            if menu.entry(entry) {
                actions.push(Action::SetLibrarySort(choice));
            }
        }
    });

    let grid = if state.library_expanded {
        state.settings.library_expanded_grid
    } else {
        state.settings.library_grid
    };
    // The button shows the way it will go, as the Electron app's does.
    let (icon, tooltip) = if grid {
        (Icon::List, "Show as list")
    } else {
        (Icon::LayoutGrid, "Show as grid")
    };
    row.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if widgets::icon_button(ui, palette, icon, 16.0, tooltip).clicked() {
            actions.push(Action::ToggleLibraryGrid);
        }
    });
}
