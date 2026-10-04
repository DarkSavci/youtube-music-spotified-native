//! What the person, or the backend, asked for, and the one function that
//! carries it out.
//!
//! `apply` only changes [`State`]; anything with a side effect (disk, the
//! core, a timer) is returned as an [`Effect`] for the app to run. That
//! keeps every rule here testable without a window.

mod account;
mod desktop;
mod equalizer;
mod library;
mod listening;
mod loading;
mod migration;
mod paging;
mod playback;
mod preferences;
mod songs;
mod together;
mod types;
pub mod video;

pub use account::busy as account_busy;
pub use types::{Action, Effect, VideoAsk};

use spotified_client::session::Command;

use crate::backend::Request;
use crate::settings::RightPanel;
use crate::share;
use crate::sidecar::CoreStatus;
use crate::state::{Dialog, Loadable, MiniOpened, MiniPanel, Page, State, Whole};
use crate::theme;
use crate::update;
use library::organise;
use listening::listening;
use loading::{choose_mood, load_current_page, play_collection, run_new_search, run_search, store};
use paging::paging;
use playback::{control, enqueue, want_lyrics};

/// How many browse tiles have their pictures fetched at the same time.
const TILE_ART_AT_ONCE: usize = 2;

/// The longest crossfade on offer.
pub const MAX_CROSSFADE_SECONDS: u32 = 12;

/// A page has just been opened: what belonged to the last one goes, and
/// what this one shows is asked for.
fn arrived(state: &mut State) -> Vec<Effect> {
    state.selection.clear();
    // The library, given the page's room, hands it back to the page.
    state.library_expanded = false;
    state.about_expanded = false;
    load_current_page(state)
}

