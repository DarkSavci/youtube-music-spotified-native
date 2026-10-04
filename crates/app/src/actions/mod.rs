//! What the person, or the backend, asked for, and the one function that
//! carries it out.
//!
//! `apply` only changes [`State`]; anything with a side effect (disk, the
//! core, a timer) is returned as an [`Effect`] for the app to run. That
//! keeps every rule here testable without a window.

mod library;
mod loading;
mod playback;
mod together;
mod types;

pub use types::{Action, Effect};

use std::time::Instant;

use spotified_audio::eq::RANGE_DB as EQ_RANGE_DB;
use spotified_client::session::Command;

use crate::backend::Request;
use crate::settings::RightPanel;
use crate::state::{Dialog, Loadable, MiniOpened, MiniPanel, Page, Playback, State};
use crate::theme;
use crate::update;
use library::organise;
use loading::{load_current_page, play_collection, run_new_search, run_search, store};
use playback::{control, enqueue, want_lyrics};

/// How many browse tiles have their pictures fetched at the same time.
const TILE_ART_AT_ONCE: usize = 2;

/// The longest crossfade on offer.
pub const MAX_CROSSFADE_SECONDS: u32 = 12;

pub fn apply(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::Open(page) => {
            state.nav.open(page);
            state.selection.clear();
            load_current_page(state)
        }
        Action::Back => {
            state.nav.back();
            state.selection.clear();
            load_current_page(state)
        }
        Action::Forward => {
            state.nav.forward();
            state.selection.clear();
            load_current_page(state)
        }
        Action::Select { list, row, how } => {
            state.selection.click(list, row, how);
            Vec::new()
        }
        Action::SelectAll { list, len } => {
            state.selection.select_all(list, len);
            Vec::new()
        }
        Action::StepSelection {
            list,
            step,
            len,
            extend,
        } => {
            state.selection.step(list, step, len, extend);
            Vec::new()
        }
        Action::ClearSelection => {
            state.selection.clear();
            Vec::new()
        }
        Action::ToggleSidebar => {
            state.settings.sidebar_visible = !state.settings.sidebar_visible;
            vec![Effect::SaveSettings]
        }
        Action::ResizeSidebar(width) => {
            let width = width
                .clamp(theme::SIDEBAR_MIN_WIDTH, theme::SIDEBAR_MAX_WIDTH)
                .round();
            if width == state.settings.sidebar_width {
                return Vec::new();
            }
            state.settings.sidebar_width = width;
            vec![Effect::SaveSettings]
        }
        Action::FilterLibrary(kind) => {
            state.library_filter = (state.library_filter != Some(kind)).then_some(kind);
            Vec::new()
        }
        Action::SetLibraryQuery(query) => {
            state.library_query = query;
            Vec::new()
        }
        Action::CycleLibrarySort => {
            state.settings.library_sort = state.settings.library_sort.next();
            vec![Effect::SaveSettings]
        }
        Action::SetSearchQuery(query) => {
            let has_query = !query.trim().is_empty();
            state.search.query = query;
            if !has_query {
                state.search.results = Loadable::NotLoaded;
                state.search.suggestions.clear();
                // The empty search page has things of its own to show.
                return load_current_page(state);
            }
            // Typing a query is a wish to see results, as in Spotify.
            state.nav.open(Page::Search);
            vec![Effect::DebounceSearch]
        }
        Action::RunSearch => run_new_search(state),
        Action::BrowseAll => {
            state.search.query.clear();
            state.search.results = Loadable::NotLoaded;
            state.search.suggestions.clear();
            state.nav.open(Page::Search);
            load_current_page(state)
        }
        Action::WantTileArt(id, params) => {
            // Each picture costs a request to YouTube, so they are fetched
            // a couple at a time; a tile not served now asks again.
            let waiting = state.tile_art.values().filter(|art| art.is_none()).count();
            let key = (id, params);
            if waiting >= TILE_ART_AT_ONCE || state.tile_art.contains_key(&key) {
                return Vec::new();
            }
            state.tile_art.insert(key.clone(), None);
            vec![Effect::Fetch(Request::TileArt(key.0, key.1))]
        }
        Action::Search(query) => {
            state.search.query = query;
            state.nav.open(Page::Search);
            run_new_search(state)
        }
        Action::StartRadio(track) => vec![Effect::Fetch(Request::StartRadio {
            device_id: state.settings.device_id.clone(),
            track: Box::new(track),
        })],
        Action::StartMix { seed, origin } => vec![Effect::Fetch(Request::StartMix {
            device_id: state.settings.device_id.clone(),
            seed,
            origin,
        })],
        Action::ClearCache => vec![Effect::Fetch(Request::ClearCache)],
        Action::SetPinned { .. }
        | Action::MoveToFolder { .. }
        | Action::NewFolder
        | Action::DeleteFolder(_)
        | Action::ToggleFolder(_) => organise(state, action),
        Action::SetStatsPeriod(days) => {
            state.stats_days = days;
            state.stats = Loadable::NotLoaded;
            load_current_page(state)
        }
        Action::SetSearchFilter(filter) => {
            state.search.filter = filter;
            run_search(state)
        }
        Action::CoreChanged(status) => {
            state.core = status;
            if !state.core_ready() {
                return Vec::new();
            }
            let mut effects = load_current_page(state);
            state.library = Loadable::Loading;
            effects.push(Effect::Fetch(Request::Library));
            effects.push(Effect::Fetch(Request::Liked));
            effects.push(Effect::Fetch(Request::Mixes));
            effects.push(Effect::Fetch(Request::Account));
            effects.push(Effect::Fetch(Request::Folders));
            effects
        }
        Action::Play {
            tracks,
            index,
            origin,
        } => vec![Effect::Command(Command::Play {
            tracks,
            start_index: index,
            origin,
        })],
        Action::PlayCollection(page) => play_collection(state, page),
        Action::Next => vec![Effect::Command(Command::Next)],
        Action::Previous => vec![Effect::Command(Command::Previous)],
        Action::JumpTo(index) => vec![Effect::Command(Command::Jump(index))],
        Action::RemoveFromQueue(index) => vec![Effect::Command(Command::Remove(index))],
        Action::MoveInQueue { from, to } => vec![Effect::Command(Command::Move { from, to })],
        Action::AddToQueue(tracks) => enqueue(state, tracks, false),
        Action::PlayNext(tracks) => enqueue(state, tracks, true),
        Action::ToggleLike(track) => {
            let liked = state.likes.toggle(&track.id);
            vec![Effect::Fetch(Request::SetLiked {
                track_id: track.id,
                liked,
            })]
        }
        Action::LikeAll(tracks) => {
            let mut effects = Vec::new();
            for track in tracks {
                if !state.likes.is_liked(&track.id) {
                    state.likes.toggle(&track.id);
                    effects.push(Effect::Fetch(Request::SetLiked {
                        track_id: track.id,
                        liked: true,
                    }));
                }
            }
            effects
        }
        Action::AddToPlaylist {
            playlist_id,
            playlist_title,
            track_ids,
        } => vec![Effect::Fetch(Request::AddToPlaylist {
            playlist_id,
            playlist_title,
            track_ids,
        })],
        Action::NewPlaylist { track_ids } => {
            state.dialog = Some(Dialog::NewPlaylist {
                name: String::new(),
                track_ids,
            });
            Vec::new()
        }
        Action::AskDeletePlaylist { playlist_id, title } => {
            state.dialog = Some(Dialog::DeletePlaylist { playlist_id, title });
            Vec::new()
        }
        Action::SetDialogText(text) => {
            if let Some(Dialog::NewPlaylist { name, .. } | Dialog::NewFolder { name }) =
                &mut state.dialog
            {
                *name = text;
            }
            Vec::new()
        }
        Action::CloseDialog => {
            state.dialog = None;
            Vec::new()
        }
        Action::ConfirmDialog => match state.dialog.take() {
            Some(Dialog::NewPlaylist { name, track_ids }) if !name.trim().is_empty() => {
                vec![Effect::Fetch(Request::CreatePlaylist {
                    title: name.trim().to_owned(),
                    track_ids,
                })]
            }
            Some(Dialog::DeletePlaylist { playlist_id, title }) => {
                vec![Effect::Fetch(Request::DeletePlaylist {
                    playlist_id,
                    title,
                })]
            }
            Some(Dialog::NewFolder { name }) if !name.trim().is_empty() => {
                vec![Effect::Fetch(Request::CreateFolder(name.trim().to_owned()))]
            }
            // Nothing typed yet: the dialog stays for a name.
            unnamed => {
                state.dialog = unnamed;
                Vec::new()
            }
        },
        Action::RemoveFromPlaylist { playlist_id, items } => {
            vec![Effect::Fetch(Request::RemoveFromPlaylist {
                playlist_id,
                items,
            })]
        }
        Action::ToggleFollow(artist_id) => {
            // Shown at once; the answer confirms it or puts it back.
            let Some(artist) = state.artists.loaded_mut(&artist_id) else {
                return Vec::new();
            };
            artist.following = !artist.following;
            vec![Effect::Fetch(Request::SetFollowing {
                follow: artist.following,
                artist_id,
            })]
        }
        Action::CopyLink(link) => {
            state.toast("Link copied");
            vec![Effect::CopyToClipboard(link)]
        }
        Action::ToggleQueue => {
            state.settings.panel = state.settings.panel.toggled(RightPanel::Queue);
            vec![Effect::SaveSettings]
        }
        Action::SetEqualizerOn(on) => {
            state.settings.equalizer_on = on;
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::SetEqualizerBand(band, decibels) => {
            let Some(gain) = state.settings.equalizer.get_mut(band) else {
                return Vec::new();
            };
            *gain = decibels.clamp(-EQ_RANGE_DB, EQ_RANGE_DB);
            // Moving a slider is a wish to hear it.
            state.settings.equalizer_on = true;
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::SetEqualizer(gains) => {
            state.settings.equalizer = gains;
            state.settings.equalizer_on = true;
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::SetStartAtLogin(on) => {
            state.starts_at_login = on;
            vec![Effect::SetStartAtLogin(on)]
        }
        Action::SetSystemTitleBar(on) => {
            state.settings.system_title_bar = on;
            vec![Effect::SaveSettings, Effect::SetDecorations(on)]
        }
        Action::SetCloseToTray(on) => {
            state.settings.close_to_tray = on;
            vec![Effect::SaveSettings]
        }
        // The palette itself follows the settings on the next frame.
        Action::SetTheme(choice) => {
            state.settings.theme = choice;
            state.settings.custom_theme = None;
            vec![Effect::SaveSettings]
        }
        Action::SetCustomTheme(file) => {
            state.settings.custom_theme = Some(file);
            vec![Effect::SaveSettings]
        }
        Action::OpenThemesFolder => vec![Effect::OpenThemesFolder],
        Action::ReloadThemes => vec![Effect::ReloadThemes],
        Action::SetVisualizer(on) => {
            state.settings.visualizer = on;
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::SetCrossfade(seconds) => {
            state.settings.crossfade_seconds = seconds.min(MAX_CROSSFADE_SECONDS);
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::SetNormaliseVolume(on) => {
            state.settings.normalise_volume = on;
            vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
        }
        Action::ToggleMiniPlayer => {
            state.mini_player = !state.mini_player;
            if state.mini_player {
                state.mini_panel = MiniPanel::Art;
                state.mini_opened = MiniOpened {
                    size: state.settings.mini_size,
                    position: state.settings.mini_position,
                };
                return Vec::new();
            }
            // Where it was left is where it opens next time.
            vec![Effect::SaveSettings]
        }
        Action::SetMiniPanel(panel) => {
            state.mini_panel = panel;
            if panel == MiniPanel::Art {
                return Vec::new();
            }
            // The queue and the lyrics need a tall window to show in.
            let mut effects = vec![Effect::GrowMini(crate::views::mini::PANEL_SIZE)];
            effects.extend(want_lyrics(state));
            effects
        }
        Action::SetMiniOnTop(on) => {
            state.settings.mini_on_top = on;
            vec![Effect::SaveSettings]
        }
        Action::MiniMoved { position, size } => {
            state.settings.mini_position = Some(position);
            state.settings.mini_size = size;
            Vec::new()
        }
        Action::ShowMainWindow => vec![Effect::ShowMainWindow],
        Action::SetLyricsFullscreen(on) => {
            state.lyrics_fullscreen = on;
            let mut effects = vec![Effect::SetFullscreen(on)];
            effects.extend(want_lyrics(state));
            effects
        }
        Action::ToggleLyrics => {
            state.settings.panel = state.settings.panel.toggled(RightPanel::Lyrics);
            let mut effects = vec![Effect::SaveSettings];
            effects.extend(want_lyrics(state));
            effects
        }
        Action::TogglePlay
        | Action::SetPlaying(_)
        | Action::Seek(_)
        | Action::SeekBy(_)
        | Action::VolumeBy(_)
        | Action::SetVolume(_)
        | Action::ToggleMute
        | Action::ToggleShuffle
        | Action::CycleRepeat => control(state, action),
        Action::SessionChanged(projection) => {
            state.playback = Some(Playback {
                session: projection.state,
                received: Instant::now(),
                offline: projection.offline,
                following_room: projection.following_room,
                room_ended: projection
                    .room
                    .filter(|room| room.ended)
                    .map(|room| room.entry),
            });
            want_lyrics(state)
        }
        Action::SwitchChannel(channel_id) => {
            if channel_id == state.channel_id {
                return Vec::new();
            }
            state.channel_id.clone_from(&channel_id);
            vec![Effect::SwitchChannel(channel_id)]
        }
        Action::OpenLogs => vec![Effect::OpenLogs],
        Action::UpdateResolver => {
            if state.updating_resolver {
                return Vec::new();
            }
            state.updating_resolver = true;
            vec![Effect::UpdateResolver]
        }
        Action::ResolverUpdated(result) => {
            state.updating_resolver = false;
            match result {
                Ok(said) => state.toast(said),
                Err(error) => state.toast_error(format!("yt-dlp could not be updated: {error}")),
            }
            Vec::new()
        }
        Action::TogetherField(..)
        | Action::TogetherMode(_)
        | Action::TogetherCreate
        | Action::TogetherJoin
        | Action::TogetherLeave
        | Action::TogetherEvent(_)
        | Action::TogetherTick => together::together(state, action),
        Action::CopyText { text, said } => {
            state.toast(said);
            vec![Effect::CopyToClipboard(text)]
        }
        Action::CheckForUpdate => {
            if state.update.busy() {
                return Vec::new();
            }
            state.update = update::Status::Checking;
            vec![Effect::CheckForUpdate]
        }
        Action::UpdateChanged(status) => {
            if let update::Status::Ready { version, .. } = &status
                && state.update != status
            {
                state.toast(format!(
                    "Version {version} is ready. Restart to update, in Settings."
                ));
            }
            state.update = status;
            Vec::new()
        }
        Action::InstallUpdate => match &state.update {
            update::Status::Ready { installer, .. } => {
                vec![Effect::InstallUpdate(installer.clone())]
            }
            _ => Vec::new(),
        },
        Action::SignIn => {
            if state.signing_in {
                return Vec::new();
            }
            state.signing_in = true;
            state.import_error = None;
            vec![Effect::SignIn]
        }
        Action::SignOut => vec![Effect::SignOut],
        Action::ImportSignIn => match &state.import_source {
            Some(source) => vec![Effect::ImportSignIn(source.clone())],
            None => Vec::new(),
        },
        Action::AccountChanged => {
            state.import_error = None;
            state.signing_in = false;
            state.forget_account_data();
            Vec::new()
        }
        Action::SignInFailed(message) => {
            state.signing_in = false;
            state.import_error = Some(message);
            Vec::new()
        }
        Action::Loaded(response) => store(state, *response),
    }
}

#[cfg(test)]
mod tests;
