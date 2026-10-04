//! Short messages that confirm what just happened, or say what went wrong.
//! They sit above the player bar at the right and leave by themselves.

use eframe::egui::{self, Align2, Frame, Margin, Ui, vec2};

use crate::state::State;
use crate::theme::{self, Icon};

pub fn show(state: &State, ui: &mut Ui) {
    if state.toasts.is_empty() {
        return;
    }
    let palette = &state.palette;
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(
            Align2::RIGHT_BOTTOM,
            vec2(-20.0, -(theme::PLAYER_BAR_HEIGHT + 24.0)),
        )
        .interactable(false)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for toast in &state.toasts {
                Frame::new()
                    .fill(palette.overlay)
                    .stroke((1.0, palette.outline))
                    .corner_radius(theme::RADIUS)
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if toast.error {
                                ui.add(Icon::CircleAlert.image(palette.danger, 16.0));
                            }
                            ui.set_max_width(280.0);
                            ui.label(egui::RichText::new(&toast.text).font(theme::medium(13.5)));
                        });
                    });
            }
        });
}
