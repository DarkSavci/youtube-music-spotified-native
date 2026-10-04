//! Album, playlist, artist, podcast and mix pages: a hero, a row of
//! buttons, then what the page holds.

mod about;
mod artist;
mod hero;
pub mod songs;

use eframe::egui::{self, Rect, Sense, Ui, vec2};
use spotified_client::models::{Album, Artwork, Playlist, Podcast};

use super::actions_menu::Entity;
use super::pages::{Skeleton, loaded_or};
use super::widgets::{self, ArtShape};
use super::{browse, format, page_card, tracks};
use crate::actions::Action;
use crate::share;
use crate::state::{Loadable, Page, State};
use crate::theme::{self, Icon};
use hero::{Hero, Part, Plays};

pub use artist::artist;

/// What stands in for a page of songs while it loads.
const LOADING: Skeleton = Skeleton::Tracks(8);
/// How near the end of the songs read so far must come to the bottom of
/// the page before the next are asked for.
const MORE_AHEAD: f32 = 600.0;

/// The tint for the open page, from its hero's cover.
fn page_tint(state: &State, ui: &Ui, art: &[Artwork]) -> Option<egui::Color32> {
    // The hero draws its cover at one of two sizes; the tint is of
    // whichever is on screen.
    [hero::COVER, hero::COVER_NARROW]
        .into_iter()
        .find_map(|width| widgets::artwork_tint(ui, state, art, width))
}

fn songs_and_length(count: usize, length: String) -> [Part; 2] {
    [Part::plain(format::songs(count)), Part::plain(length)]
}

/// A track table under a row of buttons that may have stuck to the top of
/// the page: once the rest is drawn, the buttons are drawn over it, and
/// the table's header under them, so both stay in sight.
struct Listing<'a> {
    entity: Entity<'a>,
    tint: Option<egui::Color32>,
    columns: tracks::Columns,
    editable_playlist: Option<&'a str>,
    mode: tracks::Mode<'a>,
}

impl Listing<'_> {
    /// `after` draws what the page holds below its songs. It is drawn
    /// before the stuck buttons, which must lie over all of the page.
    fn show(
        &self,
        state: &State,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
        empty: (&str, &str),
        after: impl FnOnce(&mut Ui, &mut Vec<Action>),
    ) {
        let bar = hero::actions(state, ui, actions, &self.entity);
        let mut header = None;
        if self.entity.tracks.is_empty() {
            let palette = &state.palette;
            widgets::empty_state(ui, palette, Icon::Music, empty.0, empty.1);
        } else {
            let list = tracks::List {
                tracks: self.entity.tracks,
                origin: self.entity.title,
                editable_playlist: self.editable_playlist,
                columns: self.columns,
                mode: self.mode,
            };
            let table = tracks::table(state, ui, actions, list);
            // The table reaches to where the page's content has got to.
            header = Some((table, ui.cursor().top()));
        }
        after(ui, actions);
        let Some(bar) = bar else {
            return;
        };
        // The header follows the buttons down the page until the table's
        // last row is about to leave.
        if let Some((natural, table_bottom)) = header {
            let pinned_bottom = bar.bottom() + tracks::HEADER_HEIGHT;
            if natural.top() < bar.bottom() && table_bottom > pinned_bottom {
                let card = page_card(ui);
                let across = Rect::from_min_size(
                    egui::pos2(card.left(), bar.bottom()),
                    vec2(card.width(), tracks::HEADER_HEIGHT),
                );
                ui.interact(across, ui.id().with("stuck-header"), Sense::click());
                ui.painter().rect_filled(across, 0.0, state.palette.panel);
                let place = Rect::from_x_y_ranges(natural.x_range(), across.y_range());
                let last = tracks::last_column(self.entity.tracks);
                tracks::header(state, ui, place, self.columns, last);
            }
        }
        hero::stuck(state, ui, actions, &self.entity, (bar, self.tint));
    }
}

