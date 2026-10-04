//! The bar across the top of the window: history, search, the account, and
//! on Windows the window's own buttons. It is the title bar: dragging it
//! moves the window.

use eframe::egui::{
    self, Align, CornerRadius, Frame, Layout, Margin, Sense, TextEdit, Ui, Vec2, vec2,
};

use super::{chrome, widgets};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon, Palette};

const HISTORY_BUTTON: f32 = 32.0;
const SEARCH_HEIGHT: f32 = 36.0;
/// The room the Browse all button takes inside the search field.
const BROWSE_BUTTON: f32 = 40.0;
const MENU_WIDTH: f32 = 248.0;
const SEARCH_WIDTH: std::ops::RangeInclusive<f32> = 160.0..=460.0;
/// The room kept at the right of the bar for the account and settings.
const RIGHT_CONTROLS: f32 = 190.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    // Not a card: the bar sits on the window itself, as a title bar does.
    let frame = Frame::new().fill(palette.window).inner_margin(Margin {
        left: theme::GUTTER,
        right: 0,
        top: theme::HALF_GUTTER,
        bottom: 0,
    });
    egui::Panel::top("top-bar")
        .exact_size(theme::TOP_BAR_HEIGHT + f32::from(theme::HALF_GUTTER))
        .resizable(false)
        .show_separator_line(false)
        .frame(frame)
        .show(ui, |ui| {
            let bar = ui.max_rect();
            // Registered before the controls, which then keep their clicks:
            // what is left of the bar moves the window.
            chrome::drag(
                state,
                ui,
                bar.expand2(vec2(0.0, f32::from(theme::HALF_GUTTER))),
            );
            let buttons = chrome::buttons_width(state);
            let (controls, window_buttons) = bar.split_left_right_at_x(bar.right() - buttons);
            // The window's buttons reach the top edge, as the system's do.
            let window_buttons =
                window_buttons.with_min_y(bar.top() - f32::from(theme::HALF_GUTTER));
            chrome::buttons(state, ui, window_buttons);
            let mut controls_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(controls)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            self::controls(state, &mut controls_ui, actions);
        });
}

/// History, the search field in the middle, and the account at the right.
fn controls(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let bar = ui.max_rect();
    if !state.settings.sidebar_visible {
        if widgets::icon_button(ui, palette, Icon::PanelLeft, 19.0, "Show sidebar").clicked() {
            actions.push(Action::ToggleSidebar);
        }
        if widgets::icon_button(ui, palette, Icon::House, 19.0, "Home").clicked() {
            actions.push(Action::Open(Page::Home));
        }
    }
    let back = history_button(
        ui,
        palette,
        Icon::ChevronLeft,
        state.nav.can_go_back(),
        "Back",
    );
    if back {
        actions.push(Action::Back);
    }
    let forward = history_button(
        ui,
        palette,
        Icon::ChevronRight,
        state.nav.can_go_forward(),
        "Forward",
    );
    if forward {
        actions.push(Action::Forward);
    }
    // The search field sits in the middle of the window when there is room
    // either side, and otherwise in what the two ends leave it.
    let start = ui.cursor().left() + 8.0;
    let end = bar.right() - RIGHT_CONTROLS;
    let width = (end - start).clamp(*SEARCH_WIDTH.start(), *SEARCH_WIDTH.end());
    let centred = ui.ctx().content_rect().center().x - width / 2.0;
    let left = centred.min(end - width).max(start);
    ui.add_space(left - ui.cursor().left());
    search_field(state, ui, actions, width);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(theme::GUTTER.into());
        avatar(state, ui, actions);
        let offline = state
            .playback
            .as_ref()
            .is_some_and(|playback| playback.offline);
        if offline {
            ui.label(
                egui::RichText::new("Offline")
                    .font(theme::medium(12.5))
                    .color(palette.warning),
            )
            .on_hover_text("No connection to YouTube Music. Cached songs still play.");
        }
        if widgets::icon_button(ui, palette, Icon::Settings, 19.0, "Settings").clicked() {
            actions.push(Action::Open(Page::Settings));
        }
        // Lit while in a room, so the way back to it is easy to find.
        let together = widgets::IconButton {
            icon: Icon::Users,
            size: 19.0,
            tooltip: "Listen Together",
            active: state.together.in_room(),
        };
        let at = ui.allocate_space(Vec2::splat(31.0)).1.center();
        if together.show_at(ui, palette, at).clicked() {
            actions.push(Action::Open(Page::Together));
        }
    });
}

/// A round back or forward button. Returns whether it was clicked.
fn history_button(
    ui: &mut Ui,
    palette: &Palette,
    icon: Icon,
    enabled: bool,
    tooltip: &str,
) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(HISTORY_BUTTON), sense);
    widgets::name(ui, &response, tooltip);
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.panel, palette.surface_hover, lift);
    ui.painter()
        .circle_filled(rect.center(), HISTORY_BUTTON / 2.0, fill);
    let color = if enabled {
        crate::tint::blend(palette.secondary, palette.text, lift)
    } else {
        palette.dim
    };
    widgets::paint_icon(ui, icon, rect, 20.0, color);
    response.on_hover_text(tooltip).clicked()
}

