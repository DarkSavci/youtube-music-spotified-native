//! Your listening.
//!
//! Built entirely from the plays this app has recorded on this computer,
//! which is why it can exist at all: nothing here comes from YouTube Music
//! but the artists' pictures, and nothing here is to be had anywhere else.

mod detail;
mod lookup;

use eframe::egui::{self, Align2, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{AlbumStat, StatKind, Stats, Summary, channel_id};

use super::pages::{Skeleton, loaded_or};
use super::widgets::{self, ArtShape};
use super::{cards, tracks};
use crate::actions::Action;
use crate::state::{Page, State};
use crate::theme::{self, Icon};

/// The periods on offer, in days, and what each is called.
const PERIODS: [(u32, &str); 4] = [
    (7, "Last week"),
    (30, "Last month"),
    (365, "Last year"),
    (3650, "All time"),
];
/// How many artists have their own pictures looked up: each is a whole
/// artist page from YouTube, so only the first row's worth.
const PHOTOS: usize = 6;
const TILE_MIN: f32 = 140.0;
const TILE_GAP: f32 = 12.0;
const ALBUM_ROW: f32 = 64.0;
const ALBUM_ART: f32 = 48.0;
/// What a queue played from the top tracks is called.
const TOP_ORIGIN: &str = "Your top tracks";

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Your listening").font(theme::bold(28.0)));
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (days, label) in PERIODS {
            let active = state.stats_days == days;
            if widgets::chip(ui, &state.palette, label, active).clicked() && !active {
                actions.push(Action::SetStatsPeriod(days));
            }
        }
    });
    ui.add_space(20.0);
    loaded_or(
        state,
        ui,
        &state.stats,
        Skeleton::Shelves(2),
        |ui, stats| {
            tiles(state, ui, &summary_tiles(&stats.summary), false);
            ui.add_space(20.0);
            lookup::show(state, ui, actions);
            ui.add_space(20.0);
            detail::show(state, ui, actions);
            if stats.tracks.is_empty() {
                // Expected on a fresh install, so it says what will fill it.
                let text = "Play something and it will show up here. Your listening history stays \
                        on this machine.";
                let palette = &state.palette;
                widgets::empty_state(ui, palette, Icon::Music, "Nothing tracked yet", text);
                return;
            }
            sections(state, ui, actions, stats);
        },
    );
}

fn sections(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, stats: &Stats) {
    let page = &state.stats_page;
    let table = |ui: &mut Ui, actions: &mut Vec<Action>, tracks, origin| {
        let list = tracks::List {
            tracks,
            origin,
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: false,
            },
            mode: tracks::Mode::List,
        };
        tracks::table(state, ui, actions, list);
    };
    if !page.on_repeat.is_empty() {
        ui.push_id("on-repeat", |ui| {
            cards::noted_title(state, ui, "On repeat", "LAST 30 DAYS");
            table(ui, actions, &page.on_repeat, "On repeat");
        });
    }
    top_artists(state, ui, actions, stats);
    ui.push_id("top-tracks", |ui| {
        if cards::title_with(state, ui, "Top tracks", Some("Play all")) {
            actions.push(Action::Play {
                tracks: page.top.clone(),
                index: 0,
                origin: TOP_ORIGIN.to_owned(),
            });
        }
        table(ui, actions, &page.top, TOP_ORIGIN);
    });
    top_albums(state, ui, actions, &stats.albums);
}

/// "1,204".
pub(super) fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// "12 plays".
pub(super) fn plural(count: u32, word: &str) -> String {
    let count_text = grouped(u64::from(count));
    if count == 1 {
        format!("{count_text} {word}")
    } else {
        format!("{count_text} {word}s")
    }
}

/// How long was listened: minutes under an hour, then hours.
pub(super) fn listened(ms: u64) -> String {
    let minutes = (ms as f64 / 60_000.0).round();
    if minutes < 60.0 {
        return format!("{minutes} min");
    }
    let hours = minutes / 60.0;
    if hours < 100.0 {
        format!("{hours:.1} hrs")
    } else {
        format!("{} hrs", grouped(hours.round() as u64))
    }
}

