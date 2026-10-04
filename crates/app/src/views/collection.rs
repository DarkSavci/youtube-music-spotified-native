//! Album, playlist and artist pages: a hero, then what it holds.

use eframe::egui::{self, Rect, Sense, Ui, pos2, vec2};
use spotified_client::models::{
    Album, Artist, Artwork, BrowseLink, MixSeed, Playlist, Podcast, Track, join_names,
};

use super::format::{self, bulleted};
use super::pages::loaded;
use super::widgets::{self, ArtShape};
use super::{cards, tracks};
use crate::actions::Action;
use crate::state::{Loadable, Page, State, Surface};
use crate::theme::{self, Icon};

const COVER: f32 = 212.0;
const PLAY_BUTTON: f32 = 56.0;
const COVER_NARROW: f32 = 160.0;
/// At this page width and below, the cover and title shrink.
const NARROW_PAGE: f32 = 720.0;
const TITLE_SIZES: std::ops::RangeInclusive<u8> = 22..=56;
/// How many of an artist's top songs show under "Popular".
const POPULAR: usize = 5;
/// A biography or a show's notes are read as a column, not across the window.
const ABOUT_WIDTH: f32 = 680.0;

/// The tint for the open page, from its hero's cover.
pub fn page_tint(state: &State, ui: &Ui) -> Option<egui::Color32> {
    let art = match state.nav.page() {
        Page::Album(id) => &loaded_page(state.albums.get(id))?.artwork,
        Page::Playlist(id) => &loaded_page(state.playlists.get(id))?.artwork,
        Page::Artist(id) => &loaded_page(state.artists.get(id))?.artwork,
        Page::Podcast(id) => &loaded_page(state.podcasts.get(id))?.artwork,
        // A mix has no cover of its own; its first song's stands in.
        Page::Mix(id) => {
            let mix = state.mixes.iter().find(|mix| &mix.id == id)?;
            &mix.tracks.first()?.artwork
        }
        Page::Home
        | Page::Search
        | Page::Settings
        | Page::Stats
        | Page::Browse(_)
        | Page::History
        | Page::Changelog
        | Page::Together => return None,
    };
    // The hero draws its cover at one of two sizes; the tint is of
    // whichever is on screen.
    [COVER, COVER_NARROW]
        .into_iter()
        .find_map(|width| widgets::artwork_tint(ui, state, art, width))
}

fn loaded_page<T>(page: &Loadable<T>) -> Option<&T> {
    match page {
        Loadable::Loaded(page) => Some(page),
        _ => None,
    }
}

struct Hero<'a> {
    art: &'a [Artwork],
    shape: ArtShape,
    placeholder: Icon,
    kind: &'a str,
    title: &'a str,
    byline: String,
}

fn hero(state: &State, ui: &mut Ui, hero: Hero<'_>) {
    let palette = &state.palette;
    let narrow = ui.available_width() <= NARROW_PAGE;
    let cover = if narrow { COVER_NARROW } else { COVER };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), cover), Sense::hover());
    let image = Rect::from_min_size(rect.min, vec2(cover, cover));
    widgets::artwork(ui, state, hero.art, image, hero.shape, hero.placeholder);

    let left = image.right() + 24.0;
    let width = (rect.right() - left).max(80.0);
    // The largest size at which the title fits on one line; a title too
    // long even for the smallest is cut there.
    let largest = if narrow { 40 } else { *TITLE_SIZES.end() };
    let title = (*TITLE_SIZES.start()..=largest)
        .rev()
        .step_by(6)
        .map(|size| {
            let font = theme::bold(f32::from(size));
            ui.painter()
                .layout_no_wrap(hero.title.to_owned(), font, palette.text)
        })
        .find(|galley| galley.size().x <= width)
        .unwrap_or_else(|| {
            let font = theme::bold(f32::from(*TITLE_SIZES.start()));
            widgets::elided(ui, hero.title, font, palette.text, width, 1)
        });

    // Stacked upwards from the cover's bottom edge: byline, title, kind.
    let byline_top = rect.bottom() - 20.0;
    let title_top = byline_top - 8.0 - title.size().y;
    let byline = widgets::elided(
        ui,
        &hero.byline,
        theme::regular(13.5),
        palette.secondary,
        width,
        1,
    );
    ui.painter()
        .galley(pos2(left, byline_top), byline, palette.secondary);
    ui.painter()
        .galley(pos2(left, title_top), title, palette.text);
    widgets::text_at(
        ui,
        pos2(left, title_top - 4.0),
        egui::Align2::LEFT_BOTTOM,
        hero.kind,
        theme::medium(12.5),
        palette.text,
    );
    ui.add_space(20.0);
}

