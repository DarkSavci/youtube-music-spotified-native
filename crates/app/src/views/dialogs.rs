//! The dialog in front of everything: a question that needs an answer
//! before anything else happens.

use eframe::egui::{self, Align, Frame, Key, Layout, Margin, TextEdit, Ui};

use super::widgets;
use crate::actions::Action;
use crate::state::{Dialog, State};
use crate::theme;

const WIDTH: f32 = 420.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let Some(dialog) = &state.dialog else {
        return;
    };
    let palette = &state.palette;
    let frame = Frame::new()
        .fill(palette.overlay)
        .stroke((1.0, palette.outline))
        .corner_radius(12)
        .inner_margin(Margin::same(24));
    let modal = egui::Modal::new(egui::Id::new("dialog"))
        .frame(frame)
        .show(ui.ctx(), |ui| {
            ui.set_width(WIDTH);
            match dialog {
                Dialog::NewPlaylist { name, track_ids } => {
                    let note = match track_ids.len() {
                        0 => None,
                        1 => Some("1 song will be added.".to_owned()),
                        songs => Some(format!("{songs} songs will be added.")),
                    };
                    let naming = Naming {
                        title: "New playlist",
                        hint: "My playlist",
                        name,
                        note,
                    };
                    self::naming(state, ui, actions, naming);
                }
                Dialog::NewFolder { name } => {
                    let naming = Naming {
                        title: "New folder",
                        hint: "My folder",
                        name,
                        note: None,
                    };
                    self::naming(state, ui, actions, naming);
                }
                Dialog::DeletePlaylist { title, .. } => delete_playlist(state, ui, actions, title),
            }
        });
    // A click outside, or Escape, is "never mind".
    if modal.should_close() {
        actions.push(Action::CloseDialog);
    }
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::bold(20.0)));
    ui.add_space(8.0);
}

/// Cancel and the confirming button, right-aligned with the confirming one
/// outermost. Returns whether it was confirmed.
fn buttons(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, confirm: &str, enabled: bool) {
    ui.add_space(16.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            if widgets::pill_button(ui, &state.palette, confirm).clicked() {
                actions.push(Action::ConfirmDialog);
            }
        });
        if ui.button("Cancel").clicked() {
            actions.push(Action::CloseDialog);
        }
    });
}

/// A dialog that asks for a name.
struct Naming<'a> {
    title: &'a str,
    hint: &'a str,
    name: &'a str,
    /// Said under the field, when there is something to say.
    note: Option<String>,
}

fn naming(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, naming: Naming<'_>) {
    title(ui, naming.title);
    // The view may not change state, so it edits a copy and asks.
    let mut edited = naming.name.to_owned();
    let field = ui.add(
        TextEdit::singleline(&mut edited)
            .hint_text(naming.hint)
            .desired_width(f32::INFINITY),
    );
    // The name is what the dialog is for: the caret starts there.
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }
    if field.changed() {
        actions.push(Action::SetDialogText(edited));
    }
    let named = !naming.name.trim().is_empty();
    if named && field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
        actions.push(Action::ConfirmDialog);
    }
    if let Some(note) = naming.note {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(note)
                .font(theme::regular(13.0))
                .color(state.palette.secondary),
        );
    }
    buttons(state, ui, actions, "Create", named);
}

fn delete_playlist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, name: &str) {
    title(ui, "Delete playlist?");
    ui.label(
        egui::RichText::new(format!(
            "“{name}” will be deleted from your YouTube Music library."
        ))
        .font(theme::regular(14.0))
        .color(state.palette.secondary),
    );
    buttons(state, ui, actions, "Delete", true);
}
