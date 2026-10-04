//! The panel on the right: what has played, what is playing and what
//! comes next. The whole of it scrolls as one list, with what is playing
//! brought to the top when the track changes, so what has played sits just
//! out of sight above.

use std::time::Duration;

use eframe::egui::{self, Align, Align2, Color32, Layout, Margin, Rect, Sense, Ui, pos2, vec2};

use super::widgets::menu::{self, Entry, Menu};
use super::widgets::{self, ArtShape};
use super::{drag, format};
use crate::actions::Action;
use crate::state::{Page, Playback, State};
use crate::theme::{self, Icon};

const WIDTH: f32 = 360.0;
const WIDEST: f32 = 560.0;
const ROW_HEIGHT: f32 = 56.0;
const COVER: f32 = 40.0;
/// How much of a played song's colour is left: it is behind, not gone.
const PLAYED: f32 = 0.6;
/// How often the equalizer on the playing row is redrawn.
const EQUALIZER_REDRAW: Duration = Duration::from_millis(125);

/// Where a row stands against what is playing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Played,
    Playing,
    Upcoming,
}

/// A row of the queue being carried to another place in it.
struct Carried {
    from: usize,
}

/// `most` is the widest the panel may be in this window.
pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, most: f32) {
    let palette = &state.palette;
    let panel = egui::Panel::right("queue")
        .resizable(true)
        .show_separator_line(false)
        .default_size(WIDTH.min(most))
        .size_range(super::RIGHT_PANEL_MIN..=most.min(WIDEST))
        .frame(widgets::card(palette, super::RIGHT_PANEL_GUTTERS).inner_margin(Margin::same(12)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Queue").font(theme::bold(16.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, palette, Icon::X, 16.0, "Close").clicked() {
                        actions.push(Action::ToggleQueue);
                    }
                    super::equalizer::button(state, ui, actions);
                });
            });
            continue_from_remote(state, ui, actions);
            let playing = state
                .playback
                .as_ref()
                .filter(|playback| playback.current().is_some());
            match playing {
                Some(playback) => contents(state, ui, actions, playback),
                None => widgets::empty_state(
                    ui,
                    palette,
                    Icon::ListMusic,
                    "Nothing queued",
                    "Play something and it will show up here.",
                ),
            }
        });
    // Songs dropped anywhere on the panel join the end of the queue.
    if let Some(tracks) = drag::target(state, ui, &panel.response) {
        actions.push(Action::AddToQueue(tracks));
    }
}

/// Picks up the queue the account has on another device, the phone or the
/// website, and plays it here from where that device was. Signed in only:
/// signed out there is no such queue. Read on the click and never before,
/// since every read counts against the account's requests.
fn continue_from_remote(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    if state.account.is_none() {
        return;
    }
    let reading = state.reading_remote_queue;
    let label = if reading {
        "Loading your queue…"
    } else {
        "Continue from YouTube Music"
    };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.add_enabled_ui(!reading, |ui| {
            let chip = widgets::chip(ui, &state.palette, label, false)
                .on_hover_text("Replace this queue with the one on your other devices");
            if chip.clicked() {
                actions.push(Action::ContinueFromRemote);
            }
        });
    });
    ui.add_space(4.0);
}

/// The queue's sections, scrolling as one list in whatever room `ui` has.
pub(super) fn contents(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, playback: &Playback) {
    let queue = &playback.session.queue;
    let current = queue.index.min(queue.items.len().saturating_sub(1));
    egui::ScrollArea::vertical()
        .id_salt("queue-rows")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if current > 0 {
                heading(ui, &state.palette, "Played");
            }
            for index in 0..current {
                row(state, ui, actions, playback, index, Place::Played);
            }
            let now_playing = heading(ui, &state.palette, "Now playing");
            row(state, ui, actions, playback, current, Place::Playing);
            // A new track brings itself to the top, once; after that the
            // list is the person's to scroll.
            let memory = ui.id().with("at-top");
            let id = &queue.items[current].id;
            let shown = ui.data(|data| data.get_temp::<String>(memory));
            if shown.as_ref() != Some(id) {
                ui.scroll_to_rect(now_playing, Some(Align::TOP));
                ui.data_mut(|data| data.insert_temp(memory, id.clone()));
            }
            if current + 1 >= queue.items.len() {
                return;
            }
            let next = if queue.origin.is_empty() {
                "Next up".to_owned()
            } else {
                format!("Next from: {}", queue.origin)
            };
            heading(ui, &state.palette, &next);
            for index in current + 1..queue.items.len() {
                row(state, ui, actions, playback, index, Place::Upcoming);
            }
            // Room under the last row, so it can be brought clear of the edge.
            ui.add_space(24.0);
        });
}

