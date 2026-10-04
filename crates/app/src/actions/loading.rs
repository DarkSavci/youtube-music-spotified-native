//! Fetching: what to ask the core for, and where its answers go.

use spotified_client::models::Track;
use spotified_client::session::Command;

use super::Effect;
use crate::backend::{Request, Response};
use crate::state::{Loadable, Page, State, Surface};

/// Asks for the open page's data unless it is already here or on its way.
/// A page that failed is asked for again: reopening it is the retry.
pub(super) fn load_current_page(state: &mut State) -> Vec<Effect> {
    load_page(state, state.nav.page().clone())
}

/// Asks for a page's data, as [`load_current_page`] does for the open one.
fn load_page(state: &mut State, page: Page) -> Vec<Effect> {
    // The release notes need nothing from the core.
    if !state.core_ready() && page != Page::Changelog {
        return Vec::new();
    }
    let request = match page {
        Page::Home if state.home.needs_fetch() => {
            state.home = Loadable::Loading;
            Request::Home
        }
        Page::Album(id) if state.albums.get(&id).needs_fetch() => {
            state.albums.insert(id.clone(), Loadable::Loading);
            Request::Album(id)
        }
        Page::Artist(id) if state.artists.get(&id).needs_fetch() => {
            state.artists.insert(id.clone(), Loadable::Loading);
            Request::Artist(id)
        }
        Page::Playlist(id) if state.playlists.get(&id).needs_fetch() => {
            state.playlists.insert(id.clone(), Loadable::Loading);
            Request::Playlist(id)
        }
        Page::Podcast(id) if state.podcasts.get(&id).needs_fetch() => {
            state.podcasts.insert(id.clone(), Loadable::Loading);
            Request::Podcast(id)
        }
        // Already here: mark it used, so it outlives pages opened earlier.
        Page::Album(id) => {
            state.albums.touch(&id);
            return Vec::new();
        }
        Page::Artist(id) => {
            state.artists.touch(&id);
            return Vec::new();
        }
        Page::Playlist(id) => {
            state.playlists.touch(&id);
            return Vec::new();
        }
        Page::Podcast(id) => {
            state.podcasts.touch(&id);
            return Vec::new();
        }
        // Counted afresh on every visit: plays change as music plays.
        Page::Stats => {
            if !matches!(state.stats, Loadable::Loaded(_)) {
                state.stats = Loadable::Loading;
            }
            Request::Stats(state.stats_days)
        }
        Page::Browse(surface) => return load_surface(state, &surface),
        // Opening the notes is reading them: the mark that says there are
        // new ones goes.
        Page::Changelog => {
            let latest = crate::changelog::latest();
            if state.settings.release_notes_read == latest {
                return Vec::new();
            }
            state.settings.release_notes_read = latest.to_owned();
            return vec![Effect::SaveSettings];
        }
        // Asked afresh on every visit: it changes as music plays.
        Page::History => {
            if !matches!(state.history, Loadable::Loaded(_)) {
                state.history = Loadable::Loading;
            }
            Request::History
        }
        Page::Search if state.search.query.trim().is_empty() => return load_search_start(state),
        // A query typed before the core was ready, or one that failed.
        Page::Search if state.search.results.needs_fetch() => return run_new_search(state),
        // The room the kept songs take changes as music plays.
        Page::Settings => Request::CacheUsage,
        Page::Home | Page::Search | Page::Mix(_) | Page::Together => return Vec::new(),
    };
    vec![Effect::Fetch(request)]
}

fn load_surface(state: &mut State, surface: &Surface) -> Vec<Effect> {
    let key = surface.key();
    if !state.surfaces.get(&key).needs_fetch() {
        state.surfaces.touch(&key);
        return Vec::new();
    }
    state.surfaces.insert(key.clone(), Loadable::Loading);
    vec![Effect::Fetch(Request::Browse(key.0, key.1))]
}

/// The search page before anything is typed shows what was searched for
/// lately, and the moods and genres to browse.
fn load_search_start(state: &mut State) -> Vec<Effect> {
    let mut effects = load_surface(state, &Surface::moods());
    effects.push(Effect::Fetch(Request::RecentSearches));
    effects
}

