//! Fetching: what to ask the core for, and where its answers go.

use spotified_client::models::Track;
use spotified_client::session::Command;

use super::{Effect, listening, paging, songs, together, video};
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
            Request::Home(state.home_mood.clone())
        }
        Page::Album(id) if state.albums.get(&id).needs_fetch() => {
            state.albums.insert(id.clone(), Loadable::Loading);
            Request::Album(id)
        }
        // The listener's own history with the artist is read on every
        // visit: it changes as their music plays.
        Page::Artist(id) => {
            let mut effects = vec![Effect::Fetch(Request::Affinity(id.clone()))];
            effects.extend(load_artist(state, id));
            return effects;
        }
        Page::ArtistSongs(id) => {
            let mut effects = load_artist(state, id.clone());
            effects.extend(songs::opened(state, &id));
            return effects;
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
        Page::Changelog => return listening::mark_notes_read(state),
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

/// Asks for an artist's page unless it is here or on its way.
fn load_artist(state: &mut State, id: String) -> Vec<Effect> {
    if !state.artists.get(&id).needs_fetch() {
        // Already here: mark it used, so it outlives pages opened earlier.
        state.artists.touch(&id);
        return Vec::new();
    }
    state.artists.insert(id.clone(), Loadable::Loading);
    vec![Effect::Fetch(Request::Artist(id))]
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
    effects.push(Effect::Fetch(Request::SearchHistory));
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
        | Page::ArtistSongs(_)
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
    // An artist is every song of theirs, not the few their page shows.
    if let Page::Artist(id) = page {
        return paging::play_artist(state, id, false);
    }
    if let Some(effects) = play_whole_playlist(state, &page) {
        return effects;
    }
    if let Some((tracks, origin)) = collection(state, &page) {
        return vec![play_command(tracks, origin)];
    }
    state.pending_play = Some(page.clone());
    load_page(state, page)
}

/// A playlist with songs still to be read is read to its end, then
/// played. `None` for anything else, which is played as it stands.
fn play_whole_playlist(state: &mut State, page: &Page) -> Option<Vec<Effect>> {
    let Page::Playlist(id) = page else {
        return None;
    };
    if !state.playlist_tails.contains_key(id) {
        return None;
    }
    let Loadable::Loaded(playlist) = state.playlists.get(id) else {
        return None;
    };
    let first = playlist.tracks.iter().position(|track| track.playable)?;
    let then = crate::state::Whole::Play(first);
    Some(paging::whole_playlist(state, id.clone(), then))
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
        | Page::ArtistSongs(_)
        | Page::Together => false,
    };
    if still_loading {
        return Vec::new();
    }
    state.pending_play = None;
    if let Some(effects) = play_whole_playlist(state, &page) {
        return effects;
    }
    match collection(state, &page) {
        Some((tracks, origin)) => vec![play_command(tracks, origin)],
        None => {
            state.toast_error("Nothing there could be played");
            Vec::new()
        }
    }
}

/// Reads Home again through a mood chip, or plainly when the chip chosen
/// is the one already on.
pub(super) fn choose_mood(state: &mut State, params: String) -> Vec<Effect> {
    state.home_mood = if params == state.home_mood {
        String::new()
    } else {
        params
    };
    state.home = Loadable::NotLoaded;
    paging::home_arrived(state, None);
    load_page(state, Page::Home)
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
        effects.extend(listening::remember_search(state, &query));
        effects.push(Effect::Fetch(Request::Suggest(query)));
    }
    effects
}

