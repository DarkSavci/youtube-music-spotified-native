//! Short messages that confirm what just happened, or say what went wrong.
//! They sit above the player bar at the right and leave by themselves.

use eframe::egui::{self, Align2, Frame, Margin, Ui, vec2};

use super::widgets;
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if state.toasts.is_empty() {
        return;
    }
    let palette = &state.palette;
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(
            Align2::RIGHT_BOTTOM,
            vec2(-20.0, -(theme::PLAYER_BAR_HEIGHT + 24.0)),
        )
        // Only a toast with a button, or one whose text can be selected,
        // takes clicks; the rest let them through to what is under them.
        .interactable(
            state
                .toasts
                .iter()
                .any(|toast| toast.link.is_some() || toast.error),
        )
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
                            // What went wrong can be selected and copied
                            // while it is up; a confirmation is only read.
                            let text = egui::RichText::new(&toast.text).font(theme::medium(13.5));
                            ui.add(egui::Label::new(text).selectable(toast.error));
                            if let Some(link) = &toast.link {
                                ui.add_space(4.0);
                                if widgets::chip(ui, palette, link.label, false).clicked() {
                                    actions.push(Action::Open(link.page.clone()));
                                }
                            }
                        });
                    });
            }
        });
}
