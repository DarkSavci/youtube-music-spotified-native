//! Controls shared by the views.

use std::f32::consts::TAU;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Margin, Rect, Response, Sense, Stroke, Ui, Vec2,
    pos2, vec2,
};

use crate::theme::{self, Icon, Palette};

mod inputs;

pub use inputs::{TextField, slider};

/// Names a hand-drawn control for screen readers, and for the tests that
/// find controls the same way.
pub fn name(ui: &Ui, response: &Response, label: &str) {
    hand(ui, response);
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
}

/// Shows the hand while the pointer is on something that can be clicked,
/// so what is a control and what is only drawn can be told apart.
pub fn hand(ui: &Ui, response: &Response) {
    if ui.is_enabled() && response.sense.senses_click() && response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}

/// How long a hover takes to arrive, and to leave.
const HOVER_TIME: f32 = 0.12;

/// How far the pointer's arrival on a control has got, from 0 to 1. A
/// hover that fades in and out reads as the control answering; one that
/// snaps reads as flicker when the pointer crosses a list.
pub fn hover(ui: &Ui, response: &Response) -> f32 {
    hover_of(ui, response.id, response.hovered())
}

/// As [`hover`], for a control that works out for itself whether the
/// pointer is on it.
pub fn hover_of(ui: &Ui, id: egui::Id, hovered: bool) -> f32 {
    let on = hovered && ui.is_enabled();
    ui.ctx()
        .animate_bool_with_time(id.with("hover"), on, HOVER_TIME)
}

/// The wash a row or a round button takes under the pointer: up to eight
/// parts in a hundred of white on a dark theme, of black on a light one,
/// which sets it off from any colour it lies on.
pub fn wash(ui: &Ui, amount: f32) -> Color32 {
    let alpha = (amount.clamp(0.0, 1.0) * 20.0).round() as u8;
    if ui.visuals().dark_mode {
        Color32::from_white_alpha(alpha)
    } else {
        Color32::from_black_alpha(alpha)
    }
}

/// Washes a row while the pointer is on it.
pub fn row_hover(ui: &Ui, response: &Response, rect: Rect) {
    let amount = hover(ui, response);
    if amount > 0.0 {
        ui.painter()
            .rect_filled(rect, theme::RADIUS_ROW, wash(ui, amount));
    }
}

/// Paints an icon centred in `rect` without allocating space.
pub fn paint_icon(ui: &Ui, icon: Icon, rect: Rect, size: f32, color: Color32) {
    let center = rect.center() + icon.optical_offset(size);
    icon.image(color, size)
        .paint_at(ui, Rect::from_center_size(center, Vec2::splat(size)));
}

/// A frameless icon control whose colour lifts on hover. The hit area is the
/// icon plus six points each side.
pub fn icon_button(
    ui: &mut Ui,
    palette: &Palette,
    icon: Icon,
    size: f32,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 12.0), Sense::click());
    name(ui, &response, tooltip);
    paint_icon_button(ui, palette, icon, size, rect, &response);
    response.on_hover_text(tooltip)
}

/// An icon control placed where the caller has worked out, for layouts
/// laid out by hand.
pub struct IconButton<'a> {
    pub icon: Icon,
    pub size: f32,
    pub tooltip: &'a str,
    /// Drawn in the accent: a mode that is switched on.
    pub active: bool,
}

impl IconButton<'_> {
    pub fn show_at(&self, ui: &mut Ui, palette: &Palette, center: egui::Pos2) -> Response {
        let rect = Rect::from_center_size(center, Vec2::splat(self.size + 12.0));
        let response = ui.interact(rect, ui.id().with(self.tooltip), Sense::click());
        name(ui, &response, self.tooltip);
        if self.active && ui.is_enabled() {
            let pressed = if response.is_pointer_button_down_on() {
                0.92
            } else {
                1.0
            };
            let lift = hover(ui, &response);
            ui.painter()
                .circle_filled(rect.center(), rect.width() / 2.0, wash(ui, lift));
            paint_icon(ui, self.icon, rect, self.size * pressed, palette.accent);
        } else {
            paint_icon_button(ui, palette, self.icon, self.size, rect, &response);
        }
        response.on_hover_text(self.tooltip)
    }
}

