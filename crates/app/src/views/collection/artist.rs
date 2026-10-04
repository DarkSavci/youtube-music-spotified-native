//! An artist's page: who they are, what of theirs to play, and their
//! releases.

use eframe::egui::Ui;
use spotified_client::models::{Affinity, Artist, BrowseLink, MixSeed, Track};

use super::hero::{self, Hero, Part, Plays};
use super::{LOADING, about, actions_row, page_tint};
use crate::actions::Action;
use crate::share;
use crate::state::{Loadable, Page, State, Surface};
use crate::theme::Icon;
use crate::views::actions_menu::Entity;
use crate::views::pages::loaded_or;
use crate::views::widgets::{self, ArtShape};
use crate::views::{cards, tracks};

/// YouTube gives the subscriber count as a bare number ("7.17M").
fn subscribers(count: &str) -> String {
    if count.is_empty() || count.contains(' ') {
        count.to_owned()
    } else {
        format!("{count} subscribers")
    }
}

/// The listener's own history with the artist, in words: shown where a
/// count of strangers would otherwise go, since only this app can say it.
/// Nothing for an artist never played.
/// The figures are set heavier than the words around them, as the
/// Electron app set them: they are what the eye is looking for.
fn history(affinity: &Affinity) -> Vec<Part> {
    if affinity.plays_all_time == 0 {
        return Vec::new();
    }
    let tracks = |count: u32| match count {
        1 => "1 track".to_owned(),
        count => format!("{count} tracks"),
    };
    let figure = if affinity.plays30d > 0 {
        format!("{} in the last 30 days", tracks(affinity.plays30d))
    } else {
        tracks(affinity.plays_all_time)
    };
    let mut parts = vec![
        Part::plain("You've played"),
        Part::strong(figure, None).joined(),
    ];
    if affinity.rank_among_your_artists > 0 {
        let rank = format!("#{}", affinity.rank_among_your_artists);
        parts.push(Part::strong(rank, None));
        parts.push(Part::plain("in your top artists").joined());
    }
    parts
}

pub fn artist(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, page: &Loadable<Artist>) {
    loaded_or(state, ui, page, LOADING, |ui, artist| {
        let tint = page_tint(state, ui, &artist.artwork);
        let mut byline = vec![
            Part::strong(artist.monthly_listeners.as_str(), None),
            Part::plain(subscribers(&artist.subscribers)),
        ];
        let affinity = state
            .affinity
            .as_ref()
            .filter(|(id, _)| id == &artist.id)
            .map(|(_, affinity)| affinity);
        if let Some(affinity) = affinity {
            byline.extend(history(affinity));
        }
        hero::show(
            state,
            ui,
            actions,
            Hero {
                art: &artist.artwork,
                shape: ArtShape::Circle,
                placeholder: Icon::User,
                kind: "Artist",
                title: &artist.name,
                avatar: &[],
                byline,
                tint,
            },
        );
        actions_row(ui, |ui| buttons(state, ui, actions, artist));
        popular(state, ui, actions, artist);
        for (title, albums, more) in [
            ("Albums", &artist.albums, &artist.albums_more),
            ("Singles and EPs", &artist.singles, &artist.singles_more),
        ] {
            if albums.is_empty() {
                continue;
            }
            let whole = more.as_ref().map(|link| discography(artist, title, link));
            cards::linked_title(state, ui, actions, (title, "Show all"), whole);
            cards::row(ui, albums.len(), |ui, index| {
                cards::album(state, ui, actions, &albums[index]);
            });
        }
        if !artist.related.is_empty() {
            cards::section_title(ui, "Fans also like");
            cards::row(ui, artist.related.len(), |ui, index| {
                cards::artist(state, ui, actions, &artist.related[index]);
            });
        }
        if !artist.description.is_empty() {
            let source = &artist.description_url;
            about::show(state, ui, actions, &artist.description, source);
        }
    });
}

