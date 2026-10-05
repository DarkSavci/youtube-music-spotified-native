//! The window: the top bar across it, a player bar along the bottom, the
//! library down the left, and the page in what remains. Each panel is a
//! rounded card; the window's own, darker colour shows in the gutters
//! between them.

mod actions_menu;
mod browse;
mod cards;
mod changelog;
mod chrome;
mod collection;
mod dialogs;
mod drag;
mod equalizer;
pub mod flyout;
pub(crate) mod format;
mod fullscreen;
pub mod keys;
mod library;
mod lyrics;
mod menus;
mod migration;
pub mod mini;
mod notice;
mod pages;
mod player_bar;
mod queue;
mod search;
mod settings;
mod sidebar;
mod skeleton;
mod speed;
mod stats;
mod toasts;
mod together;
mod topbar;
mod tracks;
mod video;
mod visualizer;
mod volume;
pub mod widgets;

use eframe::egui::{self, Frame, Margin, Ui};

use crate::actions::Action;
use crate::settings::RightPanel;
use crate::state::State;
use crate::theme::{self, GUTTER, HALF_GUTTER};

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

/// Installs the Winamp skins dropped on the window this frame: a `.wsz`
/// let go over either window is put in the skins folder and worn.
pub fn drop_skins(ctx: &egui::Context, actions: &mut Vec<Action>) {
    let skins: Vec<std::path::PathBuf> = ctx.input(|input| {
        let dropped = input.raw.dropped_files.iter();
        dropped
            .map(|file| file.path().to_path_buf())
            .filter(|path| crate::skins::is_skin_file(path))
            .collect()
    });
    if !skins.is_empty() {
        actions.push(Action::Skin(crate::skins::Ask::Install(skins)));
    }
}

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    keys::handle(ui.ctx(), actions);
    drop_skins(ui.ctx(), actions);
    notice::announce(state, ui);
    // What is playing, given the whole window, with its own transport.
    // With nothing playing there is nothing to give it to, and it closes.
    if state.fullscreen_player {
        match state
            .playback
            .as_ref()
            .filter(|playback| playback.current().is_some())
        {
            Some(playback) => {
                fullscreen::show(state, ui, actions, playback);
                toasts::show(state, ui, actions);
                return;
            }
            None => actions.push(Action::SetFullscreenPlayer(false)),
        }
    }
    // The lyrics, given the whole window, bring their own transport.
    if state.lyrics_fullscreen {
        lyrics::fullscreen(state, ui, actions);
        drag::ghost(state, ui);
        toasts::show(state, ui, actions);
        return;
    }
    // egui gives outer panels their space first, so the order here is the
    // order of precedence: the bars span the window, the sidebar takes the
    // left of what is between them.
    player_bar::show(state, ui, actions);
    topbar::show(state, ui, actions);
    let room = Room::in_window(state, ui.ctx().content_rect().width());
    // The library, given the page's room, is drawn where the page would be.
    let expanded = state.library_expanded;
    match room.sidebar {
        Side::Full if !expanded => {
            let most = room.sidebar_most;
            sidebar::show(state, ui, actions, sidebar::Shape::Full { most });
        }
        Side::Rail if !expanded => sidebar::show(state, ui, actions, sidebar::Shape::Rail),
        Side::Full | Side::Rail | Side::Away => {}
    }
    match state.settings.panel {
        RightPanel::Queue => queue::show(state, ui, actions, room.right_most),
        RightPanel::Lyrics => lyrics::show(state, ui, actions, room.right_most),
        RightPanel::Closed => {}
    }
    if expanded {
        sidebar::expanded(state, ui, actions, page_gutters(state, false));
    } else {
        page(state, ui, actions, room.sidebar != Side::Away);
    }
    chrome::resize(state, ui);
    drag::ghost(state, ui);
    dialogs::show(state, ui, actions);
    notice::show(state, ui, actions);
    toasts::show(state, ui, actions);
}