fn paint_icon_button(
    ui: &Ui,
    palette: &Palette,
    icon: Icon,
    size: f32,
    rect: Rect,
    response: &Response,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    // Under the pointer the icon brightens on a round wash, as the old
    // app's buttons did.
    let lift = hover(ui, response);
    let color = if !ui.is_enabled() {
        palette.dim
    } else if response.has_focus() {
        palette.text
    } else {
        crate::tint::blend(palette.secondary, palette.text, lift)
    };
    let pressed = if response.is_pointer_button_down_on() {
        0.92
    } else {
        1.0
    };
    ui.painter()
        .circle_filled(rect.center(), rect.width() / 2.0, wash(ui, lift));
    paint_icon(ui, icon, rect, size * pressed, color);
}

/// A pill that filters a list. The active one is filled with the text colour.
pub fn chip(ui: &mut Ui, palette: &Palette, label: &str, active: bool) -> Response {
    let padding = vec2(12.0, 7.0);
    let text_color = if active { palette.window } else { palette.text };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(13.0), text_color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + padding * 2.0, Sense::click());
    hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
    });
    if ui.is_rect_visible(rect) {
        let fill = if active {
            palette.text
        } else {
            let lift = hover(ui, &response);
            crate::tint::blend(palette.surface, palette.surface_active, lift)
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
        ui.painter().galley(rect.min + padding, galley, text_color);
    }
    response
}

/// The one filled button on a surface: what the person most likely wants.
pub fn pill_button(ui: &mut Ui, palette: &Palette, label: &str) -> Response {
    let padding = vec2(18.0, 8.0);
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), theme::semibold(13.0), palette.on_accent);
    let (rect, response) = ui.allocate_exact_size(galley.size() + padding * 2.0, Sense::click());
    name(ui, &response, label);
    if ui.is_rect_visible(rect) {
        // It brightens and swells a little under the pointer, and gives
        // when pressed.
        let lift = hover(ui, &response);
        let fill = crate::tint::blend(palette.accent, palette.accent_hover, lift);
        let pressed = response.is_pointer_button_down_on();
        let grown = rect.expand(if pressed { -1.0 } else { lift * 1.5 });
        ui.painter()
            .rect_filled(grown, CornerRadius::same(u8::MAX), fill);
        ui.painter()
            .galley(rect.min + padding, galley, palette.on_accent);
    }
    response
}

/// The quieter button: an outline, for a choice that is not the main one.
pub fn outline_button(ui: &mut Ui, palette: &Palette, label: &str) -> Response {
    let padding = vec2(18.0, 8.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::semibold(13.0), palette.text);
    let (rect, response) = ui.allocate_exact_size(galley.size() + padding * 2.0, Sense::click());
    name(ui, &response, label);
    if ui.is_rect_visible(rect) {
        let lift = hover(ui, &response);
        let outline = crate::tint::blend(palette.dim, palette.text, lift);
        ui.painter()
            .rect_filled(rect, CornerRadius::same(u8::MAX), wash(ui, lift * 0.6));
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(u8::MAX),
            Stroke::new(1.0, outline),
            egui::StrokeKind::Inside,
        );
        ui.painter()
            .galley(rect.min + padding, galley, palette.text);
    }
    response
}

/// An on/off switch: a pill with a knob at one end.
pub fn switch(ui: &mut Ui, palette: &Palette, on: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
    hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label)
    });
    if ui.is_rect_visible(rect) {
        // The knob slides rather than jumps.
        let travel = ui.ctx().animate_bool(response.id, on);
        let fill = if on {
            palette.accent
        } else {
            palette.surface_active
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
        let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), travel);
        ui.painter()
            .circle_filled(pos2(x, rect.center().y), 8.0, Color32::WHITE);
    }
    response
}

/// An arc that turns: the only loading indicator. There are no skeletons.
pub fn spinner(ui: &mut Ui, palette: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    const TURNS_PER_SECOND: f64 = 1.2;
    const SWEEP: f32 = 250.0 / 360.0 * TAU;
    const SEGMENTS: usize = 24;
    let start = (ui.input(|input| input.time) * TURNS_PER_SECOND).fract() as f32 * TAU;
    let radius = size / 2.0 - 1.0;
    let points = (0..=SEGMENTS)
        .map(|step| {
            let angle = start + SWEEP * step as f32 / SEGMENTS as f32;
            rect.center() + radius * vec2(angle.cos(), angle.sin())
        })
        .collect();
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(2.0, palette.accent)));
    ui.ctx().request_repaint();
}

