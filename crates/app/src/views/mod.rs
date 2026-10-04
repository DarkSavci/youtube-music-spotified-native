//! The window: the top bar across it, a player bar along the bottom, the
//! library down the left, and the page in what remains. Each panel is a
//! rounded card; the window's own, darker colour shows in the gutters
//! between them.

mod browse;
mod cards;
mod changelog;
mod chrome;
mod collection;
mod dialogs;
mod drag;
mod format;
pub mod keys;
mod library;
mod lyrics;
mod menus;
pub mod mini;
mod pages;
mod player_bar;
mod queue;
mod settings;
mod sidebar;
mod stats;
mod toasts;
mod together;
mod topbar;
mod tracks;
mod visualizer;
pub mod widgets;

use eframe::egui::{self, Frame, Margin, Ui};

use crate::actions::Action;
use crate::settings::RightPanel;
use crate::state::State;
use crate::theme::{self, GUTTER, HALF_GUTTER};

/// How far down a page its cover's colour reaches before it has faded
/// into the panel's own.
const TINT_HEIGHT: f32 = 340.0;
/// How much of the cover's colour the top of the page takes.
const TINT_STRENGTH: f32 = 0.85;
const TINT_STRENGTH_LIGHT: f32 = 0.22;

/// The least width the page keeps, whatever is open beside it.
const PAGE_MIN: f32 = 420.0;
/// The least a panel at the right can be.
pub(super) const RIGHT_PANEL_MIN: f32 = 300.0;

/// The gutters of a panel at the right of the window, under the top bar.
pub(super) const RIGHT_PANEL_GUTTERS: Margin = Margin {
    left: HALF_GUTTER,
    right: GUTTER,
    top: HALF_GUTTER,
    bottom: GUTTER,
};

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    keys::handle(ui.ctx(), actions);
    // The lyrics, given the whole window, bring their own transport.
    if state.lyrics_fullscreen {
        lyrics::fullscreen(state, ui, actions);
        drag::ghost(state, ui);
        toasts::show(state, ui);
        return;
    }
    // egui gives outer panels their space first, so the order here is the
    // order of precedence: the bars span the window, the sidebar takes the
    // left of what is between them.
    player_bar::show(state, ui, actions);
    topbar::show(state, ui, actions);
    let room = Room::in_window(state, ui.ctx().content_rect().width());
    if room.sidebar {
        sidebar::show(state, ui, actions, room.sidebar_most);
    }
    match state.settings.panel {
        RightPanel::Queue => queue::show(state, ui, actions, room.right_most),
        RightPanel::Lyrics => lyrics::show(state, ui, actions, room.right_most),
        RightPanel::Closed => {}
    }
    page(state, ui, actions, room.sidebar);
    chrome::resize(state, ui);
    drag::ghost(state, ui);
    dialogs::show(state, ui, actions);
    toasts::show(state, ui);
}

/// How the window's width is shared out. The page always keeps enough to
/// be read; the panels either side take what is over, and in a window too
/// narrow for all three the sidebar steps aside.
#[derive(Debug, PartialEq)]
struct Room {
    sidebar: bool,
    /// The widest the sidebar and the right panel may be dragged to.
    sidebar_most: f32,
    right_most: f32,
}

impl Room {
    fn in_window(state: &State, width: f32) -> Self {
        let wanted = state.settings.sidebar_visible;
        let right_open = state.settings.panel != RightPanel::Closed;
        Self::share(width, wanted, right_open, state.settings.sidebar_width)
    }

    fn share(width: f32, sidebar_wanted: bool, right_open: bool, sidebar_width: f32) -> Self {
        let right_least = if right_open { RIGHT_PANEL_MIN } else { 0.0 };
        let for_sidebar = width - PAGE_MIN - right_least;
        let sidebar = sidebar_wanted && for_sidebar >= theme::SIDEBAR_MIN_WIDTH;
        let sidebar_most = for_sidebar.clamp(theme::SIDEBAR_MIN_WIDTH, theme::SIDEBAR_MAX_WIDTH);
        let taken = if sidebar {
            sidebar_width.min(sidebar_most)
        } else {
            0.0
        };
        Self {
            sidebar,
            sidebar_most,
            right_most: (width - PAGE_MIN - taken).max(RIGHT_PANEL_MIN),
        }
    }
}

/// The page, in the card between the sidebar and the right panel.
fn page(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, beside_sidebar: bool) {
    let palette = &state.palette;
    // A whole gutter where the card meets the window's edge, half of one
    // where it meets another card.
    let beside = |neighbour: bool| if neighbour { HALF_GUTTER } else { GUTTER };
    let gutters = Margin {
        left: beside(beside_sidebar),
        right: beside(state.settings.panel != RightPanel::Closed),
        top: HALF_GUTTER,
        bottom: GUTTER,
    };
    egui::CentralPanel::default()
        .frame(widgets::card(palette, gutters))
        .show(ui, |ui| {
            let card = ui.max_rect();
            if let Some(tint) = collection::page_tint(state, ui) {
                let header =
                    egui::Rect::from_min_size(card.min, egui::vec2(card.width(), TINT_HEIGHT));
                // The tint is a dark colour: a light page takes a breath
                // of it, where a dark one takes it nearly whole.
                let strength = if palette.dark {
                    TINT_STRENGTH
                } else {
                    TINT_STRENGTH_LIGHT
                };
                let top = crate::tint::blend(palette.panel, tint, strength);
                widgets::vertical_gradient(ui, header, top, palette.panel);
                widgets::round_off(ui, card, palette);
            }
            egui::ScrollArea::vertical()
                .id_salt(("page", state.nav.page()))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Frame::new()
                        .inner_margin(Margin {
                            left: theme::PAGE_PADDING as i8,
                            right: theme::PAGE_PADDING as i8,
                            top: 16,
                            bottom: 48,
                        })
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            pages::show(state, ui, actions);
                        });
                });
        });
}

#[cfg(test)]
mod tests;