/// YouTube gives the subscriber count as a bare number ("7.17M").
fn subscribers(count: &str) -> String {
    if count.is_empty() || count.contains(' ') {
        count.to_owned()
    } else {
        format!("{count} subscribers")
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
    let cover = mix.tracks.first().map_or(&[][..], |track| &track.artwork);
    let duration: u64 = mix.tracks.iter().map(|track| track.duration_ms).sum();
    hero(
        state,
        ui,
        Hero {
            art: cover,
            shape: ArtShape::Rounded(theme::RADIUS_ROW),
            placeholder: Icon::ListMusic,
            kind: "Mix",
            title: &mix.title,
            byline: bulleted([mix.description.as_str(), &total(mix.tracks.len(), duration)]),
        },
    );
    play_button(state, ui, actions, &mix.tracks, &mix.title);
    let list = tracks::List {
        tracks: &mix.tracks,
        origin: &mix.title,
        editable_playlist: None,
        columns: tracks::Columns {
            cover: true,
            album: true,
        },
    };
    tracks::table(state, ui, actions, list);
}

/// The big button under a hero: plays the collection from the top, or
/// pauses it when it is what is playing.
fn play_button(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    tracks: &[Track],
    origin: &str,
) {
    let Some(first) = tracks.iter().position(|track| track.playable) else {
        return;
    };
    let palette = &state.palette;
    let playing_this = state.playback.as_ref().is_some_and(|playback| {
        playback.wants_to_play() && playback.session.queue.origin == origin
    });
    let (rect, response) = ui.allocate_exact_size(vec2(PLAY_BUTTON, PLAY_BUTTON), Sense::click());
    let label = if playing_this {
        "Pause".to_owned()
    } else {
        format!("Play {origin}")
    };
    widgets::name(ui, &response, &label);
    let lift = widgets::hover(ui, &response);
    let fill = crate::tint::blend(palette.accent, palette.accent_hover, lift);
    // It swells a little under the pointer and gives when pressed.
    let pressed = if response.is_pointer_button_down_on() {
        0.95
    } else {
        1.0 + 0.05 * lift
    };
    ui.painter()
        .circle_filled(rect.center(), PLAY_BUTTON / 2.0 * pressed, fill);
    let icon = if playing_this {
        Icon::PauseFilled
    } else {
        Icon::PlayFilled
    };
    widgets::paint_icon(ui, icon, rect, 22.0, palette.on_accent);
    if response.on_hover_text(label).clicked() {
        actions.push(if playing_this {
            Action::TogglePlay
        } else {
            Action::Play {
                tracks: tracks.to_vec(),
                index: first,
                origin: origin.to_owned(),
            }
        });
    }
    ui.add_space(12.0);
}

fn total(count: usize, duration_ms: u64) -> String {
    if duration_ms == 0 {
        format::songs(count)
    } else {
        format!(
            "{}, {}",
            format::songs(count),
            format::long_duration(duration_ms)
        )
    }
}

pub fn album(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Album>) {
    loaded(state, ui, page, |ui, album| {
        let artists = join_names(&album.artists);
        let count = album.tracks.len().max(album.track_count as usize);
        hero(
            state,
            ui,
            Hero {
                art: &album.artwork,
                shape: ArtShape::Rounded(theme::RADIUS_ROW),
                placeholder: Icon::Music,
                kind: if album.kind.is_empty() {
                    "Album"
                } else {
                    &album.kind
                },
                title: &album.title,
                byline: bulleted([
                    artists.as_str(),
                    &album.year,
                    &total(count, album.duration_ms),
                ]),
            },
        );
        play_button(state, ui, actions, &album.tracks, &album.title);
        let columns = tracks::Columns {
            cover: false,
            album: false,
        };
        let list = tracks::List {
            tracks: &album.tracks,
            origin: &album.title,
            editable_playlist: None,
            columns,
        };
        tracks::table(state, ui, actions, list);
    });
}

pub fn playlist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Playlist>) {
    loaded(state, ui, page, |ui, playlist| {
        let count = playlist.tracks.len().max(playlist.track_count as usize);
        hero(
            state,
            ui,
            Hero {
                art: &playlist.artwork,
                shape: ArtShape::Rounded(theme::RADIUS_ROW),
                placeholder: Icon::ListMusic,
                kind: "Playlist",
                title: &playlist.title,
                byline: bulleted([playlist.owner.as_str(), &total(count, playlist.duration_ms)]),
            },
        );
        play_button(state, ui, actions, &playlist.tracks, &playlist.title);
        if playlist.tracks.is_empty() {
            widgets::empty_state(
                ui,
                &state.palette,
                Icon::Music,
                "Nothing here yet",
                "Added songs appear here.",
            );
            return;
        }
        let columns = tracks::Columns {
            cover: true,
            album: true,
        };
        let list = tracks::List {
            tracks: &playlist.tracks,
            origin: &playlist.title,
            editable_playlist: playlist.editable.then_some(playlist.id.as_str()),
            columns,
        };
        tracks::table(state, ui, actions, list);
    });
}

