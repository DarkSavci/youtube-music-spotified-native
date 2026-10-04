//! Worker threads that ask the core for things.
//!
//! The UI thread never makes a request. It sends a [`Request`] here and
//! later finds the [`Response`] among its events. Requests in flight are
//! not aborted: an answer nobody wants any more is dropped when it arrives,
//! by the key it carries.

mod answer;
mod artist;

use crossbeam_channel::{Receiver, Sender};
use spotified_client::models::{
    Account, Affinity, Album, Artist, Artwork, BrowsePage, CacheUsage, Channel, Folder,
    LibraryItem, LibraryKind, LookupResults, Lyrics, Mix, MixSeed, Playlist, PlaylistPage, Podcast,
    RemoteQueue, SearchFilter, SearchHistoryEntry, SearchResults, StatDetail, StatKind, Stats,
    Track,
};
use spotified_client::{ApiError, Client};

pub use artist::{ArtistQueue, ArtistSeed};

/// Enough to load a page and its sidebar at once without queueing; the
/// core's own governor paces what reaches YouTube.
const WORKERS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Home, read through the mood chip with these params; empty for none.
    Home(String),
    /// The next few shelves of Home under this mood, from this token.
    HomeMore(String, String),
    Library,
    Album(String),
    Artist(String),
    /// A playlist's first page of songs.
    Playlist(String),
    /// The page of a playlist this token fetches.
    PlaylistMore {
        id: String,
        token: String,
    },
    /// Every page of a playlist from this token on.
    PlaylistRest {
        id: String,
        token: String,
    },
    Podcast(String),
    /// The listener's own history with this artist.
    Affinity(String),
    /// Play an artist: every song of theirs, most played first, or a
    /// shuffle of them. `known` is what their page gave, when it is here.
    PlayArtist {
        device_id: String,
        artist_id: String,
        known: Option<ArtistSeed>,
        shuffle: bool,
    },
    /// A page of the list of all an artist's songs.
    SongsPage {
        artist_id: String,
        songs_id: String,
        token: String,
    },
    /// The whole of an artist's albums and of their singles, from the
    /// surfaces their page links to: each an id and its params.
    Discography {
        artist_id: String,
        albums: Option<(String, String)>,
        singles: Option<(String, String)>,
    },
    /// One of an artist's releases, opened for its songs.
    Release {
        artist_id: String,
        album_id: String,
    },
    /// An artist's picture, for a card that has only their name.
    ArtistPhoto(String),
    /// A surface by its id and params.
    Browse(String, String),
    /// A picture for the tile of this surface: its page's first cover.
    TileArt(String, String),
    /// What the account has played lately.
    History,
    /// Ways this query might go on.
    Suggest(String),
    SearchHistory,
    /// Remove these from the account's search history. `all` when it is
    /// the whole of what was shown.
    ForgetSearches {
        tokens: Vec<String>,
        all: bool,
    },
    /// The queue the account has on another device.
    RemoteQueue,
    Account,
    Channels,
    CacheUsage,
    ClearCache,
    /// Play this song, then songs like it.
    StartRadio {
        device_id: String,
        track: Box<Track>,
    },
    /// Play one of YouTube's own queues, under this name.
    StartMix {
        device_id: String,
        seed: MixSeed,
        origin: String,
    },
    Folders,
    CreateFolder(String),
    DeleteFolder(String),
    /// Pin a library item or file it in a folder; `None` leaves that be.
    Organise {
        kind: LibraryKind,
        item_id: String,
        pinned: Option<bool>,
        folder_id: Option<String>,
    },
    Liked,
    Mixes,
    /// The most played, over this many days.
    Stats(u32),
    /// What of the listener's history matches this text.
    StatsLookup(String),
    /// The listener's figures for one song, artist or album.
    StatDetail(StatKind, String),
    /// The lyrics of this track.
    Lyrics(Box<Track>),
    SetLiked {
        track_id: String,
        liked: bool,
    },
    AddToPlaylist {
        playlist_id: String,
        /// Carried through for the message that confirms it.
        playlist_title: String,
        track_ids: Vec<String>,
    },
    /// Create a playlist, and put these tracks in it.
    CreatePlaylist {
        title: String,
        track_ids: Vec<String>,
    },
    DeletePlaylist {
        playlist_id: String,
        title: String,
    },
    RemoveFromPlaylist {
        playlist_id: String,
        /// Track id, and the id of its place in the playlist.
        items: Vec<(String, String)>,
    },
    SetFollowing {
        artist_id: String,
        follow: bool,
    },
    /// Songs to add to a Listen Together room, counted as searches are.
    RoomSearch {
        serial: u64,
        query: String,
    },
    /// The songs this one's radio would play, for a room to carry on with.
    Radio(String),
    /// The song and the music video YouTube pairs with this track.
    Versions(String),
    /// `serial` counts searches, so an answer to an older query can be told
    /// from the answer to the one on screen.
    Search {
        serial: u64,
        query: String,
        filter: SearchFilter,
    },
}