fn summary_tiles(summary: &Summary) -> [(&'static str, String); 5] {
    let count = |number: u32| grouped(u64::from(number));
    [
        ("Listening time", listened(summary.total_ms)),
        ("Plays", count(summary.plays)),
        ("Songs", count(summary.distinct_tracks)),
        ("Artists", count(summary.distinct_artists)),
        ("Albums", count(summary.distinct_albums)),
    ]
}

/// Figures as a row of tiles: a name over a number. As many to a line as
/// fit, sharing it evenly. The compact ones sit inside a detail panel.
pub(super) fn tiles(state: &State, ui: &mut Ui, tiles: &[(&str, String)], compact: bool) {
    let palette = &state.palette;
    let room = ui.available_width();
    let fit = ((room + TILE_GAP) / (TILE_MIN + TILE_GAP)).floor().max(1.0);
    let columns = (fit as usize).min(tiles.len()).max(1);
    let width = ((room - TILE_GAP * (columns as f32 - 1.0)) / columns as f32).floor();
    let (padding, value_size, fill) = if compact {
        (12.0, 18.0, palette.panel)
    } else {
        (16.0, 22.0, palette.surface)
    };
    let height = padding * 2.0 + 18.0 + 4.0 + value_size + 6.0;
    let lines = tiles.len().div_ceil(columns);
    let size = vec2(room, lines as f32 * (height + TILE_GAP) - TILE_GAP);
    let (area, _) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(area) {
        return;
    }
    for (index, (label, value)) in tiles.iter().enumerate() {
        let at = pos2(
            area.left() + (index % columns) as f32 * (width + TILE_GAP),
            area.top() + (index / columns) as f32 * (height + TILE_GAP),
        );
        let tile = Rect::from_min_size(at, vec2(width, height));
        ui.painter().rect_filled(tile, theme::RADIUS, fill);
        let inner = tile.shrink(padding);
        let label = widgets::elided(
            ui,
            label,
            theme::regular(13.0),
            palette.secondary,
            inner.width(),
            1,
        );
        ui.painter().galley(inner.min, label, palette.secondary);
        let font = theme::bold(value_size);
        let value = widgets::elided(ui, value, font, palette.text, inner.width(), 1);
        let at = pos2(inner.left(), inner.top() + 22.0);
        ui.painter().galley(at, value, palette.text);
    }
}

/// The most played artists as cards with their pictures. The play log
/// has no artist pictures, so the first few are looked up; until one
/// arrives, and for an artist that has none, the card shows the cover of
/// one of their songs rather than an empty circle.
fn top_artists(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, stats: &Stats) {
    // Only artists with a channel have a page for a card to open.
    let shown: Vec<usize> = (0..stats.artists.len())
        .filter(|index| channel_id(&stats.artists[*index].artist_id).is_some())
        .collect();
    if shown.is_empty() {
        return;
    }
    cards::section_title(ui, "Top artists");
    cards::row(ui, shown.len(), |ui, place| {
        let index = shown[place];
        let artist = &stats.artists[index];
        let photo = state.artist_photos.get(&artist.artist_id);
        if photo.is_none() && place < PHOTOS {
            actions.push(Action::WantArtistPhoto(artist.artist_id.clone()));
        }
        let cover = state.stats_page.artist_covers.get(index);
        let art = photo
            .and_then(Option::as_ref)
            .filter(|photo| !photo.is_empty())
            .or(cover)
            .map_or(&[][..], Vec::as_slice);
        // The card's second line carries the listener's own figures.
        let figures = format!(
            "{} · {}",
            plural(artist.plays, "play"),
            listened(artist.total_ms)
        );
        let who = (artist.artist_id.as_str(), artist.artist.as_str());
        cards::artist_as(state, ui, actions, who, art, figures);
    });
}

fn top_albums(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, albums: &[AlbumStat]) {
    let palette = &state.palette;
    cards::section_title(ui, "Top albums");
    if albums.is_empty() {
        let text = "Albums are counted from now on: plays from before this version did not \
                    record which album a song came from.";
        ui.label(
            egui::RichText::new(text)
                .font(theme::regular(13.0))
                .color(palette.secondary),
        );
        return;
    }
    let size = vec2(ui.available_width(), tracks::HEADER_HEIGHT);
    let (head, _) = ui.allocate_exact_size(size, Sense::hover());
    let columns = AlbumColumns::of(head);
    let caption = |x: f32, anchor: Align2, text: &str| {
        let font = theme::regular(12.0);
        let galley = widgets::tracked(ui, text, font, palette.secondary, 0.7, (f32::MAX, 1));
        let at = anchor
            .anchor_size(pos2(x, head.center().y), galley.size())
            .min;
        ui.painter().galley(at, galley, palette.secondary);
    };
    caption(columns.rank, Align2::CENTER_CENTER, "#");
    caption(columns.main, Align2::LEFT_CENTER, "ALBUM");
    caption(columns.plays, Align2::RIGHT_CENTER, "PLAYS");
    caption(columns.time, Align2::RIGHT_CENTER, "TIME");
    ui.painter()
        .hline(head.x_range(), head.bottom(), (1.0, palette.outline));
    for (index, album) in albums.iter().enumerate() {
        album_row(state, ui, actions, (index, album));
    }
}

/// Where the columns of the album table sit.
struct AlbumColumns {
    rank: f32,
    main: f32,
    plays: f32,
    time: f32,
}

impl AlbumColumns {
    fn of(row: Rect) -> Self {
        Self {
            rank: row.left() + 32.0,
            main: row.left() + 64.0,
            plays: row.right() - 120.0,
            time: row.right() - 16.0,
        }
    }
}

/// One album: a click on the row opens the listener's figures for it, and
/// its name leads to the album itself.
fn album_row(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    (index, album): (usize, &AlbumStat),
) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), ALBUM_ROW);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    widgets::name(ui, &response, &format!("Your listening: {}", album.album));
    if !ui.is_rect_visible(rect) {
        return;
    }
    widgets::row_hover(ui, &response, rect);
    let columns = AlbumColumns::of(rect);
    let middle = rect.center().y;
    let quiet = |ui: &Ui, x: f32, anchor: Align2, text: &str| {
        let font = theme::regular(14.0);
        widgets::text_at(ui, pos2(x, middle), anchor, text, font, palette.secondary);
    };
    quiet(
        ui,
        columns.rank,
        Align2::CENTER_CENTER,
        &(index + 1).to_string(),
    );
    let art = Rect::from_center_size(
        pos2(columns.main + ALBUM_ART / 2.0, middle),
        vec2(ALBUM_ART, ALBUM_ART),
    );
    let cover = state
        .stats_page
        .album_covers
        .get(index)
        .map_or(&[][..], Vec::as_slice);
    widgets::artwork(ui, state, cover, art, ArtShape::Rounded(4), Icon::Music);
    let left = art.right() + 12.0;
    let width = (columns.plays - 72.0 - left).max(40.0);
    let name = widgets::Link {
        text: &album.album,
        font: theme::medium(14.0),
        color: palette.text,
        width,
    };
    let leads = !album.album_id.is_empty();
    let id = response.id.with("album");
    let mut opened = false;
    if name.show(ui, id, pos2(left, middle - 19.0), leads) {
        actions.push(Action::Open(Page::Album(album.album_id.clone())));
        opened = true;
    }
    let second = format!(
        "{} · {}",
        album.artist,
        plural(album.distinct_tracks, "song")
    );
    let font = theme::regular(12.0);
    let second = widgets::elided(ui, &second, font, palette.secondary, width, 1);
    ui.painter()
        .galley(pos2(left, middle + 2.0), second, palette.secondary);
    quiet(
        ui,
        columns.plays,
        Align2::RIGHT_CENTER,
        &grouped(u64::from(album.plays)),
    );
    quiet(
        ui,
        columns.time,
        Align2::RIGHT_CENTER,
        &listened(album.total_ms),
    );
    if response.clicked() && !opened {
        actions.push(Action::OpenStat {
            kind: StatKind::Album,
            id: album.key.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_are_grouped_and_counted_in_words() {
        assert_eq!(grouped(1_204), "1,204");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000_000), "1,000,000");
        assert_eq!(plural(1, "play"), "1 play");
        assert_eq!(plural(1_200, "play"), "1,200 plays");
    }

    #[test]
    fn listening_time_is_minutes_then_hours() {
        assert_eq!(listened(59 * 60_000), "59 min");
        assert_eq!(listened(90 * 60_000), "1.5 hrs");
        assert_eq!(listened(1_500 * 3_600_000), "1,500 hrs");
    }
}