pub fn podcast(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Podcast>) {
    loaded(state, ui, page, |ui, podcast| {
        let episodes = match podcast.episodes.len() {
            1 => "1 episode".to_owned(),
            count => format!("{count} episodes"),
        };
        hero(
            state,
            ui,
            Hero {
                art: &podcast.artwork,
                shape: ArtShape::Rounded(theme::RADIUS_ROW),
                placeholder: Icon::MicVocal,
                kind: "Podcast",
                title: &podcast.title,
                byline: bulleted([podcast.author.as_str(), &episodes]),
            },
        );
        play_button(state, ui, actions, &podcast.episodes, &podcast.title);
        if !podcast.description.is_empty() {
            about(state, ui, &podcast.description);
            ui.add_space(8.0);
        }
        let list = tracks::List {
            tracks: &podcast.episodes,
            origin: &podcast.title,
            editable_playlist: None,
            columns: tracks::Columns {
                cover: true,
                album: false,
            },
        };
        tracks::table(state, ui, actions, list);
    });
}

/// A paragraph about what the page shows, held to a column.
fn about(state: &State, ui: &mut Ui, text: &str) {
    ui.scope(|ui| {
        ui.set_max_width(ABOUT_WIDTH);
        ui.label(
            egui::RichText::new(text)
                .font(theme::regular(14.0))
                .color(state.palette.secondary),
        );
    });
}

pub fn artist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Artist>) {
    loaded(state, ui, page, |ui, artist| {
        hero(
            state,
            ui,
            Hero {
                art: &artist.artwork,
                shape: ArtShape::Circle,
                placeholder: Icon::User,
                kind: "Artist",
                title: &artist.name,
                byline: bulleted([
                    artist.monthly_listeners.as_str(),
                    &subscribers(&artist.subscribers),
                ]),
            },
        );
        ui.horizontal(|ui| {
            play_button(state, ui, actions, &artist.top_tracks, &artist.name);
            let label = if artist.following {
                "Following"
            } else {
                "Follow"
            };
            if widgets::outline_button(ui, &state.palette, label).clicked() {
                actions.push(Action::ToggleFollow(artist.id.clone()));
            }
            artist_mixes(state, ui, actions, artist);
        });
        ui.add_space(12.0);
        if !artist.top_tracks.is_empty() {
            let all_songs =
                (!artist.songs_id.is_empty()).then(|| Page::Playlist(artist.songs_id.clone()));
            cards::linked_title(state, ui, actions, "Popular", all_songs);
            let shown = &artist.top_tracks[..artist.top_tracks.len().min(POPULAR)];
            let columns = tracks::Columns {
                cover: true,
                album: true,
            };
            let list = tracks::List {
                tracks: shown,
                origin: &artist.name,
                editable_playlist: None,
                columns,
            };
            tracks::rows(state, ui, actions, list);
        }
        for (title, albums, more) in [
            ("Albums", &artist.albums, &artist.albums_more),
            ("Singles and EPs", &artist.singles, &artist.singles_more),
        ] {
            if albums.is_empty() {
                continue;
            }
            let whole = more.as_ref().map(|link| discography(artist, title, link));
            cards::linked_title(state, ui, actions, title, whole);
            cards::row(ui, (title, &artist.id), |ui| {
                for album in albums {
                    cards::album(state, ui, actions, album);
                }
            });
        }
        if !artist.related.is_empty() {
            cards::shelf(ui, "Fans also like", ("related", &artist.id), |ui| {
                for related in &artist.related {
                    cards::artist(state, ui, actions, related);
                }
            });
        }
        if !artist.description.is_empty() {
            cards::section_title(ui, "About");
            about(state, ui, &artist.description);
        }
    });
}

/// The whole of an artist's albums, or of their singles.
fn discography(artist: &Artist, title: &str, link: &BrowseLink) -> Page {
    Page::Browse(Surface {
        id: link.id.clone(),
        params: link.params.clone(),
        title: format!("{} — {title}", artist.name),
    })
}

/// The queues YouTube makes of an artist: a shuffle of their own songs, and
/// a radio of theirs and others like them. Each is offered only when
/// YouTube names both the list and the song it starts from.
fn artist_mixes(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist: &Artist) {
    let radio = format!("{} radio", artist.name);
    let mixes = [
        (
            "Shuffle songs",
            &artist.shuffle_id,
            &artist.shuffle_seed,
            &artist.shuffle_params,
            artist.name.as_str(),
        ),
        (
            "Artist radio",
            &artist.radio_id,
            &artist.radio_seed,
            &artist.radio_params,
            radio.as_str(),
        ),
    ];
    for (label, list, seed, params, origin) in mixes {
        if list.is_empty() || seed.is_empty() {
            continue;
        }
        if widgets::outline_button(ui, &state.palette, label).clicked() {
            actions.push(Action::StartMix {
                seed: MixSeed {
                    playlist_id: list.clone(),
                    video_id: seed.clone(),
                    params: params.clone(),
                },
                origin: origin.to_owned(),
            });
        }
    }
}
