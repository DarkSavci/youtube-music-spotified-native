//! The Playback card of the Settings page, and the list of shortcuts.

use eframe::egui::{self, Ui};

use super::super::{keys, widgets};
use super::row;
use crate::actions::{Action, MAX_CROSSFADE_SECONDS};
use crate::settings::{CACHE_SIZES_MB, VolumeLevel};
use crate::state::State;
use crate::theme;

/// The room between one setting and the next.
const BETWEEN: f32 = 10.0;

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

/// Quiet, normal or loud, from a list.
fn volume_level(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.volume_level;
    let combo = egui::ComboBox::from_id_salt("volume-level")
        .selected_text(chosen.label())
        .show_ui(ui, |ui| {
            for level in VolumeLevel::EVERY {
                let current = level == chosen;
                if ui.selectable_label(current, level.label()).clicked() && !current {
                    actions.push(Action::SetVolumeLevel(level));
                }
            }
        });
    name_list(&combo.response, "Volume level");
}

/// The cap on the songs kept on disk, from a list.
pub(super) fn cache_size(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.cache_max_mb;
    let combo = egui::ComboBox::from_id_salt("cache-size")
        .selected_text(megabytes(chosen))
        .show_ui(ui, |ui| {
            for size in CACHE_SIZES_MB {
                let current = size == chosen;
                if ui.selectable_label(current, megabytes(size)).clicked() && !current {
                    actions.push(Action::SetCacheSize(size));
                }
            }
        });
    name_list(&combo.response, "Song cache size");
}

/// Names a list for screen readers, and for the tests that find it so.
fn name_list(response: &egui::Response, label: &str) {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, label));
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
