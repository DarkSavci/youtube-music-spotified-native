//! Moving in from the Electron app: the dialog that says what it has, takes
//! the choice of what to bring, and shows the bringing and what it came to;
//! and the row in Settings that opens it.

use eframe::egui::{self, Align, Layout, Response, Sense, Stroke, Ui, pos2, vec2};

use super::{format, widgets};
use crate::actions::{Action, account_busy};
use crate::migrate::{Found, Kind, Line, Mark, Progress, day};
use crate::state::State;
use crate::theme::{self, Icon};

/// The side of a tick box, and of the mark before a line of the outcome.
const BOX: f32 = 18.0;
/// Between a box, or a mark, and its words.
const GAP: f32 = 12.0;

/// The dialog's contents, whichever stage the move is at.
pub fn dialog(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let migration = &state.migration;
    if let Some(progress) = &migration.running {
        running(state, ui, actions, progress);
    } else if let Some(lines) = &migration.outcome {
        // What was brought and what failed can be copied into a report.
        ui.scope(|ui| {
            widgets::selectable(ui);
            outcome(state, ui, actions, lines);
        });
    } else if let Some(found) = &migration.found {
        choosing(state, ui, actions, found);
    } else {
        // The profile went away while the dialog was open.
        title(ui, "Nothing to bring over");
        note(
            state,
            ui,
            "The old app's data is no longer on this computer.",
        );
        buttons(ui, |ui| close(ui, actions, "Close"));
    }
}

/// Which of its stages the dialog is at: choosing, bringing, or done.
pub fn stage(state: &State) -> u8 {
    match (&state.migration.running, &state.migration.outcome) {
        (Some(_), _) => 1,
        (None, Some(_)) => 2,
        (None, None) => 0,
    }
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::bold(20.0)));
    ui.add_space(8.0);
}

fn note(state: &State, ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(theme::regular(13.5))
            .color(state.palette.secondary),
    );
}

/// The dialog's buttons, right-aligned with the first outermost.
fn buttons(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    ui.add_space(18.0);
    ui.with_layout(Layout::right_to_left(Align::Center), contents);
}

/// The plain button that closes the dialog, under whatever it is called
/// at this stage.
fn close(ui: &mut Ui, actions: &mut Vec<Action>, label: &str) {
    if ui.button(label).clicked() {
        actions.push(Action::CloseDialog);
    }
}

/// What was found, a tick for each kind, and the button that goes ahead.
fn choosing(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, found: &Found) {
    title(ui, "Bring your things over");
    note(
        state,
        ui,
        "The earlier Youtube Music Spotified app is on this computer. Choose what to bring \
         here. Nothing is taken out of the old app, and you can do this again later for what \
         you listen to there in the meantime.",
    );
    ui.add_space(14.0);
    let available = found.available();
    let choice = state.migration.choice;
    for kind in Kind::EVERY {
        let there = available.get(kind);
        let on = there && choice.get(kind);
        let about = about(kind, found);
        ui.add_enabled_ui(there, |ui| {
            if tick_row(state, ui, on, kind.label(), &about).clicked() {
                actions.push(Action::SetMigrationKind(kind, !on));
            }
        });
        ui.add_space(10.0);
    }
    let busy = account_busy(state);
    if busy {
        note(
            state,
            ui,
            "An account is being changed. This can go ahead when that is done.",
        );
    }
    buttons(ui, |ui| {
        ui.add_enabled_ui(choice.any() && !busy, |ui| {
            if widgets::pill_button(ui, &state.palette, "Bring it over").clicked() {
                actions.push(Action::StartMigration);
            }
        });
        close(ui, actions, "Not now");
    });
}

