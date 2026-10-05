//! The skin's equalizer window, between the main window and the playlist.
//!
//! Winamp's preamp, ten bands, power switch, presets and response graph,
//! from `eqmain.bmp`, moving the app's own equalizer: the same ten bands and
//! the same twelve decibels either way. Rolled up, it is a bar with the
//! volume along it, from `eq_ex.bmp`.

use eframe::egui::{self, Color32, Sense};
use spotified_audio::eq::RANGE_DB;

use super::super::super::volume;
use super::{Slid, View, balance_of, menu};
use crate::actions::Action;
use crate::equalizer::{self, Ask as Eq};
use crate::skin::Sheet;
use crate::skin::layout::{self, Area};
use crate::skin::sprites::{self, Sprite};
use crate::skins::Ask;
use crate::state::{Playback, State};

/// A slider's thumb is eleven pixels in a track of sixty-three.
const THUMB: u32 = 11;
const TRAVEL: u32 = layout::EQ_PREAMP.height - THUMB;
/// The graph's bands sit twelve pixels apart, two in from its edge.
const GRAPH_STEP: u32 = 12;
const GRAPH_PAD: u32 = 2;

pub(super) fn show(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) {
    let settings = &state.settings;
    if settings.skin_equalizer_shaded {
        return shade(state, view, actions, focused, playback);
    }
    let whole = Area::new(0, 0, layout::WINDOW_WIDTH, layout::EQ_HEIGHT);
    view.sprite(sprites::EQ_BACKGROUND, whole);
    let bar = if focused {
        sprites::EQ_TITLE_BAR_ACTIVE
    } else {
        sprites::EQ_TITLE_BAR_INACTIVE
    };
    view.sprite(bar, layout::EQ_TITLE_BAR);
    let title = view.title_bar(layout::EQ_TITLE_BAR, "Equalizer title bar");
    if title.double_clicked() {
        actions.push(Action::Skin(Ask::ToggleEqualizerShade));
    }
    if view
        .lamp_button(
            layout::EQ_SHADE,
            sprites::EQ_SHADE_BUTTON_PRESSED,
            false,
            "Roll the equalizer up",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleEqualizerShade));
    }
    if view
        .button(
            layout::EQ_CLOSE,
            sprites::EQ_CLOSE_BUTTON,
            sprites::EQ_CLOSE_BUTTON_PRESSED,
            "Close the equalizer",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleEqualizer));
    }

    let on = settings.equalizer_on;
    let (normal, pressed) = if on {
        (sprites::EQ_ON_ON, sprites::EQ_ON_ON_PRESSED)
    } else {
        (sprites::EQ_ON_OFF, sprites::EQ_ON_OFF_PRESSED)
    };
    if view
        .button(layout::EQ_ON, normal, pressed, "Equalizer on")
        .clicked()
    {
        actions.push(Action::Equalizer(Eq::On(!on)));
    }
    // Winamp's AUTO loaded a preset for each song, which nothing here can
    // stand in for; the button lays the bands flat.
    if view
        .button(
            layout::EQ_AUTO,
            sprites::EQ_AUTO_OFF,
            sprites::EQ_AUTO_OFF_PRESSED,
            "Reset the equalizer",
        )
        .on_hover_text("Reset")
        .clicked()
    {
        actions.push(Action::Equalizer(Eq::Reset));
    }
    let presets = view.button(
        layout::EQ_PRESETS_BUTTON,
        sprites::EQ_PRESETS,
        sprites::EQ_PRESETS_PRESSED,
        "Presets",
    );
    menu(egui::Popup::menu(&presets), view.skin(), view.unit, |ui| {
        presets_menu(state, ui, actions);
    });

    graph(view, settings.equalizer_preamp, &settings.equalizer);
    let preamp = fraction(settings.equalizer_preamp);
    match slider(view, layout::EQ_PREAMP, "Preamp", preamp) {
        Slid::Dragging(value) => actions.push(Action::Equalizer(Eq::Preamp(decibels(value)))),
        Slid::Released(_) => actions.push(Action::Equalizer(Eq::Keep)),
        Slid::No => {}
    }
    for (band, gain) in settings.equalizer.into_iter().enumerate() {
        let name = format!("Band {}", band + 1);
        match slider(view, layout::eq_band(band), &name, fraction(gain)) {
            Slid::Dragging(value) => {
                actions.push(Action::Equalizer(Eq::Band(band, decibels(value))));
            }
            Slid::Released(_) => actions.push(Action::Equalizer(Eq::Keep)),
            Slid::No => {}
        }
    }
}

