//! The music, drawn behind the player bar: a spectrum of bars that jump
//! with the sound and fall back at their own pace.

use std::time::Duration;

use eframe::egui::{Color32, Rect, Ui, pos2};

use crate::state::{Playback, State};

const BARS: usize = 48;
const GAP: f32 = 2.0;
/// How fast a bar falls once the sound has left it, in bar heights a second.
const FALL_PER_SECOND: f32 = 3.0;
/// How long a drawing stands before the next: thirty a second. Asking for
/// every frame the display can show costs six times as much on a fast
/// monitor, for movement nobody can tell apart.
const REDRAW: Duration = Duration::from_millis(33);
/// How much of the bar's colour shows; it sits behind the controls.
const OPACITY: f32 = 0.2;

/// Draws the spectrum across `area` while the visualizer is on and a song
/// is playing, and asks for the next frame. Returns whether it drew.
pub fn show(state: &State, ui: &Ui, area: Rect, color: Color32) -> bool {
    let playing = state.playback.as_ref().is_some_and(Playback::is_playing);
    let (true, true, Some(tap)) = (state.settings.visualizer, playing, &state.audio_tap) else {
        return false;
    };
    let heard = tap.spectrum(BARS);
    // Each bar remembers how high it was: it rises at once and falls
    // gradually, which is what makes the movement readable.
    let id = ui.id().with("visualizer");
    let fall = ui.input(|input| input.stable_dt) * FALL_PER_SECOND;
    let mut levels = ui
        .data(|data| data.get_temp::<Vec<f32>>(id))
        .filter(|levels| levels.len() == BARS)
        .unwrap_or_else(|| vec![0.0; BARS]);
    for (level, now) in levels.iter_mut().zip(&heard) {
        *level = now.max(*level - fall);
    }
    let width = (area.width() - GAP * (BARS as f32 - 1.0)) / BARS as f32;
    let fill = color.gamma_multiply(OPACITY);
    for (bar, level) in levels.iter().enumerate() {
        let left = area.left() + bar as f32 * (width + GAP);
        let top = area.bottom() - level * area.height() * 0.9;
        let rect = Rect::from_min_max(pos2(left, top), pos2(left + width, area.bottom()));
        ui.painter().rect_filled(rect, 1.0, fill);
    }
    ui.data_mut(|data| data.insert_temp(id, levels));
    ui.ctx().request_repaint_after(REDRAW);
    true
}