/// What there is of a kind, as the line under its name.
fn about(kind: Kind, found: &Found) -> String {
    match kind {
        Kind::SignIn => {
            let names: Vec<String> = found
                .people()
                .map(|account| {
                    let name = match account.name.as_str() {
                        "" => "An account",
                        name => name,
                    };
                    if account.here.is_some() {
                        format!("{name} (already here)")
                    } else if account.signed_in {
                        name.to_owned()
                    } else {
                        format!("{name} (signed out)")
                    }
                })
                .collect();
            match names.len() {
                0 => "No accounts found.".to_owned(),
                _ => format!("Stay signed in as {}.", names.join(", ")),
            }
        }
        Kind::History => {
            let history = found.history();
            let unread = found.accounts.iter().find_map(|a| a.unread.as_deref());
            if history.is_empty() {
                return match unread {
                    Some(reason) => format!("It could not be read: {reason}"),
                    None => "Nothing listened to yet.".to_owned(),
                };
            }
            let plays = match history.plays {
                1 => "1 play".to_owned(),
                plays => format!("{plays} plays"),
            };
            let (first, last) = (day(&history.first_play), day(&history.last_play));
            let when = match (first.as_str(), last.as_str()) {
                ("", _) | (_, "") => String::new(),
                (first, last) if first == last => format!("on {first}"),
                (first, last) => format!("from {first} to {last}"),
            };
            let pins = match history.pins {
                0 => String::new(),
                1 => "1 pin".to_owned(),
                pins => format!("{pins} pins"),
            };
            let folders = match history.folders {
                0 => String::new(),
                1 => "1 folder".to_owned(),
                folders => format!("{folders} folders"),
            };
            let counted = format!("{plays} {when}");
            let said = format::middle_dotted([counted.trim(), &pins, &folders]);
            format!(
                "Your statistics, pins and folders: {said}. Plays already here are not counted twice."
            )
        }
        Kind::Songs => match found.songs.count {
            0 => "No songs kept on disk.".to_owned(),
            count => format!(
                "{} ({}), copied as far as the song cache has room.",
                format::songs(count),
                format::bytes(found.songs.bytes)
            ),
        },
        Kind::Preferences => match (found.prefs.summary().as_str(), &found.prefs.unread) {
            ("", Some(reason)) => format!("They could not be read: {reason}"),
            ("", None) => "None found.".to_owned(),
            (summary, _) => format!("{summary}."),
        },
    }
}

/// A tick box with its kind's name and what there is of it. The whole row
/// is the control.
fn tick_row(state: &State, ui: &mut Ui, on: bool, label: &str, about: &str) -> Response {
    let palette = &state.palette;
    let width = ui.available_width();
    let text_width = width - BOX - GAP;
    let name = ui
        .painter()
        .layout_no_wrap(label.to_owned(), theme::medium(14.0), palette.text);
    let said = about;
    let about = ui.painter().layout(
        said.to_owned(),
        theme::regular(12.5),
        palette.secondary,
        text_width,
    );
    let height = name.size().y + 2.0 + about.size().y;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    widgets::hand(ui, &response);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label)
    });
    // The line under the name is painted, not laid out as a label; this
    // is what lets a screen reader find it all the same.
    let about_at = pos2(rect.left() + BOX + GAP, rect.top() + name.size().y + 2.0);
    let about_rect = egui::Rect::from_min_size(about_at, about.size());
    ui.interact(about_rect, response.id.with("about"), Sense::hover())
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, said));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let faded = if ui.is_enabled() { 1.0 } else { 0.5 };
    let square = egui::Rect::from_min_size(rect.min + vec2(0.0, 1.0), vec2(BOX, BOX));
    let lift = widgets::hover(ui, &response);
    if on {
        ui.painter().rect_filled(square, 5.0, palette.accent);
        widgets::paint_icon(ui, Icon::Check, square, 13.0, palette.on_accent);
    } else {
        let edge = crate::tint::blend(palette.dim, palette.text, lift).gamma_multiply(faded);
        ui.painter()
            .rect_filled(square, 5.0, widgets::wash(ui, lift * 0.6));
        let stroke = Stroke::new(1.5, edge);
        ui.painter()
            .rect_stroke(square, 5.0, stroke, egui::StrokeKind::Inside);
    }
    let name_at = pos2(about_at.x, rect.top());
    ui.painter()
        .galley(name_at, name, palette.text.gamma_multiply(faded));
    ui.painter()
        .galley(about_at, about, palette.secondary.gamma_multiply(faded));
    response
}

