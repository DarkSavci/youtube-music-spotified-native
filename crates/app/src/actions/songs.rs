//! The page of all an artist's songs: reading YouTube's list of them to
//! its end, and, for the orders that go by date, opening the artist's
//! releases a couple at a time until enough are open.

use spotified_client::ApiError;
use spotified_client::models::BrowseLink;

use super::{Action, Effect};
use crate::backend::{Request, Response};
use crate::state::{ArtistSongs, Loadable, Page, State, Tail};

/// How many releases are opened at once.
const RELEASES_AT_ONCE: usize = 2;

pub(super) fn songs(state: &mut State, action: Action) -> Vec<Effect> {
    let Some(songs) = &mut state.artist_songs else {
        return Vec::new();
    };
    match action {
        Action::SetSongOrder(order) => songs.order = order,
        // The failures are tried again first, within the new batch.
        Action::OpenMoreReleases => songs.open_more(),
        // `apply` sends nothing else here.
        _ => return Vec::new(),
    }
    advance(state)
}

/// The page has been opened for this artist. What was read for them last
/// time is kept, so coming back does not ask YouTube again; another
/// artist starts afresh.
pub(super) fn opened(state: &mut State, artist_id: &str) -> Vec<Effect> {
    match &mut state.artist_songs {
        Some(songs) if songs.artist_id == artist_id => {
            // Opening the page again is the retry for a list that failed:
            // from its start if nothing came, else from where it stopped.
            if songs.listed.is_empty() {
                songs.list = songs.list.take().filter(|tail| tail.failed.is_none());
            } else if let Some(tail) = &mut songs.list {
                tail.failed = None;
            }
        }
        _ => state.artist_songs = Some(ArtistSongs::of(artist_id.to_owned())),
    }
    advance(state)
}

fn link(link: Option<&BrowseLink>) -> Option<(String, String)> {
    link.filter(|link| !link.id.is_empty())
        .map(|link| (link.id.clone(), link.params.clone()))
}

/// Works out what the page shows, and asks for whatever it still lacks:
/// the next page of the list, the discography, the next releases. Nothing
/// is asked for once the page has been left.
pub(super) fn advance(state: &mut State) -> Vec<Effect> {
    let Page::ArtistSongs(id) = state.nav.page() else {
        return Vec::new();
    };
    let Some(songs) = state
        .artist_songs
        .as_mut()
        .filter(|songs| &songs.artist_id == id)
    else {
        return Vec::new();
    };
    let Loadable::Loaded(artist) = state.artists.get(id) else {
        return Vec::new();
    };
    if artist.songs_id.is_empty() {
        return Vec::new();
    }
    let mut effects = Vec::new();
    // Ordering needs the whole list, and these are a few hundred songs.
    let token = match &mut songs.list {
        None => {
            songs.list = Some(Tail::starting());
            Some(String::new())
        }
        Some(tail) if tail.idle() => {
            tail.loading = true;
            Some(tail.next.clone())
        }
        Some(_) => None,
    };
    if let Some(token) = token {
        effects.push(Effect::Fetch(Request::SongsPage {
            artist_id: id.clone(),
            songs_id: artist.songs_id.clone(),
            token,
        }));
    }
    // The discography behind the page's shelves, for the years those do
    // not show. Only when a dated order asks for it.
    if songs.order.dated() && !songs.discography_asked {
        songs.discography_asked = true;
        effects.push(Effect::Fetch(Request::Discography {
            artist_id: id.clone(),
            albums: link(artist.albums_more.as_ref()),
            singles: link(artist.singles_more.as_ref()),
        }));
    }
    songs.refresh(artist);
    while songs.opening.len() < RELEASES_AT_ONCE {
        let Some(album_id) = songs.next_release().map(str::to_owned) else {
            break;
        };
        songs.opening.push(album_id.clone());
        effects.push(Effect::Fetch(Request::Release {
            artist_id: id.clone(),
            album_id,
        }));
    }
    if !effects.is_empty() {
        // What was just asked for changes what the status line says.
        songs.refresh(artist);
    }
    effects
}

/// The songs page of this artist, if it is the one held.
fn held<'a>(state: &'a mut State, artist_id: &str) -> Option<&'a mut ArtistSongs> {
    state
        .artist_songs
        .as_mut()
        .filter(|songs| songs.artist_id == artist_id)
}

/// What comes of the answers this module asked for.
pub(super) fn answered(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::SongsPage {
            artist_id,
            token,
            result,
        } => {
            let list = held(state, &artist_id).and_then(|songs| {
                let tail = songs.list.as_mut()?;
                (tail.next == token && tail.loading).then_some((&mut songs.listed, tail))
            });
            if let Some((listed, tail)) = list {
                match result {
                    Ok(page) => {
                        listed.extend(page.playlist.tracks);
                        tail.arrived(page.next);
                    }
                    Err(error) => tail.failed(error.to_string()),
                }
            }
        }
        Response::Discography {
            artist_id,
            albums,
            singles,
        } => {
            if let Some(songs) = held(state, &artist_id) {
                songs.discography = Some((albums, singles));
            }
        }
        Response::Release {
            artist_id,
            album_id,
            result,
        } => {
            if let Some(songs) = held(state, &artist_id) {
                songs.opening.retain(|opening| opening != &album_id);
                // A refusal stops the run until the listener asks again.
                if result == Err(ApiError::RateLimited) {
                    songs.limited = true;
                }
                if !songs.is_opened(&album_id) {
                    songs.opened.push((album_id, result.ok()));
                }
            }
        }
        // `store` sends nothing else here.
        _ => {}
    }
    advance(state)
}
