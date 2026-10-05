//! The Settings page: a few cards of rows, each a label, a line that says
//! what it does, and its control on the right.

use eframe::egui::{self, Align, Frame, Layout, Margin, Ui};

use super::{cards, equalizer, format, widgets};
use crate::actions::Action;
use crate::report;
use crate::state::{Page, State};
use crate::theme;
use crate::update::Status;

mod accounts;
mod blocked;
mod playback;

/// Cards stop growing here; a row's label and its control would otherwise
/// drift apart on a wide window.
const MAX_WIDTH: f32 = 760.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.label(egui::RichText::new("Settings").font(theme::bold(28.0)));
    // No wider than reads well, and never wider than there is room for.
    ui.set_max_width(MAX_WIDTH.min(ui.available_width()));

    section(state, ui, "Account", |ui| {
        accounts::show(state, ui, actions);
        accounts::old_app(state, ui, actions);
    });
    section(state, ui, "Playback", |ui| {
        playback::show(state, ui, actions)
    });
    section(state, ui, "Equalizer", |ui| {
        equalizer::panel(state, ui, actions, equalizer::Place::Page);
    });
    section(state, ui, "Appearance", |ui| {
        themes(state, ui, actions);
        ui.add_space(12.0);
        skins(state, ui, actions);
        ui.add_space(12.0);
        milkdrop(state, ui, actions);
        ui.add_space(12.0);
        let on = state.settings.visualizer;
        let label = "Player bar visualizer";
        let about = "Draws the music behind the player bar. The window is redrawn thirty \
                     times a second while a song plays, which costs some processor time.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetVisualizer(!on));
            }
        });
        ui.add_space(10.0);
        let on = state.settings.reduce_motion;
        let label = "Reduce motion";
        let about = "Minimise animation: hovers, switches and loading shapes change at \
                     once, and nothing fades.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetReduceMotion(!on));
            }
        });
    });
    section(state, ui, "Content", |ui| {
        let on = state.settings.show_music_videos;
        let label = "Show music videos";
        let about = "Off by default. This is an audio-first player; videos appear in their own shelves when enabled. Videos in a playlist always show.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetShowMusicVideos(!on));
            }
        });
    });
    section(state, ui, "Blocked", |ui| blocked::show(state, ui, actions));
    section(state, ui, "Storage", |ui| {
        let about = "Songs you play, and the ones about to play, are kept on disk so they \
                     start instantly. The least recently played are removed first.";
        row(state, ui, "Song cache", about, |ui| {
            playback::cache_size(state, ui, actions);
        });
        ui.add_space(10.0);
        let kept = match state.cache_usage {
            Some(usage) if usage.tracks > 0 => format!(
                "{} in {}.",
                format::bytes(usage.bytes),
                format::songs(usage.tracks as usize)
            ),
            Some(_) => "Nothing kept yet.".to_owned(),
            None => String::new(),
        };
        let about = format!(
            "Songs you play are kept on this computer so they start at once and \
             play offline. {kept}"
        );
        row(state, ui, "Downloaded songs", &about, |ui| {
            let empty = state.cache_usage.is_none_or(|usage| usage.tracks == 0);
            ui.add_enabled_ui(!empty, |ui| {
                if widgets::outline_button(ui, &state.palette, "Delete").clicked() {
                    actions.push(Action::ClearCache);
                }
            });
        });
    });
    section(state, ui, "Window", |ui| {
        if cfg!(windows) {
            let on = state.settings.system_title_bar;
            let label = "Use the system title bar";
            let about = "Gives the window Windows' own title bar and frame instead of \
                         the app's.";
            row(state, ui, label, about, |ui| {
                if widgets::switch(ui, &state.palette, on, label).clicked() {
                    actions.push(Action::SetSystemTitleBar(!on));
                }
            });
            ui.add_space(10.0);
        }
        let on = state.settings.close_to_tray;
        let label = "Keep playing when the window is closed";
        let about = "Closing the window leaves the app in the notification area. \
                     Quit from its menu there.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetCloseToTray(!on));
            }
        });
        ui.add_space(10.0);
        let on = state.starts_at_login;
        let label = "Start with Windows";
        let about = "Starts in the notification area when you sign in to Windows.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetStartAtLogin(!on));
            }
        });
    });
    section(state, ui, "Keyboard shortcuts", |ui| {
        playback::shortcuts(state, ui);
    });
    section(state, ui, "Troubleshooting", |ui| {
        problem_report(state, ui, actions);
        ui.add_space(10.0);
        let about = "yt-dlp finds the audio for each song. Update it if songs stop playing.";
        row(state, ui, "Stream resolver", about, |ui| {
            if state.updating_resolver {
                widgets::spinner(ui, &state.palette, 18.0);
            } else if widgets::outline_button(ui, &state.palette, "Update").clicked() {
                actions.push(Action::UpdateResolver);
            }
        });
        ui.add_space(10.0);
        let about = "Restore every setting on this page to its default.";
        row(state, ui, "Reset preferences", about, |ui| {
            if widgets::outline_button(ui, &state.palette, "Reset").clicked() {
                actions.push(Action::ResetPreferences);
            }
        });
    });
    section(state, ui, "About", |ui| {
        // The version is what a report of a fault begins with.
        widgets::selectable(ui);
        let version = format!("{} {}", crate::APP_NAME, env!("CARGO_PKG_VERSION"));
        row(state, ui, &version, &update_note(&state.update), |ui| {
            update_button(state, ui, actions);
            if widgets::outline_button(ui, &state.palette, "What's new").clicked() {
                actions.push(Action::Open(Page::Changelog));
            }
        });
        ui.add_space(10.0);
        let credit = egui::RichText::new(
            "Built with Rust and egui. Not affiliated with Spotify or YouTube.",
        );
        let credit = credit
            .font(theme::regular(12.5))
            .color(state.palette.secondary);
        ui.add(egui::Label::new(credit).selectable(true));
    });
}

