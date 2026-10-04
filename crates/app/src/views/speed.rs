//! The playback speed button, and the panel it opens.
//!
//! The button shows the speed as its label (the number is the state) and
//! takes the accent off 1×, so a sped-up player is never mistaken for a
//! normal one. The panel follows YouTube's own: the speed large, a slider
//! with a step down and up either side for anything in between, and presets
//! below. In Listen Together the room holds everyone at 1×, and the button
//! says so instead of opening.

use eframe::egui::{
    self, Align2, CornerRadius, Frame, Margin, Pos2, Rect, RectAlign, Sense, Ui, vec2,
};

use super::widgets;
use crate::actions::Action;
use crate::settings::{SPEED_RANGE, SPEED_STEP, clamp_speed};
use crate::state::State;
use crate::theme::{self, Palette};

/// One tap away, as YouTube offers them. Slower is on the slider.
const PRESETS: [f32; 6] = [1.0, 1.25, 1.5, 1.75, 2.0, 3.0];
const PANEL_WIDTH: f32 = 360.0;
/// The one-row panel's width, and the window height it is used below.
const COMPACT_WIDTH: f32 = 480.0;
const COMPACT_BELOW: f32 = 260.0;
const BUTTON_HEIGHT: f32 = 28.0;
const BUTTON_LEAST: f32 = 32.0;

/// "1×", "1.25×": the button's label.
pub fn label(speed: f32) -> String {
    format!("{}×", (speed * 100.0).round() / 100.0)
}

/// A preset as its pill names it: "1.0", "1.25".
fn preset_label(speed: f32) -> String {
    if speed.fract() == 0.0 {
        format!("{speed:.1}")
    } else {
        format!("{speed}")
    }
}

/// The button, centred on `at`, with its panel when it is open.
pub fn button(state: &State, palette: &Palette, ui: &mut Ui, actions: &mut Vec<Action>, at: Pos2) {
    let speed = state.speed();
    let pinned = state.speed_pinned();
    let text = label(speed);
    let color = if speed == 1.0 {
        palette.secondary
    } else {
        palette.accent
    };
    let font = theme::semibold(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.clone(), font.clone(), color);
    let width = (galley.size().x + 16.0).max(BUTTON_LEAST);
    let rect = Rect::from_center_size(at, vec2(width, BUTTON_HEIGHT));
    let response = ui.interact(rect, ui.id().with("speed"), Sense::click());
    let name = if pinned {
        "Playback speed: 1× in Listen Together".to_owned()
    } else {
        format!("Playback speed: {text}")
    };
    widgets::name(ui, &response, &name);
    let lift = widgets::hover(ui, &response);
    ui.painter()
        .rect_filled(rect, CornerRadius::same(u8::MAX), widgets::wash(ui, lift));
    let color = if speed == 1.0 {
        crate::tint::blend(palette.secondary, palette.text, lift)
    } else {
        color
    };
    // Held at 1× by a room, it is drawn as a control that will not answer.
    let color = if pinned {
        color.gamma_multiply(0.5)
    } else {
        color
    };
    widgets::text_at(ui, at, Align2::CENTER_CENTER, &text, font, color);
    let response = response.on_hover_text(name);
    if pinned {
        // Asking for a speed in a room is what says why there is none.
        if response.clicked() {
            actions.push(Action::SetSpeed(speed));
        }
        return;
    }
    // A window too short for the whole panel, the mini player as a strip,
    // gets the presets alone with a step either side: the part used most.
    let window = ui.ctx().content_rect().size();
    let compact = window.y < COMPACT_BELOW;
    let (most, margin) = if compact {
        (COMPACT_WIDTH, 8)
    } else {
        (PANEL_WIDTH, 16)
    };
    let width = most.min(window.x - 16.0);
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke((1.0, palette.dim))
        .corner_radius(12)
        .inner_margin(Margin::same(margin));
    let inner = width - f32::from(margin) * 2.0;
    // Above the button where there is room, below it where there is not,
    // and over the window itself where there is room for neither.
    egui::Popup::from_toggle_button_response(&response)
        .align(RectAlign::TOP)
        .gap(8.0)
        .frame(frame)
        .width(width)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(inner);
            if compact {
                strip(state, palette, ui, actions);
            } else {
                panel(state, palette, ui, actions);
            }
        });
}

/// A step down or up beside the slider, or beside the presets.
fn step(
    palette: &Palette,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (chosen, towards): (f32, f32),
    at: Pos2,
) {
    let (sign, name) = if towards < 0.0 {
        ("−", "Slower")
    } else {
        ("+", "Faster")
    };
    let to = clamp_speed(chosen + towards * SPEED_STEP);
    ui.add_enabled_ui(to != chosen, |ui| {
        if round_button(ui, palette, at, sign, name).clicked() {
            actions.push(Action::SetSpeed(to));
        }
    });
}

