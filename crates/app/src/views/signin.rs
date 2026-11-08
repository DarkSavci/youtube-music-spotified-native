//! The sign-in dialog: what is about to happen, how far it has got, and why
//! it failed. Every way into signing in leads here, and the dialog can be
//! put away and brought back while the browser is open.

use eframe::egui::{self, Align, Color32, Layout, Ui, vec2};

use super::widgets;
use crate::actions::Action;
use crate::signin;
use crate::state::{SignInStage, State};
use crate::theme;

pub fn dialog(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let browser = state.sign_in_browser.unwrap_or("your browser");
    match (state.sign_in, &state.import_error) {
        (SignInStage::Idle, Some(error)) => failed(state, ui, actions, error),
        (SignInStage::Idle, None) => before(state, ui, actions, browser),
        (SignInStage::Browser, _) => in_browser(state, ui, actions, browser),
        (SignInStage::Reading, _) => finishing(state, ui, actions, "Reading your sign-in…", true),
        (SignInStage::Starting, _) => {
            finishing(state, ui, actions, "Loading your account…", false);
        }
    }
}

/// What will happen, before anything does.
fn before(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, browser: &str) {
    let palette = &state.palette;
    title(ui, "Sign in to YouTube Music");
    body(
        ui,
        palette.text,
        &format!(
            "{} opens Google's sign-in page in a window of its own.",
            capital(browser)
        ),
    );
    ui.add_space(12.0);
    steps(state, ui, None);
    ui.add_space(12.0);
    body(
        ui,
        palette.secondary,
        "That window starts empty, without your saved passwords, so have yours to hand. \
         Only the sign-in is kept; nothing else of your browser is read.",
    );
    footer(ui, |ui| {
        if widgets::pill_button(ui, palette, "Continue to Google").clicked() {
            actions.push(Action::StartSignIn);
        }
        if widgets::outline_button(ui, palette, "Not now").clicked() {
            actions.push(Action::CloseDialog);
        }
    });
    old_app(state, ui, actions);
}

/// The browser is open: where the person is, and the ways out.
fn in_browser(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, browser: &str) {
    let palette = &state.palette;
    title(ui, &format!("Finish signing in in {browser}"));
    steps(state, ui, Some(0));
    ui.add_space(12.0);
    let note = if cfg!(windows) {
        "You can put this away; the sign-in carries on, and Sign in at the top brings it back."
    } else {
        "If the window stays open a while after YouTube Music has loaded, close it yourself. \
         You can put this away; Sign in at the top brings it back."
    };
    body(ui, palette.secondary, note);
    footer(ui, |ui| {
        if signin::CAN_RAISE
            && widgets::pill_button(ui, palette, "Show the browser window").clicked()
        {
            actions.push(Action::ShowSignInBrowser);
        }
        if widgets::outline_button(ui, palette, "Cancel").clicked() {
            actions.push(Action::CancelSignIn);
        }
    });
}

/// The browser has done its part; the app is doing the rest.
fn finishing(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, doing: &str, cancel: bool) {
    let palette = &state.palette;
    title(ui, "Almost done");
    steps(state, ui, Some(1));
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        widgets::spinner(ui, palette, 16.0);
        ui.add_space(4.0);
        body(ui, palette.secondary, doing);
    });
    if cancel {
        footer(ui, |ui| {
            if widgets::outline_button(ui, palette, "Cancel").clicked() {
                actions.push(Action::CancelSignIn);
            }
        });
    }
}

/// Why it did not work, and another go.
fn failed(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, error: &str) {
    let palette = &state.palette;
    title(ui, "Couldn't sign in");
    body(ui, palette.text, error);
    footer(ui, |ui| {
        if widgets::pill_button(ui, palette, "Try again").clicked() {
            actions.push(Action::StartSignIn);
        }
        if widgets::outline_button(ui, palette, "Close").clicked() {
            actions.push(Action::CloseDialog);
        }
    });
    old_app(state, ui, actions);
}

/// The two things that happen, the one under way marked.
fn steps(state: &State, ui: &mut Ui, at: Option<usize>) {
    const STEPS: [&str; 2] = [
        "Sign in with your Google account in the browser window.",
        "When YouTube Music opens there, the window closes by itself and you're back here.",
    ];
    let palette = &state.palette;
    for (index, text) in STEPS.iter().enumerate() {
        let done = at.is_some_and(|at| index < at);
        let current = at == Some(index);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
            let (fill, ink) = if done || current {
                (palette.accent, palette.on_accent)
            } else {
                (palette.surface_active, palette.text)
            };
            ui.painter().circle_filled(rect.center(), 11.0, fill);
            let mark = if done {
                "✓".to_owned()
            } else {
                (index + 1).to_string()
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                mark,
                theme::bold(12.0),
                ink,
            );
            ui.add_space(6.0);
            let color = if current || at.is_none() {
                palette.text
            } else {
                palette.secondary
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(*text)
                        .font(theme::regular(13.0))
                        .color(color),
                )
                .wrap(),
            );
        });
        ui.add_space(6.0);
    }
}

/// A sign-in the Electron app on this computer already has.
fn old_app(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let has_one = state
        .migration
        .found
        .as_ref()
        .is_some_and(|found| found.available().sign_in);
    if !has_one {
        return;
    }
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);
    body(
        ui,
        state.palette.secondary,
        "You're signed in to the earlier Youtube Music Spotified app on this computer.",
    );
    ui.add_space(6.0);
    if widgets::chip(ui, &state.palette, "Use that account instead", false).clicked() {
        actions.push(Action::OpenMigration);
    }
}

fn title(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).font(theme::bold(20.0)));
    ui.add_space(10.0);
}

fn body(ui: &mut Ui, color: Color32, text: &str) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .font(theme::regular(13.0))
                .color(color),
        )
        .wrap(),
    );
}

/// The dialog's buttons, right-aligned with the first outermost.
fn footer(ui: &mut Ui, buttons: impl FnOnce(&mut Ui)) {
    ui.add_space(20.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        buttons(ui);
    });
}

/// "your browser" starts a sentence as "Your browser".
fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
