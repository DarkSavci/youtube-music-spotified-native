//! The Listen Together page's own controls: its panels, its three kinds of
//! button, a tick box, a drop-down, and a listener's picture.

use eframe::egui::{
    self, Align2, CornerRadius, Frame, Margin, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use spotified_client::models::{Artwork, artwork_url};

use super::super::widgets;
use crate::state::State;
use crate::theme::{self, Icon, Palette};

pub const PANEL_RADIUS: u8 = 16;
pub const PANEL_PADDING: i8 = 24;
pub const AVATAR: f32 = 38.0;
pub const AVATAR_SMALL: f32 = 24.0;

pub fn quiet(state: &State, ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::regular(13.0))
            .color(state.palette.secondary),
    );
}

/// The smallest print: a caption under or beside a control.
pub fn small(state: &State, ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::regular(11.0))
            .color(state.palette.secondary),
    );
}

/// The line above a heading: small, bold, widely spaced, in the accent.
pub fn eyebrow(state: &State, ui: &mut Ui, text: &str) {
    let color = state.palette.accent;
    let width = ui.available_width();
    let galley = widgets::tracked(ui, text, theme::bold(10.5), color, 1.8, (width, 1));
    let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
    read_out(&response, text);
    ui.painter().galley(rect.min, galley, color);
}

/// Gives words that were painted by hand to screen readers, and to the
/// tests that read the page the same way.
pub fn read_out(response: &Response, text: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
}

pub fn heading(ui: &mut Ui, text: &str, size: f32) {
    ui.add(egui::Label::new(egui::RichText::new(text).font(theme::bold(size))).wrap());
}

/// An outlined panel, as wide as there is room for.
pub fn panel(state: &State, ui: &mut Ui, contents: impl FnOnce(&mut Ui)) -> Rect {
    panel_filled(state, ui, widgets::wash(ui, 0.12), contents)
}

pub fn panel_filled(
    state: &State,
    ui: &mut Ui,
    fill: egui::Color32,
    contents: impl FnOnce(&mut Ui),
) -> Rect {
    Frame::new()
        .fill(fill)
        .stroke((1.0, state.palette.outline))
        .corner_radius(PANEL_RADIUS)
        .inner_margin(Margin::same(PANEL_PADDING))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui);
        })
        .response
        .rect
}

/// The square with rounded corners that starts a panel: its subject.
pub fn feature_icon(state: &State, ui: &mut Ui, icon: Icon) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
    let accent = state.palette.accent;
    ui.painter()
        .rect_filled(rect, 14, accent.gamma_multiply(0.12));
    widgets::paint_icon(ui, icon, rect, 24.0, accent);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Filled: what the panel is for.
    Primary,
    /// Outlined: another choice.
    Secondary,
    /// Outlined in the colour of harm: what cannot be taken back.
    Danger,
}

/// One of the page's pills, with an icon before its label if it has one.
pub fn button(
    ui: &mut Ui,
    palette: &Palette,
    kind: Kind,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    button_named(ui, palette, kind, icon, (label, label))
}

/// As [`button`], called something longer than it reads: one of many with
/// the same words, told apart by what each is about.
pub fn button_named(
    ui: &mut Ui,
    palette: &Palette,
    kind: Kind,
    icon: Option<Icon>,
    (label, named): (&str, &str),
) -> Response {
    let ink = match kind {
        Kind::Primary => palette.on_accent,
        Kind::Secondary => palette.text,
        Kind::Danger => palette.danger,
    };
    let ink = if ui.is_enabled() {
        ink
    } else {
        ink.gamma_multiply(0.45)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::semibold(13.0), ink);
    let icon_room = if icon.is_some() { 24.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_room + 40.0, 40.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, named);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let lift = widgets::hover(ui, &response);
    let round = CornerRadius::same(u8::MAX);
    match kind {
        Kind::Primary => {
            let fill = crate::tint::blend(palette.accent, palette.accent_hover, lift);
            let fill = if ui.is_enabled() {
                fill
            } else {
                fill.gamma_multiply(0.4)
            };
            ui.painter().rect_filled(rect, round, fill);
        }
        Kind::Secondary | Kind::Danger => {
            ui.painter()
                .rect_filled(rect, round, widgets::wash(ui, 0.3 + lift * 0.5));
            let outline = match kind {
                Kind::Danger => palette.danger.gamma_multiply(0.35),
                _ => crate::tint::blend(palette.dim, palette.text, lift),
            };
            let stroke = Stroke::new(1.0, outline);
            ui.painter()
                .rect_stroke(rect, round, stroke, egui::StrokeKind::Inside);
        }
    }
    let mut left = rect.left() + 20.0;
    if let Some(icon) = icon {
        let at = Rect::from_center_size(pos2(left + 8.0, rect.center().y), Vec2::splat(16.0));
        widgets::paint_icon(ui, icon, at, 16.0, ink);
        left += icon_room;
    }
    let at = pos2(left, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, ink);
    response
}

/// A button that is only its words: quiet until the pointer is on it.
pub fn text_button(ui: &mut Ui, palette: &Palette, icon: Option<Icon>, label: &str) -> Response {
    words(ui, palette, icon, (label, label))
}

