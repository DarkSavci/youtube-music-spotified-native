//! What can be asked for, and what carrying it out can call for.

use std::path::PathBuf;

use spotified_client::models::{LibraryKind, MixSeed, SearchFilter, Track};
use spotified_client::session::{Command, Projection};

use crate::backend::{Request, Response};
use crate::sidecar::CoreStatus;
use crate::state::{MiniPanel, Page, Select};
use crate::themes;
use crate::together;
use crate::update;

#[derive(Debug)]
pub enum Action {
    Open(Page),
    Back,
    Forward,
    ToggleSidebar,
    /// The sidebar was dragged to this width.
    ResizeSidebar(f32),
    /// A chip in the sidebar was clicked. Clicking the active one clears it.
    FilterLibrary(LibraryKind),
    /// The library's search field changed.
    SetLibraryQuery(String),
    /// The library's sort button was clicked.
    CycleLibrarySort,
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
    /// Ask for a name for a new playlist that will hold these songs.
    NewPlaylist {
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
    SetNormaliseVolume(bool),
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
    SetEqualizerOn(bool),
    /// Set one band, in decibels.
    SetEqualizerBand(usize, f32),
    /// Set every band at once, as a preset does.
    SetEqualizer([f32; 10]),
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
    /// Open a browser window to sign in with.
    SignIn,
    SignOut,
    /// Copy the Electron app's sign-in into this app.
    ImportSignIn,
    /// The copy worked and the core is restarting with it.
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
    /// Copy the credentials at this path, then restart the core.
    ImportSignIn(PathBuf),
    /// Run the browser sign-in, then restart the core.
    SignIn,
    /// Delete the credentials, then restart the core.
    SignOut,
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
    /// Name this channel in the credentials, then restart the core.
    SwitchChannel(String),
    OpenLogs,
    /// Open the line to a Listen Together relay and enter a room.
    TogetherConnect(together::Options),
    /// Close the line, leaving the room.
    TogetherDisconnect,
    /// Send the room a command of this kind with these fields.
    TogetherCommand(&'static str, serde_json::Value),
    /// Tell the room what this player is doing with this entry.
    TogetherStatus(&'static str, Option<String>),
    /// Look for a newer release, and download it if there is one.
    CheckForUpdate,
    /// Run this installer and quit.
    InstallUpdate(PathBuf),
    /// Run yt-dlp's own updater.
    UpdateResolver,
    OpenThemesFolder,
    /// Read the themes folder again.
    ReloadThemes,
}
