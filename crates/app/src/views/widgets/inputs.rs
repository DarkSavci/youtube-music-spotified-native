//! The controls that take input by hand: a field to type in, and the thin
//! bar that seeks and sets the volume.

use eframe::egui::{self, Frame, Margin, Rect, Sense, Ui, pos2, vec2};

use super::{hand, hover_of};
use crate::theme::{self, Icon, Palette};

/// A single line of text to type into, in the app's look: a filled, rounded
/// box that outlines itself while it has the caret.
pub struct TextField<'a> {
    pub text: &'a str,
    /// Shown, dimmed, while the field is empty.
    pub hint: &'a str,
    /// What a screen reader, and a test, calls the field.
    pub label: &'a str,
    pub icon: Option<Icon>,
    pub width: f32,
    /// Smaller, with smaller text: the library's own search field.
    pub compact: bool,
}

impl TextField<'_> {
    const HEIGHT: f32 = 36.0;
    const COMPACT_HEIGHT: f32 = 32.0;

    /// Draws the field. The view may not change state, so the field edits
    /// a copy: what was typed is returned on the frame it changes.
    pub fn show(&self, ui: &mut Ui, palette: &Palette) -> Option<String> {
        let id = ui.id().with(("text-field", self.label));
        let focused = ui.memory(|memory| memory.has_focus(id));
        // The compact field has no outline until the caret is in it.
        let outline = match (focused, self.compact) {
            (true, true) => palette.text,
            (true, false) => palette.secondary,
            (false, true) => palette.surface,
            (false, false) => palette.outline,
        };
        let mut edited = self.text.to_owned();
        let mut changed = false;
        let (height, radius, text) = if self.compact {
            (Self::COMPACT_HEIGHT, 5, 12.0)
        } else {
            (Self::HEIGHT, theme::RADIUS, 14.0)
        };
        Frame::new()
            .fill(palette.surface)
            .stroke((1.0, outline))
            .corner_radius(radius)
            .inner_margin(Margin::symmetric(10, 0))
            .show(ui, |ui| {
                ui.set_height(height);
                ui.set_width((self.width - 22.0).max(40.0));
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some(icon) = self.icon {
                        ui.add(icon.image(palette.secondary, 15.0));
                    }
                    let edit = egui::TextEdit::singleline(&mut edited)
                        .id(id)
                        .hint_text(egui::RichText::new(self.hint).font(theme::regular(text)))
                        .font(theme::regular(text))
                        .frame(Frame::NONE)
                        .desired_width(ui.available_width());
                    let response = ui.add(edit);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, self.label)
                    });
                    changed = response.changed();
                });
            });
        changed.then_some(edited)
    }
}

/// What a slider reports this frame, as fractions of its length.
pub struct SliderResponse {
    /// Where the knob is while it is held.
    pub dragging: Option<f32>,
    /// Where it was let go, on the frame that happens.
    pub released: Option<f32>,
}

/// A thin bar with a filled part, for seeking and volume. The fill turns to
/// the accent and a knob appears only under the pointer, as in Spotify.
pub fn slider(
    ui: &mut Ui,
    palette: &Palette,
    rect: Rect,
    fraction: f32,
    name: &str,
) -> SliderResponse {
    let id = ui.id().with(name);
    let sense = if ui.is_enabled() {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    hand(ui, &response);
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(fraction), name));
    let held = response.dragged() || response.is_pointer_button_down_on();
    let under_pointer = response
        .interact_pointer_pos()
        .map(|pointer| ((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0));
    // The pointer's place is gone by the frame the button is let go, so the
    // last place it was held is remembered until then.
    if let Some(at) = under_pointer.filter(|_| held) {
        ui.data_mut(|data| data.insert_temp(id, at));
    }
    let remembered = ui.data(|data| data.get_temp::<f32>(id));
    let dragging = remembered.filter(|_| held);
    let released = if response.drag_stopped() || response.clicked() {
        ui.data_mut(|data| data.remove::<f32>(id));
        under_pointer.or(remembered)
    } else {
        None
    };

    let shown = dragging.unwrap_or(fraction).clamp(0.0, 1.0);
    // The fill takes the accent and the knob grows out of it as the pointer
    // arrives; while held it stays so wherever the pointer goes.
    let lit = hover_of(ui, id, held || response.hovered());
    let track = Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0));
    // The unfilled part must show on a light panel as well as a dark one.
    ui.painter().rect_filled(track, 2.0, palette.surface_active);
    let end = track.left() + track.width() * shown;
    let filled = Rect::from_min_max(track.min, pos2(end, track.bottom()));
    let fill = if ui.is_enabled() {
        crate::tint::blend(palette.text, palette.accent, lit)
    } else {
        palette.dim
    };
    ui.painter().rect_filled(filled, 2.0, fill);
    if lit > 0.0 {
        ui.painter()
            .circle_filled(pos2(end, track.center().y), 6.0 * lit, palette.text);
    }
    SliderResponse { dragging, released }
}