fn search_field(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, width: f32) {
    let palette = &state.palette;
    Frame::new()
        .fill(palette.surface)
        .corner_radius(CornerRadius::same(u8::MAX))
        .inner_margin(Margin {
            left: 12,
            right: 4,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.set_height(SEARCH_HEIGHT);
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.add(Icon::Search.image(palette.secondary, 16.0));
            // The view may not change state, so it edits a copy and asks.
            let mut query = state.search.query.clone();
            let field = TextEdit::singleline(&mut query)
                .hint_text("What do you want to play?")
                .frame(Frame::NONE)
                .desired_width(width - 48.0 - BROWSE_BUTTON);
            if ui.add(field).changed() {
                actions.push(Action::SetSearchQuery(query));
            }
            // Browsing is the other way to find something, so its button
            // shares the field, behind a hairline.
            let line = ui.cursor().left() - 2.0;
            let reach = ui.max_rect().y_range().shrink(8.0);
            ui.painter().vline(line, reach, (1.0, palette.outline));
            let browse = widgets::icon_button(ui, palette, Icon::LayoutGrid, 16.0, "Browse all");
            if browse.clicked() {
                actions.push(Action::BrowseAll);
            }
        });
}

/// The account button and the menu it opens: who is signed in, and the
/// pages that are about them.
fn avatar(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
    widgets::name(ui, &response, "Account");
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.surface, palette.surface_active, lift);
    ui.painter().circle_filled(rect.center(), 18.0, fill);
    initial_or_icon(
        state,
        ui,
        rect,
        crate::tint::blend(palette.secondary, palette.text, lift),
    );
    // A dot says there are release notes not yet opened.
    if state.release_notes_unread() {
        let at = rect.right_top() + vec2(-5.0, 5.0);
        ui.painter().circle_filled(at, 4.0, palette.accent);
    }
    egui::Popup::menu(&response).show(|ui| {
        ui.set_width(MENU_WIDTH);
        ui.spacing_mut().item_spacing.y = 2.0;
        if let Some(action) = account_menu(state, ui) {
            actions.push(action);
            ui.close();
        }
    });
}

/// The account's initial once it is known, a figure until then.
fn initial_or_icon(state: &State, ui: &Ui, rect: egui::Rect, color: egui::Color32) {
    let initial = state
        .account
        .as_ref()
        .and_then(|account| account.name.chars().next());
    match initial {
        Some(initial) => {
            let letter: String = initial.to_uppercase().collect();
            let font = theme::bold(rect.height() * 0.42);
            let anchor = egui::Align2::CENTER_CENTER;
            widgets::text_at(ui, rect.center(), anchor, &letter, font, color);
        }
        None => widgets::paint_icon(ui, Icon::User, rect, rect.height() * 0.5, color),
    }
}

/// What the account's menu holds. Returns what was chosen.
fn account_menu(state: &State, ui: &mut Ui) -> Option<Action> {
    let palette = &state.palette;
    // Who this is: a disc, a name, and the handle or what signing in gives.
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
    let disc = egui::Rect::from_center_size(
        egui::pos2(head.left() + 28.0, head.center().y),
        Vec2::splat(40.0),
    );
    ui.painter()
        .circle_filled(disc.center(), 20.0, palette.accent.gamma_multiply(0.2));
    initial_or_icon(state, ui, disc, palette.accent);
    let (name, detail) = match &state.account {
        Some(account) if account.handle.is_empty() => (account.name.as_str(), "YouTube Music"),
        Some(account) => (account.name.as_str(), account.handle.as_str()),
        None => ("Not signed in", "Sign in for your library"),
    };
    let left = disc.right() + 12.0;
    let width = head.right() - left - 8.0;
    let title = widgets::elided(ui, name, theme::semibold(14.0), palette.text, width, 1);
    ui.painter().galley(
        egui::pos2(left, head.center().y - 18.0),
        title,
        palette.text,
    );
    let font = theme::regular(12.0);
    let second = widgets::elided(ui, detail, font, palette.secondary, width, 1);
    ui.painter().galley(
        egui::pos2(left, head.center().y + 2.0),
        second,
        palette.secondary,
    );
    ui.separator();

    let mut chosen = None;
    let unread = state.release_notes_unread();
    let pages = [
        (Icon::Clock, "Recently played", Page::History, false),
        (Icon::AudioLines, "Your listening", Page::Stats, false),
        (Icon::Users, "Listen Together", Page::Together, false),
        (Icon::Info, "What's new", Page::Changelog, unread),
        (Icon::Settings, "Settings", Page::Settings, false),
    ];
    for (icon, label, page, marked) in pages {
        if menu_item(state, ui, icon, label, marked) {
            chosen = Some(Action::Open(page));
        }
    }
    ui.separator();
    let (icon, label, action) = match &state.account {
        Some(_) => (Icon::LogOut, "Sign out", Action::SignOut),
        None => (Icon::User, "Sign in", Action::SignIn),
    };
    if menu_item(state, ui, icon, label, false) {
        chosen = Some(action);
    }
    chosen
}

/// A row of the account's menu: an icon, a label, and a dot when there is
/// something new behind it. Returns whether it was clicked.
fn menu_item(state: &State, ui: &mut Ui, icon: Icon, label: &str, marked: bool) -> bool {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), 36.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, label);
    widgets::row_hover(ui, &response, rect);
    let lift = widgets::hover(ui, &response);
    let color = crate::tint::blend(palette.secondary, palette.text, lift);
    let at = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 20.0, rect.center().y),
        Vec2::splat(17.0),
    );
    widgets::paint_icon(ui, icon, at, 17.0, color);
    let anchor = egui::Align2::LEFT_CENTER;
    let text = egui::pos2(rect.left() + 42.0, rect.center().y);
    widgets::text_at(ui, text, anchor, label, theme::medium(13.5), palette.text);
    if marked {
        let dot = egui::pos2(rect.right() - 14.0, rect.center().y);
        ui.painter().circle_filled(dot, 4.0, palette.accent);
    }
    response.clicked()
}
