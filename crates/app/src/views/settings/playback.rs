//! The Playback card of the Settings page, and the list of shortcuts.

use eframe::egui::{self, Response, Ui};

use super::super::widgets::menu::{self, Entry};
use super::super::{keys, widgets};
use super::{row, row_with_room};
use crate::actions::{Action, MAX_CROSSFADE_SECONDS};
use crate::settings::{CACHE_SIZES_MB, VolumeLevel};
use crate::state::State;
use crate::theme;

/// The room between one setting and the next.
const BETWEEN: f32 = 10.0;

/// How wide the list of sound devices is: their names run long.
const OUTPUT_WIDTH: f32 = 240.0;
/// What following the system's choice of sound device is called.
const SYSTEM_DEFAULT: &str = "System default";
/// How wide a list of a few short choices is, and how tall any list.
pub(super) const LIST_WIDTH: f32 = 132.0;
const LIST_HEIGHT: f32 = 40.0;

/// A setting that is on or off.
fn toggle(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (label, about): (&str, &str),
    on: bool,
    action: fn(bool) -> Action,
) {
    row(state, ui, label, about, |ui| {
        if widgets::switch(ui, &state.palette, on, label).clicked() {
            actions.push(action(!on));
        }
    });
}

pub(super) fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let settings = &state.settings;
    let about = "Where the music plays. System default follows whichever device Windows \
                 is set to; a device chosen here that is unplugged is waited for, with the \
                 system's standing in.";
    let words = ("Output device", about);
    row_with_room(state, ui, words, OUTPUT_WIDTH + 24.0, |ui| {
        output_device(state, ui, actions);
    });
    ui.add_space(BETWEEN);
    let about = "Fades each song into the next as it ends. Off at zero.";
    row(state, ui, "Crossfade", about, |ui| {
        // The view may not change state, so it moves a copy and asks.
        let mut seconds = settings.crossfade_seconds;
        let slider = egui::Slider::new(&mut seconds, 0..=MAX_CROSSFADE_SECONDS).suffix(" s");
        if ui.add(slider).changed() {
            actions.push(Action::SetCrossfade(seconds));
        }
    });
    ui.add_space(BETWEEN);
    let text = (
        "Gapless playback",
        "Start the next track without a pause between them.",
    );
    toggle(
        state,
        ui,
        actions,
        text,
        settings.gapless,
        Action::SetGapless,
    );
    ui.add_space(BETWEEN);
    let text = (
        "Normalise volume",
        "Turns louder songs down so they all play at a similar level. \
         Applies from the next song.",
    );
    let normalise = settings.normalise_volume;
    toggle(
        state,
        ui,
        actions,
        text,
        normalise,
        Action::SetNormaliseVolume,
    );
    if normalise {
        ui.add_space(BETWEEN);
        let about = "Louder settings leave less headroom, so dynamic tracks have less room \
                     to breathe.";
        row(state, ui, "Volume level", about, |ui| {
            volume_level(state, ui, actions);
        });
    }
    ui.add_space(BETWEEN);
    let text = (
        "Autoplay",
        "When your queue runs low, keep playing similar songs from YouTube Music's radio.",
    );
    toggle(
        state,
        ui,
        actions,
        text,
        settings.autoplay,
        Action::SetAutoplay,
    );
    ui.add_space(BETWEEN);
    let text = (
        "Volume boost",
        "Let the volume go up to 200% for quiet tracks. A limiter keeps it from \
         distorting, but loud passages are flattened past 100%.",
    );
    let boost = settings.volume_boost;
    toggle(state, ui, actions, text, boost, Action::SetVolumeBoost);
    ui.add_space(BETWEEN);
    let text = (
        "Resume on launch",
        "Restore the last queue when the app starts.",
    );
    let resume = settings.resume_on_launch;
    toggle(state, ui, actions, text, resume, Action::SetResumeOnLaunch);
    ui.add_space(BETWEEN);
    let text = (
        "Continue from YouTube Music",
        "When the app starts, pick up the queue from your phone or the YouTube Music \
         website, paused where it was. Only when nothing is playing here. Needs you to \
         be signed in.",
    );
    let pick_up = settings.continue_from_youtube_music;
    let action = Action::SetContinueFromYouTubeMusic;
    toggle(state, ui, actions, text, pick_up, action);
    ui.add_space(BETWEEN);
    // The only setting here that writes to the account, so it says so
    // plainly and does not describe only the benefit.
    let text = (
        "Send listening to YouTube",
        "Counts plays towards your YouTube history and recommendations, and lets a \
         track started on another device carry on here. This is the only setting that \
         writes to your account \u{2014} everything else only reads.",
    );
    let report = settings.report_to_youtube;
    toggle(state, ui, actions, text, report, Action::SetReportToYouTube);
}

