//! What is read a part at a time: the rest of Home as it is scrolled to,
//! and the rest of a long playlist. And what playing asks of each: a
//! playlist is played whole, so its unread songs are read first; an artist
//! is played from the list of all their songs, not the few on their page.

use spotified_client::ApiError;
use spotified_client::models::{PlaylistPage, RemoteQueue, Track};
use spotified_client::session::Command;

use super::playback::enqueue;
use super::{Action, Effect};
use crate::backend::{ArtistQueue, ArtistSeed, Request, Response};
use crate::state::{Dialog, HomeMore, LaunchPickup, Loadable, State, Tail, Whole};

pub(super) fn paging(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::MoreHome { retry } => more_home(state, retry),
        Action::MorePlaylist { id, retry } => more_playlist(state, id, retry),
        Action::PlayPlaylist { id, index } => whole_playlist(state, id, Whole::Play(index)),
        Action::WholePlaylist { id, then } => whole_playlist(state, id, then),
        Action::PlayArtist { artist_id, shuffle } => play_artist(state, artist_id, shuffle),
        Action::ContinueFromRemote => {
            if state.reading_remote_queue {
                return Vec::new();
            }
            state.reading_remote_queue = true;
            // The launch's own read is under way: its answer is this one's.
            if state.launch_pickup == LaunchPickup::Reading {
                state.launch_pickup = LaunchPickup::Done;
                return Vec::new();
            }
            vec![Effect::Fetch(Request::RemoteQueue)]
        }
        // `apply` sends nothing else here.
        _ => Vec::new(),
    }
}

/// Asks for the next few shelves of Home. Only when its end has been
/// scrolled to, never ahead of it: every page is a request to YouTube. A
/// page that failed waits to be asked for again by hand.
fn more_home(state: &mut State, retry: bool) -> Vec<Effect> {
    let tail = &mut state.home_more.tail;
    if retry {
        tail.failed = None;
    }
    if !tail.idle() || !matches!(state.home, Loadable::Loaded(_)) {
        return Vec::new();
    }
    tail.loading = true;
    let request = Request::HomeMore(state.home_mood.clone(), tail.next.clone());
    vec![Effect::Fetch(request)]
}

/// Home's first page has come, or failed to: what was read below the last
/// one goes, and reading starts over from this one's token.
pub(super) fn home_arrived(state: &mut State, continuation: Option<&str>) {
    state.home_more = HomeMore {
        shelves: Vec::new(),
        tail: Tail::after(continuation.unwrap_or_default().to_owned()),
    };
}

fn more_playlist(state: &mut State, id: String, retry: bool) -> Vec<Effect> {
    let Some(tail) = state.playlist_tails.get_mut(&id) else {
        return Vec::new();
    };
    if retry {
        tail.failed = None;
    }
    if !tail.idle() {
        return Vec::new();
    }
    tail.loading = true;
    let token = tail.next.clone();
    vec![Effect::Fetch(Request::PlaylistMore { id, token })]
}

/// Does something with the whole of a playlist. Browsing one is a page at
/// a time, but playing it, or queueing it, means all of it: the songs not
/// read yet are read first, and never is only the part on screen taken in
/// silence.
pub(super) fn whole_playlist(state: &mut State, id: String, then: Whole) -> Vec<Effect> {
    if !matches!(state.playlists.get(&id), Loadable::Loaded(_)) {
        return Vec::new();
    }
    state.preparing_playlist = Some((id.clone(), then));
    match state.playlist_tails.get_mut(&id) {
        None => prepared(state, &id),
        // A page is on its way already; the rest is asked for when it is
        // here, from where it leaves off.
        Some(tail) if tail.loading => Vec::new(),
        Some(tail) => {
            tail.loading = true;
            tail.failed = None;
            let token = tail.next.clone();
            vec![Effect::Fetch(Request::PlaylistRest { id, token })]
        }
    }
}

/// Every song of the playlist that was waiting is here: what was asked of
/// it is done.
fn prepared(state: &mut State, id: &str) -> Vec<Effect> {
    let Some((_, then)) = state
        .preparing_playlist
        .take_if(|(waiting, _)| waiting == id)
    else {
        return Vec::new();
    };
    let Loadable::Loaded(playlist) = state.playlists.get(&id.to_owned()) else {
        return Vec::new();
    };
    if playlist.tracks.is_empty() {
        return Vec::new();
    }
    let tracks = playlist.tracks.clone();
    let track_ids = || tracks.iter().map(|track| track.id.clone()).collect();
    match then {
        Whole::Play(index) => vec![Effect::Command(Command::Play {
            start_index: index.min(tracks.len() - 1),
            origin: playlist.title.clone(),
            tracks,
        })],
        Whole::Queue { next } => enqueue(state, tracks, next),
        Whole::NewPlaylist => {
            state.dialog = Some(Dialog::NewPlaylist {
                name: playlist.title.clone(),
                track_ids: track_ids(),
            });
            Vec::new()
        }
        Whole::AddTo {
            playlist_id,
            playlist_title,
        } => vec![Effect::Fetch(Request::AddToPlaylist {
            playlist_id,
            playlist_title,
            track_ids: track_ids(),
        })],
    }
}