/// Puts an answer where it belongs. Some answers call for more: a playlist
/// that was created or deleted changes the library, which is fetched again.
pub(super) fn store(state: &mut State, response: Response) -> Vec<Effect> {
    // Each topic takes the answers it asked for.
    let mut effects = match response {
        Response::HomeMore(..)
        | Response::Playlist(..)
        | Response::PlaylistMore { .. }
        | Response::PlaylistRest { .. }
        | Response::ArtistQueue { .. }
        | Response::Affinity(..)
        | Response::RemoteQueue(_) => paging::answered(state, response),
        Response::StatsLookup(..)
        | Response::StatDetail(..)
        | Response::ArtistPhoto(..)
        | Response::SearchHistory(_)
        | Response::SearchesForgotten { .. } => listening::answered(state, response),
        Response::SongsPage { .. } | Response::Discography { .. } | Response::Release { .. } => {
            songs::answered(state, response)
        }
        Response::RoomSearch { .. } | Response::Radio(..) => {
            return together::answered(state, response);
        }
        Response::Versions { track_id, result } => {
            return video::answered(state, track_id, result);
        }
        Response::Album(id, result) => {
            let learnt = super::blocking::album_read(state, &id, &result);
            let mut effects = store_rest(state, Response::Album(id, result));
            effects.extend(learnt);
            return effects;
        }
        other => return store_rest(state, other),
    };
    effects.extend(play_pending(state));
    effects
}

/// The answers that are not a topic's own.
fn store_rest(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::PlaylistCreated { title, result } => {
            return match result {
                Ok(_) => {
                    let from_room = state.together.saving_history.take_if(|name| *name == title);
                    match from_room {
                        Some(_) => state.toast("Room history saved to your library."),
                        None => state.toast(format!("Created {title}")),
                    }
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
            return match &state.account {
                Some(account) => {
                    // The list of saved accounts learns what this one is
                    // called, to tell it from the others.
                    let remember = Effect::RememberAccount {
                        name: account.name.clone(),
                        avatar_url: account.avatar_url.clone(),
                    };
                    let mut effects = vec![Effect::Fetch(Request::Channels), remember];
                    effects.extend(paging::pick_up_at_launch(state));
                    effects
                }
                None => {
                    // A signed-out launch never asks.
                    state.launch_pickup = crate::state::LaunchPickup::Done;
                    Vec::new()
                }
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
        Response::Channels(result) => {
            let answered = result.is_ok();
            state.channels = result.unwrap_or_default();
            // An answer that failed says nothing about which there are;
            // one that came is noted for the list of saved accounts.
            if answered {
                return vec![Effect::RememberChannels(state.channels.clone())];
            }
        }
        other => store_data(state, other),
    }
    let mut effects = play_pending(state);
    // The artist whose songs page is open may just have arrived.
    effects.extend(songs::advance(state));
    effects
}

/// The answers that are simply data to hold.
fn store_data(state: &mut State, response: Response) {
    match response {
        Response::Home(mood, result) => {
            // An answer for a mood that is no longer the one chosen.
            if mood != state.home_mood {
                return;
            }
            // The row of moods outlives a page that fails, so the others
            // can still be chosen.
            if let Ok(page) = &result
                && !page.chips.is_empty()
            {
                state.home_chips.clone_from(&page.chips);
            }
            let continuation = result.as_ref().ok().map(|page| page.continuation.as_str());
            paging::home_arrived(state, continuation);
            state.home = Loadable::from_result(result);
        }
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
        // Handled by `store`, which is the only caller.
        Response::Account(_) | Response::Channels(_) => {}
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
                if let Ok(stats) = &result {
                    state.stats_page.arrived(stats);
                }
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
        Response::HomeMore(..)
        | Response::Playlist(..)
        | Response::PlaylistMore { .. }
        | Response::PlaylistRest { .. }
        | Response::Affinity(..)
        | Response::ArtistQueue { .. }
        | Response::SongsPage { .. }
        | Response::Discography { .. }
        | Response::Release { .. }
        | Response::ArtistPhoto(..)
        | Response::SearchHistory(_)
        | Response::SearchesForgotten { .. }
        | Response::RemoteQueue(_)
        | Response::StatsLookup(..)
        | Response::StatDetail(..)
        | Response::PlaylistCreated { .. }
        | Response::PlaylistDeleted { .. }
        | Response::RemovedFromPlaylist { .. }
        | Response::RoomSearch { .. }
        | Response::Radio(..)
        | Response::Versions { .. }
        | Response::FollowingSet { .. } => {}
        Response::Search { serial, result } => {
            // An answer to a query that has since been replaced, or cleared.
            if serial == state.search.serial && !state.search.query.trim().is_empty() {
                state.search.results = Loadable::from_result(result);
            }
        }
    }
}