/// A drop-down `width` wide, closed, at the right of its row: the app's
/// own, as its menu is, so that both answer the pointer as every other
/// does. The caller hangs the menu of choices on what this returns.
pub(super) fn list(state: &State, ui: &mut Ui, label: &str, chosen: &str, width: f32) -> Response {
    let size = egui::vec2(width, LIST_HEIGHT);
    let closed = |ui: &mut Ui| widgets::select(ui, &state.palette, label, chosen, true);
    ui.allocate_ui(size, closed).inner
}

/// The system's own device or one of those there are, from a list. The
/// list is asked for again each time it is opened: devices come and go.
fn output_device(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.output_device.as_ref();
    let here = |id: &str| state.output_devices.iter().any(|device| device.id == id);
    let shown = chosen.map_or(SYSTEM_DEFAULT, |device| device.name.as_str());
    let select = list(state, ui, "Output device", shown, OUTPUT_WIDTH);
    if select.clicked() {
        actions.push(Action::ListOutputDevices);
    }
    menu::popup(&select, &state.palette, |menu| {
        let following = chosen.is_none();
        if menu.entry(Entry::plain(SYSTEM_DEFAULT).checked(following)) && !following {
            actions.push(Action::SetOutputDevice(None));
        }
        for device in &state.output_devices {
            let current = chosen.is_some_and(|chosen| chosen.id == device.id);
            if menu.entry(Entry::plain(&device.name).checked(current)) && !current {
                actions.push(Action::SetOutputDevice(Some(device.clone())));
            }
        }
        // Still the choice, though there is nothing to play through.
        if let Some(gone) = chosen.filter(|chosen| !here(&chosen.id)) {
            menu.entry(Entry::plain(&gone.name).checked(true).enabled(false));
            menu.note("Not connected. Playing through the system's.");
        }
    });
}

/// Quiet, normal or loud, from a list.
fn volume_level(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.volume_level;
    let select = list(state, ui, "Volume level", chosen.label(), LIST_WIDTH);
    menu::popup(&select, &state.palette, |menu| {
        for level in VolumeLevel::EVERY {
            let current = level == chosen;
            if menu.entry(Entry::plain(level.label()).checked(current)) && !current {
                actions.push(Action::SetVolumeLevel(level));
            }
        }
    });
}

/// The cap on the songs kept on disk, from a list.
pub(super) fn cache_size(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.cache_max_mb;
    let select = list(state, ui, "Song cache size", &megabytes(chosen), LIST_WIDTH);
    menu::popup(&select, &state.palette, |menu| {
        for size in CACHE_SIZES_MB {
            let current = size == chosen;
            if menu.entry(Entry::plain(&megabytes(size)).checked(current)) && !current {
                actions.push(Action::SetCacheSize(size));
            }
        }
    });
}

/// A cache size as the list names it: `512 MB`, `2 GB`.
fn megabytes(megabytes: u32) -> String {
    if megabytes >= 1024 {
        format!("{} GB", megabytes / 1024)
    } else {
        format!("{megabytes} MB")
    }
}

/// Every shortcut, under the kind of thing it does.
pub(super) fn shortcuts(state: &State, ui: &mut Ui) {
    for (index, group) in keys::Group::EVERY.into_iter().enumerate() {
        if index > 0 {
            ui.add_space(14.0);
        }
        ui.label(
            egui::RichText::new(group.label())
                .font(theme::semibold(12.0))
                .color(state.palette.secondary),
        );
        ui.add_space(6.0);
        egui::Grid::new(("shortcuts", index))
            .num_columns(2)
            .min_col_width(150.0)
            .spacing([32.0, 10.0])
            .show(ui, |ui| {
                for shortcut in keys::SHORTCUTS {
                    if shortcut.group != group {
                        continue;
                    }
                    let combination = egui::KeyboardShortcut::new(shortcut.modifiers, shortcut.key);
                    ui.label(
                        egui::RichText::new(ui.ctx().format_shortcut(&combination))
                            .font(theme::semibold(13.0)),
                    );
                    ui.label(
                        egui::RichText::new(shortcut.description)
                            .font(theme::regular(13.5))
                            .color(state.palette.secondary),
                    );
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_sizes_are_named_as_the_old_list_named_them() {
        let named = CACHE_SIZES_MB.map(megabytes);
        assert_eq!(named, ["512 MB", "1 GB", "2 GB", "5 GB", "10 GB"]);
    }
}