/// The one thing to do when something breaks: save the log and send it.
/// The line under it says what is in the file and what is not, because
/// "send us your logs" is only a fair thing to ask when people can see
/// that it is safe to.
fn problem_report(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let about = match &state.report {
        report::Status::Saved(zip) => format!(
            "Saved to {}. Send that file along with what happened and roughly when.",
            zip.display()
        ),
        report::Status::Failed(reason) => format!("Could not create the report: {reason}"),
        report::Status::Idle | report::Status::Working => {
            "Saves a zip to your Downloads folder with the app's log and a short system \
             summary, for sending with a bug report. It lists the songs that played, but \
             never your cookies, sign-in or email address."
                .to_owned()
        }
    };
    let words = ("Problem report", about.as_str());
    row_with_room(state, ui, words, TWO_BUTTONS_ROOM, |ui| {
        if widgets::outline_button(ui, &state.palette, "Open log folder").clicked() {
            actions.push(Action::OpenLogs);
        }
        let working = state.report == report::Status::Working;
        ui.add_enabled_ui(!working, |ui| {
            let label = if working { "Saving…" } else { "Save report" };
            if widgets::outline_button(ui, &state.palette, label).clicked() {
                actions.push(Action::SaveReport);
            }
        });
    });
}

/// The built-in themes and those in the themes folder, as chips. The mini
/// player wears whichever the main window does.
fn themes(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    let about = "Follow system uses Windows' light or dark setting. Themes in the \
                 folder are small JSON files; copy one to make your own.";
    row_with_room(state, ui, ("Theme", about), TWO_BUTTONS_ROOM, |ui| {
        if widgets::outline_button(ui, palette, "Reload").clicked() {
            actions.push(Action::ReloadThemes);
        }
        if widgets::outline_button(ui, palette, "Open folder").clicked() {
            actions.push(Action::OpenThemesFolder);
        }
    });
    ui.add_space(8.0);
    let custom = state.settings.custom_theme.as_deref();
    // A custom theme whose file has gone is not worn, so none is lit.
    let worn = custom.filter(|file| state.themes.iter().any(|theme| theme.file == *file));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for choice in crate::themes::Choice::EVERY {
            let active = worn.is_none() && state.settings.theme == choice;
            if widgets::chip(ui, palette, choice.label(), active).clicked() && !active {
                actions.push(Action::SetTheme(choice));
            }
        }
        for theme in &state.themes {
            let active = worn == Some(theme.file.as_str());
            if widgets::chip(ui, palette, &theme.label(), active).clicked() && !active {
                actions.push(Action::SetCustomTheme(theme.file.clone()));
            }
        }
    });
}

