//! The left panel: where to go, and the library.

use eframe::egui::{self, Align, Align2, Layout, Margin, Rect, Sense, Ui, vec2};
use spotified_client::models::LibraryKind;

use super::{library, widgets};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon};

const NAV_ROW_HEIGHT: f32 = 40.0;
const GUTTERS: Margin = Margin {
    left: theme::GUTTER,
    right: theme::HALF_GUTTER,
    top: theme::HALF_GUTTER,
    bottom: theme::GUTTER,
};

const CHIPS: [(&str, LibraryKind); 3] = [
    ("Playlists", LibraryKind::Playlist),
    ("Albums", LibraryKind::Album),
    ("Artists", LibraryKind::Artist),
];

/// `most` is the widest the panel may be dragged in this window.
pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, most: f32) {
    let palette = &state.palette;
    let panel = egui::Panel::left("sidebar")
        .resizable(true)
        .show_separator_line(false)
        .default_size(state.settings.sidebar_width)
        .size_range(theme::SIDEBAR_MIN_WIDTH..=most)
        .frame(widgets::card(palette, GUTTERS).inner_margin(Margin {
            left: 12,
            right: 8,
            top: 12,
            bottom: 8,
        }))
        .show(ui, |ui| {
            nav_row(state, ui, actions, Icon::House, "Home", Page::Home);
            nav_row(state, ui, actions, Icon::Search, "Search", Page::Search);
            ui.add_space(6.0);
            ui.painter().hline(
                ui.max_rect().x_range(),
                ui.cursor().top(),
                (1.0, palette.outline),
            );
            ui.add_space(10.0);
            library_header(state, ui, actions);
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                for (label, kind) in CHIPS {
                    let active = state.library_filter == Some(kind);
                    if widgets::chip(ui, palette, label, active).clicked() {
                        actions.push(Action::FilterLibrary(kind));
                    }
                }
            });
            ui.add_space(8.0);
            library::show(state, ui, actions);
        });
    // The width that is remembered is the card's with its gutters, which
    // is what the panel is given back as its size.
    let width = panel.response.rect.width();
    // A width the window forced on the panel is not one the person chose.
    let squeezed = width >= most - 1.0 && state.settings.sidebar_width > most;
    if !squeezed && (width - state.settings.sidebar_width).abs() >= 1.0 {
        actions.push(Action::ResizeSidebar(width));
    }
}

fn nav_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    icon: Icon,
    label: &str,
    page: Page,
) {
    let palette = &state.palette;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), NAV_ROW_HEIGHT), Sense::click());
    widgets::name(ui, &response, label);
    let active = state.nav.page() == &page;
    let lift = if active {
        1.0
    } else {
        widgets::hover(ui, &response)
    };
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    widgets::row_hover(ui, &response, rect);
    let icon_rect = Rect::from_min_size(
        rect.left_center() + vec2(8.0, -11.0),
        egui::Vec2::splat(22.0),
    );
    widgets::paint_icon(ui, icon, icon_rect, 22.0, color);
    widgets::text_at(
        ui,
        rect.left_center() + vec2(44.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        theme::bold(15.0),
        color,
    );
    if response.clicked() {
        actions.push(Action::Open(page));
    }
}

fn library_header(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(Icon::Library.image(palette.secondary, 22.0));
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Library")
                .font(theme::bold(15.0))
                .color(palette.secondary),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            if widgets::icon_button(ui, palette, Icon::PanelLeft, 16.0, "Hide sidebar").clicked() {
                actions.push(Action::ToggleSidebar);
            }
            let create =
                widgets::icon_button(ui, palette, Icon::Plus, 16.0, "Create a playlist or folder");
            egui::Popup::menu(&create).show(|ui| {
                ui.set_min_width(160.0);
                if ui.button("New playlist").clicked() {
                    actions.push(Action::NewPlaylist {
                        track_ids: Vec::new(),
                    });
                    ui.close();
                }
                if ui.button("New folder").clicked() {
                    actions.push(Action::NewFolder);
                    ui.close();
                }
            });
        });
    });
}
