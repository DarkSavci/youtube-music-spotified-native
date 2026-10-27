//! The saved-server picker, and the form that adds or edits one.

use eframe::egui::{self, Align, Layout, Ui};

use super::super::widgets::TextField;
use super::super::widgets::menu::{self, Entry};
use super::parts::{self, Kind};
use crate::actions::Action;
use crate::state::State;
use crate::theme::{self, Icon};
use crate::together::servers::CHECKING;
use crate::together::{Ask, Phase, ServerForm};

/// The room a row of the Connection card leaves at its right.
const CONTROLS: f32 = 300.0;

/// What a row of the Connection card is, as Settings words its rows.
pub fn row_words(state: &State, ui: &mut Ui, label: &str, about: &str) {
    ui.label(egui::RichText::new(label).font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(about)
            .font(theme::regular(12.5))
            .color(state.palette.secondary),
    );
}

/// A row of the Connection card: its words at the left, its controls at
/// the right, or under the words where the two do not fit side by side.
pub fn row(
    state: &State,
    ui: &mut Ui,
    (label, about): (&str, &str),
    controls: impl FnOnce(&mut Ui),
) {
    if ui.available_width() < CONTROLS + 200.0 {
        row_words(state, ui, label, about);
        ui.add_space(4.0);
        ui.horizontal_wrapped(controls);
        return;
    }
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width() - CONTROLS);
            row_words(state, ui, label, about);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), controls);
    });
}

/// Which server rooms are made and joined on.
pub fn bar(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let together = &state.together;
    let chosen = state.settings.together_server();
    let about = "Rooms are made and joined here. Your friends need the same server.";
    row(state, ui, ("Server", about), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        // The server is not changed under a room.
        ui.add_enabled_ui(together.phase == Phase::Idle, |ui| {
            let label = if chosen.is_some() {
                "Manage"
            } else {
                "Add server"
            };
            let manage = parts::text_button(ui, palette, Some(Icon::Settings), label);
            if manage.clicked() {
                actions.push(Action::Room(Ask::ToggleManage));
            }
            let shown = chosen.map_or("Choose a server", |server| &server.name);
            let select = parts::select(ui, palette, "Room server", shown, false);
            menu::popup(&select, palette, |menu| {
                for server in &state.settings.together_servers {
                    let is_chosen = chosen.is_some_and(|chosen| chosen.id == server.id);
                    if menu.entry(Entry::plain(&server.name).checked(is_chosen)) {
                        let select = Ask::SelectServer(server.id.clone());
                        actions.push(Action::Room(select));
                    }
                }
                if state.settings.together_servers.is_empty() {
                    menu.note("No saved servers yet.");
                }
            });
        });
    });
}

/// Adds a server, or renames, tests or removes the one chosen.
pub fn form(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, form: &ServerForm) {
    let palette = &state.palette;
    let editing = form.editing.is_some();
    parts::panel(state, ui, |ui| {
        let title = if editing {
            "Edit server"
        } else {
            "Add a server"
        };
        parts::heading(ui, title, 18.0);
        parts::quiet(
            state,
            ui,
            "Save this once. Your friends select the same server to join with a PIN.",
        );
        ui.add_space(12.0);
        ui.columns(2, |columns| {
            let fields = [
                ("Name", "Our music room", &form.name, true),
                ("Address", "wss://listen.example.com", &form.url, false),
            ];
            for (ui, (label, hint, text, is_name)) in columns.iter_mut().zip(fields) {
                parts::caption(state, ui, label);
                let field = TextField {
                    text,
                    hint,
                    label: if is_name {
                        "Server name"
                    } else {
                        "Server address"
                    },
                    icon: None,
                    width: ui.available_width(),
                    compact: false,
                };
                if let Some(typed) = field.show(ui, palette) {
                    actions.push(Action::Room(if is_name {
                        Ask::ServerName(typed)
                    } else {
                        Ask::ServerAddress(typed)
                    }));
                }
            }
        });
        ui.add_space(12.0);
        parts::actions_row(ui, |ui| {
            if parts::button(ui, palette, Kind::Primary, None, "Save server").clicked() {
                actions.push(Action::Room(Ask::SaveServer));
            }
            ui.add_enabled_ui(form.check != CHECKING, |ui| {
                let test = parts::button(ui, palette, Kind::Secondary, None, "Test connection");
                if test.clicked() {
                    actions.push(Action::Room(Ask::TestServer));
                }
            });
            if !form.check.is_empty() {
                parts::small(state, ui, &form.check);
            }
            if parts::button(ui, palette, Kind::Secondary, None, "Add another").clicked() {
                actions.push(Action::Room(Ask::AddAnother));
            }
            if editing && parts::text_button(ui, palette, None, "Remove saved server").clicked() {
                actions.push(Action::Room(Ask::RemoveServer));
            }
            if parts::text_button(ui, palette, None, "Cancel").clicked() {
                actions.push(Action::Room(Ask::ToggleManage));
            }
        });
    });
}
