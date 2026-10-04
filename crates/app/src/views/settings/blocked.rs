//! The Blocked card of the Settings page: what is never played, each with
//! the button that lets it play again.

use eframe::egui::{self, Ui};

use super::super::widgets;
use super::row;
use crate::actions::Action;
use crate::blocked::Kind;
use crate::state::State;
use crate::theme;

pub(super) fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let blocked = &state.settings.blocked;
    let about = if blocked.is_empty() {
        "Nothing is blocked. Right-click a song, an artist or an album and choose Block: \
         the queue then steps over it, and radios leave it out."
    } else {
        "The queue steps over these, and radios leave them out."
    };
    let about = egui::RichText::new(about)
        .font(theme::regular(12.5))
        .color(state.palette.secondary);
    ui.label(about);
    for kind in [Kind::Artist, Kind::Album, Kind::Song] {
        for entry in blocked.of(kind) {
            ui.add_space(10.0);
            // One blocked before it had a name is still to be told apart.
            let name = if entry.name.is_empty() {
                &entry.id
            } else {
                &entry.name
            };
            // The button's id is its entry's: several say the same.
            ui.push_id((kind.noun(), &entry.id), |ui| {
                row(state, ui, name, kind.noun(), |ui| {
                    if widgets::outline_button(ui, &state.palette, "Unblock").clicked() {
                        actions.push(Action::SetBlocked {
                            kind,
                            id: entry.id.clone(),
                            name: entry.name.clone(),
                            blocked: false,
                        });
                    }
                });
            });
        }
    }
}