/// How the window's width is shared out. The page always keeps enough to
/// be read; the panels either side take what is over, and in a window too
/// narrow for all three the sidebar steps aside.
#[derive(Debug, PartialEq)]
struct Room {
    sidebar: Side,
    /// The widest the sidebar and the right panel may be dragged to.
    sidebar_most: f32,
    right_most: f32,
}

/// What the window has room for at its left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The library, at the width it was dragged to.
    Full,
    /// A rail of covers: collapsed on purpose, or for want of room.
    Rail,
    /// Nothing: the window is too narrow even for the rail.
    Away,
}

impl Room {
    fn in_window(state: &State, width: f32) -> Self {
        let collapsed = state.settings.sidebar_collapsed;
        let right_open = state.settings.panel != RightPanel::Closed;
        Self::share(width, collapsed, right_open, state.settings.sidebar_width)
    }

    fn share(width: f32, collapsed: bool, right_open: bool, sidebar_width: f32) -> Self {
        let right_least = if right_open { RIGHT_PANEL_MIN } else { 0.0 };
        let for_sidebar = width - PAGE_MIN - right_least;
        let sidebar = if !collapsed && for_sidebar >= theme::SIDEBAR_MIN_WIDTH {
            Side::Full
        } else if for_sidebar >= theme::SIDEBAR_RAIL_WIDTH {
            Side::Rail
        } else {
            Side::Away
        };
        let sidebar_most = for_sidebar.clamp(theme::SIDEBAR_MIN_WIDTH, theme::SIDEBAR_MAX_WIDTH);
        let taken = match sidebar {
            Side::Full => sidebar_width.min(sidebar_most),
            Side::Rail => theme::SIDEBAR_RAIL_WIDTH,
            Side::Away => 0.0,
        };
        Self {
            sidebar,
            sidebar_most,
            right_most: (width - PAGE_MIN - taken).max(RIGHT_PANEL_MIN),
        }
    }
}

/// The gutters of the card in the middle: a whole one where it meets the
/// window's edge, half of one where it meets another card.
fn page_gutters(state: &State, beside_sidebar: bool) -> Margin {
    let beside = |neighbour: bool| if neighbour { HALF_GUTTER } else { GUTTER };
    Margin {
        left: beside(beside_sidebar),
        right: beside(state.settings.panel != RightPanel::Closed),
        top: HALF_GUTTER,
        bottom: GUTTER,
    }
}

/// Where the page's card is, for a page that paints or pins something
/// against its edges. Told to the views through egui's own memory, since
/// they are handed nothing but the state.
pub(super) fn page_card(ui: &Ui) -> egui::Rect {
    ui.data(|data| data.get_temp(egui::Id::new(PAGE_CARD)))
        .unwrap_or_else(|| ui.clip_rect())
}

const PAGE_CARD: &str = "page-card";

/// The page, in the card between the sidebar and the right panel.
fn page(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, beside_sidebar: bool) {
    let palette = &state.palette;
    egui::CentralPanel::default()
        .frame(widgets::card(palette, page_gutters(state, beside_sidebar)))
        .show(ui, |ui| {
            let card = ui.max_rect();
            ui.data_mut(|data| data.insert_temp(egui::Id::new(PAGE_CARD), card));
            // The music video sits above the page, which scrolls under it.
            video::notice_line(state, ui);
            if state.video.enabled {
                let height = video::main_height(ui.ctx().content_rect().height());
                let size = egui::vec2(ui.available_width(), height);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                video::surface(state, ui, actions, rect, true);
            }
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("page", state.nav.page()))
                .auto_shrink([false, false]);
            if let Some(offset) = state.held_scroll {
                area = area.vertical_scroll_offset(offset);
            }
            area.show(ui, |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: theme::PAGE_PADDING as i8,
                        right: theme::PAGE_PADDING as i8,
                        top: 0,
                        bottom: 48,
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        pages::show(state, ui, actions);
                    });
            });
            // A page's header is painted to the card's edges, and is
            // square where the card is round.
            widgets::round_off(ui, card, palette);
        });
}

#[cfg(test)]
mod tests;