pub fn mix(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, id: &str) {
    let Some(mix) = state.mixes.iter().find(|mix| mix.id == id) else {
        widgets::empty_state(
            ui,
            &state.palette,
            Icon::ListMusic,
            "This mix is not here",
            "Mixes are made again as you listen.",
        );
        return;
    };
    // A mix has no cover of its own; its first song's stands in.
    let cover = mix.tracks.first().map_or(&[][..], |track| &track.artwork);
    let duration: u64 = mix.tracks.iter().map(|track| track.duration_ms).sum();
    let tint = page_tint(state, ui, cover);
    let mut byline = vec![Part::plain(mix.description.as_str())];
    byline.extend(songs_and_length(
        mix.tracks.len(),
        format::duration(duration),
    ));
    hero::show(
        state,
        ui,
        actions,
        Hero {
            art: cover,
            shape: ArtShape::Rounded(4),
            placeholder: Icon::ListMusic,
            kind: "Mix",
            title: &mix.title,
            avatar: &[],
            byline,
            tint,
        },
    );
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        hero::play_button(state, ui, actions, Plays::Tracks(&mix.tracks), &mix.title);
    });
    ui.add_space(12.0);
    let list = tracks::List {
        tracks: &mix.tracks,
        origin: &mix.title,
        editable_playlist: None,
        columns: tracks::Columns {
            cover: true,
            album: true,
        },
        mode: tracks::Mode::List,
    };
    tracks::table(state, ui, actions, list);
}

pub fn album(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Album>) {
    loaded_or(state, ui, page, LOADING, |ui, album| {
        let count = album.tracks.len().max(album.track_count as usize);
        let tint = page_tint(state, ui, &album.artwork);
        // Each artist leads to their page.
        let mut byline: Vec<Part> = album
            .artists
            .iter()
            .filter(|artist| !artist.name.is_empty())
            .map(|artist| {
                let page = (!artist.id.is_empty()).then(|| Page::Artist(artist.id.clone()));
                Part::strong(artist.name.as_str(), page)
            })
            .collect();
        byline.push(Part::plain(album.year.as_str()));
        if count > 0 {
            byline.push(Part::plain(format::songs(count)));
        }
        if album.duration_ms > 0 {
            byline.push(Part::plain(format::release_length(album.duration_ms)));
        }
        hero::show(
            state,
            ui,
            actions,
            Hero {
                art: &album.artwork,
                shape: ArtShape::Rounded(4),
                placeholder: Icon::Music,
                kind: if album.kind.is_empty() {
                    "Album"
                } else {
                    &album.kind
                },
                title: &album.title,
                avatar: &album.artist_artwork,
                byline,
                tint,
            },
        );
        let listing = Listing {
            entity: Entity {
                kind: share::Kind::Album,
                id: &album.id,
                title: &album.title,
                tracks: &album.tracks,
                deletable: false,
            },
            tint,
            columns: tracks::Columns {
                cover: false,
                album: false,
            },
            editable_playlist: None,
            mode: tracks::Mode::List,
        };
        let empty = ("No tracks", "This album returned no playable tracks.");
        listing.show(state, ui, actions, empty, |ui, actions| {
            if !album.description.is_empty() {
                about::show(state, ui, actions, &album.description, "");
            }
            // The rows YouTube puts under an album: "Releases for you".
            browse::shelves(state, ui, actions, &album.shelves, "album");
        });
    });
}

pub fn playlist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Playlist>) {
    loaded_or(state, ui, page, LOADING, |ui, playlist| {
        let count = playlist.tracks.len().max(playlist.track_count as usize);
        let tint = page_tint(state, ui, &playlist.artwork);
        let mut byline = vec![
            Part::strong(playlist.owner.as_str(), None),
            Part::plain(format::songs(count)),
        ];
        if playlist.duration_ms > 0 {
            byline.push(Part::plain(format::duration(playlist.duration_ms)));
        }
        hero::show(
            state,
            ui,
            actions,
            Hero {
                art: &playlist.artwork,
                shape: ArtShape::Rounded(4),
                placeholder: Icon::ListMusic,
                kind: "Playlist",
                title: &playlist.title,
                avatar: &[],
                byline,
                tint,
            },
        );
        let listing = Listing {
            entity: Entity {
                kind: share::Kind::Playlist,
                id: &playlist.id,
                title: &playlist.title,
                tracks: &playlist.tracks,
                deletable: playlist.editable,
            },
            tint,
            columns: tracks::Columns {
                cover: true,
                album: true,
            },
            editable_playlist: playlist.editable.then_some(playlist.id.as_str()),
            mode: tracks::Mode::Playlist(&playlist.id),
        };
        let empty = ("This playlist is empty", "Find something to add to it.");
        listing.show(state, ui, actions, empty, |ui, actions| {
            more_songs(state, ui, actions, playlist);
        });
    });
}

