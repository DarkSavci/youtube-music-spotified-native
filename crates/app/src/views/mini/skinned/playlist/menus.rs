//! The menus behind the buttons along the playlist's bottom: ADD, REM,
//! SEL, MISC and LIST OPTS. Winamp's added files, sorted lists and wrote
//! them to disk; these do what a queue of songs from YouTube Music can.

use std::collections::BTreeSet;

use eframe::egui::{self, Sense};
use spotified_client::models::Track;
use spotified_client::session::{Queue, Repeat};

use super::super::super::super::widgets;
use super::super::{View, menu};
use super::Kept;
use crate::actions::Action;
use crate::settings::RightPanel;
use crate::share;
use crate::skin::layout::{self, Area};
use crate::state::{Page, Playback, State};

/// The rows to take out of a queue, last first, so that taking one out
/// does not move the ones still to go. What is playing is never among
/// them: it is not in the queue to be taken out of.
pub(super) fn removals(rows: impl IntoIterator<Item = usize>, playing: usize) -> Vec<usize> {
    let mut rows: Vec<usize> = rows.into_iter().filter(|row| *row != playing).collect();
    rows.sort_unstable_by(|a, b| b.cmp(a));
    rows.dedup();
    rows
}

pub(super) fn show(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
    kept: &mut Kept,
    height: u32,
) {
    let bottom = height - layout::PLAYLIST_BOTTOM_HEIGHT;
    let unit = view.unit;
    for (name, x) in layout::PLAYLIST_MENUS {
        let label = match name {
            "add" => "Add",
            "rem" => "Remove",
            "sel" => "Select",
            "misc" => "Miscellaneous",
            _ => "List options",
        };
        let button = view.interact(Area::new(x, bottom + 8, 22, 18), label, Sense::click());
        menu(egui::Popup::menu(&button), view.skin(), unit, |ui| {
            let Some(playback) = playback else {
                ui.label("Nothing is queued");
                return;
            };
            let queue = &playback.session.queue;
            match name {
                "add" => add(ui, actions, queue, kept),
                "rem" => remove(ui, actions, queue, kept),
                "sel" => select(ui, queue, kept),
                "misc" => miscellaneous(state, ui, actions, queue, kept),
                _ => list_options(state, ui, actions, playback),
            }
        });
    }
}

/// The songs selected, in the queue's order.
fn chosen(queue: &Queue, kept: &Kept) -> Vec<Track> {
    let rows = kept.selected.iter();
    rows.filter_map(|row| queue.items.get(*row).cloned())
        .collect()
}

fn add(ui: &mut egui::Ui, actions: &mut Vec<Action>, queue: &Queue, kept: &Kept) {
    if ui.button("Find something to add\u{2026}").clicked() {
        actions.push(Action::ShowMainWindow);
        actions.push(Action::FocusSearch);
    }
    let tracks = chosen(queue, kept);
    ui.add_enabled_ui(!tracks.is_empty(), |ui| {
        if ui.button("Play the selected next").clicked() {
            actions.push(Action::PlayNext(tracks.clone()));
        }
        if ui.button("Add the selected to the end").clicked() {
            actions.push(Action::AddToQueue(tracks.clone()));
        }
    });
}

fn remove(ui: &mut egui::Ui, actions: &mut Vec<Action>, queue: &Queue, kept: &mut Kept) {
    let playing = queue.index;
    let mut take = |rows: Vec<usize>, kept: &mut Kept| {
        actions.extend(rows.into_iter().map(Action::RemoveFromQueue));
        kept.selected.clear();
        kept.anchor = None;
    };
    let selected = removals(kept.selected.iter().copied(), playing);
    ui.add_enabled_ui(!selected.is_empty(), |ui| {
        if ui.button("Remove the selected").clicked() {
            take(selected.clone(), kept);
        }
        // Everything but the selection, and what is playing.
        if ui.button("Crop to the selected").clicked() {
            let others = (0..queue.items.len()).filter(|row| !kept.selected.contains(row));
            take(removals(others, playing), kept);
        }
    });
    let after = removals(playing + 1..queue.items.len(), playing);
    ui.add_enabled_ui(!after.is_empty(), |ui| {
        if ui.button("Remove all after this song").clicked() {
            take(after.clone(), kept);
        }
    });
}

