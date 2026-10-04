//! The "…" menu on an album's, a playlist's or an artist's page: what can
//! be done with the whole of it.

use eframe::egui;
use spotified_client::models::{LibraryKind, Track};

use super::widgets::menu::{self, Entry, Menu};
use crate::actions::Action;
use crate::blocked;
use crate::share;
use crate::state::{Loadable, State, Whole};
use crate::theme::Icon;

/// YouTube Music's own playlist of liked songs: liking is how songs get
/// there, and it cannot be deleted.
const LIKED_PLAYLIST: &str = "LM";
/// How many of the person's playlists the menu offers to add to.
const PLAYLISTS_OFFERED: usize = 12;

/// What the page shows.
pub struct Entity<'a> {
    pub kind: share::Kind,
    pub id: &'a str,
    pub title: &'a str,
    pub tracks: &'a [Track],
    /// A playlist of the person's own, which they may delete.
    pub deletable: bool,
}

/// Hangs the menu on `button`.
pub fn show(state: &State, actions: &mut Vec<Action>, button: &egui::Response, entity: &Entity) {
    menu::popup(button, &state.palette, |menu| {
        if let Some(action) = entries(state, menu, entity) {
            actions.push(whole(state, entity, action));
        }
    });
}

/// What was chosen, as it is asked of a playlist only part of which has
/// been read: the menu promises the whole of it, so the rest is read
/// first, and never is only the part on screen queued or copied in
/// silence. Anything else is asked as it stands.
fn whole(state: &State, entity: &Entity, action: Action) -> Action {
    let part_read =
        entity.kind == share::Kind::Playlist && state.playlist_tails.contains_key(entity.id);
    if !part_read {
        return action;
    }
    let then = match action {
        Action::AddToQueue(_) => Whole::Queue { next: false },
        Action::PlayNext(_) => Whole::Queue { next: true },
        Action::NewPlaylist { .. } => Whole::NewPlaylist,
        Action::AddToPlaylist {
            playlist_id,
            playlist_title,
            ..
        } => Whole::AddTo {
            playlist_id,
            playlist_title,
        },
        other => return other,
    };
    Action::WholePlaylist {
        id: entity.id.to_owned(),
        then,
    }
}

/// The menu's entries. Returns what was chosen.
fn entries(state: &State, menu: &mut Menu<'_>, entity: &Entity) -> Option<Action> {
    let mut chosen = None;
    let track_ids = || entity.tracks.iter().map(|track| track.id.clone()).collect();
    if !entity.tracks.is_empty() {
        if menu.item("Add to queue") {
            chosen = Some(Action::AddToQueue(entity.tracks.to_vec()));
        }
        if menu.item("Play next") {
            chosen = Some(Action::PlayNext(entity.tracks.to_vec()));
        }
        menu.separator();
        if menu.item("Add all to a new playlist…") {
            chosen = Some(Action::NewPlaylist {
                name: entity.title.to_owned(),
                track_ids: track_ids(),
            });
        }
        menu.submenu(Entry::new("Add all to…").icon(Icon::Plus), |menu| {
            let Loadable::Loaded(library) = &state.library else {
                menu.note("Sign in to add to your playlists");
                return;
            };
            // The page's own playlist is not somewhere to add it to.
            let mut playlists = library
                .iter()
                .filter(|item| item.kind == LibraryKind::Playlist)
                .filter(|item| item.id != LIKED_PLAYLIST && item.id != entity.id)
                .take(PLAYLISTS_OFFERED)
                .peekable();
            if playlists.peek().is_none() {
                menu.note("No playlists yet");
            }
            for playlist in playlists {
                let label = format!("Add all to {}", playlist.title);
                if menu.entry(Entry::new(&label).icon(Icon::SquareLibrary)) {
                    chosen = Some(Action::AddToPlaylist {
                        playlist_id: playlist.id.clone(),
                        playlist_title: playlist.title.clone(),
                        track_ids: track_ids(),
                    });
                }
            }
        });
    }
    if entity.deletable && entity.id != LIKED_PLAYLIST {
        menu.separator();
        if menu.entry(Entry::new("Delete playlist").danger()) {
            chosen = Some(Action::AskDeletePlaylist {
                playlist_id: entity.id.to_owned(),
                title: entity.title.to_owned(),
            });
        }
    }
    let blockable = match entity.kind {
        share::Kind::Album => Some(blocked::Kind::Album),
        share::Kind::Artist => Some(blocked::Kind::Artist),
        _ => None,
    };
    if let Some(kind) = blockable {
        menu.separator();
        let block = super::menus::block(state, menu, kind, entity.id, entity.title);
        chosen = block.or(chosen);
    }
    menu.separator();
    if menu.item("Share") {
        chosen = Some(Action::Share {
            kind: entity.kind,
            id: entity.id.to_owned(),
        });
    }
    chosen
}
