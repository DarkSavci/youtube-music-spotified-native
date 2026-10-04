//! What can be asked for, and what carrying it out can call for.

use std::path::PathBuf;
use std::time::Instant;

use spotified_client::models::{Channel, LibraryKind, MixSeed, SearchFilter, StatKind, Track};
use spotified_client::session::{Command, Projection};

use crate::accounts::Accounts;
use crate::backend::{Request, Response};
use crate::migrate;
use crate::settings::{LibrarySort, VolumeLevel};
use crate::share;
use crate::sidecar::CoreStatus;
use crate::state::Notice;
use crate::state::{MiniPanel, Page, Select, SongOrder, Whole};
use crate::themes;
use crate::together;
use crate::update;

/// What is asked about the music video of what plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoAsk {
    /// Show the video, switching to that edit of the song if need be, or
    /// go back to the song.
    Set(bool),
    /// The pointer has reached a video button: find out whether the song
    /// has a video, if that is not known.
    Check,
    /// Fetch the picture again after it failed.
    Retry,
    /// The picture is on its way, or has arrived.
    Loading(bool),
    /// The picture could not be had.
    Failed,
    /// The player refused a command, which may have been the switch.
    Refused,
}

#[derive(Debug)]
pub enum Action {
    Open(Page),
    Back,
    Forward,
    /// Collapse the sidebar to a rail of covers, or widen it again.
    ToggleSidebar,
    /// Give the library the page's room, or take it back.
    ToggleLibraryExpanded,
    /// Show the library as a grid of covers, or as rows again.
    ToggleLibraryGrid,
    /// The sidebar was dragged to this width.
    ResizeSidebar(f32),
    /// A chip in the sidebar was clicked. Clicking the active one clears it.
    FilterLibrary(LibraryKind),
    /// The library's search field changed.
    SetLibraryQuery(String),
    /// An order was chosen from the library's sort menu.
    SetLibrarySort(LibrarySort),
    /// A mood chip on Home was clicked: its params. Clicking the chosen
    /// one goes back to plain Home.
    ChooseMood(String),
    /// The end of Home has been scrolled to: read its next few shelves.
    /// `retry` after a page that failed, which is not asked for again
    /// without it.
    MoreHome {
        retry: bool,
    },
    /// The end of the songs read of a playlist is near: read the next.
    MorePlaylist {
        id: String,
        retry: bool,
    },
    /// Play a playlist from this place in it: all of it, read to its end
    /// first if it has not been.
    PlayPlaylist {
        id: String,
        index: usize,
    },
    /// Do this with the whole of a playlist, read to its end first if it
    /// has not been.
    WholePlaylist {
        id: String,
        then: Whole,
    },
    /// Play every song of an artist's, most played first, or a shuffle of
    /// them.
    PlayArtist {
        artist_id: String,
        shuffle: bool,
    },
    /// Replace the queue with the one on the account's other devices.
    ContinueFromRemote,
    /// An order was chosen on the page of all an artist's songs.
    SetSongOrder(SongOrder),
    /// Open another batch of the artist's releases for their songs.
    OpenMoreReleases,
    /// Show the whole of the text about what the page shows, or clip it.
    ToggleAbout,
    /// Take this out of the recent searches, the account's too.
    ForgetSearch(String),
    /// Empty the recent searches.
    ClearSearches,
    /// The listening page's lookup field changed.
    SetStatsLookup(String),
    /// The typing in the lookup field has paused: ask for its matches.
    RunStatsLookup,
    /// Put the lookup's matches away.
    CloseStatsLookup,
    /// Show the listener's figures for a song, an artist or an album.
    OpenStat {
        kind: StatKind,
        id: String,
    },
    CloseStat,
    /// An artist's card came into view with no picture of them yet.
    WantArtistPhoto(String),
    /// Show the latest release notes over the page.
    ShowWhatsNew,
    /// Hold the page this far down, to look at the lower part of it.
    HoldScroll(f32),
    /// The search field's text changed.
    SetSearchQuery(String),
    /// The person has stopped typing: send the query on screen.
    RunSearch,
    /// Empty the search field and show what there is to browse.
    BrowseAll,
    /// A browse tile came into view and has no picture yet.
    WantTileArt(String, String),
    /// Search for this at once: a suggestion or an earlier search was chosen.
    Search(String),
    /// Play this song, then songs like it.
    StartRadio(Track),
    /// Play one of YouTube's own queues, such as an artist's shuffle.
    StartMix {
        seed: MixSeed,
        /// What the queue is called.
        origin: String,
    },
    /// Delete every song kept on disk.
    ClearCache,
    /// Keep a library item at the top, or stop.
    SetPinned {
        kind: LibraryKind,
        item_id: String,
        pinned: bool,
    },
    /// File a library item in a folder; an empty id takes it out of one.
    MoveToFolder {
        kind: LibraryKind,
        item_id: String,
        folder_id: String,
    },
    /// Ask for a name for a new folder.
    NewFolder,
    DeleteFolder(String),
    /// Show what a folder holds, or stop.
    ToggleFolder(String),
    /// A period chip on the stats page was clicked.
    SetStatsPeriod(u32),
    /// A chip on the search page was clicked.
    SetSearchFilter(SearchFilter),
    CoreChanged(CoreStatus),
    /// Queue these tracks and start at `index`.
    Play {
        tracks: Vec<Track>,
        index: usize,
        /// What the queue is called: the album's or playlist's name.
        origin: String,
    },
    /// Play an album, a playlist, or an artist's top songs from the start.
    PlayCollection(Page),
    TogglePlay,
    Next,
    Previous,
    /// Play if `true`, pause if `false`; nothing if it already is.
    SetPlaying(bool),
    Seek(u64),
    /// Move this many milliseconds from where the track is.
    SeekBy(i64),
    SetVolume(f32),
    VolumeBy(f32),
    /// Play the queue from this place in it.
    JumpTo(usize),
    RemoveFromQueue(usize),
    /// Move a queued song; `to` is its place once it has been taken out.
    MoveInQueue {
        from: usize,
        to: usize,
    },
    /// Add to the end of the queue.
    AddToQueue(Vec<Track>),
    /// Add to the queue right after what is playing.
    PlayNext(Vec<Track>),
    /// Like a song, or take the like back.
    ToggleLike(Track),
    /// Like every one of these that is not liked already.
    LikeAll(Vec<Track>),
    AddToPlaylist {
        playlist_id: String,
        playlist_title: String,
        track_ids: Vec<String>,
    },
    CopyLink(String),
    /// Copy the YouTube Music link to this, and say so.
    Share {
        kind: share::Kind,
        id: String,
    },
    /// Ask for a name for a new playlist that will hold these songs.
    NewPlaylist {
        /// What the name starts as; empty for nothing.
        name: String,
        track_ids: Vec<String>,
    },
    /// Ask whether a playlist is really to be deleted.
    AskDeletePlaylist {
        playlist_id: String,
        title: String,
    },
    /// The text in the open dialog's field changed.
    SetDialogText(String),
    ConfirmDialog,
    CloseDialog,
    RemoveFromPlaylist {
        playlist_id: String,
        /// Track id, and the id of its place in the playlist.
        items: Vec<(String, String)>,
    },
    /// Follow an artist, or stop.
    ToggleFollow(String),
    /// A row of a track list was clicked.
    Select {
        list: u64,
        row: usize,
        how: Select,
    },
    SelectAll {
        list: u64,
        len: usize,
    },
    /// An arrow key in a list with a selection.
    StepSelection {
        list: u64,
        step: isize,
        len: usize,
        extend: bool,
    },
    ClearSelection,
    ToggleQueue,
    ToggleLyrics,
    SetLyricsFullscreen(bool),
    /// Open the mini player's window, or close it.
    ToggleMiniPlayer,
    /// Show this above the mini player's controls.
    SetMiniPanel(MiniPanel),
    SetMiniOnTop(bool),
    /// The mini player's window was moved or resized.
    MiniMoved {
        position: [f32; 2],
        size: [f32; 2],
    },
    /// Bring the main window forward.
    ShowMainWindow,
    /// Give the window to what is playing: its cover edge to edge, with the
    /// transport over it. Or take it back.
    SetFullscreenPlayer(bool),
    ToggleFullscreenPlayer,
    SetNormaliseVolume(bool),
    /// How loud evening out the loudness leaves the songs.
    SetVolumeLevel(VolumeLevel),
    /// Let the volume go past 100%, or hold it to that.
    SetVolumeBoost(bool),
    SetGapless(bool),
    /// Keep playing similar songs when the queue runs out, or stop.
    SetAutoplay(bool),
    SetResumeOnLaunch(bool),
    SetContinueFromYouTubeMusic(bool),
    SetReportToYouTube(bool),
    /// Hold the kept songs to this many megabytes.
    SetCacheSize(u32),
    SetReduceMotion(bool),
    /// Show music videos among the songs of shelves and lists, or leave
    /// them out.
    SetShowMusicVideos(bool),
    Video(VideoAsk),
    /// Show the time left at the end of the seek bar, or the length again.
    ToggleRemainingTime,
    /// Play this many times as fast as normal.
    SetSpeed(f32),
    /// Put the Settings page back as a new profile has it.
    ResetPreferences,
    /// Like the song that is playing, or take the like back.
    SaveCurrent,
    /// Open the search page with the caret in its field.
    FocusSearch,
    /// Something stands in the way of playback that is worth saying.
    Notify(Notice),
    /// The notice has been read.
    DismissNotice,
    SetCrossfade(u32),
    SetVisualizer(bool),
    /// Wear a built-in theme.
    SetTheme(themes::Choice),
    /// Wear the theme in this file of the themes folder.
    SetCustomTheme(String),
    OpenThemesFolder,
    /// Read the themes folder again.
    ReloadThemes,
    SetCloseToTray(bool),
    SetSystemTitleBar(bool),
    SetStartAtLogin(bool),
    /// Something asked of the equalizer's panel.
    Equalizer(crate::equalizer::Ask),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    /// The core's session changed.
    SessionChanged(Box<Projection>),
    /// Act as another of the account's channels; empty for its own.
    SwitchChannel(String),
    /// Show the folder the logs are written to.
    OpenLogs,
    /// Fetch the newest yt-dlp.
    UpdateResolver,
    /// That ended: with what yt-dlp said, or with why it could not run.
    ResolverUpdated(Result<String, String>),
    /// Something typed on the Listen Together page.
    TogetherField(together::Field, String),
    TogetherMode(together::Mode),
    TogetherCreate,
    TogetherJoin,
    /// Leave the room, or stop trying to enter one.
    TogetherLeave,
    /// The line to the relay has something to say.
    TogetherEvent(Box<together::Event>),
    /// Once a second in a room: keep the player with it.
    TogetherTick,
    /// Anything else the Listen Together page asks for.
    Room(together::Ask),
    /// Copy this, and say that it was done.
    CopyText {
        text: String,
        said: &'static str,
    },
    /// Look for a newer version of the app.
    CheckForUpdate,
    /// The look, or the download it led to, has got this far.
    UpdateChanged(update::Status),
    /// Run the downloaded installer and quit.
    InstallUpdate,
    /// Open a browser window to sign in with: the account it comes back
    /// with is added to those saved, and is the one in use.
    SignIn,
    /// Ask whether the account in use is really to be signed out.
    SignOut,
    /// Use another of the saved accounts.
    SwitchAccount(String),
    /// Ask whether a saved account is really to be removed.
    AskRemoveAccount(String),
    /// The list of saved accounts is now this.
    AccountsChanged(Box<Accounts>),
    /// Ask the account in use for its channels again.
    RefreshChannels,
    /// Open the account's menu, as a click on the account would.
    OpenAccountMenu,
    /// The system has said whether the app starts with it.
    StartAtLoginKnown(bool),
    /// The tray icon was clicked: open the flyout with its corner here, or
    /// close it if it is open.
    ToggleFlyout {
        position: [f32; 2],
        now: Instant,
    },
    HideFlyout,
    /// Save a problem report to the Downloads folder.
    SaveReport,
    /// That ended: with where the report is, or with why there is none.
    ReportSaved(Result<PathBuf, String>),
    /// Show what the Electron app has, to choose what to bring over.
    OpenMigration,
    /// Tick or untick a kind of thing to bring.
    SetMigrationKind(migrate::Kind, bool),
    /// Bring over what is ticked.
    StartMigration,
    /// The Electron app's profile was looked at. `brought` is what earlier
    /// runs took; `offer` asks for the choice to be shown unprompted.
    MigrationFound {
        found: Option<Box<migrate::Found>>,
        brought: migrate::Kinds,
        offer: bool,
    },
    MigrationProgress(migrate::Progress),
    /// The run ended, with what it came to.
    MigrationDone(Box<migrate::Outcome>),
    /// The account in use changed and the core is restarting on it.
    AccountChanged,
    SignInFailed(String),
    /// Boxed: a page of results is far larger than any other action.
    Loaded(Box<Response>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    SaveSettings,
    Fetch(Request),
    /// Send [`Action::RunSearch`] once typing has paused.
    DebounceSearch,
    /// Send [`Action::RunStatsLookup`] once typing has paused.
    DebounceStatsLookup,
    /// Bring these kinds of thing over from the Electron app.
    Migrate(migrate::Kinds),
    /// The choice of what to bring was shown and closed: it is not to be
    /// shown again unasked.
    MigrationSeen,
    /// Run the browser sign-in, then restart the core on the new account.
    SignIn,
    /// Restart the core on this saved account.
    SwitchAccount(String),
    /// Sign this saved account out and forget it; if it is the one in use,
    /// the core restarts with nobody signed in.
    RemoveAccount(String),
    /// Note what the account in use is called, and its picture.
    RememberAccount {
        name: String,
        avatar_url: String,
    },
    /// Note the channels the account in use can act as.
    RememberChannels(Vec<Channel>),
    /// Gather the logs and a summary into a zip, on a thread of its own.
    SaveReport,
    /// Send this to the core's session.
    Command(Command),
    CopyToClipboard(String),
    /// Add or remove the entry that starts the app with Windows.
    SetStartAtLogin(bool),
    /// Give the window the whole screen, or take it back.
    SetFullscreen(bool),
    /// Make the mini player's window at least this big.
    GrowMini([f32; 2]),
    /// Show the main window and bring it forward.
    ShowMainWindow,
    /// Give the window the system's frame, or take it away.
    SetDecorations(bool),
    /// Tell the engine what the audio settings now are.
    ApplyAudioSettings,
    /// Tell the engine the equalizer alone: a dragged slider changes it
    /// many times a second, and nothing else has changed.
    ApplyEqualizer,
    /// Name this channel in the credentials, then restart the core.
    SwitchChannel(String),
    OpenLogs,
    /// Open the line to a Listen Together relay and enter a room.
    TogetherConnect(together::Options),
    /// Close the line, leaving the room.
    TogetherDisconnect,
    /// Leave the room, handing it to this member; the line is then closed.
    TogetherHandOver(String),
    /// See whether this address answers as a relay does.
    TogetherProbe(String),
    /// Send [`together::Ask::RunSearch`] once typing has paused.
    DebounceRoomSearch,
    /// Send the room a command of this kind with these fields.
    TogetherCommand(&'static str, serde_json::Value),
    /// Tell the room what this player is doing with this entry.
    TogetherStatus(&'static str, Option<String>),
    /// Look for a newer release, and download it if there is one.
    CheckForUpdate,
    /// Run this installer and quit.
    InstallUpdate(PathBuf),
    /// Have Windows say that this version is downloaded and waiting.
    NotifyUpdate(String),
    /// Run yt-dlp's own updater.
    UpdateResolver,
    OpenThemesFolder,
    /// Read the themes folder again.
    ReloadThemes,
}