pub fn podcast(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Podcast>) {
    loaded_or(state, ui, page, LOADING, |ui, podcast| {
        let episodes = match podcast.episodes.len() {
            0 => String::new(),
            1 => "1 episode".to_owned(),
            count => format!("{count} episodes"),
        };
        let tint = page_tint(state, ui, &podcast.artwork);
        hero::show(
            state,
            ui,
            actions,
            Hero {
                art: &podcast.artwork,
                shape: ArtShape::Rounded(4),
                placeholder: Icon::MicVocal,
                kind: "Podcast",
                title: &podcast.title,
                avatar: &[],
                byline: vec![
                    Part::strong(podcast.author.as_str(), None),
                    Part::plain(episodes),
                ],
                tint,
            },
        );
        let entity = Entity {
            kind: share::Kind::Podcast,
            id: &podcast.id,
            title: &podcast.title,
            tracks: &podcast.episodes,
            deletable: false,
        };
        actions_row(ui, |ui| {
            let episodes = Plays::Tracks(&podcast.episodes);
            hero::play_button(state, ui, actions, episodes, &podcast.title);
            hero::more_button(state, ui, actions, &entity);
        });
        if !podcast.description.is_empty() {
            about::text(state, ui, &podcast.description, false);
            ui.add_space(16.0);
        }
        if podcast.episodes.is_empty() {
            let text = "This show has no episodes we can read.";
            widgets::empty_state(ui, &state.palette, Icon::MicVocal, "No episodes", text);
            return;
        }
        let list = tracks::List {
            tracks: &podcast.episodes,
            origin: &podcast.title,
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: false,
            },
            mode: tracks::Mode::List,
        };
        tracks::table(state, ui, actions, list);
    });
}

/// A row of buttons that scrolls with the page: room above, the buttons
/// with a gap between them, and a little room below.
fn actions_row(ui: &mut Ui, buttons: impl FnOnce(&mut Ui)) {
    ui.add_space(24.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        buttons(ui);
    });
    ui.add_space(12.0);
}

/// The end of a playlist read a page at a time: the next page asked for
/// as the end of what is here comes near, and a button that says where
/// that has got to and asks again when it failed.
fn more_songs(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, playlist: &Playlist) {
    let palette = &state.palette;
    let preparing = state
        .preparing_playlist
        .as_ref()
        .is_some_and(|(id, _)| id == &playlist.id);
    if preparing {
        ui.add_space(16.0);
        widgets::loading(ui, palette, "Preparing the full playlist…");
    }
    let Some(tail) = state.playlist_tails.get(&playlist.id) else {
        return;
    };
    ui.add_space(24.0);
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        let label = match (&tail.failed, tail.loading) {
            (Some(_), _) => "Could not load more songs — retry",
            (None, true) => "Loading more songs…",
            (None, false) => "Load more songs",
        };
        ui.add_enabled_ui(!tail.loading, |ui| {
            if widgets::outline_button(ui, palette, label).clicked() {
                actions.push(Action::MorePlaylist {
                    id: playlist.id.clone(),
                    retry: true,
                });
            }
        });
        let loaded = format!("{} songs loaded", playlist.tracks.len());
        ui.label(
            egui::RichText::new(loaded)
                .font(theme::regular(14.0))
                .color(palette.secondary),
        );
    });
    ui.add_space(24.0);
    // Asked for a little ahead of being seen, so scrolling does not stop
    // at the end of each page.
    let near = ui.clip_rect().expand2(vec2(0.0, MORE_AHEAD));
    if tail.idle() && near.intersects(row.response.rect) {
        actions.push(Action::MorePlaylist {
            id: playlist.id.clone(),
            retry: false,
        });
    }
}
