//! Your listening: what has been played most, from the plays this app has
//! recorded on this computer.

use eframe::egui::{self, Align2, Sense, Ui, pos2, vec2};
use spotified_client::models::Stats;

use super::pages::loaded;
use super::{cards, widgets};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon};

const ROW_HEIGHT: f32 = 44.0;
/// The periods on offer, in days, and what each is called.
const PERIODS: [(u32, &str); 4] = [
    (7, "Last week"),
    (30, "Last month"),
    (365, "Last year"),
    (3650, "All time"),
];

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.label(egui::RichText::new("Your listening").font(theme::bold(28.0)));
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (days, label) in PERIODS {
            let active = state.stats_days == days;
            if widgets::chip(ui, &state.palette, label, active).clicked() && !active {
                actions.push(Action::SetStatsPeriod(days));
            }
        }
    });
    loaded(state, ui, &state.stats, |ui, stats| {
        if stats.tracks.is_empty() && stats.artists.is_empty() {
            widgets::empty_state(
                ui,
                &state.palette,
                Icon::Music,
                "Nothing played yet",
                "Songs you play here are counted from now on.",
            );
            return;
        }
        songs(state, ui, stats);
        artists(state, ui, actions, stats);
    });
}

fn plays(count: u32) -> String {
    match count {
        1 => "1 play".to_owned(),
        count => format!("{count} plays"),
    }
}

/// A ranked line: its place, a name, a second line, and how often.
fn row(state: &State, ui: &mut Ui, rank: usize, name: &str, detail: &str, count: u32) -> bool {
    let palette = &state.palette;
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    widgets::name(ui, &response, name);
    widgets::row_hover(ui, &response, rect);
    let middle = rect.center().y;
    widgets::text_at(
        ui,
        pos2(rect.left() + 24.0, middle),
        Align2::CENTER_CENTER,
        &rank.to_string(),
        theme::regular(14.0),
        palette.secondary,
    );
    let left = rect.left() + 52.0;
    let width = (rect.width() - 52.0 - 110.0).max(40.0);
    let (name_y, has_detail) = if detail.is_empty() {
        (middle - 9.0, false)
    } else {
        (middle - 17.0, true)
    };
    let title = widgets::elided(ui, name, theme::medium(14.0), palette.text, width, 1);
    ui.painter().galley(pos2(left, name_y), title, palette.text);
    if has_detail {
        let line = widgets::elided(
            ui,
            detail,
            theme::regular(12.5),
            palette.secondary,
            width,
            1,
        );
        ui.painter()
            .galley(pos2(left, middle + 1.0), line, palette.secondary);
    }
    widgets::text_at(
        ui,
        pos2(rect.right() - 8.0, middle),
        Align2::RIGHT_CENTER,
        &plays(count),
        theme::regular(13.0),
        palette.secondary,
    );
    response.clicked()
}

fn songs(state: &State, ui: &mut Ui, stats: &Stats) {
    if stats.tracks.is_empty() {
        return;
    }
    cards::section_title(ui, "Top songs");
    for (index, track) in stats.tracks.iter().enumerate() {
        row(
            state,
            ui,
            index + 1,
            &track.title,
            &track.artist,
            track.plays,
        );
    }
}

fn artists(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, stats: &Stats) {
    if stats.artists.is_empty() {
        return;
    }
    cards::section_title(ui, "Top artists");
    for (index, artist) in stats.artists.iter().enumerate() {
        let clicked = row(state, ui, index + 1, &artist.artist, "", artist.plays);
        if clicked && !artist.artist_id.is_empty() {
            actions.push(Action::Open(Page::Artist(artist.artist_id.clone())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_play_is_singular() {
        assert_eq!(plays(1), "1 play");
        assert_eq!(plays(12), "12 plays");
    }
}
