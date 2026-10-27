//! Other music services: the link to something there, copied from its
//! share menu here, and a link from there, pasted into search, searched
//! for here.
//!
//! An album is looked for by some of its songs, which its page has; one
//! whose page has not been read is read first, and looked for once it
//! arrives.

use spotified_client::models::{Album, Track};

use super::Effect;
use super::loading::run_new_search;
use super::types::Shared;
use crate::backend::Request;
use crate::services::{Found, FoundKind, Service, Wanted};
use crate::state::{Loadable, Page, State};
use spotified_client::models::SearchFilter;

/// How many of an album's songs it is looked for by.
const ALBUM_SONGS: usize = 3;

pub(super) fn share_on(state: &mut State, service: Service, shared: Shared) -> Vec<Effect> {
    let wanted = match shared {
        Shared::Song(track) => song(&track),
        Shared::Artist { id, name } => Wanted::Artist { name, channel: id },
        Shared::Album(id) => match state.albums.get(&id) {
            Loadable::Loaded(album) => album_of(album),
            _ => {
                state.share_waiting = Some((service, id.clone()));
                state.toast(format!("Looking for the album on {}…", service.name()));
                return vec![Effect::Fetch(Request::Album(id))];
            }
        },
    };
    state.toast(format!(
        "Looking for the {} on {}…",
        wanted.noun(),
        service.name()
    ));
    vec![Effect::FindOn(service, wanted)]
}

/// An album's page has been read: if a link to it was waiting on that,
/// it is looked for now.
pub(super) fn album_read(state: &mut State, id: &str) -> Vec<Effect> {
    let Some((service, _)) = state.share_waiting.take_if(|(_, waiting)| waiting == id) else {
        return Vec::new();
    };
    match state.albums.get(&id.to_owned()) {
        Loadable::Loaded(album) => vec![Effect::FindOn(service, album_of(album))],
        Loadable::Failed(error) => {
            let error = format!(
                "Couldn't read the album to find it on {}: {error}",
                service.name()
            );
            state.toast_error(error);
            Vec::new()
        }
        Loadable::NotLoaded | Loadable::Loading => Vec::new(),
    }
}

fn song(track: &Track) -> Wanted {
    Wanted::Song {
        title: track.title.clone(),
        artist: first_artist(&track.artists),
        album: track
            .album
            .as_ref()
            .map(|album| album.name.clone())
            .unwrap_or_default(),
    }
}

fn album_of(album: &Album) -> Wanted {
    Wanted::Album {
        title: album.title.clone(),
        artist: first_artist(&album.artists),
        songs: album
            .tracks
            .iter()
            .take(ALBUM_SONGS)
            .map(|track| track.title.clone())
            .collect(),
    }
}

/// The artist a thing is looked for by: the first, whose it most is.
fn first_artist(artists: &[spotified_client::models::ArtistRef]) -> String {
    artists
        .first()
        .map(|artist| artist.name.clone())
        .unwrap_or_default()
}

pub(super) fn found_on(
    state: &mut State,
    service: Service,
    noun: &str,
    found: Result<Option<String>, String>,
) -> Vec<Effect> {
    match found {
        Ok(Some(link)) => {
            state.toast(format!("{} link copied to clipboard", service.name()));
            vec![Effect::CopyToClipboard(link)]
        }
        Ok(None) => {
            state.toast(format!("Couldn't find this {noun} on {}", service.name()));
            Vec::new()
        }
        Err(error) => {
            state.toast_error(format!("Couldn't reach {}: {error}", service.name()));
            Vec::new()
        }
    }
}

/// A link from another service was pasted into search: what it is to is
/// read, and that is searched for instead.
pub(super) fn read_link(state: &mut State) -> Vec<Effect> {
    state.search.results = Loadable::Loading;
    state.search.suggestions.clear();
    state.nav.open(Page::Search);
    state.library_expanded = false;
    vec![Effect::ReadLink(state.search.query.trim().to_owned())]
}

pub(super) fn link_read(state: &mut State, link: &str, read: Result<Found, String>) -> Vec<Effect> {
    // Something else has been typed since: that is the search now.
    if state.search.query.trim() != link {
        return Vec::new();
    }
    match read {
        Ok(found) => {
            state.search.query = found.query();
            state.search.filter = match found.kind {
                FoundKind::Song => SearchFilter::Songs,
                FoundKind::Album => SearchFilter::Albums,
                FoundKind::Artist => SearchFilter::Artists,
                FoundKind::Playlist => SearchFilter::Playlists,
            };
            run_new_search(state)
        }
        Err(error) => {
            state.search.results = Loadable::NotLoaded;
            state.toast_error(format!("Couldn't read that link: {error}"));
            Vec::new()
        }
    }
}