pub fn loading(ui: &mut Ui, palette: &Palette, text: &str) {
    ui.horizontal(|ui| {
        spinner(ui, palette, 18.0);
        ui.label(
            egui::RichText::new(text)
                .font(theme::regular(13.0))
                .color(palette.secondary),
        );
    });
}

pub fn error(ui: &mut Ui, palette: &Palette, message: &str) {
    ui.horizontal(|ui| {
        ui.add(Icon::CircleAlert.image(palette.danger, 16.0));
        ui.label(egui::RichText::new(message).font(theme::regular(13.0)));
    });
}

/// What a list shows when it has nothing: an icon, a title and a line.
pub fn empty_state(ui: &mut Ui, palette: &Palette, icon: Icon, title: &str, body: &str) {
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.add(icon.image(palette.dim, 40.0));
        ui.add_space(8.0);
        ui.label(egui::RichText::new(title).font(theme::semibold(16.0)));
        ui.label(
            egui::RichText::new(body)
                .font(theme::regular(13.5))
                .color(palette.secondary),
        );
    });
}

/// Text painted at a point, for layouts that place things by hand.
pub fn text_at(
    ui: &Ui,
    pos: egui::Pos2,
    anchor: Align2,
    text: &str,
    font: egui::FontId,
    color: Color32,
) -> Rect {
    ui.painter().text(pos, anchor, text, font, color)
}

/// Lays text out to fit `width`, ending in an ellipsis after `rows` lines.
pub fn elided(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    width: f32,
    rows: usize,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, width);
    job.wrap.max_rows = rows;
    // A single line is cut where it runs out; several lines break at words.
    job.wrap.break_anywhere = rows == 1;
    ui.painter().layout_job(job)
}

/// A line of text that leads somewhere: underlined, with the hand cursor,
/// while the pointer is on it. The only place the hand appears.
pub struct Link<'a> {
    pub text: &'a str,
    pub font: egui::FontId,
    pub color: Color32,
    /// The room it has; longer text ends in an ellipsis.
    pub width: f32,
}

impl Link<'_> {
    /// Paints the text with its top left at `at` and returns whether it
    /// was clicked. With `leads_somewhere` false it is plain text.
    pub fn show(&self, ui: &Ui, id: egui::Id, at: egui::Pos2, leads_somewhere: bool) -> bool {
        self.show_measured(ui, id, at, leads_somewhere).0
    }

    /// As [`Link::show`], and how wide the text came out, for a caller
    /// that places something after it.
    pub fn show_measured(
        &self,
        ui: &Ui,
        id: egui::Id,
        at: egui::Pos2,
        leads_somewhere: bool,
    ) -> (bool, f32) {
        let galley = elided(ui, self.text, self.font.clone(), self.color, self.width, 1);
        let rect = Rect::from_min_size(at, galley.size());
        let mut clicked = false;
        if leads_somewhere && !self.text.is_empty() {
            let response = ui.interact(rect, id, Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter()
                    .hline(rect.x_range(), rect.bottom() - 1.0, (1.0, self.color));
            }
            clicked = response.clicked();
        }
        ui.painter().galley(at, galley, self.color);
        (clicked, rect.width())
    }
}

/// The shape artwork is cut to.
#[derive(Clone, Copy)]
pub enum ArtShape {
    Rounded(u8),
    /// Artists are shown round.
    Circle,
}

/// Paints artwork into `rect`, cropped to fill it, or a placeholder until it
/// has loaded. Asks for the image at the size it is drawn.
pub fn artwork(
    ui: &Ui,
    state: &crate::state::State,
    art: &[spotified_client::models::Artwork],
    rect: Rect,
    shape: ArtShape,
    placeholder: Icon,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let radius = match shape {
        ArtShape::Rounded(radius) => CornerRadius::same(radius),
        ArtShape::Circle => CornerRadius::same(u8::MAX),
    };
    let texture = artwork_address(ui, art, rect.width()).and_then(|url| state.images.texture(&url));
    let Some((id, size)) = texture else {
        let palette = &state.palette;
        ui.painter()
            .rect_filled(rect, radius, palette.surface_hover);
        let icon_size = (rect.width() * 0.42).clamp(12.0, 64.0);
        paint_icon(ui, placeholder, rect, icon_size, palette.dim);
        return;
    };
    egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
        .uv(centre_crop(size, rect.size()))
        .corner_radius(radius)
        .paint_at(ui, rect);
}