/// The playlist could not be read to its end, so nothing is done with a
/// part of it.
fn give_up_preparing(state: &mut State, id: &str) {
    if state
        .preparing_playlist
        .take_if(|(waiting, _)| waiting == id)
        .is_some()
    {
        state.toast_error("Could not load the complete playlist. Please try again.");
    }
}

fn playlist_arrived(state: &mut State, id: String, result: Result<PlaylistPage, ApiError>) {
    state.playlist_tails.remove(&id);
    if let Ok(page) = &result
        && !page.next.is_empty()
    {
        let tail = Tail::after(page.next.clone());
        state.playlist_tails.insert(id.clone(), tail);
    }
    let playlist = result.map(|page| page.playlist);
    state.playlists.insert(id, Loadable::from_result(playlist));
}

fn more_arrived(
    state: &mut State,
    id: String,
    token: &str,
    result: Result<PlaylistPage, ApiError>,
) -> Vec<Effect> {
    let Some(tail) = state.playlist_tails.get_mut(&id) else {
        return Vec::new();
    };
    // An answer for a page the list has since moved past.
    if tail.next != token || !tail.loading {
        return Vec::new();
    }
    let page = match result {
        Ok(page) => page,
        Err(error) => {
            tail.failed(error.to_string());
            give_up_preparing(state, &id);
            return Vec::new();
        }
    };
    tail.arrived(page.next);
    let more = tail.more();
    if let Some(playlist) = state.playlists.loaded_mut(&id) {
        playlist.tracks.extend(page.playlist.tracks);
    }
    if !more {
        state.playlist_tails.remove(&id);
        return prepared(state, &id);
    }
    // Asked to play while this page was on its way: now the rest.
    let waiting = state
        .preparing_playlist
        .take_if(|(waiting, _)| waiting == &id);
    match waiting {
        Some((_, then)) => whole_playlist(state, id, then),
        None => Vec::new(),
    }
}

fn rest_arrived(
    state: &mut State,
    id: &str,
    token: &str,
    result: Result<Vec<Track>, ApiError>,
) -> Vec<Effect> {
    let Some(tail) = state.playlist_tails.get_mut(id) else {
        return Vec::new();
    };
    if tail.next != token {
        return Vec::new();
    }
    match result {
        Ok(tracks) => {
            state.playlist_tails.remove(id);
            if let Some(playlist) = state.playlists.loaded_mut(&id.to_owned()) {
                playlist.tracks.extend(tracks);
            }
            prepared(state, id)
        }
        Err(error) => {
            tail.failed(error.to_string());
            give_up_preparing(state, id);
            Vec::new()
        }
    }
}

/// Plays an artist, or a shuffle of them. What their page gave is sent
/// along when it is here; played from a card, it is read on the way.
pub(super) fn play_artist(state: &mut State, artist_id: String, shuffle: bool) -> Vec<Effect> {
    let known = match state.artists.get(&artist_id) {
        Loadable::Loaded(artist) => Some(ArtistSeed::of(artist)),
        _ => None,
    };
    vec![Effect::Fetch(Request::PlayArtist {
        device_id: state.settings.device_id.clone(),
        artist_id,
        known,
        shuffle,
    })]
}

fn artist_queue(
    state: &mut State,
    shuffle: bool,
    result: Result<ArtistQueue, ApiError>,
) -> Vec<Effect> {
    match result {
        Ok(ArtistQueue::Started) => Vec::new(),
        Ok(ArtistQueue::Songs { tracks, origin }) if !tracks.is_empty() => {
            vec![Effect::Command(Command::Play {
                tracks,
                start_index: 0,
                origin,
            })]
        }
        Ok(ArtistQueue::Songs { .. }) | Err(_) => {
            state.toast_error(if shuffle {
                "Could not shuffle this artist."
            } else {
                "Nothing by this artist can be played right now."
            });
            Vec::new()
        }
    }
}

