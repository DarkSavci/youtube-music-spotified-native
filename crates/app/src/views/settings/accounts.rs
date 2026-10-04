//! The Account card: the Google accounts kept signed in, which of them is
//! in use and as which of its channels, and the way to add another.

use eframe::egui::{self, Ui};

use super::{TWO_BUTTONS_ROOM, row_with_room};
use crate::accounts::SavedAccount;
use crate::actions::{Action, account_busy};
use crate::state::State;
use crate::theme;
use crate::views::{migration, widgets};

pub(super) fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let saved = &state.accounts;
    // One change at a time: a second on top of a sign-in in the browser,
    // or of the core starting on another account, would race it.
    let busy = account_busy(state);
    if !saved.accounts.is_empty() {
        ui.label(
            egui::RichText::new(
                "Google accounts and their YouTube channels are separate choices. Choose \
                 the channel whose library you want to use.",
            )
            .font(theme::regular(12.5))
            .color(palette.secondary),
        );
        ui.add_space(12.0);
    }
    for account in &saved.accounts {
        let active = saved.is_active(&account.id);
        let about = match (active, account.channel_name()) {
            (true, _) => "Active account",
            (false, Some(channel)) => channel,
            (false, None) => "Google account",
        };
        let words = (account.name.as_str(), about);
        row_with_room(state, ui, words, TWO_BUTTONS_ROOM, |ui| {
            ui.add_enabled_ui(!busy, |ui| {
                let remove = format!("Remove {}", account.name);
                if widgets::outline_button_named(ui, palette, "Remove account", &remove).clicked() {
                    actions.push(Action::AskRemoveAccount(account.id.clone()));
                }
                if !active {
                    let named = format!("Use {}", account.name);
                    if widgets::outline_button_named(ui, palette, "Use account", &named).clicked() {
                        actions.push(Action::SwitchAccount(account.id.clone()));
                    }
                } else if account.channels.len() > 1 {
                    // Most accounts hold one channel, and then there is
                    // nothing to choose.
                    channel_choice(account, ui, actions);
                }
            });
        });
        ui.add_space(10.0);
    }

    let (label, about) = match (saved.active.is_some(), saved.accounts.is_empty()) {
        (true, _) => (
            "Another account",
            "Your library, likes and playlists come from the account in use. Each account \
             keeps its own listening history on this computer.",
        ),
        (false, true) => (
            "Signed out",
            "Browsing and search work. Your library and reliable playback need an account.",
        ),
        (false, false) => (
            "Signed out",
            "No account is active. Add an account or choose a saved one.",
        ),
    };
    // Up to three buttons: too many to sit beside the words, so they go
    // under them.
    ui.label(egui::RichText::new(label).font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(about)
            .font(theme::regular(12.5))
            .color(palette.secondary),
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_enabled_ui(!busy, |ui| {
            let add = if state.signing_in {
                "Finish in your browser…"
            } else {
                "Add Google account"
            };
            if widgets::pill_button(ui, palette, add).clicked() {
                actions.push(Action::SignIn);
            }
            if saved.active.is_some()
                && widgets::outline_button(ui, palette, "Refresh channels").clicked()
            {
                actions.push(Action::RefreshChannels);
            }
        });
    });

    let note = if let Some(error) = &state.import_error {
        Some((error.as_str(), palette.danger))
    } else if state.signing_in {
        Some((
            "Sign in to YouTube Music in the browser window. It closes by itself when you are in.",
            palette.secondary,
        ))
    } else {
        None
    };
    if let Some((text, color)) = note {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(text)
                .font(theme::regular(12.0))
                .color(color),
        );
    }
}

/// The earlier, Electron app, when it is on this computer: what it has,
/// and the way to bring it here.
pub(super) fn old_app(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let Some(about) = migration::summary(state) else {
        return;
    };
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(14.0);
    super::row(state, ui, "Move from the old app", &about, |ui| {
        let label = if state.migration.running.is_some() {
            "Show progress"
        } else {
            "Bring over…"
        };
        if widgets::outline_button(ui, &state.palette, label).clicked() {
            actions.push(Action::OpenMigration);
        }
    });
}

/// The channel the account in use acts as, to choose another from.
fn channel_choice(account: &SavedAccount, ui: &mut Ui, actions: &mut Vec<Action>) {
    let current = account.channel_name().unwrap_or("Choose");
    let chosen = egui::ComboBox::from_id_salt("channel")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for channel in &account.channels {
                let chosen = channel.id == account.channel;
                let label = if channel.handle.is_empty() {
                    channel.name.clone()
                } else {
                    format!("{} ({})", channel.name, channel.handle)
                };
                if ui.selectable_label(chosen, label).clicked() && !chosen {
                    actions.push(Action::SwitchChannel(channel.id.clone()));
                }
            }
        });
    chosen.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, "YouTube channel")
    });
}
