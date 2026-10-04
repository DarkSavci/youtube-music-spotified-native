//! Blocking a song, an artist or an album, and letting one play again.
//!
//! An album is blocked as its songs. A song met on a Home shelf or among
//! search results does not say what album it is on, so the album's name
//! alone would let it through there; the album's page is read, and each
//! song on it is blocked for as long as the album is.

use spotified_client::ApiError;
use spotified_client::models::Album;

use super::Effect;
use crate::backend::Request;
use crate::blocked::Kind;
use crate::state::{Loadable, State};

/// What a change to what is blocked calls for: it is kept, and the core,
/// which is what steps over a blocked song, is told at once.
fn changed() -> Vec<Effect> {
    vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
}

pub(super) fn set(
    state: &mut State,
    kind: Kind,
    id: &str,
    name: &str,
    blocked: bool,
) -> Vec<Effect> {
    if !state.settings.blocked.set(kind, id, name, blocked) {
        return Vec::new();
    }
    // One blocked without a name is called what it is.
    let name = if name.is_empty() { kind.noun() } else { name };
    state.toast(match (blocked, kind) {
        (false, _) => format!("{name} unblocked"),
        (true, Kind::Song) => format!("{name} blocked. It will be skipped."),
        (true, Kind::Artist) => format!("{name} blocked. Their songs will be skipped."),
        (true, Kind::Album) => format!("{name} blocked. Its songs will be skipped."),
    });
    let mut effects = changed();
    if blocked && kind == Kind::Album {
        effects.extend(read_album(state, id));
    }
    effects
}

/// Finds out what songs a blocked album holds: from its page if that has
/// been read, and by reading it if not.
fn read_album(state: &mut State, id: &str) -> Vec<Effect> {
    let key = id.to_owned();
    if let Loadable::Loaded(album) = state.albums.get(&key) {
        let songs = album.tracks.iter().map(|track| track.id.clone()).collect();
        state.settings.blocked.learn_album(id, songs);
        return Vec::new();
    }
    if !state.core_ready() || !state.albums.get(&key).needs_fetch() {
        // Being read already, or to be read once there is a core to ask.
        return Vec::new();
    }
    state.albums.insert(key.clone(), Loadable::Loading);
    vec![Effect::Fetch(Request::Album(key))]
}

/// An album's page has been read: if the album is blocked, so are the
/// songs it turned out to hold.
pub(super) fn album_read(
    state: &mut State,
    id: &str,
    result: &Result<Album, ApiError>,
) -> Vec<Effect> {
    let Ok(album) = result else {
        return Vec::new();
    };
    let songs = album.tracks.iter().map(|track| track.id.clone()).collect();
    if state.settings.blocked.learn_album(id, songs) {
        changed()
    } else {
        Vec::new()
    }
}

/// The core is up: the blocked albums whose songs are not known (blocked
/// before albums were blocked by their songs, or while the page could not
/// be read) are read now.
pub(super) fn core_ready(state: &mut State) -> Vec<Effect> {
    let unread = state.settings.blocked.albums_unread();
    unread.iter().flat_map(|id| read_album(state, id)).collect()
}