/// The address artwork is asked for at when drawn `width` points wide.
fn artwork_address(
    ui: &Ui,
    art: &[spotified_client::models::Artwork],
    width: f32,
) -> Option<String> {
    let pixels = (width * ui.pixels_per_point()).ceil() as u32;
    spotified_client::models::artwork_url(art, pixels)
}

/// How far a picture on a browse tile is tipped over.
const TIPPED: f32 = 22.0 * std::f32::consts::PI / 180.0;

/// Paints artwork tipped over about its middle, as on a browse tile, and
/// cut off at `within`, which it leans out of. Nothing is drawn until it
/// has loaded: the tile's colour is enough.
pub fn artwork_tipped(
    ui: &mut Ui,
    state: &crate::state::State,
    art: &[spotified_client::models::Artwork],
    rect: Rect,
    within: Rect,
) {
    let texture = artwork_address(ui, art, rect.width()).and_then(|url| state.images.texture(&url));
    let Some((id, size)) = texture else {
        return;
    };
    let image = egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
        .uv(centre_crop(size, rect.size()))
        .rotate(TIPPED, Vec2::splat(0.5));
    let mut clipped = ui.new_child(egui::UiBuilder::new().max_rect(within));
    clipped.set_clip_rect(within.intersect(ui.clip_rect()));
    // Its shadow first, a little below and behind.
    let shadow = image.clone().tint(Color32::from_black_alpha(70));
    shadow.paint_at(&clipped, rect.translate(vec2(0.0, 5.0)).expand(2.0));
    image.paint_at(&clipped, rect);
}

/// The tint of artwork that is drawn `width` points wide somewhere on
/// screen, once it has loaded.
pub fn artwork_tint(
    ui: &Ui,
    state: &crate::state::State,
    art: &[spotified_client::models::Artwork],
    width: f32,
) -> Option<Color32> {
    artwork_address(ui, art, width).and_then(|url| state.images.tint(&url))
}

/// A panel's frame: a rounded card, with the window showing in the gutters
/// around it.
pub fn card(palette: &Palette, gutters: Margin) -> Frame {
    Frame::new()
        .fill(palette.panel)
        .corner_radius(theme::RADIUS)
        .outer_margin(gutters)
}

/// Hides what was painted past a card's rounded corners: a gradient or a
/// row of bars is square, and would poke out of them. A band of the
/// window's colour is drawn around the card, where the gutter is.
pub fn round_off(ui: &Ui, card: Rect, palette: &Palette) {
    let band = f32::from(theme::HALF_GUTTER);
    ui.painter().with_clip_rect(card.expand(band)).rect_stroke(
        card,
        theme::RADIUS,
        (band, palette.window),
        egui::StrokeKind::Outside,
    );
}

/// Fills `rect` with a colour that changes evenly from its top to its
/// bottom.
pub fn vertical_gradient(ui: &Ui, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// The part of an image to show so it fills a box of another shape without
/// stretching: a video thumbnail is wide, and its box is square.
fn centre_crop(image: Vec2, target: Vec2) -> Rect {
    let image_aspect = image.x / image.y.max(1.0);
    let target_aspect = target.x / target.y.max(1.0);
    let (width, height) = if image_aspect > target_aspect {
        (target_aspect / image_aspect, 1.0)
    } else {
        (1.0, image_aspect / target_aspect)
    };
    Rect::from_center_size(pos2(0.5, 0.5), vec2(width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_image_loses_its_sides_in_a_square_box() {
        let uv = centre_crop(vec2(1280.0, 720.0), vec2(148.0, 148.0));
        assert!((uv.width() - 0.5625).abs() < 1e-4);
        assert_eq!(uv.height(), 1.0);
        assert!((uv.center().x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_square_image_is_shown_whole_in_a_square_box() {
        let uv = centre_crop(vec2(226.0, 226.0), vec2(148.0, 148.0));
        assert_eq!(uv, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
    }
}