/// The Winamp skins the mini player can wear, as chips, with the ways to
/// get more. Choosing one opens the mini player in it.
fn skins(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    use crate::skins::{self, Ask};
    let palette = &state.palette;
    let about = "The mini player can wear a Winamp skin: a classic one (.wsz), or a modern \
                 one (.wal), which is drawn as it rests, since its scripts are not run. Drop \
                 the file on the window to add it, or choose one with Add a skin.";
    ui.label(egui::RichText::new("Mini player skin").font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(about)
            .font(theme::regular(12.5))
            .color(palette.secondary),
    );
    ui.add_space(8.0);
    // On a line of their own, which wraps: four buttons beside the words
    // would run into them in a narrow window.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        if widgets::outline_button(ui, palette, "Add a skin\u{2026}").clicked() {
            actions.push(Action::Skin(Ask::Pick));
        }
        if widgets::outline_button(ui, palette, "Get skins").clicked() {
            actions.push(Action::Skin(Ask::OpenMuseum));
        }
        if widgets::outline_button(ui, palette, "Skins folder").clicked() {
            actions.push(Action::Skin(Ask::OpenFolder));
        }
        if widgets::outline_button(ui, palette, "Reload skins").clicked() {
            actions.push(Action::Skin(Ask::Reload));
        }
    });
    ui.add_space(8.0);
    let worn = state.settings.mini_skin.as_deref();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        if widgets::chip(ui, palette, "No skin", worn.is_none()).clicked() && worn.is_some() {
            actions.push(Action::Skin(Ask::Wear(None)));
        }
        let built_in = std::iter::once(skins::BUILT_IN);
        let installed = state.skins.iter().map(|skin| skin.file.as_str());
        for file in built_in.chain(installed) {
            let active = worn == Some(file);
            if widgets::chip(ui, palette, skins::label(file), active).clicked() && !active {
                actions.push(Action::Skin(Ask::Wear(Some(file.to_owned()))));
            }
        }
    });
    if worn.is_none() {
        return;
    }
    ui.add_space(10.0);
    let about = "How large the skin is drawn. Its pixels stay sharp at every size.";
    ui.label(egui::RichText::new("Skin size").font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(about)
            .font(theme::regular(12.5))
            .color(palette.secondary),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        for scale in skins::SCALES {
            let active = state.settings.skin_scale == scale;
            let label = format!("{scale}x");
            if widgets::chip(ui, palette, &label, active).clicked() && !active {
                actions.push(Action::Skin(Ask::Scale(scale)));
            }
        }
    });
}

