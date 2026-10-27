//! What a right click on a song offers, or one on an artist's name.
//!
//! Built here and not in each list, so a song means the same wherever it
//! is shown. The entries, their order and their words are the Electron
//! app's (`trackmenu.ts`).

use spotified_client::models::{AlbumRef, ArtistRef, LibraryKind, Track};

use super::widgets;
use super::widgets::menu::{Entry, Menu};
use crate::actions::{Action, Shared};
use crate::blocked::Kind;
use crate::services::Service;
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
    // Never to be played, or to be played again: the song, each artist
    // that has a page, and the album.
    menu.separator();
    actions.extend(block(state, menu, Kind::Song, &track.id, &track.title));
    for artist in &track.artists {
        actions.extend(block(state, menu, Kind::Artist, &artist.id, &artist.name));
    }
    if let Some(album) = &track.album {
        actions.extend(block(state, menu, Kind::Album, &album.id, &album.name));
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
    let youtube = || Action::Share {
        kind: crate::share::Kind::Track,
        id: track.id.clone(),
    };
    let elsewhere = Shared::Song(Box::new(track.clone()));
    actions.extend(share(menu, youtube, Some(elsewhere)));
}

/// The entry that blocks a song, an artist or an album, or lets one that
/// is blocked play again. Nothing without an id can be blocked, and has no
/// entry. Returns what was asked for, if it was chosen.
pub fn block(
    state: &State,
    menu: &mut Menu<'_>,
    kind: Kind,
    id: &str,
    name: &str,
) -> Option<Action> {
    if id.is_empty() {
        return None;
    }
    let blocked = state.settings.blocked.has(kind, id);
    let verb = if blocked { "Unblock" } else { "Block" };
    let label = match kind {
        Kind::Song => format!("{verb} this song"),
        Kind::Album => format!("{verb} this album"),
        // A song can have several, so each is named.
        Kind::Artist => format!("{verb} {name}"),
    };
    // The icon is given: an artist's name may hold any word a label is
    // told apart by.
    menu.entry(Entry::new(&label).icon(Icon::Ban))
        .then(|| Action::SetBlocked {
            kind,
            id: id.to_owned(),
            name: name.to_owned(),
            blocked: !blocked,
        })
}

/// What a right click on an artist's name offers, where the name is a
/// link to their page.
pub fn artist(state: &State, menu: &mut Menu<'_>, actions: &mut Vec<Action>, artist: &ArtistRef) {
    let label = format!("Go to {}", artist.name);
    if menu.entry(Entry::new(&label).icon(Icon::User)) {
        let id = widgets::artist_page_id(&artist.id).to_owned();
        actions.push(Action::Open(Page::Artist(id)));
    }
    menu.separator();
    actions.extend(block(state, menu, Kind::Artist, &artist.id, &artist.name));
    menu.separator();
    let youtube = || Action::Share {
        kind: crate::share::Kind::Artist,
        id: artist.id.clone(),
    };
    let elsewhere = Shared::Artist {
        id: artist.id.clone(),
        name: artist.name.clone(),
    };
    actions.extend(share(menu, youtube, Some(elsewhere)));
}

/// What a right click on an album's name offers, where the name is a link
/// to its page.
pub fn album(state: &State, menu: &mut Menu<'_>, actions: &mut Vec<Action>, album: &AlbumRef) {
    // One with no page can only be read.
    if album.id.is_empty() {
        menu.note(&album.name);
        return;
    }
    if menu.item("Go to album") {
        actions.push(Action::Open(Page::Album(album.id.clone())));
    }
    menu.separator();
    actions.extend(block(state, menu, Kind::Album, &album.id, &album.name));
    menu.separator();
    let youtube = || Action::Share {
        kind: crate::share::Kind::Album,
        id: album.id.clone(),
    };
    let elsewhere = Shared::Album(album.id.clone());
    actions.extend(share(menu, youtube, Some(elsewhere)));
}

/// "Share": chosen, it copies the YouTube Music link; beside it, a menu
/// of the links there are to choose from, which are the same one and the
/// ones on other services. `elsewhere` is what is looked for there, for
/// what is on them: a playlist here is YouTube Music's alone.
pub fn share(
    menu: &mut Menu<'_>,
    youtube: impl Fn() -> Action,
    elsewhere: Option<Shared>,
) -> Option<Action> {
    let mut chosen = None;
    let entry = Entry::new("Share").icon(Icon::Share);
    if menu.entry_with_more(entry, |menu| chosen = links(menu, &youtube, elsewhere)) {
        chosen = Some(youtube());
    }
    chosen
}

/// What a share button does for the song that is playing: a click copies
/// its YouTube Music link, and a right click offers every link there is.
pub fn share_button(
    state: &State,
    response: &eframe::egui::Response,
    actions: &mut Vec<Action>,
    track: &Track,
) {
    let youtube = || Action::Share {
        kind: crate::share::Kind::Track,
        id: track.id.clone(),
    };
    if response.clicked() {
        actions.push(youtube());
    }
    widgets::menu::context(response, &state.palette, |menu| {
        let elsewhere = Shared::Song(Box::new(track.clone()));
        actions.extend(links(menu, &youtube, Some(elsewhere)));
    });
}

/// One entry for each link there is to copy.
pub fn links(
    menu: &mut Menu<'_>,
    youtube: &dyn Fn() -> Action,
    elsewhere: Option<Shared>,
) -> Option<Action> {
    let mut chosen = None;
    if menu.entry(Entry::new("YouTube Music link").icon(Icon::Copy)) {
        chosen = Some(youtube());
    }
    let Some(shared) = elsewhere else {
        return chosen;
    };
    for service in Service::EVERY {
        let label = format!("{} link", service.name());
        if menu.entry(Entry::new(&label).icon(Icon::Copy)) {
            chosen = Some(Action::ShareOn(service, shared.clone()));
        }
    }
    chosen
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