/// The presets, sharing `row` evenly; "Normal" under 1× where `noted`.
fn presets(
    palette: &Palette,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (row, chosen, noted): (Rect, f32, bool),
) {
    let gap = 8.0;
    let width = (row.width() - gap * (PRESETS.len() - 1) as f32) / PRESETS.len() as f32;
    for (index, speed) in PRESETS.into_iter().enumerate() {
        let left = row.left() + index as f32 * (width + gap);
        let pill = Rect::from_min_size(egui::pos2(left, row.top()), vec2(width, 28.0));
        if preset(ui, palette, pill, speed, chosen == speed).clicked() {
            actions.push(Action::SetSpeed(speed));
        }
        if noted && speed == 1.0 {
            let under = egui::pos2(pill.center().x, pill.bottom() + 4.0);
            let font = theme::regular(11.0);
            widgets::text_at(
                ui,
                under,
                Align2::CENTER_TOP,
                "Normal",
                font,
                palette.secondary,
            );
        }
    }
}

/// The panel for a window with no room for it: one row.
fn strip(state: &State, palette: &Palette, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.playback_speed;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let slower = row.left_center() + vec2(16.0, 0.0);
    step(palette, ui, actions, (chosen, -1.0), slower);
    let faster = row.right_center() - vec2(16.0, 0.0);
    step(palette, ui, actions, (chosen, 1.0), faster);
    let pills = Rect::from_min_max(
        row.left_center() + vec2(44.0, -14.0),
        row.right_center() + vec2(-44.0, 14.0),
    );
    presets(palette, ui, actions, (pills, chosen, false));
}

fn panel(state: &State, palette: &Palette, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.playback_speed;
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.label(
        egui::RichText::new("Playback speed")
            .font(theme::semibold(12.0))
            .color(palette.secondary),
    );
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(format!("{:.2}×", state.speed())).font(theme::semibold(24.0)));
    });
    ui.add_space(12.0);

    // A step down, the slider, a step up.
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::hover());
    let slower = row.left_center() + vec2(16.0, 0.0);
    step(palette, ui, actions, (chosen, -1.0), slower);
    let faster = row.right_center() - vec2(16.0, 0.0);
    step(palette, ui, actions, (chosen, 1.0), faster);
    let bar = Rect::from_min_max(
        row.left_center() + vec2(44.0, -8.0),
        row.right_center() + vec2(-44.0, 8.0),
    );
    let (slowest, fastest) = SPEED_RANGE;
    let fraction = (chosen - slowest) / (fastest - slowest);
    let slider = widgets::slider(ui, palette, bar, fraction, "Playback speed");
    if let Some(at) = slider.dragging.or(slider.released) {
        let speed = clamp_speed(slowest + at * (fastest - slowest));
        if speed != chosen {
            actions.push(Action::SetSpeed(speed));
        }
    }

    ui.add_space(16.0);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
    presets(palette, ui, actions, (row, chosen, true));
}

/// One of the two round steps beside the slider.
fn round_button(
    ui: &mut Ui,
    palette: &Palette,
    at: Pos2,
    sign: &str,
    name: &str,
) -> egui::Response {
    let rect = Rect::from_center_size(at, vec2(32.0, 32.0));
    let response = ui.interact(rect, ui.id().with(name), Sense::click());
    widgets::name(ui, &response, name);
    let lift = widgets::hover(ui, &response);
    ui.painter()
        .circle_filled(at, 16.0, widgets::wash(ui, 0.6 + lift));
    let color = if ui.is_enabled() {
        palette.text
    } else {
        palette.text.gamma_multiply(0.4)
    };
    widgets::text_at(
        ui,
        at,
        Align2::CENTER_CENTER,
        sign,
        theme::regular(18.0),
        color,
    );
    response
}

/// A preset's pill. The one in force is filled with the text colour.
fn preset(ui: &mut Ui, palette: &Palette, rect: Rect, speed: f32, active: bool) -> egui::Response {
    let text = preset_label(speed);
    let response = ui.interact(rect, ui.id().with(("preset", &text)), Sense::click());
    widgets::hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, &text)
    });
    let lift = widgets::hover(ui, &response);
    let (fill, color) = if active {
        (palette.text, palette.window)
    } else {
        (
            crate::tint::blend(palette.surface, palette.surface_hover, lift),
            palette.text,
        )
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(u8::MAX), fill);
    widgets::text_at(
        ui,
        rect.center(),
        Align2::CENTER_CENTER,
        &text,
        theme::semibold(12.0),
        color,
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_speed_is_named_without_noughts_it_does_not_need() {
        assert_eq!(label(1.0), "1×");
        assert_eq!(label(1.25), "1.25×");
        assert_eq!(label(1.5), "1.5×");
        assert_eq!(label(clamp_speed(0.5 + 0.05 * 3.0)), "0.65×");
    }

    #[test]
    fn a_whole_preset_keeps_one_decimal_as_the_old_panel_had_it() {
        let named = PRESETS.map(preset_label);
        assert_eq!(named, ["1.0", "1.25", "1.5", "1.75", "2.0", "3.0"]);
    }
}