/// What of a queue from another device can be played here, and where to
/// start: the entry that device was on, or the first playable one after
/// it. `None` when nothing playable is left.
pub(super) fn pick_remote(queue: RemoteQueue) -> Option<(Vec<Track>, usize, String)> {
    let current = queue.index.min(queue.tracks.len().saturating_sub(1));
    let mut start = None;
    let mut tracks = Vec::new();
    for (index, track) in queue.tracks.into_iter().enumerate() {
        if !track.playable {
            continue;
        }
        if start.is_none() && index >= current {
            start = Some(tracks.len());
        }
        tracks.push(track);
    }
    let last = tracks.len().checked_sub(1)?;
    let origin = match queue.title.trim() {
        "" => "YouTube Music".to_owned(),
        title => title.to_owned(),
    };
    Some((tracks, start.unwrap_or(last), origin))
}

/// Picks up the account's queue from another device when the app starts,
/// if the setting asks for it. Decided once, as soon as the account is
/// known: turning the setting on later does not reach back to a launch that
/// has passed.
pub(super) fn pick_up_at_launch(state: &mut State) -> Vec<Effect> {
    if state.launch_pickup != LaunchPickup::Undecided {
        return Vec::new();
    }
    if !state.settings.continue_from_youtube_music || !state.idle() {
        state.launch_pickup = LaunchPickup::Done;
        return Vec::new();
    }
    state.launch_pickup = LaunchPickup::Reading;
    vec![Effect::Fetch(Request::RemoteQueue)]
}

/// The queue read at launch has arrived. Quiet where the button is not:
/// nobody asked at this moment, so a failed or empty read says nothing and
/// changes nothing. The queue is put in place paused, and only if nothing
/// started playing here while it was being read. Already on the same song,
/// it is left alone: this device kept up.
fn picked_up_at_launch(state: &mut State, result: Result<RemoteQueue, ApiError>) -> Vec<Effect> {
    state.launch_pickup = LaunchPickup::Done;
    let Some((tracks, start_index, origin)) = result.ok().and_then(pick_remote) else {
        return Vec::new();
    };
    if !state.idle() {
        return Vec::new();
    }
    let first = &tracks[start_index];
    let here = state
        .playback
        .as_ref()
        .and_then(|playback| playback.current());
    if here.is_some_and(|track| track.id == first.id) {
        return Vec::new();
    }
    state.toast(format!(
        "Picked up your queue from YouTube Music: {}",
        first.title
    ));
    vec![Effect::Command(Command::Load {
        tracks,
        start_index,
        origin,
    })]
}

fn remote_queue(state: &mut State, result: Result<RemoteQueue, ApiError>) -> Vec<Effect> {
    if state.launch_pickup == LaunchPickup::Reading {
        return picked_up_at_launch(state, result);
    }
    state.reading_remote_queue = false;
    let queue = match result {
        Ok(queue) => queue,
        Err(ApiError::RateLimited) => {
            let text = "YouTube is limiting requests right now. Try again in a few minutes.";
            state.toast_error(text);
            return Vec::new();
        }
        Err(_) => {
            state.toast_error("Couldn't read your queue from YouTube Music.");
            return Vec::new();
        }
    };
    match pick_remote(queue) {
        Some((tracks, start_index, origin)) => vec![Effect::Command(Command::Play {
            tracks,
            start_index,
            origin,
        })],
        None => {
            state.toast("Nothing is queued on your other devices.");
            Vec::new()
        }
    }
}

/// What comes of the answers this module asked for.
pub(super) fn answered(state: &mut State, response: Response) -> Vec<Effect> {
    match response {
        Response::HomeMore(mood, token, result) => {
            let tail = &mut state.home_more.tail;
            // For a mood, or a first page, that is no longer the one shown.
            if mood != state.home_mood || token != tail.next || !tail.loading {
                return Vec::new();
            }
            match result {
                Ok(page) => {
                    tail.arrived(page.continuation);
                    state.home_more.shelves.extend(page.shelves);
                }
                Err(ApiError::RateLimited) => {
                    tail.failed("Couldn't load more of Home. Try again in a bit.".to_owned());
                }
                Err(_) => tail.failed("Couldn't load more of Home.".to_owned()),
            }
            Vec::new()
        }
        Response::Playlist(id, result) => {
            playlist_arrived(state, id, result);
            Vec::new()
        }
        Response::PlaylistMore { id, token, result } => more_arrived(state, id, &token, result),
        Response::PlaylistRest { id, token, result } => rest_arrived(state, &id, &token, result),
        Response::ArtistQueue { shuffle, result } => artist_queue(state, shuffle, result),
        Response::Affinity(id, result) => {
            state.affinity = result.ok().map(|affinity| (id, affinity));
            Vec::new()
        }
        Response::RemoteQueue(result) => remote_queue(state, result),
        // `store` sends nothing else here.
        _ => Vec::new(),
    }
}
