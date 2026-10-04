//! The dialog in front of everything: a question that needs an answer
//! before anything else happens.

use eframe::egui::{self, Align, Frame, Key, Layout, Margin, TextEdit, Ui};

use super::{changelog, migration, widgets};
use crate::actions::Action;
use crate::state::{Dialog, Page, State};
use crate::theme::{self, Icon};

const WIDTH: f32 = 420.0;
/// The release notes are wider than a question, and as tall as the window
/// has room for.
const NOTES_WIDTH: f32 = 560.0;
/// How many releases the notes over the page show; the rest are on the
/// page of their own.
const NOTES_SHOWN: usize = 3;
/// The move from the old app lists what it found, a line or two for each.
const MIGRATION_WIDTH: f32 = 520.0;

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
    // The move from the old app changes what it shows as it goes; each
    // stage is laid out afresh, or it would keep the height of the tallest.
    let id = match dialog {
        Dialog::Migration => egui::Id::new("dialog").with(migration::stage(state)),
        _ => egui::Id::new("dialog"),
    };
    let modal = egui::Modal::new(id).frame(frame).show(ui.ctx(), |ui| {
        ui.set_width(match dialog {
            Dialog::WhatsNew => NOTES_WIDTH.min(ui.ctx().content_rect().width() - 80.0),
            Dialog::Migration => MIGRATION_WIDTH.min(ui.ctx().content_rect().width() - 80.0),
            _ => WIDTH,
        });
        match dialog {
            Dialog::WhatsNew => whats_new(state, ui, actions),
            Dialog::Migration => migration::dialog(state, ui, actions),
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
            Dialog::RemoveServer { name, .. } => {
                let name = if name.is_empty() { "this server" } else { name };
                let question = Question {
                    title: format!("Remove {name}?"),
                    body: "You can add it again later with its address.",
                    confirm: "Remove",
                };
                self::question(state, ui, actions, &question);
            }
            Dialog::RemoveListener { name, .. } => {
                let question = Question {
                    title: format!("Remove {name}?"),
                    body: "They leave the room and the PIN changes, so share the new PIN \
                               with everyone who is still joining.",
                    confirm: "Remove",
                };
                self::question(state, ui, actions, &question);
            }
            Dialog::SignOut => {
                let question = Question {
                    title: "Sign out?".to_owned(),
                    body: "Youtube Music Spotified will forget this account until you \
                               sign in again. Your listening history stays on this machine.",
                    confirm: "Sign out",
                };
                self::question(state, ui, actions, &question);
            }
            Dialog::RemoveAccount { .. } => {
                let question = Question {
                    title: "Remove saved account?".to_owned(),
                    body: "This signs this Google account out of the app. Other saved \
                               accounts remain available. Local listening history stays on \
                               this computer.",
                    confirm: "Remove",
                };
                self::question(state, ui, actions, &question);
            }
            Dialog::SaveRoomHistory { name, track_ids } => {
                let naming = Naming {
                    title: "Save room discoveries",
                    hint: "Playlist name",
                    name,
                    note: Some(match track_ids.len() {
                        1 => "1 song will be added.".to_owned(),
                        songs => format!("{songs} songs will be added."),
                    }),
                };
                self::naming(state, ui, actions, naming);
            }
        }
    });
    // A click outside, or Escape, is "never mind".
    if modal.should_close() {
        actions.push(Action::CloseDialog);
    }
}

/// The newest release notes, with a way to all of them.
fn whats_new(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("What's new").font(theme::bold(20.0)));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = widgets::icon_button(ui, palette, Icon::X, 18.0, "Close release notes");
            if close.clicked() {
                actions.push(Action::CloseDialog);
            }
        });
    });
    let room = (ui.ctx().content_rect().height() - 220.0).max(160.0);
    egui::ScrollArea::vertical()
        .id_salt("whats-new")
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            changelog::releases(state, ui, NOTES_SHOWN);
            ui.add_space(8.0);
        });
    ui.add_space(16.0);
    if widgets::chip(ui, palette, "View all releases", false).clicked() {
        actions.push(Action::CloseDialog);
        actions.push(Action::Open(Page::Changelog));
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

/// A yes-or-no question about removing something.
struct Question {
    title: String,
    body: &'static str,
    /// What the button that says yes reads.
    confirm: &'static str,
}

fn question(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, question: &Question) {
    title(ui, &question.title);
    ui.label(
        egui::RichText::new(question.body)
            .font(theme::regular(14.0))
            .color(state.palette.secondary),
    );
    buttons(state, ui, actions, question.confirm, true);
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
