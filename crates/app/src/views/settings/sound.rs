//! The Sound quality card of the Settings page: the presets, what is
//! playing and how it reaches the device, the stream asked for, exclusive
//! output, and headphone correction.

use eframe::egui::{self, Ui};
use spotified_audio::OutputFormat;
use spotified_audio::tap::Served;

use super::super::widgets::menu::{self, Entry};
use super::super::widgets::{self, TextField};
use super::playback::{LIST_WIDTH, list};
use super::{row, row_with_room};
use crate::actions::Action;
use crate::headphones;
use crate::settings::{SoundPreset, StreamQuality};
use crate::state::State;
use crate::theme;

/// The room between one setting and the next.
const BETWEEN: f32 = 10.0;
/// The rate Opus decodes at, and so the one a device is best set to.
const OPUS_RATE: u32 = 48_000;
/// Above this a stream is YouTube's Premium tier (about 256 kbps); the
/// standard tier is at most about 160.
const PREMIUM_KBPS: u32 = 200;
/// What a list of presets shows when the settings match none of them.
const CUSTOM: &str = "Custom";
/// How wide the field to look for headphones is.
const SEARCH_WIDTH: f32 = 260.0;

pub(super) fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let matching = SoundPreset::matching(&state.settings);
    let about = matching.map_or(
        "Your own mix of the settings below and under Playback.",
        SoundPreset::about,
    );
    row(state, ui, "Preset", about, |ui| {
        preset(state, ui, actions, matching);
    });
    ui.add_space(BETWEEN);
    playing(state, ui);
    ui.add_space(BETWEEN);
    let about = "High is the best YouTube has: Opus at about 256 kbps with YouTube Music \
                 Premium, about 160 without. Standard is at most about 170 kbps, and uses \
                 less data.";
    row(state, ui, "Stream quality", about, |ui| {
        stream_quality(state, ui, actions);
    });
    if cfg!(windows) {
        ui.add_space(BETWEEN);
        let on = state.settings.exclusive_output;
        let label = "Exclusive output";
        let about = "Takes the sound device for this app alone, bypassing Windows' mixer, \
                     sound effects and resampling: what plays is exactly what the app made, \
                     at 48 kHz where the device supports it. Other apps can't play on that \
                     device meanwhile, and the Windows volume no longer applies.";
        row(state, ui, label, about, |ui| {
            if widgets::switch(ui, &state.palette, on, label).clicked() {
                actions.push(Action::SetExclusiveOutput(!on));
            }
        });
    }
    ui.add_space(BETWEEN);
    headphone_correction(state, ui, actions);
}

/// The presets, from a list; "Custom" when none of them is in force.
fn preset(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, matching: Option<SoundPreset>) {
    let shown = matching.map_or(CUSTOM, SoundPreset::label);
    let select = list(state, ui, "Sound preset", shown, LIST_WIDTH);
    menu::popup(&select, &state.palette, |menu| {
        for preset in SoundPreset::EVERY {
            let current = matching == Some(preset);
            if menu.entry(Entry::plain(preset.label()).checked(current)) && !current {
                actions.push(Action::SetSoundPreset(preset));
            }
        }
        if matching.is_none() {
            menu.entry(Entry::plain(CUSTOM).checked(true).enabled(false));
        }
    });
}

fn stream_quality(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let chosen = state.settings.stream_quality;
    let select = list(state, ui, "Stream quality", chosen.label(), LIST_WIDTH);
    menu::popup(&select, &state.palette, |menu| {
        for quality in StreamQuality::EVERY {
            let current = quality == chosen;
            if menu.entry(Entry::plain(quality.label()).checked(current)) && !current {
                actions.push(Action::SetStreamQuality(quality));
            }
        }
    });
}

/// What is playing, how it reaches the device, and anything on the way
/// that costs quality, said plainly with what to do about it.
fn playing(state: &State, ui: &mut Ui) {
    let served = state.audio_tap.as_ref().and_then(|tap| tap.served());
    let (label, notes) = match &served {
        Some((stream, output)) => (describe(stream, output), notes(stream, output)),
        None => (
            "Nothing playing".to_owned(),
            vec!["Play a song to see its format and how it reaches your device.".to_owned()],
        ),
    };
    let palette = &state.palette;
    ui.label(egui::RichText::new("Now playing").font(theme::medium(14.0)));
    ui.label(
        egui::RichText::new(label)
            .font(theme::semibold(13.0))
            .color(palette.accent),
    );
    for note in notes {
        ui.label(
            egui::RichText::new(note)
                .font(theme::regular(12.5))
                .color(palette.secondary),
        );
    }
}

/// "Opus 256 kbps, 48 kHz → device at 48 kHz, 32-bit float, exclusive".
pub(crate) fn describe(stream: &Served, output: &OutputFormat) -> String {
    let bitrate = if stream.kbps > 0 {
        format!(" {} kbps", stream.kbps)
    } else {
        String::new()
    };
    let depth = match output.sample_format {
        "f32" => "32-bit float",
        "i16" => "16-bit",
        "i24" => "24-bit",
        _ => "32-bit",
    };
    let mode = if output.exclusive {
        "exclusive"
    } else {
        "shared"
    };
    format!(
        "{}{bitrate}, {} \u{2192} device at {}, {depth}, {mode}",
        stream.codec,
        kilohertz(stream.sample_rate),
        kilohertz(output.sample_rate),
    )
}