/// The step under way, and how far through it, where that can be counted.
fn running(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, progress: &Progress) {
    let palette = &state.palette;
    title(ui, "Bringing your things over");
    ui.add_space(6.0);
    if progress.total == 0 {
        widgets::loading(ui, palette, &format!("{}…", progress.step));
    } else {
        let said = format!("{}: {} of {}", progress.step, progress.done, progress.total);
        note(state, ui, &said);
        ui.add_space(8.0);
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), Sense::hover());
        let done = progress.done as f32 / progress.total as f32;
        ui.painter().rect_filled(bar, 2.0, palette.surface_active);
        let filled = bar.with_max_x(bar.left() + bar.width() * done.clamp(0.0, 1.0));
        ui.painter().rect_filled(filled, 2.0, palette.accent);
    }
    ui.add_space(10.0);
    note(
        state,
        ui,
        "You can keep listening. Closing this does not stop it.",
    );
    buttons(ui, |ui| close(ui, actions, "Hide"));
}

/// What was brought, what was left, and why.
fn outcome(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, lines: &[Line]) {
    let palette = &state.palette;
    let failed = lines.iter().any(|line| line.mark == Mark::Failed);
    let brought = lines.iter().any(|line| line.mark == Mark::Brought);
    title(
        ui,
        match (brought, failed) {
            (true, false) => "Brought over",
            (true, true) => "Brought over, in part",
            (false, true) => "It did not work",
            (false, false) => "Nothing new to bring",
        },
    );
    ui.add_space(4.0);
    for line in lines {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let (mark, _) = ui.allocate_exact_size(vec2(BOX, BOX), Sense::hover());
            match line.mark {
                Mark::Brought => {
                    widgets::paint_icon(ui, Icon::CircleCheck, mark, 16.0, palette.accent)
                }
                Mark::Failed => {
                    widgets::paint_icon(ui, Icon::CircleAlert, mark, 16.0, palette.danger)
                }
                // Left where it was: a dash, not a warning.
                Mark::Skipped => {
                    let stroke = Stroke::new(1.5, palette.dim);
                    let (left, right) = (
                        mark.center() - vec2(4.0, 0.0),
                        mark.center() + vec2(4.0, 0.0),
                    );
                    ui.painter().line_segment([left, right], stroke);
                }
            }
            let color = match line.mark {
                Mark::Skipped => palette.secondary,
                Mark::Brought | Mark::Failed => palette.text,
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&line.text)
                        .font(theme::regular(13.5))
                        .color(color),
                )
                .wrap(),
            );
        });
        ui.add_space(8.0);
    }
    buttons(ui, |ui| {
        if widgets::pill_button(ui, palette, "Done").clicked() {
            actions.push(Action::CloseDialog);
        }
        if state.migration.found.is_some() && ui.button("Bring more").clicked() {
            actions.push(Action::OpenMigration);
        }
    });
}

/// What the row in Settings says of the old app, and its button. `None`
/// when there is no old app to speak of.
pub fn summary(state: &State) -> Option<String> {
    let migration = &state.migration;
    let found = migration.found.as_ref()?;
    if let Some(progress) = &migration.running {
        return Some(match progress.total {
            0 => format!("{}…", progress.step),
            total => format!("{}: {} of {total}…", progress.step, progress.done),
        });
    }
    let history = found.history();
    let accounts = match found.people().count() {
        0 => String::new(),
        1 => "1 account".to_owned(),
        count => format!("{count} accounts"),
    };
    let plays = match history.plays {
        0 => String::new(),
        1 => "1 play".to_owned(),
        plays => format!("{plays} plays"),
    };
    let songs = match found.songs.count {
        0 => String::new(),
        count => format!("{} downloaded", format::songs(count)),
    };
    let there = format::middle_dotted([accounts.as_str(), &plays, &songs]);
    let there = if there.is_empty() {
        "preferences".to_owned()
    } else {
        there
    };
    let before = if migration.brought.any() {
        "Bring over again to pick up what you have listened to there since."
    } else {
        "Bring your accounts, statistics, songs and preferences here. Nothing is \
         taken out of the old app."
    };
    Some(format!(
        "The earlier app is on this computer: {there}. {before}"
    ))
}