fn select(ui: &mut egui::Ui, queue: &Queue, kept: &mut Kept) {
    let every = 0..queue.items.len();
    if ui.button("Select all").clicked() {
        kept.selected = every.clone().collect();
    }
    if ui.button("Select none").clicked() {
        kept.selected.clear();
    }
    if ui.button("Invert the selection").clicked() {
        let inverted: BTreeSet<usize> = every.filter(|row| !kept.selected.contains(row)).collect();
        kept.selected = inverted;
    }
}

/// About one song: the first selected, or else the one playing.
fn miscellaneous(
    state: &State,
    ui: &mut egui::Ui,
    actions: &mut Vec<Action>,
    queue: &Queue,
    kept: &Kept,
) {
    let row = kept.selected.first().copied().unwrap_or(queue.index);
    let Some(track) = queue.items.get(row) else {
        ui.label("Nothing is queued");
        return;
    };
    let album = track.album.as_ref().filter(|album| !album.id.is_empty());
    ui.add_enabled_ui(album.is_some(), |ui| {
        if ui.button("Go to the album").clicked()
            && let Some(album) = album
        {
            actions.push(Action::Open(Page::Album(album.id.clone())));
            actions.push(Action::ShowMainWindow);
        }
    });
    for artist in track.artists.iter().filter(|artist| !artist.id.is_empty()) {
        if ui.button(format!("Go to {}", artist.name)).clicked() {
            let id = widgets::artist_page_id(&artist.id).to_owned();
            actions.push(Action::Open(Page::Artist(id)));
            actions.push(Action::ShowMainWindow);
        }
    }
    if ui.button("Start a radio from it").clicked() {
        actions.push(Action::StartRadio(track.clone()));
    }
    let liked = state.likes.is_liked(&track.id);
    let like = if liked {
        "Remove from Liked Songs"
    } else {
        "Save to Liked Songs"
    };
    if ui.button(like).clicked() {
        actions.push(Action::ToggleLike(track.clone()));
    }
    if ui.button("Copy its link").clicked() {
        actions.push(Action::Share {
            kind: share::Kind::Track,
            id: track.id.clone(),
        });
    }
}

fn list_options(state: &State, ui: &mut egui::Ui, actions: &mut Vec<Action>, playback: &Playback) {
    let session = &playback.session;
    if ui.button("Open the queue in the app").clicked() {
        if state.settings.panel != RightPanel::Queue {
            actions.push(Action::ToggleQueue);
        }
        actions.push(Action::ShowMainWindow);
    }
    // The name is asked for in the main window, which comes forward.
    if ui.button("Save the queue as a playlist\u{2026}").clicked() {
        actions.push(Action::NewPlaylist {
            name: session.queue.origin.clone(),
            track_ids: session.queue.items.iter().map(|t| t.id.clone()).collect(),
        });
        actions.push(Action::ShowMainWindow);
    }
    ui.separator();
    let mut shuffle = session.shuffle;
    if ui.checkbox(&mut shuffle, "Shuffle").clicked() {
        actions.push(Action::ToggleShuffle);
    }
    let repeat = match session.repeat {
        Repeat::Off => "Repeat: off",
        Repeat::All => "Repeat: everything",
        Repeat::One => "Repeat: this song",
    };
    if ui.button(repeat).clicked() {
        actions.push(Action::CycleRepeat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_are_taken_out_last_first_and_never_the_one_playing() {
        assert_eq!(removals([1, 4, 2], 0), [4, 2, 1]);
        assert_eq!(removals([0, 1, 2, 3], 2), [3, 1, 0]);
        assert_eq!(removals([2], 2), Vec::<usize>::new());
        assert_eq!(removals(3..3, 2), Vec::<usize>::new());
    }
}