/// The equalizer rolled up: its bar, with the volume along it.
fn shade(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) {
    let bar = if focused {
        sprites::EQ_SHADE_BAR_ACTIVE
    } else {
        sprites::EQ_SHADE_BAR_INACTIVE
    };
    let area = Area::new(0, 0, layout::WINDOW_WIDTH, layout::EQ_SHADE_HEIGHT);
    view.sprite(bar, area);
    let title = view.title_bar(area, "Equalizer title bar");
    if title.double_clicked() {
        actions.push(Action::Skin(Ask::ToggleEqualizerShade));
    }

    let most = state.settings.max_volume();
    let volume = playback.map_or(0.0, |playback| playback.session.volume);
    let level = (volume / most).clamp(0.0, 1.0);
    let track = layout::EQ_SHADE_VOLUME;
    let (response, slid) = view.slider(track, "Volume, on the equalizer", layout::EQ_SHADE_THUMB);
    if let Slid::Dragging(to) | Slid::Released(to) = slid
        && (to - level).abs() >= 0.005
    {
        actions.push(Action::SetVolume(to * most));
    }
    if playback.is_some()
        && response.hovered()
        && let Some(step) = volume::wheel(view.ui)
    {
        actions.push(Action::VolumeBy(step));
    }
    let thumb = look(
        level,
        [
            sprites::EQ_SHADE_VOLUME_THUMB_LOW,
            sprites::EQ_SHADE_VOLUME_THUMB_MIDDLE,
            sprites::EQ_SHADE_VOLUME_THUMB_HIGH,
        ],
    );
    let travel = (track.width - layout::EQ_SHADE_THUMB) as f32;
    view.sprite_at(thumb, track.x + (level * travel).round() as u32, track.y);
    // Balance, with the same snap to the middle as the main window's.
    let track = layout::EQ_SHADE_BALANCE;
    let (_, slid) = view.slider(track, "Balance, on the equalizer", layout::EQ_SHADE_THUMB);
    match slid {
        Slid::Dragging(to) => actions.push(Action::Skin(Ask::Balance(balance_of(to)))),
        Slid::Released(to) => {
            actions.push(Action::Skin(Ask::Balance(balance_of(to))));
            actions.push(Action::Skin(Ask::Keep));
        }
        Slid::No => {}
    }
    let fraction = (state.settings.balance + 1.0) / 2.0;
    let thumb = look(
        fraction,
        [
            sprites::EQ_SHADE_BALANCE_THUMB_LEFT,
            sprites::EQ_SHADE_BALANCE_THUMB_MIDDLE,
            sprites::EQ_SHADE_BALANCE_THUMB_RIGHT,
        ],
    );
    let travel = (track.width - layout::EQ_SHADE_THUMB) as f32;
    view.sprite_at(thumb, track.x + (fraction * travel).round() as u32, track.y);

    if view
        .lamp_button(
            layout::EQ_SHADE,
            sprites::EQ_UNSHADE_BUTTON_PRESSED,
            false,
            "Roll the equalizer down",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleEqualizerShade));
    }
    if view
        .button(
            layout::EQ_CLOSE,
            sprites::EQ_SHADE_CLOSE_BUTTON,
            sprites::EQ_SHADE_CLOSE_BUTTON_PRESSED,
            "Close the equalizer",
        )
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleEqualizer));
    }
}

/// Which of a mini slider's three looks goes with a value from 0 to 1.
fn look(value: f32, looks: [Sprite; 3]) -> Sprite {
    if value < 1.0 / 3.0 {
        looks[0]
    } else if value < 2.0 / 3.0 {
        looks[1]
    } else {
        looks[2]
    }
}

/// A gain as a fraction of the slider, 0 at the bottom.
fn fraction(gain: f32) -> f32 {
    ((gain + RANGE_DB) / (2.0 * RANGE_DB)).clamp(0.0, 1.0)
}

fn decibels(fraction: f32) -> f32 {
    fraction * 2.0 * RANGE_DB - RANGE_DB
}

