//! The bar across the top of the window: history, Home and search in the
//! middle, and at the right what is new, Listen Together, settings and the
//! account, then on Windows the window's own buttons. It is the title bar:
//! dragging it moves the window.

use eframe::egui::{
    self, Align, CornerRadius, Frame, Layout, Margin, Rect, Sense, TextEdit, Ui, Vec2, pos2, vec2,
};

use super::{chrome, widgets};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon, Palette};

mod account;

/// A back or forward button: taller than wide, as in the Electron app.
const HISTORY_BUTTON: Vec2 = vec2(36.0, 40.0);
const HOME_BUTTON: f32 = 44.0;
const SEARCH_HEIGHT: f32 = 40.0;
/// The room the Browse all button takes at the right of the search field.
const BROWSE_BUTTON: f32 = 48.0;
/// The gap between the bar's groups, and between the things in them.
const GAP: f32 = 12.0;
/// Home and the search field together are at most this wide, and the
/// field alone never narrower than the least.
const SEARCH_GROUP_MOST: f32 = 560.0;
const SEARCH_FIELD_LEAST: f32 = 120.0;

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
                    .max_rect(controls.shrink2(vec2(GAP, 0.0)))
                    .layout(Layout::left_to_right(Align::Center)),
            );
            self::controls(state, &mut controls_ui, actions);
        });
}

/// History at the left, the account and its neighbours at the right, and
/// Home with the search field in what is between, in the middle of the
/// window when there is room either side.
fn controls(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let bar = ui.max_rect();
    ui.spacing_mut().item_spacing.x = 8.0;
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
    let start = ui.cursor().left() + GAP - 8.0;

    // The right-hand group is laid out first, from the right, so the
    // search field can be given exactly what it leaves.
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bar)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right.spacing_mut().item_spacing.x = GAP;
    right_controls(state, &mut right, actions);
    let end = right.min_rect().left() - GAP;

    let width = (end - start).clamp(HOME_BUTTON + GAP + SEARCH_FIELD_LEAST, SEARCH_GROUP_MOST);
    let centred = ui.ctx().content_rect().center().x - width / 2.0;
    let left = centred.min(end - width).max(start);
    let group = Rect::from_min_size(pos2(left, bar.top()), vec2(width, bar.height()));
    let mut middle = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(group)
            .layout(Layout::left_to_right(Align::Center)),
    );
    middle.spacing_mut().item_spacing.x = GAP;
    if home_button(state, &mut middle) {
        actions.push(Action::Open(Page::Home));
    }
    search_field(state, &mut middle, actions, width - HOME_BUTTON - GAP);
}

/// What sits at the right of the bar, outermost first.
fn right_controls(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    account::show(state, ui, actions);
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
    if widgets::icon_button(ui, palette, Icon::Settings, 20.0, "Settings").clicked() {
        actions.push(Action::Open(Page::Settings));
    }
    // Lit while in a room, so the way back to it is easy to find.
    let together = widgets::IconButton {
        icon: Icon::Headphones,
        size: 20.0,
        tooltip: "Listen Together",
        active: state.together.in_room(),
    };
    let at = ui.allocate_space(Vec2::splat(32.0)).1.center();
    if together.show_at(ui, palette, at).clicked() {
        actions.push(Action::Open(Page::Together));
    }
    let news = widgets::icon_button(ui, palette, Icon::Gift, 20.0, "What's new");
    // A dot says there are release notes not yet opened.
    if state.release_notes_unread() {
        let at = news.rect.right_top() + vec2(-6.0, 6.0);
        ui.painter().circle_filled(at, 3.0, palette.accent);
    }
    // The newest notes open over the page; all of them have a page.
    if news.clicked() {
        actions.push(Action::ShowWhatsNew);
    }
}

/// A back or forward chevron. Returns whether it was clicked.
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
    let (rect, response) = ui.allocate_exact_size(HISTORY_BUTTON, sense);
    widgets::name(ui, &response, tooltip);
    let lift = widgets::hover(ui, &response);
    let color = if enabled {
        crate::tint::blend(palette.secondary, palette.text, lift)
    } else {
        palette.dim
    };
    let pressed = if response.is_pointer_button_down_on() {
        0.94
    } else {
        1.0
    };
    widgets::paint_icon(ui, icon, rect, 30.0 * pressed, color);
    response.on_hover_text(tooltip).clicked()
}