/// As [`text_button`], called something longer than it reads.
pub fn text_button_named(ui: &mut Ui, palette: &Palette, label: (&str, &str)) -> Response {
    words(ui, palette, None, label)
}

fn words(
    ui: &mut Ui,
    palette: &Palette,
    icon: Option<Icon>,
    (label, named): (&str, &str),
) -> Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), theme::semibold(12.0), palette.secondary);
    let icon_room = if icon.is_some() { 22.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_room + 16.0, 32.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, named);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let lift = widgets::hover(ui, &response);
    let ink = if ui.is_enabled() {
        crate::tint::blend(palette.secondary, palette.text, lift)
    } else {
        palette.dim
    };
    let mut left = rect.left() + 8.0;
    if let Some(icon) = icon {
        let at = Rect::from_center_size(pos2(left + 8.0, rect.center().y), Vec2::splat(16.0));
        widgets::paint_icon(ui, icon, at, 16.0, ink);
        left += icon_room;
    }
    let at = pos2(left, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, ink);
    response
}

/// A tick box with its label. Returns whether it was clicked.
pub fn check(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> bool {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), theme::regular(12.0), palette.secondary);
    let size = vec2(14.0 + 9.0 + galley.size().x, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label)
    });
    if ui.is_rect_visible(rect) {
        let lift = widgets::hover(ui, &response);
        let square =
            Rect::from_center_size(pos2(rect.left() + 7.0, rect.center().y), Vec2::splat(14.0));
        if on {
            ui.painter().rect_filled(square, 3, palette.accent);
            let tick = [
                square.left_top() + vec2(3.2, 7.4),
                square.left_top() + vec2(5.9, 10.0),
                square.left_top() + vec2(10.8, 4.4),
            ];
            let stroke = Stroke::new(1.6, palette.on_accent);
            ui.painter().add(egui::Shape::line(tick.to_vec(), stroke));
        } else {
            let outline = crate::tint::blend(palette.dim, palette.text, lift);
            let stroke = Stroke::new(1.0, outline);
            ui.painter()
                .rect_stroke(square, 3, stroke, egui::StrokeKind::Inside);
        }
        let ink = crate::tint::blend(palette.secondary, palette.text, lift);
        let at = pos2(rect.left() + 23.0, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, ink);
    }
    response.clicked()
}

/// A drop-down, closed: what is chosen, and a chevron. `framed` gives it a
/// field's box; without, it is only its words, as in the server bar. The
/// caller hangs the menu of choices on what this returns.
pub fn select(ui: &mut Ui, palette: &Palette, label: &str, chosen: &str, framed: bool) -> Response {
    let height = if framed { 40.0 } else { 32.0 };
    let width = if framed {
        ui.available_width()
    } else {
        let text =
            ui.painter()
                .layout_no_wrap(chosen.to_owned(), theme::regular(13.0), palette.text);
        (text.size().x + 44.0).min(230.0)
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    widgets::name(ui, &response, label);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let lift = widgets::hover(ui, &response);
    if framed {
        let outline = crate::tint::blend(palette.outline, palette.dim, lift);
        ui.painter().rect(
            rect,
            theme::RADIUS,
            palette.surface,
            (1.0, outline),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter()
            .rect_filled(rect, theme::RADIUS, widgets::wash(ui, lift));
    }
    let ink = if ui.is_enabled() {
        palette.text
    } else {
        palette.dim
    };
    let room = rect.width() - 12.0 - 30.0;
    let text = widgets::elided(ui, chosen, theme::regular(13.0), ink, room, 1);
    let at = pos2(rect.left() + 12.0, rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, ink);
    let chevron = Rect::from_center_size(
        pos2(rect.right() - 17.0, rect.center().y),
        Vec2::splat(16.0),
    );
    widgets::paint_icon(ui, Icon::ChevronDown, chevron, 16.0, ink);
    response
}

/// A caption above a control, as the forms label their fields.
pub fn caption(state: &State, ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::regular(12.0))
            .color(state.palette.text),
    );
    ui.add_space(2.0);
}

/// A listener's picture in a disc, or their initial when they share none
/// or it has yet to load.
pub fn avatar(state: &State, ui: &Ui, rect: Rect, name: &str, picture: &[Artwork]) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let palette = &state.palette;
    let pixels = (rect.width() * ui.pixels_per_point()).ceil() as u32;
    let texture = artwork_url(picture, pixels).and_then(|url| state.images.texture(&url));
    if let Some((id, _)) = texture {
        egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
            .corner_radius(CornerRadius::same(u8::MAX))
            .paint_at(ui, rect);
        return;
    }
    let fill = crate::tint::blend(palette.surface_active, palette.accent, 0.25);
    ui.painter()
        .circle_filled(rect.center(), rect.width() / 2.0, fill);
    let initial: String = name
        .trim()
        .chars()
        .take(1)
        .flat_map(char::to_uppercase)
        .collect();
    let font = theme::semibold((rect.width() * 0.38).max(10.0));
    widgets::text_at(
        ui,
        rect.center(),
        Align2::CENTER_CENTER,
        &initial,
        font,
        palette.text,
    );
}

/// Lays out a row of controls that goes on to the next line when it runs
/// out of room, as the old page's rows of buttons did.
pub fn actions_row(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 8.0);
        contents(ui);
    });
}