#[derive(Debug)]
pub enum Response {
    /// For the mood it was asked through.
    Home(String, Result<BrowsePage, ApiError>),
    /// For the mood and the token it was asked with.
    HomeMore(String, String, Result<BrowsePage, ApiError>),
    Library(Result<Vec<LibraryItem>, ApiError>),
    Album(String, Result<Album, ApiError>),
    /// Boxed: an artist's page is far larger than any other answer.
    Artist(String, Result<Box<Artist>, ApiError>),
    /// A playlist's first page, and the token for its next.
    Playlist(String, Result<PlaylistPage, ApiError>),
    PlaylistMore {
        id: String,
        token: String,
        result: Result<PlaylistPage, ApiError>,
    },
    /// Every song from the token on.
    PlaylistRest {
        id: String,
        token: String,
        result: Result<Vec<Track>, ApiError>,
    },
    Podcast(String, Result<Podcast, ApiError>),
    Affinity(String, Result<Affinity, ApiError>),
    /// What playing an artist came to.
    ArtistQueue {
        shuffle: bool,
        result: Result<ArtistQueue, ApiError>,
    },
    SongsPage {
        artist_id: String,
        token: String,
        result: Result<PlaylistPage, ApiError>,
    },
    /// The artist's albums, and their singles.
    Discography {
        artist_id: String,
        albums: Vec<Album>,
        singles: Vec<Album>,
    },
    Release {
        artist_id: String,
        album_id: String,
        result: Result<Album, ApiError>,
    },
    /// The picture; none if the artist's page could not be had.
    ArtistPhoto(String, Vec<Artwork>),
    Browse(String, String, Result<BrowsePage, ApiError>),
    /// The picture for a tile; none if its page could not be had.
    TileArt(String, String, Vec<Artwork>),
    History(Result<Vec<Track>, ApiError>),
    /// For this query.
    Suggestions(String, Result<Vec<String>, ApiError>),
    SearchHistory(Result<Vec<SearchHistoryEntry>, ApiError>),
    SearchesForgotten {
        all: bool,
        result: Result<(), ApiError>,
    },
    RemoteQueue(Result<RemoteQueue, ApiError>),
    Account(Result<Option<Account>, ApiError>),
    Channels(Result<Vec<Channel>, ApiError>),
    CacheUsage(Result<CacheUsage, ApiError>),
    CacheCleared(Result<CacheUsage, ApiError>),
    RadioStarted(Result<(), ApiError>),
    Folders(Result<Vec<Folder>, ApiError>),
    /// A folder was made or deleted, or an item pinned or filed.
    Organised(Result<(), ApiError>),
    Liked(Result<Playlist, ApiError>),
    Mixes(Result<Vec<Mix>, ApiError>),
    Stats(u32, Result<Stats, ApiError>),
    /// For this text.
    StatsLookup(String, Result<LookupResults, ApiError>),
    StatDetail(StatKind, String, Result<StatDetail, ApiError>),
    Lyrics {
        track_id: String,
        result: Result<Option<Lyrics>, ApiError>,
    },
    LikeSet {
        track_id: String,
        liked: bool,
        result: Result<(), ApiError>,
    },
    AddedToPlaylist {
        playlist_id: String,
        playlist_title: String,
        count: usize,
        result: Result<(), ApiError>,
    },
    PlaylistCreated {
        title: String,
        result: Result<String, ApiError>,
    },
    PlaylistDeleted {
        playlist_id: String,
        title: String,
        result: Result<(), ApiError>,
    },
    RemovedFromPlaylist {
        playlist_id: String,
        result: Result<(), ApiError>,
    },
    FollowingSet {
        artist_id: String,
        follow: bool,
        result: Result<(), ApiError>,
    },
    Search {
        serial: u64,
        result: Result<SearchResults, ApiError>,
    },
    RoomSearch {
        serial: u64,
        result: Result<Vec<Track>, ApiError>,
    },
    /// For the song it was asked of.
    Radio(String, Result<Vec<Track>, ApiError>),
    /// The song and its video, for the track they were asked of.
    Versions {
        track_id: String,
        result: Result<Vec<Track>, ApiError>,
    },
}

pub struct Backend {
    requests: Sender<Request>,
}

impl Backend {
    /// Starts the workers. `deliver` is called on a worker thread with each
    /// answer; it must hand the answer to the UI thread and wake it.
    pub fn start(origin: &str, deliver: impl Fn(Response) + Send + Clone + 'static) -> Self {
        let (requests, queue) = crossbeam_channel::unbounded::<Request>();
        let client = Client::new(origin);
        for index in 0..WORKERS {
            let queue: Receiver<Request> = queue.clone();
            let client = client.clone();
            let deliver = deliver.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("api-{index}"))
                .spawn(move || {
                    // Ends when the Backend, and with it the sender, is dropped.
                    for request in queue {
                        deliver(answer::answer(&client, request));
                    }
                });
            if let Err(error) = spawned {
                log::error!("an api worker could not start: {error}");
            }
        }
        Self { requests }
    }

    pub fn send(&self, request: Request) {
        // The workers only stop when this is dropped, so a send cannot fail
        // while there is still someone to call it.
        let _ = self.requests.send(request);
    }
}