/// The round way home, beside the search field. Returns whether it was
/// clicked.
fn home_button(state: &State, ui: &mut Ui) -> bool {
    let palette = &state.palette;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(HOME_BUTTON), Sense::click());
    widgets::name(ui, &response, "Home");
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.surface, palette.surface_active, lift);
    let pressed = if response.is_pointer_button_down_on() {
        0.96
    } else {
        1.0
    };
    ui.painter()
        .circle_filled(rect.center(), HOME_BUTTON / 2.0 * pressed, fill);
    // Lit while Home is the page on screen.
    let at_home = state.nav.page() == &Page::Home && !state.library_expanded;
    let color = if at_home {
        palette.text
    } else {
        crate::tint::blend(palette.secondary, palette.text, lift)
    };
    widgets::paint_icon(ui, Icon::House, rect, 22.0, color);
    response.on_hover_text("Home").clicked()
}

fn search_field(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, width: f32) {
    let palette = &state.palette;
    let (rect, _) = ui.allocate_exact_size(vec2(width, SEARCH_HEIGHT), Sense::hover());
    let id = ui.id().with("search");
    // Asked for from the keyboard: the caret goes to the field once for
    // each time of asking.
    let asked = ui.data(|data| data.get_temp::<u64>(id)).unwrap_or(0);
    if asked != state.search_focus {
        ui.data_mut(|data| data.insert_temp(id, state.search_focus));
        ui.memory_mut(|memory| memory.request_focus(id));
    }
    let focused = ui.memory(|memory| memory.has_focus(id));
    let round = CornerRadius::same(u8::MAX);
    ui.painter().rect_filled(rect, round, palette.surface);
    // The whole pill shows the focus, not the text inside it.
    if focused {
        let kind = egui::StrokeKind::Inside;
        ui.painter()
            .rect_stroke(rect, round, (1.0, palette.text), kind);
    }

    // The magnifier leads to the search page and puts the caret in the
    // field.
    let glass = widgets::IconButton {
        icon: Icon::Search,
        size: 20.0,
        tooltip: "Search",
        active: false,
    };
    let glass_at = pos2(rect.left() + 16.0 + 16.0, rect.center().y);
    if glass.show_at(ui, palette, glass_at).clicked() {
        ui.memory_mut(|memory| memory.request_focus(id));
        if state.nav.page() != &Page::Search {
            actions.push(Action::Open(Page::Search));
        }
    }

    // Browsing is the other way to find something, so its button shares
    // the field, behind a hairline.
    let browse = Rect::from_min_max(pos2(rect.right() - BROWSE_BUTTON, rect.top()), rect.max);
    ui.painter().vline(
        browse.left(),
        rect.y_range().shrink(6.0),
        (1.0, palette.outline),
    );
    let button = widgets::IconButton {
        icon: Icon::Archive,
        size: 20.0,
        tooltip: "Browse all",
        active: false,
    };
    if button.show_at(ui, palette, browse.center()).clicked() {
        actions.push(Action::BrowseAll);
    }

    let text = Rect::from_min_max(
        pos2(glass_at.x + 16.0 + GAP, rect.top()),
        pos2(browse.left() - 8.0, rect.bottom()),
    );
    let mut field = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text)
            .layout(Layout::left_to_right(Align::Center)),
    );
    // The view may not change state, so it edits a copy and asks.
    let mut query = state.search.query.clone();
    let hint = egui::RichText::new("What do you want to listen to?").color(palette.dim);
    let edit = TextEdit::singleline(&mut query)
        .id(id)
        .hint_text(hint)
        .frame(Frame::NONE)
        .desired_width(text.width());
    let response = field.add(edit);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Search music")
    });
    if response.changed() {
        actions.push(Action::SetSearchQuery(query));
    }
}
