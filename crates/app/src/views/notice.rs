//! What the player has to say without being asked: why playback is not
//! going on, in a pill above the player bar, and what is playing, to a
//! screen reader.

use eframe::egui::{self, Align2, Frame, Margin, Sense, Ui, accesskit, vec2};
use spotified_client::session::PlayState;

use super::widgets;
use crate::actions::Action;
use crate::state::{Playback, State};
use crate::theme::{self, Icon};
use crate::together::Ask;

/// The pill never grows wider than this.
const WIDEST: f32 = 560.0;

/// Why playback is not proceeding. It sits above the player bar and not
/// over the page, so it explains the player without covering what is being
/// read; sized to be noticed and not to alarm, since it is almost always
/// "wait a few minutes".
pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    let Some(notice) = state.notice else {
        return;
    };
    let palette = &state.palette;
    let lift = theme::PLAYER_BAR_HEIGHT + f32::from(theme::GUTTER) * 2.0;
    egui::Area::new(egui::Id::new("playback-notice"))
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -lift))
        .show(ui.ctx(), |ui| {
            Frame::new()
                .fill(palette.surface)
                .stroke((1.0, palette.outline))
                .corner_radius(u8::MAX)
                .inner_margin(Margin::symmetric(24, 10))
                .show(ui, |ui| {
                    let room = ui.ctx().content_rect().width() - 64.0;
                    ui.set_max_width(WIDEST.min(room) - 48.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        // The close button keeps its place whatever the
                        // text wraps to.
                        ui.set_max_width(ui.available_width());
                        let text = egui::RichText::new(notice.text()).font(theme::regular(12.0));
                        // In a room the notice carries the way to try the
                        // room's song again, and stays until it plays.
                        let following = state
                            .playback
                            .as_ref()
                            .is_some_and(|playback| playback.following_room);
                        let in_room = following && state.together.in_room();
                        let beside = if in_room { 132.0 } else { 40.0 };
                        let wide = ui.available_width() - beside;
                        ui.allocate_ui(vec2(wide, 0.0), |ui| ui.label(text));
                        if in_room {
                            if widgets::chip(ui, palette, "Retry playback", false).clicked() {
                                actions.push(Action::Room(Ask::RetryPlayback));
                            }
                            return;
                        }
                        // Read and understood: it can go. A new problem
                        // brings a new notice.
                        if widgets::icon_button(ui, palette, Icon::X, 14.0, "Dismiss").clicked() {
                            actions.push(Action::DismissNotice);
                        }
                    });
                });
        });
}

/// What a screen reader is told as playback changes by itself.
///
/// A player changes what it is doing without the listener acting: a song
/// ends, the next starts. None of that reaches a screen reader unless it is
/// announced. Politely: a new song is worth knowing and never worth
/// interrupting a sentence for. And the song is announced, not every state
/// it passes through: loading and buffering are several changes a song, and
/// narrating each would be noise.
pub fn announce(state: &State, ui: &mut Ui) {
    let memory = egui::Id::new("announcer");
    let said: Option<String> = ui.data(|data| data.get_temp(memory));
    let now = state.playback.as_ref().and_then(announcement);
    let message = match now {
        Some(fresh) if said.as_deref() != Some(fresh.as_str()) => {
            ui.data_mut(|data| data.insert_temp(memory, fresh.clone()));
            fresh
        }
        Some(same) => same,
        None => said.unwrap_or_default(),
    };
    if message.is_empty() {
        return;
    }
    // A point in the corner: there for assistive technology, not the eye.
    let corner = egui::Rect::from_min_size(ui.max_rect().min, vec2(1.0, 1.0));
    let response = ui.interact(corner, memory, Sense::hover());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(accesskit::Role::Status);
        node.set_live(accesskit::Live::Polite);
        node.set_label(message);
    });
}

/// The sentence for what playback is doing, when it is doing something
/// worth a sentence.
fn announcement(playback: &Playback) -> Option<String> {
    let track = playback.current()?;
    match playback.session.state {
        PlayState::Playing => {
            let artists = track.artist_names();
            Some(if artists.is_empty() {
                format!("Playing {}", track.title)
            } else {
                format!("Playing {} by {artists}", track.title)
            })
        }
        PlayState::Paused => Some("Paused".to_owned()),
        PlayState::Idle | PlayState::Loading | PlayState::Stalled => None,
    }
}
