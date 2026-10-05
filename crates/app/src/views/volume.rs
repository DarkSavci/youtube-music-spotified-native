//! Mute and volume, the same wherever playback can be controlled from: the
//! player bar, the full-screen player and the mini player. The slider runs
//! to 200% when volume boost is on, and the mouse wheel over either of the
//! two changes the level.

use eframe::egui::{self, Pos2, Rect, Ui, Vec2};

use super::widgets;
use crate::actions::Action;
use crate::state::{Playback, State};
use crate::theme::{Icon, Palette};

/// How much one notch of the wheel changes the volume. A touchpad reports
/// points instead, a hundred of which make a notch.
const WHEEL_STEP: f32 = 0.05;
const POINTS_PER_NOTCH: f32 = 100.0;

/// Where the control goes, as the caller has worked it out.
pub struct Volume {
    /// The slider. `None` where there is only room for the mute button.
    pub bar: Option<Rect>,
    pub mute_at: Pos2,
    /// The mute icon's size.
    pub icon: f32,
}

impl Volume {
    pub fn show(
        &self,
        state: &State,
        palette: &Palette,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
        playback: Option<&Playback>,
    ) {
        let volume = playback.map_or(0.0, |playback| playback.session.volume);
        let most = state.settings.max_volume();
        if let Some(bar) = self.bar {
            // Volume follows the drag as it happens: it is heard, not just
            // seen.
            let shown = (volume / most).min(1.0);
            let slider = widgets::slider(ui, palette, bar, shown, "volume");
            if let Some(level) = slider.dragging.or(slider.released)
                && (level - shown).abs() >= 0.01 / most
            {
                actions.push(Action::SetVolume(level * most));
            }
        }
        let icon = match volume {
            v if v <= 0.0 => Icon::VolumeX,
            v if v < 0.5 => Icon::Volume1,
            _ => Icon::Volume2,
        };
        let mute = widgets::IconButton {
            icon,
            size: self.icon,
            tooltip: "Mute",
            active: false,
        };
        if mute.show_at(ui, palette, self.mute_at).clicked() {
            actions.push(Action::ToggleMute);
        }

        let button = Rect::from_center_size(self.mute_at, Vec2::splat(self.icon + 12.0));
        let whole = self.bar.map_or(button, |bar| button.union(bar));
        if playback.is_some()
            && ui.rect_contains_pointer(whole)
            && let Some(step) = wheel(ui)
        {
            actions.push(Action::VolumeBy(step));
        }
    }
}

/// What the wheel asked of the volume this frame: up for louder. Read from
/// the wheel's own events, so a turn is a step whatever the page under the
/// pointer would have scrolled by.
pub(super) fn wheel(ui: &Ui) -> Option<f32> {
    let notches: f32 = ui.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::MouseWheel { unit, delta, .. } => Some(notches(*unit, *delta)),
                _ => None,
            })
            .sum()
    });
    (notches != 0.0).then_some(notches * WHEEL_STEP)
}

/// One wheel event as notches: whichever way it turned furthest.
fn notches(unit: egui::MouseWheelUnit, delta: Vec2) -> f32 {
    let turned = if delta.y.abs() >= delta.x.abs() {
        delta.y
    } else {
        delta.x
    };
    match unit {
        egui::MouseWheelUnit::Point => turned / POINTS_PER_NOTCH,
        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => turned,
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::vec2;

    use super::*;

    #[test]
    fn a_notch_of_the_wheel_is_a_step_and_a_touchpad_scales_to_it() {
        assert_eq!(notches(egui::MouseWheelUnit::Line, vec2(0.0, 1.0)), 1.0);
        assert_eq!(notches(egui::MouseWheelUnit::Line, vec2(0.0, -2.0)), -2.0);
        assert_eq!(notches(egui::MouseWheelUnit::Point, vec2(0.0, 50.0)), 0.5);
        // A wheel that tilts counts by its tilt when that is the larger.
        assert_eq!(notches(egui::MouseWheelUnit::Line, vec2(-1.0, 0.2)), -1.0);
    }
}
