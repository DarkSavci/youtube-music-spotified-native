//! What a right click on a song offers.
//!
//! Built here and not in each list, so a song means the same wherever it
//! is shown. The entries, their order and their words are the Electron
//! app's (`trackmenu.ts`).

use spotified_client::models::{LibraryKind, Track};

use super::widgets;
use super::widgets::menu::{Entry, Menu};
use crate::actions::Action;
use crate::state::{Loadable, Page, State};
use crate::theme::Icon;

/// YouTube Music's own playlist of liked songs; liking is how songs get
/// there, not adding.
const LIKED_PLAYLIST: &str = "LM";

/// What can be done with a song, or with several selected together. The
/// entries that only make sense for one (its artists, its link) are left
/// out for several.
pub fn tracks(
    state: &State,
    menu: &mut Menu<'_>,
    actions: &mut Vec<Action>,
    tracks: &[&Track],
    editable_playlist: Option<&str>,
) {
    let Some(&first) = tracks.first() else {
        return;
    };
    let single = (tracks.len() == 1).then_some(first);
    if single.is_none() {
        menu.heading(&format!("{} songs", tracks.len()));
    }
    let owned = || {
        tracks
            .iter()
            .map(|&track| track.clone())
            .collect::<Vec<_>>()
    };
    if menu.item("Add to queue") {
        actions.push(Action::AddToQueue(owned()));
    }
    if menu.item("Play next") {
        actions.push(Action::PlayNext(owned()));
    }
    menu.separator();
    menu.submenu(Entry::new("Add to playlist"), |menu| {
        playlist_choice(state, menu, actions, tracks);
    });
    // Only entries that know their place in the playlist can leave it.
    let removable: Vec<(String, String)> = tracks
        .iter()
        .filter(|track| !track.playlist_item_id.is_empty())
        .map(|track| (track.id.clone(), track.playlist_item_id.clone()))
        .collect();
    if let Some(playlist_id) = editable_playlist
        && !removable.is_empty()
        && menu.item("Remove from this playlist")
    {
        actions.push(Action::RemoveFromPlaylist {
            playlist_id: playlist_id.to_owned(),
            items: removable,
        });
    }
    let Some(track) = single else {
        return;
    };
    menu.separator();
    if menu.item("Go to song radio") {
        actions.push(Action::StartRadio(track.clone()));
    }
    // Every artist that has a page, not only the first: a song by two is
    // as much the second's.
    for artist in track.artists.iter().filter(|artist| !artist.id.is_empty()) {
        if menu.item(&format!("Go to {}", artist.name)) {
            let id = widgets::artist_page_id(&artist.id).to_owned();
            actions.push(Action::Open(Page::Artist(id)));
        }
    }
    if let Some(album) = track.album.as_ref().filter(|album| !album.id.is_empty())
        && menu.item("Go to album")
    {
        actions.push(Action::Open(Page::Album(album.id.clone())));
    }
    menu.separator();
    let like = if state.likes.is_liked(&track.id) {
        "Remove from your library"
    } else {
        "Save to your library"
    };
    if menu.item(like) {
        actions.push(Action::ToggleLike(track.clone()));
    }
    if menu.item("Share") {
        actions.push(Action::Share {
            kind: crate::share::Kind::Track,
            id: track.id.clone(),
        });
    }
}

/// The person's playlists, to add `tracks` to one.
fn playlist_choice(
    state: &State,
    menu: &mut Menu<'_>,
    actions: &mut Vec<Action>,
    tracks: &[&Track],
) {
    let track_ids = || tracks.iter().map(|track| track.id.clone()).collect();
    if menu.item("New playlist…") {
        actions.push(Action::NewPlaylist {
            // A song's own name is what a playlist begun from it is
            // offered as, as the Electron app offered it.
            name: match tracks {
                [track] => track.title.clone(),
                _ => String::new(),
            },
            track_ids: track_ids(),
        });
    }
    let Loadable::Loaded(library) = &state.library else {
        menu.note("Sign in to add to your playlists");
        return;
    };
    let mut playlists = library
        .iter()
        .filter(|item| item.kind == LibraryKind::Playlist && item.id != LIKED_PLAYLIST)
        .peekable();
    if playlists.peek().is_none() {
        menu.note("No playlists yet");
        return;
    }
    for playlist in playlists {
        let entry = Entry::new(&playlist.title).icon(Icon::SquareLibrary);
        if menu.entry(entry) {
            actions.push(Action::AddToPlaylist {
                playlist_id: playlist.id.clone(),
                playlist_title: playlist.title.clone(),
                track_ids: track_ids(),
            });
        }
    }
}