/// A section's name. Returns where it was drawn.
fn heading(ui: &mut Ui, palette: &theme::Palette, text: &str) -> Rect {
    ui.add_space(14.0);
    let label = egui::RichText::new(text)
        .font(theme::semibold(12.5))
        .color(palette.secondary);
    let rect = ui
        .horizontal(|ui| {
            ui.add_space(8.0);
            ui.add(egui::Label::new(label).truncate());
        })
        .response
        .rect;
    ui.add_space(4.0);
    rect.expand2(vec2(0.0, 10.0))
}

fn row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    index: usize,
    place: Place,
) {
    let palette = &state.palette;
    let queue = &playback.session.queue;
    let track = &queue.items[index];
    // Only what is still to come can be carried to another place.
    let sense = match place {
        Place::Upcoming => Sense::click_and_drag(),
        Place::Played | Place::Playing => Sense::click(),
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), sense);
    widgets::name(ui, &response, &track.title);
    if place != Place::Playing {
        menu::context(&response, palette, |list| {
            menu(list, actions, playback, index, place);
        });
    }
    if response.drag_started() {
        response.dnd_set_drag_payload(Carried { from: index });
    }
    if place == Place::Upcoming {
        drop_target(state, ui, actions, &response, index);
    }
    if !ui.is_rect_visible(rect) {
        return;
    }
    // The button on the row takes the pointer from it, so the row counts
    // as hovered while the pointer is anywhere inside.
    let inside = ui.rect_contains_pointer(rect) && place != Place::Playing;
    let lift = widgets::hover_of(ui, response.id, inside);
    ui.painter()
        .rect_filled(rect, theme::RADIUS_ROW, widgets::wash(ui, lift));
    // A played song is dimmed until the pointer brings it back.
    let strength = match place {
        Place::Played => PLAYED + (1.0 - PLAYED) * lift,
        Place::Playing | Place::Upcoming => 1.0,
    };
    let middle = rect.center().y;
    let cover = Rect::from_center_size(
        pos2(rect.left() + 8.0 + COVER / 2.0, middle),
        vec2(COVER, COVER),
    );
    let shape = ArtShape::Rounded(4);
    widgets::artwork(ui, state, &track.artwork, cover, shape, Icon::Music);
    if strength < 1.0 {
        let veil = palette.panel.gamma_multiply(1.0 - strength);
        ui.painter().rect_filled(cover, 4.0, veil);
    }

    let left = cover.right() + 12.0;
    let width = (rect.right() - left - 52.0).max(20.0);
    let title_color = match place {
        Place::Playing => palette.accent,
        Place::Played | Place::Upcoming => palette.text.gamma_multiply(strength),
    };
    let title = widgets::elided(ui, &track.title, theme::medium(14.0), title_color, width, 1);
    ui.painter()
        .galley(pos2(left, middle - 18.0), title, title_color);
    let quiet = palette.secondary.gamma_multiply(strength);
    let artists = widgets::Artists {
        artists: &track.artists,
        font: theme::regular(12.5),
        color: quiet,
        width,
    };
    let id = response.id.with("artists");
    if let (Some(artist), _) = artists.show(ui, id, pos2(left, middle + 1.0)) {
        actions.push(Action::Open(Page::Artist(artist)));
        return;
    }

    let end = pos2(rect.right() - 22.0, middle);
    match place {
        Place::Playing => equalizer(ui, end, palette.accent, playback.is_playing()),
        Place::Played | Place::Upcoming if inside => {
            if row_button(state, ui, actions, playback, index, place, end) {
                return;
            }
        }
        Place::Played | Place::Upcoming if track.duration_ms > 0 => {
            widgets::text_at(
                ui,
                pos2(rect.right() - 10.0, middle),
                Align2::RIGHT_CENTER,
                &format::duration(track.duration_ms),
                theme::regular(12.5),
                quiet.gamma_multiply(1.0 - lift),
            );
        }
        Place::Played | Place::Upcoming => {}
    }
    // One click plays from here; what lay between is passed over.
    if response.clicked() {
        actions.push(Action::JumpTo(index));
    }
}

/// The button a row shows under the pointer: take it out of the queue, or
/// for a song that has played, play it again next. Returns whether it was
/// clicked, so the click is not also taken as one on the row.
fn row_button(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    playback: &Playback,
    index: usize,
    place: Place,
    at: egui::Pos2,
) -> bool {
    let track = &playback.session.queue.items[index];
    let (icon, tooltip, action) = match place {
        Place::Played => (
            Icon::ListMusic,
            "Play next",
            Action::PlayNext(vec![track.clone()]),
        ),
        Place::Playing | Place::Upcoming => {
            (Icon::X, "Remove from queue", Action::RemoveFromQueue(index))
        }
    };
    let button = widgets::IconButton {
        icon,
        size: 15.0,
        tooltip,
        active: false,
    };
    let clicked = ui
        .push_id(("queue-row", index), |ui| {
            button.show_at(ui, &state.palette, at).clicked()
        })
        .inner;
    if clicked {
        actions.push(action);
    }
    clicked
}

