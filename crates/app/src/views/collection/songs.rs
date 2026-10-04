//! Every song of an artist's: YouTube's most played, newest first, or
//! album by album. What it shows is worked out as the songs arrive
//! (`state::artist_songs`); this only draws it.

use eframe::egui::{self, Align, Layout, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::Artist;

use super::LOADING;
use super::hero::{self, Plays};
use crate::actions::Action;
use crate::state::artist_songs::GroupHead;
use crate::state::{ArtistSongs, Page, SongOrder, State};
use crate::theme::{self, Icon};
use crate::views::pages::loaded_or;
use crate::views::widgets::{self, ArtShape};
use crate::views::{skeleton, tracks};

const GROUP_ART: f32 = 48.0;
const GROUP_HEAD: f32 = 56.0;

pub fn show(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist_id: &str) {
    let page = state.artists.get(&artist_id.to_owned());
    loaded_or(state, ui, page, LOADING, |ui, artist| {
        if artist.songs_id.is_empty() {
            let text = "YouTube does not list every song for this artist.";
            let title = "No song list for this artist";
            widgets::empty_state(ui, &state.palette, Icon::Music, title, text);
            return;
        }
        let Some(songs) = state
            .artist_songs
            .as_ref()
            .filter(|songs| songs.artist_id == artist.id)
        else {
            skeleton::tracks(state, ui, 8);
            return;
        };
        heading(state, ui, actions, artist);
        let origin = format!("{}: all songs", artist.name);
        controls(state, ui, actions, songs, &origin);
        body(state, ui, actions, songs, &origin);
    });
}

/// The artist's name, which leads back to their page, over "Songs".
fn heading(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist: &Artist) {
    let palette = &state.palette;
    ui.add_space(8.0);
    let link = widgets::Link {
        text: &artist.name,
        font: theme::bold(14.0),
        color: palette.secondary,
        width: ui.available_width(),
    };
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    if link.show(ui, ui.id().with("songs-artist"), line.min, true) {
        actions.push(Action::Open(Page::Artist(artist.id.clone())));
    }
    ui.add_space(4.0);
    let title = widgets::tracked(
        ui,
        "Songs",
        theme::bold(32.0),
        palette.text,
        -0.64,
        (f32::MAX, 1),
    );
    let (rect, _) = ui.allocate_exact_size(title.size(), Sense::hover());
    ui.painter().galley(rect.min, title, palette.text);
}

/// Play, and at the right the orders to choose from.
fn controls(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    songs: &ArtistSongs,
    origin: &str,
) {
    ui.add_space(16.0);
    let size = vec2(ui.available_width(), 56.0);
    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
    let mut left = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(Layout::left_to_right(Align::Center)),
    );
    // What Play plays is the list as it is shown.
    hero::play_button(
        state,
        &mut left,
        actions,
        Plays::Tracks(&songs.shown),
        origin,
    );
    let mut right = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(Layout::right_to_left(Align::Center)),
    );
    right.spacing_mut().item_spacing.x = 8.0;
    for order in SongOrder::EVERY.into_iter().rev() {
        let active = songs.order == order;
        let chip = widgets::chip(&mut right, &state.palette, order.label(), active);
        if chip.clicked() && !active {
            actions.push(Action::SetSongOrder(order));
        }
    }
    ui.add_space(12.0);
}

fn body(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, songs: &ArtistSongs, origin: &str) {
    let palette = &state.palette;
    let list = songs.list.as_ref();
    if songs.listed.is_empty() {
        match list.and_then(|tail| tail.failed.as_deref()) {
            Some(why) => widgets::error(ui, palette, why),
            None => skeleton::tracks(state, ui, 8),
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.label(
            egui::RichText::new(&songs.status)
                .font(theme::regular(14.0))
                .color(palette.secondary),
        );
        if songs.can_open > 0 {
            let label = match (songs.only_retry, songs.can_open) {
                (true, _) => "Try again".to_owned(),
                (false, 1) => "Open 1 more release".to_owned(),
                (false, count) => format!("Open {count} more releases"),
            };
            if widgets::chip(ui, palette, &label, false).clicked() {
                actions.push(Action::OpenMoreReleases);
            }
        }
    });
    ui.add_space(12.0);
    if songs.order != SongOrder::Album {
        let list = tracks::List {
            tracks: &songs.shown,
            origin,
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: true,
            },
            mode: tracks::Mode::List,
        };
        tracks::table(state, ui, actions, list);
        return;
    }
    for (index, group) in songs.groups.iter().enumerate() {
        ui.push_id(("album-group", index), |ui| {
            ui.add_space(if index == 0 { 0.0 } else { 24.0 });
            group_head(state, ui, actions, group);
            ui.add_space(8.0);
            // An album's own songs need neither its cover nor its name
            // beside each of them.
            let list = tracks::List {
                tracks: &songs.shown[group.songs.clone()],
                origin,
                editable_playlist: None,
                columns: tracks::Columns {
                    cover: false,
                    album: false,
                },
                mode: tracks::Mode::Within(&songs.shown, group.songs.start),
            };
            tracks::table(state, ui, actions, list);
        });
    }
}

/// An album's cover, its name, which leads to its page, and when it is
/// from and how many of its songs are here.
fn group_head(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, group: &GroupHead) {
    let palette = &state.palette;
    let size = vec2(ui.available_width(), GROUP_HEAD);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let art = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - GROUP_ART / 2.0),
        vec2(GROUP_ART, GROUP_ART),
    );
    let shape = ArtShape::Rounded(4);
    widgets::artwork(ui, state, &group.art, art, shape, Icon::Music);
    let left = art.right() + 12.0;
    let width = (rect.right() - left).max(40.0);
    let title = widgets::Link {
        text: &group.title,
        font: theme::bold(16.0),
        color: palette.text,
        width,
    };
    let at = pos2(left, rect.center().y - 21.0);
    let leads = !group.id.is_empty();
    if title.show(ui, ui.id().with("group-title"), at, leads) {
        actions.push(Action::Open(Page::Album(group.id.clone())));
    }
    let detail = widgets::elided(
        ui,
        &group.detail,
        theme::regular(14.0),
        palette.secondary,
        width,
        1,
    );
    ui.painter()
        .galley(pos2(left, rect.center().y + 2.0), detail, palette.secondary);
}