/// The songs of an album, a playlist or an artist, and what the queue
/// they make is called. `None` until the page has loaded with at least
/// one song that can play.
fn collection(state: &State, page: &Page) -> Option<(Vec<Track>, String)> {
    let (tracks, origin) = match page {
        Page::Album(id) => match state.albums.get(id) {
            Loadable::Loaded(album) => (&album.tracks, &album.title),
            _ => return None,
        },
        Page::Playlist(id) => match state.playlists.get(id) {
            Loadable::Loaded(playlist) => (&playlist.tracks, &playlist.title),
            _ => return None,
        },
        Page::Artist(id) => match state.artists.get(id) {
            Loadable::Loaded(artist) => (&artist.top_tracks, &artist.name),
            _ => return None,
        },
        Page::Podcast(id) => match state.podcasts.get(id) {
            Loadable::Loaded(podcast) => (&podcast.episodes, &podcast.title),
            _ => return None,
        },
        // A mix arrives whole, songs and all.
        Page::Mix(id) => {
            let mix = state.mixes.iter().find(|mix| &mix.id == id)?;
            (&mix.tracks, &mix.title)
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
    tracks
        .iter()
        .any(|track| track.playable)
        .then(|| (tracks.clone(), origin.clone()))
}

fn play_command(tracks: Vec<Track>, origin: String) -> Effect {
    let start_index = tracks.iter().position(|track| track.playable).unwrap_or(0);
    Effect::Command(Command::Play {
        tracks,
        start_index,
        origin,
    })
}

/// Plays a collection from its first playable song. If its songs are not
/// here yet they are fetched, and it plays when they arrive.
pub(super) fn play_collection(state: &mut State, page: Page) -> Vec<Effect> {
    if let Some((tracks, origin)) = collection(state, &page) {
        return vec![play_command(tracks, origin)];
    }
    state.pending_play = Some(page.clone());
    load_page(state, page)
}

/// Plays what was waiting on a fetch, if that fetch has now ended: with
/// its songs, or with a message if it brought none.
fn play_pending(state: &mut State) -> Vec<Effect> {
    let Some(page) = state.pending_play.clone() else {
        return Vec::new();
    };
    let still_loading = match &page {
        Page::Album(id) => matches!(state.albums.get(id), Loadable::Loading),
        Page::Playlist(id) => matches!(state.playlists.get(id), Loadable::Loading),
        Page::Artist(id) => matches!(state.artists.get(id), Loadable::Loading),
        Page::Podcast(id) => matches!(state.podcasts.get(id), Loadable::Loading),
        Page::Home
        | Page::Search
        | Page::Settings
        | Page::Stats
        | Page::Mix(_)
        | Page::Browse(_)
        | Page::History
        | Page::Changelog
        | Page::Together => false,
    };
    if still_loading {
        return Vec::new();
    }
    state.pending_play = None;
    match collection(state, &page) {
        Some((tracks, origin)) => vec![play_command(tracks, origin)],
        None => {
            state.toast_error("Nothing there could be played");
            Vec::new()
        }
    }
}

pub(super) fn run_search(state: &mut State) -> Vec<Effect> {
    let query = state.search.query.trim();
    if query.is_empty() || !state.core_ready() {
        return Vec::new();
    }
    state.search.serial += 1;
    state.search.results = Loadable::Loading;
    vec![Effect::Fetch(Request::Search {
        serial: state.search.serial,
        query: query.to_owned(),
        filter: state.search.filter,
    })]
}

/// Searches for the query on screen, and asks how else it might go on.
/// Narrowing a search does not ask again: the query has not changed.
pub(super) fn run_new_search(state: &mut State) -> Vec<Effect> {
    let mut effects = run_search(state);
    if !effects.is_empty() {
        let query = state.search.query.trim().to_owned();
        effects.push(Effect::Fetch(Request::Suggest(query)));
    }
    effects
}

/// Puts an answer where it belongs. Some answers call for more: a playlist
/// that was created or deleted changes the library, which is fetched again.
pub(super) fn store(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::PlaylistCreated { title, result } => {
            return match result {
                Ok(_) => {
                    state.toast(format!("Created {title}"));
                    vec![Effect::Fetch(Request::Library)]
                }
                Err(error) => {
                    state.toast_error(format!("Couldn't create {title}: {error}"));
                    Vec::new()
                }
            };
        }
        Response::PlaylistDeleted {
            playlist_id,
            title,
            result,
        } => {
            return match result {
                Ok(()) => {
                    state.toast(format!("Deleted {title}"));
                    // Its page shows a playlist that is no longer there.
                    if state.nav.page() == &Page::Playlist(playlist_id.clone()) {
                        state.nav.open(Page::Home);
                    }
                    state.playlists.insert(playlist_id, Loadable::NotLoaded);
                    vec![Effect::Fetch(Request::Library)]
                }
                Err(error) => {
                    state.toast_error(format!("Couldn't delete {title}: {error}"));
                    Vec::new()
                }
            };
        }
        Response::RemovedFromPlaylist {
            playlist_id,
            result,
        } => {
            return match result {
                Ok(()) => {
                    state.toast("Removed from playlist");
                    state.selection.clear();
                    state.playlists.insert(playlist_id, Loadable::NotLoaded);
                    load_current_page(state)
                }
                Err(error) => {
                    state.toast_error(format!("Couldn't remove from the playlist: {error}"));
                    Vec::new()
                }
            };
        }
        Response::FollowingSet {
            artist_id,
            follow,
            result,
        } => {
            return match result {
                Ok(()) => {
                    state.toast(if follow { "Following" } else { "Unfollowed" });
                    vec![Effect::Fetch(Request::Library)]
                }
                Err(error) => {
                    if let Some(artist) = state.artists.loaded_mut(&artist_id) {
                        artist.following = !follow;
                    }
                    state.toast_error(format!("Couldn't update your library: {error}"));
                    Vec::new()
                }
            };
        }
        // Only a signed-in account has channels to ask after.
        Response::Account(result) => {
            state.account = result.ok().flatten();
            return match state.account {
                Some(_) => vec![Effect::Fetch(Request::Channels)],
                None => Vec::new(),
            };
        }
        // What the library shows was changed ahead of this answer. Either
        // way the core now has the last word on it.
        Response::Organised(result) => {
            if let Err(error) = result {
                state.toast_error(format!("Couldn't update your library: {error}"));
            }
            return vec![
                Effect::Fetch(Request::Folders),
                Effect::Fetch(Request::Library),
            ];
        }
        other => store_data(state, other),
    }
    play_pending(state)
}

/// The answers that are simply data to hold.
fn store_data(state: &mut State, response: Response) {
    match response {
        Response::Home(result) => state.home = Loadable::from_result(result),
        Response::Library(result) => state.library = Loadable::from_result(result),
        Response::Browse(id, params, result) => {
            state
                .surfaces
                .insert((id, params), Loadable::from_result(result));
        }
        Response::TileArt(id, params, art) => {
            state.tile_art.insert((id, params), Some(art));
        }
        Response::History(result) => state.history = Loadable::from_result(result),
        Response::Suggestions(query, result) => {
            // For a query that has since been replaced, or cleared.
            if query == state.search.query.trim() {
                state.search.suggestions = result.unwrap_or_default();
            }
        }
        // Signed out there are none, and none is what is shown.
        Response::RecentSearches(result) => state.search.recent = result.unwrap_or_default(),
        Response::Channels(result) => state.channels = result.unwrap_or_default(),
        // Handled by `store`, which is the only caller.
        Response::Account(_) => {}
        Response::CacheUsage(result) => state.cache_usage = result.ok(),
        Response::CacheCleared(result) => match result {
            Ok(usage) => {
                state.cache_usage = Some(usage);
                state.toast("Downloaded songs deleted");
            }
            Err(error) => state.toast_error(format!("Couldn't delete them: {error}")),
        },
        Response::Folders(result) => state.folders = result.unwrap_or_default(),
        // Handled by `store`, which is the only caller.
        Response::Organised(_) => {}
        Response::RadioStarted(result) => {
            if let Err(error) = result {
                state.toast_error(format!("Couldn't start the radio: {error}"));
            }
        }
        Response::Album(id, result) => state.albums.insert(id, Loadable::from_result(result)),
        Response::Artist(id, result) => {
            let artist = result.map(|artist| *artist);
            state.artists.insert(id, Loadable::from_result(artist));
        }
        Response::Playlist(id, result) => {
            state.playlists.insert(id, Loadable::from_result(result));
        }
        Response::Podcast(id, result) => {
            state.podcasts.insert(id, Loadable::from_result(result));
        }
        Response::Lyrics { track_id, result } => {
            // The track has changed since these were asked for.
            if track_id == state.lyrics.track_id {
                state.lyrics.words = Loadable::from_result(result);
            }
        }
        // Signed out, there are no likes to show; that is not an error.
        // Without history, or signed out, there are none; not an error.
        Response::Mixes(result) => state.mixes = result.unwrap_or_default(),
        Response::Stats(days, result) => {
            // An answer for a period that is no longer the one chosen.
            if days == state.stats_days {
                state.stats = Loadable::from_result(result);
            }
        }
        Response::Liked(result) => {
            if let Ok(liked) = result {
                state
                    .likes
                    .replace(liked.tracks.into_iter().map(|track| track.id));
            }
        }
        Response::LikeSet {
            track_id,
            liked,
            result,
        } => {
            state.likes.settle(&track_id, liked, result.is_ok());
            match result {
                Ok(()) if liked => state.toast("Added to Liked Songs"),
                Ok(()) => state.toast("Removed from Liked Songs"),
                Err(error) => state.toast_error(format!("Couldn't update Liked Songs: {error}")),
            }
        }
        Response::AddedToPlaylist {
            playlist_id,
            playlist_title,
            count,
            result,
        } => match result {
            Ok(()) => {
                // What is held of the playlist is now out of date.
                state.playlists.insert(playlist_id, Loadable::NotLoaded);
                state.toast(match count {
                    1 => format!("Added to {playlist_title}"),
                    count => format!("{count} songs added to {playlist_title}"),
                });
            }
            Err(error) => {
                state.toast_error(format!("Couldn't add to {playlist_title}: {error}"));
            }
        },
        // Handled by `store`, which is the only caller.
        Response::PlaylistCreated { .. }
        | Response::PlaylistDeleted { .. }
        | Response::RemovedFromPlaylist { .. }
        | Response::FollowingSet { .. } => {}
        Response::Search { serial, result } => {
            // An answer to a query that has since been replaced, or cleared.
            if serial == state.search.serial && !state.search.query.trim().is_empty() {
                state.search.results = Loadable::from_result(result);
            }
        }
    }
}