pub fn apply(state: &mut State, action: Action) -> Vec<Effect> {
    // Something else was asked to play while a playlist was being read to
    // its end: that playlist must not take over when it is.
    if matches!(
        action,
        Action::Play { .. }
            | Action::PlayCollection(_)
            | Action::PlayArtist { .. }
            | Action::StartRadio(_)
            | Action::StartMix { .. }
            | Action::ContinueFromRemote
    ) {
        state
            .preparing_playlist
            .take_if(|(_, then)| matches!(then, Whole::Play(_)));
    }
    match action {
        Action::Open(page) => {
            state.nav.open(page);
            arrived(state)
        }
        Action::Back => {
            state.nav.back();
            arrived(state)
        }
        Action::Forward => {
            state.nav.forward();
            arrived(state)
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
        Action::ChooseMood(params) => choose_mood(state, params),
        Action::HoldScroll(offset) => {
            state.held_scroll = Some(offset.max(0.0));
            Vec::new()
        }
        Action::MoreHome { .. }
        | Action::MorePlaylist { .. }
        | Action::PlayPlaylist { .. }
        | Action::WholePlaylist { .. }
        | Action::PlayArtist { .. }
        | Action::ContinueFromRemote => paging(state, action),
        Action::SetSongOrder(_) | Action::OpenMoreReleases => songs::songs(state, action),
        Action::SetStatsLookup(_)
        | Action::RunStatsLookup
        | Action::CloseStatsLookup
        | Action::OpenStat { .. }
        | Action::CloseStat
        | Action::WantArtistPhoto(_)
        | Action::ForgetSearch(_)
        | Action::ClearSearches
        | Action::ToggleAbout
        | Action::ShowWhatsNew => listening(state, action),
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
            state.library_expanded = false;
            vec![Effect::DebounceSearch]
        }
        Action::RunSearch => run_new_search(state),
        Action::BrowseAll => {
            state.search.query.clear();
            state.search.results = Loadable::NotLoaded;
            state.search.suggestions.clear();
            state.nav.open(Page::Search);
            arrived(state)
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
            state.library_expanded = false;
            run_new_search(state)
        }
        Action::StartRadio(track) => playback::start_radio(state, track),
        Action::StartMix { seed, origin } => vec![Effect::Fetch(Request::StartMix {
            device_id: state.settings.device_id.clone(),
            seed,
            origin,
        })],
        Action::ClearCache => vec![Effect::Fetch(Request::ClearCache)],
        Action::ToggleSidebar
        | Action::ToggleLibraryExpanded
        | Action::ToggleLibraryGrid
        | Action::FilterLibrary(_)
        | Action::SetLibraryQuery(_)
        | Action::SetLibrarySort(_)
        | Action::SetPinned { .. }
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
            // A core that goes away while in use takes playback and every
            // page with it; saying so beats letting each request fail.
            if let (true, CoreStatus::Failed(message)) = (state.core_ready(), &status) {
                state.toast_error(message.clone());
            }
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
        Action::NewPlaylist { name, track_ids } => {
            state.dialog = Some(Dialog::NewPlaylist { name, track_ids });
            Vec::new()
        }
        Action::AskDeletePlaylist { playlist_id, title } => {
            state.dialog = Some(Dialog::DeletePlaylist { playlist_id, title });
            Vec::new()
        }
        Action::SetDialogText(text) => {
            if let Some(
                Dialog::NewPlaylist { name, .. }
                | Dialog::NewFolder { name }
                | Dialog::SaveRoomHistory { name, .. },
            ) = &mut state.dialog
            {
                *name = text;
            }
            Vec::new()
        }
        Action::CloseDialog => match state.dialog.take() {
            Some(Dialog::Migration) => vec![Effect::MigrationSeen],
            _ => Vec::new(),
        },
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
            Some(
                dialog @ (Dialog::RemoveServer { .. }
                | Dialog::RemoveListener { .. }
                | Dialog::SaveRoomHistory { .. }),
            ) => together::confirmed(state, dialog),
            Some(dialog @ (Dialog::SignOut | Dialog::RemoveAccount { .. })) => {
                account::confirmed(state, dialog)
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
        Action::Share { kind, id } => {
            state.toast(format!("{} link copied to clipboard", kind.noun()));
            vec![Effect::CopyToClipboard(share::url(kind, &id))]
        }
        Action::ToggleQueue => {
            state.settings.panel = state.settings.panel.toggled(RightPanel::Queue);
            vec![Effect::SaveSettings]
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
        Action::Equalizer(ask) => equalizer::asked(state, ask),
        Action::SetVisualizer(_)
        | Action::SetCrossfade(_)
        | Action::SetNormaliseVolume(_)
        | Action::SetVolumeLevel(_)
        | Action::SetVolumeBoost(_)
        | Action::SetGapless(_)
        | Action::SetAutoplay(_)
        | Action::SetResumeOnLaunch(_)
        | Action::SetContinueFromYouTubeMusic(_)
        | Action::SetReportToYouTube(_)
        | Action::SetCacheSize(_)
        | Action::SetReduceMotion(_)
        | Action::SetShowMusicVideos(_)
        | Action::ToggleRemainingTime
        | Action::SetSpeed(_)
        | Action::ResetPreferences => preferences::preferences(state, action),
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
        Action::ShowMainWindow
        | Action::ToggleFlyout { .. }
        | Action::HideFlyout
        | Action::SaveReport
        | Action::ReportSaved(_)
        | Action::StartAtLoginKnown(_) => desktop::desktop(state, action),
        Action::SetLyricsFullscreen(on) => {
            state.lyrics_fullscreen = on;
            // The player may be open over them, holding the screen itself.
            let mut effects = vec![Effect::SetFullscreen(on || state.fullscreen_player)];
            effects.extend(want_lyrics(state));
            effects
        }
        Action::SetFullscreenPlayer(_) | Action::ToggleFullscreenPlayer => {
            playback::fullscreen_player(state, action)
        }
        Action::SaveCurrent => {
            // With nothing playing there is nothing to save, and the key
            // does nothing.
            match state
                .playback
                .as_ref()
                .and_then(|playback| playback.current())
            {
                Some(track) => apply(state, Action::ToggleLike(track.clone())),
                None => Vec::new(),
            }
        }
        Action::FocusSearch => {
            state.search_focus += 1;
            if state.nav.page() == &Page::Search {
                return Vec::new();
            }
            state.nav.open(Page::Search);
            arrived(state)
        }
        Action::Notify(notice) => {
            state.notice = Some(notice);
            Vec::new()
        }
        Action::DismissNotice => {
            if matches!(state.notice, Some(crate::state::Notice::Offline { .. })) {
                state.offline_dismissed = true;
            }
            state.notice = None;
            Vec::new()
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
            let mut effects = playback::session_changed(state, *projection);
            effects.extend(video::session_changed(state));
            effects
        }
        Action::Video(ask) => video::asked(state, ask),
        Action::SwitchChannel(_)
        | Action::SignIn
        | Action::SignOut
        | Action::SwitchAccount(_)
        | Action::AskRemoveAccount(_)
        | Action::AccountsChanged(_)
        | Action::RefreshChannels
        | Action::OpenAccountMenu
        | Action::AccountChanged
        | Action::SignInFailed(_) => account::account(state, action),
        Action::OpenMigration
        | Action::SetMigrationKind(..)
        | Action::StartMigration
        | Action::MigrationFound { .. }
        | Action::MigrationProgress(_)
        | Action::MigrationDone(_) => migration::migration(state, action),
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
        Action::Room(ask) => together::asked(state, ask),
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
            let mut effects = Vec::new();
            if let update::Status::Ready { version, .. } = &status
                && state.update != status
            {
                // It installs by itself when the app is next closed; said
                // in the window, and by Windows for a window out of sight.
                state.toast(format!(
                    "Version {version} is ready. It installs when you quit, or restart to \
                     update now, in Settings."
                ));
                effects.push(Effect::NotifyUpdate(version.clone()));
            }
            state.update = status;
            effects
        }
        Action::InstallUpdate => match &state.update {
            update::Status::Ready { installer, .. } => {
                vec![Effect::InstallUpdate(installer.clone())]
            }
            _ => Vec::new(),
        },
        Action::Loaded(response) => store(state, *response),
    }
}

#[cfg(test)]
mod tests;