/// Play, a shuffle of the artist's songs, their radio, the menu, and
/// whether they are followed.
///
/// Play is the artist's whole catalogue, most played first, not the few
/// songs on the page: those looped under repeat and never reached the
/// albums. Shuffle and the radio are YouTube's own lists for the artist,
/// which the core keeps extending as they play; where YouTube names no
/// shuffle, the artist's own song list is shuffled instead.
fn buttons(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist: &Artist) {
    let playable = artist.top_tracks.iter().any(|track| track.playable);
    let plays = Plays::Artist {
        id: &artist.id,
        any: playable || !artist.songs_id.is_empty(),
    };
    hero::play_button(state, ui, actions, plays, &artist.name);
    let named = |list: &str, seed: &str| !list.is_empty() && !seed.is_empty();
    let can_shuffle = named(&artist.shuffle_id, &artist.shuffle_seed)
        || !artist.songs_id.is_empty()
        || artist.top_tracks.len() > 1;
    if can_shuffle {
        let name = format!("Shuffle {}", artist.name);
        let glyph = (Icon::Shuffle, 40.0, 24.0);
        if hero::round_button(state, ui, glyph, (&name, "Shuffle")).clicked() {
            actions.push(Action::PlayArtist {
                artist_id: artist.id.clone(),
                shuffle: true,
            });
        }
    }
    // The artist's mix is offered only when YouTube names both the list
    // and the song it starts from.
    if named(&artist.radio_id, &artist.radio_seed) {
        let name = format!("Start {} radio", artist.name);
        let tooltip = "Artist radio: their music and music like it";
        let glyph = (Icon::Radio, 40.0, 24.0);
        if hero::round_button(state, ui, glyph, (&name, tooltip)).clicked() {
            actions.push(Action::StartMix {
                seed: MixSeed {
                    playlist_id: artist.radio_id.clone(),
                    video_id: artist.radio_seed.clone(),
                    params: artist.radio_params.clone(),
                },
                origin: format!("{} radio", artist.name),
            });
        }
    }
    let entity = Entity {
        kind: share::Kind::Artist,
        id: &artist.id,
        title: &artist.name,
        tracks: &artist.top_tracks,
        deletable: false,
    };
    hero::more_button(state, ui, actions, &entity);
    // Following is a subscription to the artist's channel.
    let label = if artist.following {
        "Following"
    } else {
        "Follow"
    };
    if widgets::chip(ui, &state.palette, label, artist.following).clicked() {
        actions.push(Action::ToggleFollow(artist.id.clone()));
    }
}

/// The artist's most played songs, with how often each was played.
fn popular(state: &State, ui: &mut Ui, actions: &mut Vec<Action>, artist: &Artist) {
    if artist.top_tracks.is_empty() {
        return;
    }
    let all_songs = (!artist.songs_id.is_empty()).then(|| Page::ArtistSongs(artist.id.clone()));
    cards::linked_title(state, ui, actions, ("Popular", "Show all songs"), all_songs);
    // No covers and no album: on an artist's own page the songs are
    // told apart by their names and, since YouTube gives these no
    // lengths, their plays.
    let shown: Vec<&Track> = artist
        .top_tracks
        .iter()
        .filter(|track| state.shows(track))
        .collect();
    let list = tracks::List {
        tracks: &shown,
        origin: &artist.name,
        editable_playlist: None,
        columns: tracks::Columns {
            cover: false,
            album: false,
        },
        mode: tracks::Mode::List,
    };
    tracks::table(state, ui, actions, list);
}

/// The whole of an artist's albums, or of their singles.
fn discography(artist: &Artist, title: &str, link: &BrowseLink) -> Page {
    Page::Browse(Surface {
        id: link.id.clone(),
        params: link.params.clone(),
        title: format!("{} — {title}", artist.name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_listeners_history_with_an_artist_is_said_in_words() {
        let recent = Affinity {
            plays30d: 12,
            plays_all_time: 90,
            rank_among_your_artists: 3,
        };
        assert_eq!(
            said(&recent),
            "You've played [12 tracks in the last 30 days] · [#3] in your top artists"
        );
        let long_ago = Affinity {
            plays30d: 0,
            plays_all_time: 1,
            rank_among_your_artists: 0,
        };
        assert_eq!(said(&long_ago), "You've played [1 track]");
        assert!(history(&Affinity::default()).is_empty());
    }

    /// The line as it reads, with what is set heavier in brackets and a
    /// dot where one is drawn.
    fn said(affinity: &Affinity) -> String {
        let mut line = String::new();
        for (index, part) in history(affinity).iter().enumerate() {
            match (index, part.joined) {
                (0, _) => {}
                (_, true) => line.push(' '),
                (_, false) => line.push_str(" · "),
            }
            if part.strong {
                line.push_str(&format!("[{}]", part.text));
            } else {
                line.push_str(&part.text);
            }
        }
        line
    }
}
