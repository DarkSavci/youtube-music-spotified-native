//! Right-click menus.

use eframe::egui::{self, Ui};
use spotified_client::models::{LibraryKind, Track};

use crate::actions::Action;
use crate::state::{Loadable, Page, State};
use crate::theme;

/// YouTube Music's own playlist of liked songs; liking is how songs get
/// there, not adding.
const LIKED_PLAYLIST: &str = "LM";

/// What can be done with a song, or with several selected together. The
/// entries that only make sense for one (its artist, its link) are left
/// out for several.
pub fn tracks(
    state: &State,
    ui: &mut Ui,
    actions: &mut Vec<Action>,
    tracks: &[&Track],
    editable_playlist: Option<&str>,
) {
    let Some(&first) = tracks.first() else {
        return;
    };
    let single = (tracks.len() == 1).then_some(first);
    ui.set_min_width(220.0);
    if single.is_none() {
        let count = format!("{} songs", tracks.len());
        ui.label(egui::RichText::new(count).font(theme::semibold(13.0)));
        ui.separator();
    }
    let owned = || {
        tracks
            .iter()
            .map(|&track| track.clone())
            .collect::<Vec<_>>()
    };
    let mut chosen = None;
    if ui.button("Add to queue").clicked() {
        chosen = Some(Action::AddToQueue(owned()));
    }
    if ui.button("Play next").clicked() {
        chosen = Some(Action::PlayNext(owned()));
    }
    if let Some(track) = single
        && ui.button("Start radio").clicked()
    {
        chosen = Some(Action::StartRadio(track.clone()));
    }
    ui.separator();
    if let Some(track) = single {
        let like = if state.likes.is_liked(&track.id) {
            "Remove from Liked Songs"
        } else {
            "Save to Liked Songs"
        };
        if ui.button(like).clicked() {
            chosen = Some(Action::ToggleLike(track.clone()));
        }
    }
    ui.menu_button("Add to playlist", |ui| {
        if let Some(action) = playlist_choice(state, ui, tracks) {
            chosen = Some(action);
        }
    });
    // Only entries that know their place in the playlist can leave it.
    let removable: Vec<(String, String)> = tracks
        .iter()
        .filter(|track| !track.playlist_item_id.is_empty())
        .map(|track| (track.id.clone(), track.playlist_item_id.clone()))
        .collect();
    if let Some(playlist_id) = editable_playlist
        && !removable.is_empty()
        && ui.button("Remove from this playlist").clicked()
    {
        chosen = Some(Action::RemoveFromPlaylist {
            playlist_id: playlist_id.to_owned(),
            items: removable,
        });
    }
    if let Some(track) = single {
        ui.separator();
        if let Some(artist) = track.artists.iter().find(|artist| !artist.id.is_empty())
            && ui.button("Go to artist").clicked()
        {
            chosen = Some(Action::Open(Page::Artist(artist.id.clone())));
        }
        if let Some(album) = track.album.as_ref().filter(|album| !album.id.is_empty())
            && ui.button("Go to album").clicked()
        {
            chosen = Some(Action::Open(Page::Album(album.id.clone())));
        }
        ui.separator();
        if ui.button("Copy link").clicked() {
            chosen = Some(Action::CopyLink(format!(
                "https://music.youtube.com/watch?v={}",
                track.id
            )));
        }
    }
    if let Some(action) = chosen {
        actions.push(action);
        ui.close();
    }
}

/// The person's playlists, to add `tracks` to one.
fn playlist_choice(state: &State, ui: &mut Ui, tracks: &[&Track]) -> Option<Action> {
    ui.set_min_width(200.0);
    let track_ids = || tracks.iter().map(|track| track.id.clone()).collect();
    if ui.button("New playlist…").clicked() {
        return Some(Action::NewPlaylist {
            track_ids: track_ids(),
        });
    }
    ui.separator();
    let Loadable::Loaded(library) = &state.library else {
        ui.weak("Sign in to add to your playlists");
        return None;
    };
    let mut playlists = library
        .iter()
        .filter(|item| item.kind == LibraryKind::Playlist && item.id != LIKED_PLAYLIST)
        .peekable();
    if playlists.peek().is_none() {
        ui.weak("No playlists yet");
        return None;
    }
    let mut chosen = None;
    for playlist in playlists {
        if ui.button(&playlist.title).clicked() {
            chosen = Some(Action::AddToPlaylist {
                playlist_id: playlist.id.clone(),
                playlist_title: playlist.title.clone(),
                track_ids: track_ids(),
            });
        }
    }
    chosen
}
