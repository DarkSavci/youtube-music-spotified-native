//! Everything the views read.
//!
//! Views get `&State` and can only ask for changes through
//! [`crate::actions::Action`]; `actions::apply` is the one place state moves.

pub mod likes;
pub mod loadable;
pub mod nav;
pub mod playback;
pub mod selection;

pub use likes::Likes;
pub use loadable::{Loadable, PageCache};
pub use nav::{Nav, Page, Surface};
pub use playback::Playback;
pub use selection::{Select, Selection};

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use spotified_client::models::{
    Account, Album, Artist, Artwork, BrowsePage, CacheUsage, Channel, Folder, LibraryItem,
    LibraryKind, Lyrics, Mix, Playlist, Podcast, SearchFilter, SearchResults, Stats, Track,
};

use spotified_audio::tap::Tap;

use crate::images::Images;
use crate::settings::Settings;
use crate::sidecar::CoreStatus;
use crate::theme::{self, Palette};
use crate::together::Together;
use crate::update;

// How many fetched pages of each kind are kept. Reopening one of these is
// instant; anything older is fetched again, from the core's own cache.
const ALBUMS_KEPT: usize = 16;
const ARTISTS_KEPT: usize = 10;
const PLAYLISTS_KEPT: usize = 12;
const SURFACES_KEPT: usize = 8;
const PODCASTS_KEPT: usize = 6;
/// What unmuting returns to when nothing was heard before the mute.
pub const DEFAULT_VOLUME: f32 = 0.5;

/// A question that waits for an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    NewPlaylist {
        name: String,
        /// Songs to put in it once it exists.
        track_ids: Vec<String>,
    },
    DeletePlaylist {
        playlist_id: String,
        title: String,
    },
    NewFolder {
        name: String,
    },
}

/// What a tall mini player gives its middle to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MiniPanel {
    /// The cover.
    #[default]
    Art,
    Queue,
    Lyrics,
}

/// The mini player's window as it was asked for when opened. Held apart
/// from the settings, which follow the window as it is moved: asking the
/// system for those every frame would fight the person moving it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MiniOpened {
    pub size: [f32; 2],
    pub position: Option<[f32; 2]>,
}

/// The lyrics held, and which track they are of. They are fetched when the
/// lyrics panel is open and the track changes.
#[derive(Default)]
pub struct TrackLyrics {
    pub track_id: String,
    /// `Loaded(None)` when no source has lyrics for the track.
    pub words: Loadable<Option<Lyrics>>,
}

/// How long a toast stays.
const TOAST_LIFE: Duration = Duration::from_millis(3200);
/// How many are shown at once; an older one makes way.
const TOASTS_SHOWN: usize = 4;

pub struct Toast {
    pub text: String,
    /// Something went wrong, as against something done.
    pub error: bool,
    shown_at: Instant,
}

#[derive(Default)]
pub struct Search {
    /// What is typed in the top bar's field.
    pub query: String,
    /// What the results are narrowed to.
    pub filter: SearchFilter,
    /// Counts searches sent, so a late answer to an older query is dropped.
    pub serial: u64,
    pub results: Loadable<SearchResults>,
    /// Ways the query might go on, for the search on screen.
    pub suggestions: Vec<String>,
    /// What the account searched for lately, newest first.
    pub recent: Vec<String>,
}

pub struct State {
    pub palette: Palette,
    /// The themes found in the themes folder.
    pub themes: Vec<crate::themes::Custom>,
    pub nav: Nav,
    pub core: CoreStatus,
    pub settings: Settings,
    pub search: Search,
    pub home: Loadable<BrowsePage>,
    pub library: Loadable<Vec<LibraryItem>>,
    /// The library's folders, by name.
    pub folders: Vec<Folder>,
    /// The folders showing what they hold.
    pub open_folders: HashSet<String>,
    /// Made from local play history; empty until there is enough of it.
    pub mixes: Vec<Mix>,
    pub stats: Loadable<Stats>,
    /// The period the stats cover, in days.
    pub stats_days: u32,
    /// What is typed in the library's own search field.
    pub library_query: String,
    /// Which kind the sidebar's chips have narrowed the library to.
    pub library_filter: Option<LibraryKind>,
    pub albums: PageCache<String, Album>,
    pub artists: PageCache<String, Artist>,
    pub playlists: PageCache<String, Playlist>,
    pub podcasts: PageCache<String, Podcast>,
    /// Explore, the moods and the like, by id and params.
    pub surfaces: PageCache<(String, String), BrowsePage>,
    /// The picture on each browse tile, by its surface's id and params.
    /// `None` while it is being fetched; empty if there is none to show.
    pub tile_art: HashMap<(String, String), Option<Vec<Artwork>>>,
    /// What the account has played lately, newest first.
    pub history: Loadable<Vec<Track>>,
    /// Who is signed in, once the core has said.
    pub account: Option<Account>,
    /// The channels the account can act as. One, or none, for most.
    pub channels: Vec<Channel>,
    /// The channel in use; empty for the account's own.
    pub channel_id: String,
    /// yt-dlp is being updated.
    pub updating_resolver: bool,
    /// Where looking for a newer version of the app has got to.
    pub update: update::Status,
    /// Listen Together: the room, if in one.
    pub together: Together,
    /// How much room the kept songs take, once the core has said.
    pub cache_usage: Option<CacheUsage>,
    pub images: Images,
    pub likes: Likes,
    /// Something asked to be played whose songs are still being fetched.
    pub pending_play: Option<Page>,
    /// The question in front of everything, when there is one.
    pub dialog: Option<Dialog>,
    pub lyrics: TrackLyrics,
    /// The lyrics have the whole window, and the window the whole screen.
    pub lyrics_fullscreen: bool,
    /// The mini player's window is open.
    pub mini_player: bool,
    /// What the mini player shows above its controls when it is tall.
    pub mini_panel: MiniPanel,
    /// The size and place the mini player was opened with.
    pub mini_opened: MiniOpened,
    pub selection: Selection,
    /// Newest last.
    pub toasts: Vec<Toast>,
    /// What is being played, to draw; `None` until there is an engine.
    pub audio_tap: Option<Arc<Tap>>,
    /// `None` until the core has said what the session holds.
    pub playback: Option<Playback>,
    /// The volume to go back to when unmuting.
    pub volume_before_mute: f32,
    /// The Electron app's credentials file, when it has one to copy.
    pub import_source: Option<PathBuf>,
    /// Why the last attempt to sign in, or to copy a sign-in, failed.
    pub import_error: Option<String>,
    /// Whether the app is set to start with Windows.
    pub starts_at_login: bool,
    /// A browser window is open for the person to sign in with.
    pub signing_in: bool,
}