/// What costs quality between the stream and the device, and what to do.
pub(crate) fn notes(stream: &Served, output: &OutputFormat) -> Vec<String> {
    let mut notes = Vec::new();
    if stream.kbps > 0 && stream.kbps < PREMIUM_KBPS {
        notes.push(
            "Standard tier. YouTube's 256 kbps streams need YouTube Music Premium, \
             signed in."
                .to_owned(),
        );
    }
    if stream.sample_rate != output.sample_rate {
        notes.push(format!(
            "Resampled from {} to {}. Clean to well below hearing, but not bit for bit.",
            kilohertz(stream.sample_rate),
            kilohertz(output.sample_rate),
        ));
    }
    if !output.exclusive && output.sample_rate != OPUS_RATE {
        notes.push(format!(
            "Your device runs at {}. Opus is 48 kHz, so most songs are resampled. Set it \
             to 48 kHz in Windows' Sound settings, under the device's properties, or turn \
             on exclusive output.",
            kilohertz(output.sample_rate),
        ));
    }
    if let Some(why) = &output.exclusive_refused {
        notes.push(format!(
            "Exclusive output wasn't possible, so the device is shared: {why}."
        ));
    }
    if !output.exclusive && notes.is_empty() {
        notes.push(
            "Windows' mixer still sits between the app and the device; exclusive output \
             bypasses it and any sound effects on the device."
                .to_owned(),
        );
    }
    notes
}

/// "48 kHz", "44.1 kHz".
fn kilohertz(rate: u32) -> String {
    let khz = f64::from(rate) / 1000.0;
    if rate.is_multiple_of(1000) {
        format!("{khz:.0} kHz")
    } else {
        format!("{khz:.1} kHz")
    }
}

/// The headphones corrected for, and a search for others.
fn headphone_correction(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let settings = &state.settings;
    let palette = &state.palette;
    let label = "Headphone correction";
    match &settings.headphones {
        Some(chosen) => {
            let about = format!(
                "Correcting for {}, as measured by {}, with AutoEQ's filters. Works with or \
                 without the equalizer, and never clips.",
                chosen.name, chosen.source
            );
            let on = settings.headphones_on;
            row_with_room(state, ui, (label, &about), 200.0, |ui| {
                if widgets::switch(ui, palette, on, label).clicked() {
                    actions.push(Action::SetHeadphonesOn(!on));
                }
                if ui
                    .add(egui::Button::new(
                        egui::RichText::new("Remove").font(theme::medium(13.0)),
                    ))
                    .clicked()
                {
                    actions.push(Action::ForgetHeadphones);
                }
            });
        }
        None => {
            let about = "Makes your headphones sound neutral, from AutoEQ's measurements of \
                         about nine thousand models. Search for yours below.";
            row(state, ui, label, about, |_| {});
        }
    }
    ui.add_space(6.0);
    let search = &state.headphones;
    let field = TextField {
        text: &search.query,
        hint: "Search headphones, e.g. HD 600",
        label: "Search headphones",
        icon: Some(crate::theme::Icon::Search),
        width: SEARCH_WIDTH,
        compact: false,
    };
    if let Some(typed) = field.show(ui, palette) {
        actions.push(Action::SearchHeadphones(typed));
    }
    let note = |ui: &mut Ui, text: &str| {
        ui.label(
            egui::RichText::new(text)
                .font(theme::regular(12.5))
                .color(palette.secondary),
        );
    };
    if let Some(name) = &search.fetching {
        note(ui, &format!("Fetching the correction for {name}\u{2026}"));
        return;
    }
    if search.query.trim().is_empty() {
        return;
    }
    match &search.list {
        None => note(ui, "Fetching AutoEQ's list\u{2026}"),
        Some(Err(why)) => note(ui, why),
        Some(Ok(list)) => {
            let found = headphones::search(list, &search.query);
            if found.is_empty() {
                note(ui, "No headphones by that name.");
            }
            for headphone in found {
                let text = egui::RichText::new(format!(
                    "{}  \u{00b7}  {}",
                    headphone.name, headphone.source
                ))
                .font(theme::regular(13.0));
                let button = egui::Button::new(text).frame(false);
                if ui.add(button).clicked() {
                    actions.push(Action::ChooseHeadphones(headphone.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn served(codec: &'static str, kbps: u32, rate: u32) -> Served {
        Served {
            codec,
            kbps,
            sample_rate: rate,
            channels: 2,
        }
    }

    fn device(rate: u32, exclusive: bool) -> OutputFormat {
        OutputFormat {
            sample_rate: rate,
            sample_format: "f32",
            exclusive,
            exclusive_refused: None,
        }
    }

    #[test]
    fn the_format_is_said_plainly() {
        let said = describe(&served("Opus", 256, 48_000), &device(48_000, true));
        assert_eq!(
            said,
            "Opus 256 kbps, 48 kHz \u{2192} device at 48 kHz, 32-bit float, exclusive"
        );
        assert!(notes(&served("Opus", 256, 48_000), &device(48_000, true)).is_empty());
    }

    #[test]
    fn what_costs_quality_is_named() {
        let notes = notes(&served("AAC", 128, 44_100), &device(44_100, false));
        assert!(notes[0].contains("Premium"));
        assert!(notes.iter().all(|note| !note.starts_with("Resampled")));
        assert!(notes.iter().any(|note| note.contains("runs at 44.1 kHz")));
        let resampled = super::notes(&served("Opus", 256, 48_000), &device(44_100, false));
        assert!(resampled[0].starts_with("Resampled from 48 kHz to 44.1 kHz"));
    }
}
