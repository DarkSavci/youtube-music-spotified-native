//! Everything the views read.
//!
//! Views get `&State` and can only ask for changes through
//! [`crate::actions::Action`]; `actions::apply` is the one place state moves.

pub mod artist_songs;
pub mod likes;
pub mod loadable;
pub mod migration;
pub mod more;
pub mod nav;
pub mod playback;
pub mod searches;
pub mod selection;
pub mod stats;
pub mod video;

pub use artist_songs::{ArtistSongs, SongOrder};
pub use likes::Likes;
pub use loadable::{Loadable, PageCache};
pub use migration::Migration;
pub use more::{HomeMore, Tail, Whole};
pub use nav::{Nav, Page, Surface};
pub use playback::Playback;
pub use searches::RecentSearch;
pub use selection::{Select, Selection};
pub use stats::{StatSelection, StatsPage};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use spotified_client::models::{
    Account, Affinity, Album, Artist, Artwork, BrowsePage, CacheUsage, Channel, Folder, HomeChip,
    LibraryItem, LibraryKind, Lyrics, Mix, Playlist, Podcast, SearchFilter, SearchHistoryEntry,
    SearchResults, Stats, Track,
};

use spotified_audio::tap::Tap;

use crate::accounts::Accounts;
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
    /// What's new: the latest releases' notes, over whatever is open.
    WhatsNew,
    /// Forget a saved Listen Together server?
    RemoveServer {
        id: String,
        name: String,
    },
    /// Sign the account in use out?
    SignOut,
    /// Forget a saved account?
    RemoveAccount {
        id: String,
        name: String,
    },
    /// What the Electron app has, and which of it to bring over.
    Migration,
    /// Remove a listener from the room?
    RemoveListener {
        member: String,
        name: String,
    },
    /// A name for the playlist a room's history is saved as.
    SaveRoomHistory {
        name: String,
        track_ids: Vec<String>,
    },
}

/// Why playback is not going on, when it is something the listener can
/// wait out or act on. A song that merely failed is not said here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// YouTube is refusing this device for now. Without saying so playback
    /// simply stops, and the next thing anyone does is try again and again,
    /// which is what prolongs it.
    RateLimited,
    /// The connection has gone. `paused` when nothing will start again by
    /// itself once it is back.
    Offline { paused: bool },
}

impl Notice {
    pub fn text(self) -> &'static str {
        match self {
            Notice::RateLimited => {
                "YouTube is rate-limiting this device. Playback will work again in a few minutes."
            }
            Notice::Offline { paused: false } => {
                "You\u{2019}re offline. Playback carries on from where it stopped when the \
                 connection is back."
            }
            Notice::Offline { paused: true } => {
                "You\u{2019}re offline. Press Play once the connection is back to carry on \
                 from where it stopped."
            }
        }
    }
}

/// Picking up the account's queue from another device when the app starts.
/// Decided once in a run of the app, as soon as the account is known.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LaunchPickup {
    /// Who is signed in is not known yet.
    #[default]
    Undecided,
    /// The queue is being read.
    Reading,
    Done,
}

/// The flyout by the tray icon, while it is open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flyout {
    /// Where its top-left corner is on the screen, in points.
    pub position: [f32; 2],
    /// When it was opened: what tells one opening from the next.
    pub opened: Instant,
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
/// One with a button stays longer: there is something to reach for.
const TOAST_LIFE_WITH_LINK: Duration = Duration::from_secs(9);
/// How many are shown at once; an older one makes way.
const TOASTS_SHOWN: usize = 4;

pub struct Toast {
    pub text: String,
    /// Something went wrong, as against something done.
    pub error: bool,
    /// Somewhere the toast leads, behind a button of its own.
    pub link: Option<ToastLink>,
    shown_at: Instant,
}