/// MilkDrop: its window, and the presets it draws, of which the app comes
/// with none.
fn milkdrop(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    use crate::milkdrop::{Ask, PACKS};
    let palette = &state.palette;
    let status = state.milkdrop;
    let presets = match status.presets {
        0 => "It has no presets yet: get some here.".to_owned(),
        1 => "One preset is in its folder.".to_owned(),
        count => format!("{count} presets are in its folder."),
    };
    let about = format!(
        "The music drawn by MilkDrop's presets, in a window of its own. {presets} In the \
         window, Space is the next preset, L stays on one, and F fills the screen."
    );
    ui.label(egui::RichText::new("MilkDrop").font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(about)
            .font(theme::regular(12.5))
            .color(palette.secondary),
    );
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
        let toggle = if status.open {
            "Close MilkDrop"
        } else {
            "Open MilkDrop"
        };
        if widgets::outline_button(ui, palette, toggle).clicked() {
            actions.push(Action::MilkDrop(Ask::Toggle));
        }
        if status.fetching {
            widgets::spinner(ui, palette, 18.0);
        } else {
            for (index, pack) in PACKS.iter().enumerate() {
                if widgets::outline_button(ui, palette, pack.label).clicked() {
                    actions.push(Action::MilkDrop(Ask::GetPresets(index)));
                }
            }
        }
        if widgets::outline_button(ui, palette, "Presets folder").clicked() {
            actions.push(Action::MilkDrop(Ask::OpenFolder));
        }
    });
}

/// What the look for a newer version has come to, as a sentence.
fn update_note(status: &Status) -> String {
    match status {
        Status::Idle => "Updates download in the background and install when you restart.".into(),
        Status::Unavailable => "Only an installed copy updates itself.".into(),
        Status::Checking => "Checking for updates…".into(),
        Status::UpToDate => "You're on the latest version.".into(),
        Status::Downloading(version) => format!("Downloading {version}…"),
        Status::Ready { version, .. } => {
            format!("Version {version} is downloaded and installs when you restart.")
        }
        Status::Failed(error) => format!("Could not check for updates: {error}"),
    }
}

fn update_button(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let palette = &state.palette;
    match &state.update {
        Status::Ready { .. } => {
            if widgets::pill_button(ui, palette, "Restart to update").clicked() {
                actions.push(Action::InstallUpdate);
            }
        }
        Status::Checking | Status::Downloading(_) => widgets::spinner(ui, palette, 18.0),
        Status::Unavailable => {}
        Status::Idle | Status::UpToDate | Status::Failed(_) => {
            if widgets::outline_button(ui, palette, "Check for updates").clicked() {
                actions.push(Action::CheckForUpdate);
            }
        }
    }
}

/// A titled card.
fn section(state: &State, ui: &mut Ui, title: &str, contents: impl FnOnce(&mut Ui)) {
    cards::section_title(ui, title);
    Frame::new()
        .fill(state.palette.surface.gamma_multiply(0.7))
        .stroke((1.0, state.palette.outline))
        .corner_radius(theme::RADIUS + 2)
        .inner_margin(Margin::symmetric(20, 16))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui);
        });
}

/// The room a row leaves at its right for one control.
const CONTROL_ROOM: f32 = 140.0;
/// The least width a row's words are read at beside its controls.
const WORDS_LEAST: f32 = 200.0;
/// The room for two buttons side by side.
const TWO_BUTTONS_ROOM: f32 = 310.0;

/// A setting: what it is, what it does, and its control at the right.
fn row(state: &State, ui: &mut Ui, label: &str, about: &str, control: impl FnOnce(&mut Ui)) {
    row_with_room(state, ui, (label, about), CONTROL_ROOM, control);
}

/// A row whose controls need `room` at the right, more than one does.
fn row_with_room(
    state: &State,
    ui: &mut Ui,
    (label, about): (&str, &str),
    room: f32,
    control: impl FnOnce(&mut Ui),
) {
    let words = |ui: &mut Ui| {
        ui.label(egui::RichText::new(label).font(theme::medium(14.0)));
        ui.label(
            egui::RichText::new(about)
                .font(theme::regular(12.5))
                .color(state.palette.secondary),
        );
    };
    // In a narrow window the words and the controls do not fit side by
    // side, and the controls would be drawn over the words: they go under.
    if ui.available_width() < room + WORDS_LEAST {
        words(ui);
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            control(ui);
        });
        return;
    }
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            // Leave the control its room; the text wraps in what is left.
            ui.set_max_width(ui.available_width() - room);
            words(ui);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
}