impl State {
    pub fn new(settings: Settings) -> Self {
        Self {
            palette: theme::DARK,
            themes: Vec::new(),
            nav: Nav::default(),
            core: CoreStatus::Starting,
            settings,
            search: Search::default(),
            home: Loadable::NotLoaded,
            library: Loadable::NotLoaded,
            folders: Vec::new(),
            open_folders: HashSet::new(),
            mixes: Vec::new(),
            stats: Loadable::NotLoaded,
            stats_days: 30,
            library_query: String::new(),
            library_filter: None,
            albums: PageCache::new(ALBUMS_KEPT),
            artists: PageCache::new(ARTISTS_KEPT),
            playlists: PageCache::new(PLAYLISTS_KEPT),
            podcasts: PageCache::new(PODCASTS_KEPT),
            surfaces: PageCache::new(SURFACES_KEPT),
            tile_art: HashMap::new(),
            history: Loadable::NotLoaded,
            account: None,
            channels: Vec::new(),
            channel_id: String::new(),
            updating_resolver: false,
            update: update::Status::Idle,
            together: Together::default(),
            cache_usage: None,
            images: Images::default(),
            likes: Likes::default(),
            dialog: None,
            pending_play: None,
            lyrics: TrackLyrics::default(),
            lyrics_fullscreen: false,
            mini_player: false,
            mini_panel: MiniPanel::Art,
            mini_opened: MiniOpened::default(),
            selection: Selection::default(),
            toasts: Vec::new(),
            audio_tap: None,
            playback: None,
            volume_before_mute: DEFAULT_VOLUME,
            import_source: None,
            import_error: None,
            signing_in: false,
            starts_at_login: false,
        }
    }

    /// Forgets everything fetched, for when the account behind it changes.
    /// Artwork stays: a cover is the same whoever is signed in.
    pub fn forget_account_data(&mut self) {
        self.core = CoreStatus::Starting;
        self.playback = None;
        self.likes.clear();
        self.home = Loadable::NotLoaded;
        self.library = Loadable::NotLoaded;
        self.folders.clear();
        self.stats = Loadable::NotLoaded;
        self.mixes.clear();
        self.search.results = Loadable::NotLoaded;
        self.search.suggestions.clear();
        self.search.recent.clear();
        self.history = Loadable::NotLoaded;
        self.account = None;
        self.channels.clear();
        self.surfaces = PageCache::new(SURFACES_KEPT);
        self.podcasts = PageCache::new(PODCASTS_KEPT);
        self.albums = PageCache::new(ALBUMS_KEPT);
        self.artists = PageCache::new(ARTISTS_KEPT);
        self.playlists = PageCache::new(PLAYLISTS_KEPT);
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), false);
    }

    pub fn toast_error(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), true);
    }

    fn push_toast(&mut self, text: String, error: bool) {
        if self.toasts.len() == TOASTS_SHOWN {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast {
            text,
            error,
            shown_at: Instant::now(),
        });
    }

    /// Drops the toasts that have had their time. Returns how long until
    /// the next one is due to go, if any are left.
    pub fn expire_toasts(&mut self, now: Instant) -> Option<Duration> {
        self.toasts
            .retain(|toast| now.duration_since(toast.shown_at) < TOAST_LIFE);
        self.toasts
            .first()
            .map(|oldest| TOAST_LIFE.saturating_sub(now.duration_since(oldest.shown_at)))
    }

    /// Whether the newest release's notes are yet to be opened.
    pub fn release_notes_unread(&self) -> bool {
        self.settings.release_notes_read != crate::changelog::latest()
    }

    pub fn core_ready(&self) -> bool {
        matches!(self.core, CoreStatus::Ready { .. })
    }
}