/// A button on a toast: what it says, and the page it opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastLink {
    pub label: &'static str,
    pub page: Page,
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
    pub account: Vec<SearchHistoryEntry>,
    /// The account's searches and this computer's, as the one list shown.
    pub recent: Vec<RecentSearch>,
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
    /// The mood chip Home is read through: its params, or empty for none.
    pub home_mood: String,
    /// Home's row of moods, as last seen.
    pub home_chips: Vec<HomeChip>,
    /// The shelves of Home below its first page, read as it is scrolled.
    pub home_more: HomeMore,
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
    pub stats_page: StatsPage,
    /// Artists' pictures for cards that came with only a name, by channel.
    /// `None` while one is being fetched; empty if there is none to show.
    pub artist_photos: HashMap<String, Option<Vec<Artwork>>>,
    /// What is typed in the library's own search field.
    pub library_query: String,
    /// Which kind the sidebar's chips have narrowed the library to.
    pub library_filter: Option<LibraryKind>,
    /// The library has the page's room, to be looked at as a whole.
    pub library_expanded: bool,
    pub albums: PageCache<String, Album>,
    pub artists: PageCache<String, Artist>,
    pub playlists: PageCache<String, Playlist>,
    /// The playlists with songs still to be read, and where reading each
    /// has got to. One whose every song is here has no entry.
    pub playlist_tails: HashMap<String, Tail>,
    /// A playlist whose remaining songs are being read, and what is to be
    /// done with it once they are here: playing it, or queueing it, means
    /// all of it.
    pub preparing_playlist: Option<(String, Whole)>,
    /// The listener's own history with the artist whose page is open.
    pub affinity: Option<(String, Affinity)>,
    /// The page of all of an artist's songs, for the artist last opened.
    pub artist_songs: Option<ArtistSongs>,
    /// The text about what the page shows is given in full, not clipped.
    pub about_expanded: bool,
    /// The queue on the account's other devices is being read.
    pub reading_remote_queue: bool,
    /// How far down the page is held, when `--open scroll:` asked for the
    /// lower part of a page to be looked at.
    pub held_scroll: Option<f32>,
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
    /// The accounts kept signed in, and which of them is in use.
    pub accounts: Accounts,
    /// Counts the times the account's menu was asked to open without a
    /// click on it, as `--open account-menu` asks.
    pub account_menu_asks: u64,
    /// The flyout by the tray icon, while it is open.
    pub flyout: Option<Flyout>,
    /// When the flyout was last closed.
    pub flyout_closed: Option<Instant>,
    /// Where the making of a problem report has got to.
    pub report: crate::report::Status,
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
    /// The music video of what plays: whether it shows, and its picture.
    pub video: video::Video,
    /// The lyrics have the whole window, and the window the whole screen.
    pub lyrics_fullscreen: bool,
    /// What is playing has the whole window, and the window the screen.
    pub fullscreen_player: bool,
    /// What stands in the way of playback, while there is something to say.
    pub notice: Option<Notice>,
    /// The offline notice was dismissed during this outage.
    pub offline_dismissed: bool,
    /// Counts the times the search field was asked to take the caret.
    pub search_focus: u64,
    pub launch_pickup: LaunchPickup,
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
    /// A name being typed for a curve of the equalizer's.
    pub equalizer_naming: Option<crate::equalizer::Naming>,
    /// `None` until the core has said what the session holds.
    pub playback: Option<Playback>,
    /// The volume to go back to when unmuting.
    pub volume_before_mute: f32,
    /// What the Electron app left on this computer, and the bringing of it.
    pub migration: Migration,
    /// Why the last attempt to sign in failed.
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
            home_mood: String::new(),
            home_chips: Vec::new(),
            home_more: HomeMore::default(),
            stats_page: StatsPage::default(),
            artist_photos: HashMap::new(),
            playlist_tails: HashMap::new(),
            preparing_playlist: None,
            affinity: None,
            artist_songs: None,
            about_expanded: false,
            reading_remote_queue: false,
            held_scroll: None,
            library: Loadable::NotLoaded,
            library_expanded: false,
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
            accounts: Accounts::default(),
            account_menu_asks: 0,
            flyout: None,
            flyout_closed: None,
            report: crate::report::Status::Idle,
            updating_resolver: false,
            update: update::Status::Idle,
            together: Together::default(),
            cache_usage: None,
            images: Images::default(),
            likes: Likes::default(),
            dialog: None,
            pending_play: None,
            lyrics: TrackLyrics::default(),
            video: video::Video::default(),
            lyrics_fullscreen: false,
            fullscreen_player: false,
            notice: None,
            offline_dismissed: false,
            search_focus: 0,
            launch_pickup: LaunchPickup::Undecided,
            mini_player: false,
            mini_panel: MiniPanel::Art,
            mini_opened: MiniOpened::default(),
            selection: Selection::default(),
            toasts: Vec::new(),
            audio_tap: None,
            equalizer_naming: None,
            playback: None,
            volume_before_mute: DEFAULT_VOLUME,
            migration: Migration::default(),
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
        self.home_mood.clear();
        self.home_chips.clear();
        self.home_more = HomeMore::default();
        self.stats_page = StatsPage::default();
        self.playlist_tails.clear();
        self.preparing_playlist = None;
        self.affinity = None;
        self.artist_songs = None;
        self.library = Loadable::NotLoaded;
        self.folders.clear();
        self.stats = Loadable::NotLoaded;
        self.mixes.clear();
        self.search.results = Loadable::NotLoaded;
        self.search.suggestions.clear();
        self.search.account.clear();
        self.refresh_recent_searches();
        self.history = Loadable::NotLoaded;
        self.account = None;
        self.channels.clear();
        self.surfaces = PageCache::new(SURFACES_KEPT);
        self.podcasts = PageCache::new(PODCASTS_KEPT);
        self.albums = PageCache::new(ALBUMS_KEPT);
        self.artists = PageCache::new(ARTISTS_KEPT);
        self.playlists = PageCache::new(PLAYLISTS_KEPT);
    }

    /// Whether a song among others is shown: a music video only when the
    /// settings ask for them. An album or a playlist shows all it holds.
    pub fn shows(&self, track: &Track) -> bool {
        self.settings.show_music_videos || !track.is_video
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), false);
    }

    pub fn toast_error(&mut self, text: impl Into<String>) {
        self.push_toast(text.into(), true);
    }

    /// A toast with a button that opens a page.
    pub fn toast_with_link(&mut self, text: impl Into<String>, label: &'static str, page: Page) {
        self.push_toast(text.into(), false);
        if let Some(toast) = self.toasts.last_mut() {
            toast.link = Some(ToastLink { label, page });
        }
    }

    fn push_toast(&mut self, text: String, error: bool) {
        if self.toasts.len() == TOASTS_SHOWN {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast {
            text,
            error,
            link: None,
            shown_at: Instant::now(),
        });
    }

    /// Makes the one list of recent searches again, after the account's
    /// or this computer's has changed.
    pub fn refresh_recent_searches(&mut self) {
        let scope = self.search_scope();
        let local = self.settings.recent_searches(&scope);
        self.search.recent = searches::merged(&self.search.account, local);
    }

    /// Whose recent searches are shown: the account in use and the channel
    /// it acts as, as the Electron app told them apart; empty for nobody.
    pub fn search_scope(&self) -> String {
        match self.accounts.active() {
            Some(account) if account.channel.is_empty() => format!("{}:personal", account.id),
            Some(account) => format!("{}:{}", account.id, account.channel),
            None => String::new(),
        }
    }

    /// Drops the toasts that have had their time. Returns how long until
    /// the next one is due to go, if any are left.
    pub fn expire_toasts(&mut self, now: Instant) -> Option<Duration> {
        let left = |toast: &Toast| {
            let life = match toast.link {
                Some(_) => TOAST_LIFE_WITH_LINK,
                None => TOAST_LIFE,
            };
            life.checked_sub(now.duration_since(toast.shown_at))
                .filter(|left| !left.is_zero())
        };
        self.toasts.retain(|toast| left(toast).is_some());
        self.toasts.iter().filter_map(left).min()
    }

    /// Whether the newest release's notes are yet to be opened.
    pub fn release_notes_unread(&self) -> bool {
        self.settings.release_notes_read != crate::changelog::latest()
    }

    /// Whether a Listen Together room, entered or on the way in, is holding
    /// playback to normal speed: a member playing faster would drift out of
    /// the room.
    pub fn speed_pinned(&self) -> bool {
        self.together.phase != crate::together::Phase::Idle
    }

    /// The speed that should be playing now: the one chosen, or normal in
    /// a room.
    pub fn speed(&self) -> f32 {
        if self.speed_pinned() {
            1.0
        } else {
            self.settings.playback_speed
        }
    }

    /// Whether nothing is going on here that a queue from elsewhere would
    /// interrupt: nothing playing or about to, and no room.
    pub fn idle(&self) -> bool {
        self.playback
            .as_ref()
            .is_none_or(|playback| !playback.following_room && !playback.wants_to_play())
    }

    pub fn core_ready(&self) -> bool {
        matches!(self.core, CoreStatus::Ready { .. })
    }
}