fn menu(
    menu: &mut Menu<'_>,
    actions: &mut Vec<Action>,
    playback: &Playback,
    index: usize,
    place: Place,
) {
    let queue = &playback.session.queue;
    if menu.item("Play now") {
        actions.push(Action::JumpTo(index));
    }
    let first = place == Place::Upcoming && index == queue.index + 1;
    if menu.entry(Entry::new("Play next").enabled(!first)) {
        actions.push(match place {
            // What has played is played again; what is to come is moved up.
            Place::Played => Action::PlayNext(vec![queue.items[index].clone()]),
            Place::Playing | Place::Upcoming => Action::MoveInQueue {
                from: index,
                to: queue.index + 1,
            },
        });
    }
    if place == Place::Upcoming {
        let last = index + 1 == queue.items.len();
        let up = Entry::new("Move up").icon(Icon::ChevronUp).enabled(!first);
        if menu.entry(up) {
            actions.push(Action::MoveInQueue {
                from: index,
                to: index - 1,
            });
        }
        let down = Entry::new("Move down")
            .icon(Icon::ChevronDown)
            .enabled(!last);
        if menu.entry(down) {
            actions.push(Action::MoveInQueue {
                from: index,
                to: index + 1,
            });
        }
    }
    menu.separator();
    if menu.item("Remove from queue") {
        actions.push(Action::RemoveFromQueue(index));
    }
}

/// Shows where a carried row would land on this one, and moves it there
/// when it is let go.
fn drop_target(
    state: &State,
    ui: &Ui,
    actions: &mut Vec<Action>,
    response: &egui::Response,
    index: usize,
) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let after = pointer.y > response.rect.center().y;
    if let Some(carried) = response.dnd_hover_payload::<Carried>()
        && landing(carried.from, index, after) != carried.from
    {
        let y = if after {
            response.rect.bottom()
        } else {
            response.rect.top()
        };
        let line = response.rect.shrink2(vec2(8.0, 0.0)).x_range();
        ui.painter().hline(line, y, (2.0, state.palette.accent));
    }
    if let Some(carried) = response.dnd_release_payload::<Carried>() {
        let to = landing(carried.from, index, after);
        if to != carried.from {
            actions.push(Action::MoveInQueue {
                from: carried.from,
                to,
            });
        }
    }
}

/// The place in the queue a row carried from `from` ends up at when it is
/// dropped before or after the row at `target`. Taking it out first moves
/// everything behind it up by one.
fn landing(from: usize, target: usize, after: bool) -> usize {
    let before = target + usize::from(after);
    if from < before { before - 1 } else { before }
}

/// Four bars that rise and fall while the music plays, and rest when it
/// is paused. Timed, not measured: it says "this one", not how loud.
fn equalizer(ui: &Ui, centre: egui::Pos2, color: Color32, playing: bool) {
    const BARS: usize = 4;
    const BAR: f32 = 2.0;
    const GAP: f32 = 1.5;
    const HEIGHT: f32 = 14.0;
    const BEAT_SECONDS: f64 = 0.9;
    let time = ui.input(|input| input.time);
    let width = BARS as f32 * BAR + (BARS - 1) as f32 * GAP;
    let left = centre.x - width / 2.0;
    let bottom = centre.y + HEIGHT / 2.0;
    for bar in 0..BARS {
        let level = if playing {
            // Each bar a sixth of a beat behind the one before it.
            let phase = time / BEAT_SECONDS - bar as f64 / 6.0;
            let swing = (phase * std::f64::consts::TAU).sin() as f32 * 0.5 + 0.5;
            0.25 + 0.75 * swing
        } else {
            0.3
        };
        let x = left + bar as f32 * (BAR + GAP);
        let rect = Rect::from_min_max(pos2(x, bottom - HEIGHT * level), pos2(x + BAR, bottom));
        ui.painter().rect_filled(rect, 1.0, color);
    }
    if playing {
        ui.ctx().request_repaint_after(EQUALIZER_REDRAW);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_dropped_lands_where_the_line_was() {
        // Carried down: taking it out first moves the target up by one.
        assert_eq!(landing(2, 5, false), 4);
        assert_eq!(landing(2, 5, true), 5);
        // Carried up: the target has not moved.
        assert_eq!(landing(5, 2, false), 2);
        assert_eq!(landing(5, 2, true), 3);
    }

    #[test]
    fn dropping_a_row_beside_itself_leaves_it_where_it_is() {
        assert_eq!(landing(3, 3, false), 3);
        assert_eq!(landing(3, 3, true), 3);
        assert_eq!(landing(3, 2, true), 3);
        assert_eq!(landing(3, 4, false), 3);
    }
}