/// A vertical slider drawn from the skin's frames: where the pointer has
/// it while it is held, and that it was let go.
fn slider(view: &mut View<'_>, area: Area, name: &str, value: f32) -> Slid {
    let response = view.interact(area, name, Sense::click_and_drag());
    let frame = (value * (sprites::EQ_SLIDER_FRAMES - 1) as f32).round() as u32;
    view.sprite(sprites::eq_slider_frame(frame), area);
    let held = response.dragged() || response.is_pointer_button_down_on();
    let thumb = if held {
        sprites::EQ_THUMB_PRESSED
    } else {
        sprites::EQ_THUMB
    };
    let thumb_y = area.y + ((1.0 - value) * TRAVEL as f32).round() as u32;
    view.sprite_at(thumb, area.x + 1, thumb_y);
    if response.drag_stopped() || response.clicked() {
        return Slid::Released(value);
    }
    let Some(pos) = response.interact_pointer_pos().filter(|_| held) else {
        return Slid::No;
    };
    let along = view.skin_pos(pos).y - area.y as f32 - THUMB as f32 / 2.0;
    let to = (1.0 - along / TRAVEL as f32).clamp(0.0, 1.0);
    // A drag that has not left the step it is on asks for nothing.
    if equalizer::stepped(decibels(to)) == equalizer::stepped(decibels(value)) {
        return Slid::No;
    }
    Slid::Dragging(to)
}

/// The curve through the bands, in the skin's colours row by row, with the
/// preamp's line under it.
fn graph(view: &mut View<'_>, preamp: f32, bands: &equalizer::Curve) {
    let area = layout::EQ_GRAPH;
    view.sprite(sprites::EQ_GRAPH, area);
    let rows = area.height;
    let row_of = |fraction: f32| ((1.0 - fraction) * (rows - 1) as f32).round() as u32;
    let preamp_row = area.y + row_of(fraction(preamp));
    view.sprite_at(sprites::EQ_PREAMP_LINE, area.x, preamp_row);
    let line = sprites::EQ_GRAPH_LINE;
    let colors: Vec<Color32> = (0..rows)
        .map(|row| {
            let sheet = view.skin().sheet(Sheet::EqMain);
            sheet
                .pixel(line.x, line.y + row.min(line.height.saturating_sub(1)))
                .map_or(Color32::WHITE, |[red, green, blue, _]| {
                    Color32::from_rgb(red, green, blue)
                })
        })
        .collect();
    let points = bands.map(fraction);
    let width = GRAPH_STEP * (points.len() as u32 - 1);
    let mut last = row_of(points[0]);
    for x in 0..=width {
        let row = row_of(spline(&points, x as f32 / GRAPH_STEP as f32));
        let (top, bottom) = if row < last { (row, last) } else { (last, row) };
        for at in top..=bottom {
            let pixel = Area::new(area.x + GRAPH_PAD + x, area.y + at, 1, 1);
            view.fill(pixel, colors[at as usize]);
        }
        last = row;
    }
}

/// A smooth curve through evenly spaced points (Catmull-Rom), at `t`
/// measured in points.
fn spline(points: &[f32], t: f32) -> f32 {
    let last = points.len() - 1;
    let i = (t.floor() as usize).min(last.saturating_sub(1));
    let u = (t - i as f32).clamp(0.0, 1.0);
    let at = |index: isize| points[index.clamp(0, last as isize) as usize];
    let (p0, p1, p2, p3) = (
        at(i as isize - 1),
        at(i as isize),
        at(i as isize + 1),
        at(i as isize + 2),
    );
    let value = 0.5
        * (2.0 * p1
            + (-p0 + p2) * u
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u);
    value.clamp(0.0, 1.0)
}

/// The curves to start from, the listener's own first.
fn presets_menu(state: &State, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let settings = &state.settings;
    let named = equalizer::named(settings);
    for saved in &settings.equalizer_presets {
        let chosen = named == equalizer::Named::Saved(&saved.name);
        if ui.selectable_label(chosen, &saved.name).clicked() {
            actions.push(Action::Equalizer(Eq::Curve(saved.gains)));
        }
    }
    if !settings.equalizer_presets.is_empty() {
        ui.separator();
    }
    for (name, gains) in equalizer::PRESETS {
        let chosen = named == equalizer::Named::BuiltIn(name);
        if ui.selectable_label(chosen, name).clicked() {
            actions.push(Action::Equalizer(Eq::Curve(gains)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slider_maps_the_range_and_back() {
        assert_eq!(fraction(-RANGE_DB), 0.0);
        assert_eq!(fraction(0.0), 0.5);
        assert_eq!(fraction(RANGE_DB), 1.0);
        assert!((decibels(fraction(3.0)) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn the_curve_passes_through_the_bands_and_stays_inside() {
        let points = [0.5, 1.0, 0.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];
        assert!((spline(&points, 1.0) - 1.0).abs() < 1e-5);
        assert!((spline(&points, 2.0) - 0.0).abs() < 1e-5);
        for step in 0..=90 {
            let value = spline(&points, step as f32 / 10.0);
            assert!((0.0..=1.0).contains(&value));
        }
    }
}
