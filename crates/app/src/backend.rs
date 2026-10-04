//! Worker threads that ask the core for things.
//!
//! The UI thread never makes a request. It sends a [`Request`] here and
//! later finds the [`Response`] among its events. Requests in flight are
//! not aborted: an answer nobody wants any more is dropped when it arrives,
//! by the key it carries.

use crossbeam_channel::{Receiver, Sender};
use spotified_client::models::{
    Account, Album, Artist, Artwork, BrowsePage, CacheUsage, Channel, Folder, LibraryItem,
    LibraryKind, Lyrics, Mix, MixSeed, Playlist, Podcast, SearchFilter, SearchResults, Stats,
    Track,
};
use spotified_client::{ApiError, Client};

/// Enough to load a page and its sidebar at once without queueing; the
/// core's own governor paces what reaches YouTube.
const WORKERS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Home,
    Library,
    Album(String),
    Artist(String),
    Playlist(String),
    Podcast(String),
    /// A surface by its id and params.
    Browse(String, String),
    /// A picture for the tile of this surface: its page's first cover.
    TileArt(String, String),
    /// What the account has played lately.
    History,
    /// Ways this query might go on.
    Suggest(String),
    RecentSearches,
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
    Home(Result<BrowsePage, ApiError>),
    Library(Result<Vec<LibraryItem>, ApiError>),
    Album(String, Result<Album, ApiError>),
    /// Boxed: an artist's page is far larger than any other answer.
    Artist(String, Result<Box<Artist>, ApiError>),
    Playlist(String, Result<Playlist, ApiError>),
    Podcast(String, Result<Podcast, ApiError>),
    Browse(String, String, Result<BrowsePage, ApiError>),
    /// The picture for a tile; none if its page could not be had.
    TileArt(String, String, Vec<Artwork>),
    History(Result<Vec<Track>, ApiError>),
    /// For this query.
    Suggestions(String, Result<Vec<String>, ApiError>),
    RecentSearches(Result<Vec<String>, ApiError>),
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
                        deliver(answer(&client, request));
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

fn answer(client: &Client, request: Request) -> Response {
    match request {
        Request::Home => Response::Home(client.home()),
        Request::Library => Response::Library(client.library()),
        Request::Album(id) => {
            let result = client.album(&id);
            Response::Album(id, result)
        }
        Request::Artist(id) => {
            let result = client.artist(&id).map(Box::new);
            Response::Artist(id, result)
        }
        Request::Playlist(id) => {
            let result = client.playlist(&id);
            Response::Playlist(id, result)
        }
        Request::Podcast(id) => {
            let result = client.podcast(&id);
            Response::Podcast(id, result)
        }
        Request::Browse(id, params) => {
            let result = client.browse(&id, &params);
            Response::Browse(id, params, result)
        }
        Request::TileArt(id, params) => {
            let cover = client
                .browse(&id, &params)
                .map(|page| page.cover().to_vec())
                .unwrap_or_default();
            Response::TileArt(id, params, cover)
        }
        Request::History => Response::History(client.history()),
        Request::Suggest(query) => {
            let result = client.suggest(&query);
            Response::Suggestions(query, result)
        }
        Request::RecentSearches => Response::RecentSearches(client.recent_searches()),
        Request::Account => Response::Account(client.account()),
        Request::Channels => Response::Channels(client.channels()),
        Request::CacheUsage => Response::CacheUsage(client.cache_usage()),
        Request::ClearCache => Response::CacheCleared(client.clear_cache()),
        Request::StartRadio { device_id, track } => {
            Response::RadioStarted(client.start_radio(&device_id, &track))
        }
        Request::StartMix {
            device_id,
            seed,
            origin,
        } => Response::RadioStarted(client.start_mix(&device_id, &seed, &origin)),
        Request::Folders => Response::Folders(client.folders()),
        Request::CreateFolder(name) => Response::Organised(client.create_folder(&name)),
        Request::DeleteFolder(id) => Response::Organised(client.delete_folder(&id)),
        Request::Organise {
            kind,
            item_id,
            pinned,
            folder_id,
        } => Response::Organised(client.organise(kind, &item_id, pinned, folder_id.as_deref())),
        Request::Liked => Response::Liked(client.liked()),
        Request::Mixes => Response::Mixes(client.mixes()),
        Request::Stats(days) => Response::Stats(days, client.stats(days)),
        Request::Lyrics(track) => Response::Lyrics {
            result: client.lyrics(&track),
            track_id: track.id,
        },
        Request::SetLiked { track_id, liked } => {
            let result = client.set_liked(&track_id, liked);
            Response::LikeSet {
                track_id,
                liked,
                result,
            }
        }
        Request::AddToPlaylist {
            playlist_id,
            playlist_title,
            track_ids,
        } => {
            let result = client.add_to_playlist(&playlist_id, &track_ids);
            Response::AddedToPlaylist {
                playlist_id,
                playlist_title,
                count: track_ids.len(),
                result,
            }
        }
        Request::CreatePlaylist { title, track_ids } => {
            let result = client.create_playlist(&title).and_then(|id| {
                if !track_ids.is_empty() {
                    client.add_to_playlist(&id, &track_ids)?;
                }
                Ok(id)
            });
            Response::PlaylistCreated { title, result }
        }
        Request::DeletePlaylist { playlist_id, title } => {
            let result = client.delete_playlist(&playlist_id);
            Response::PlaylistDeleted {
                playlist_id,
                title,
                result,
            }
        }
        Request::RemoveFromPlaylist { playlist_id, items } => {
            let result = client.remove_from_playlist(&playlist_id, &items);
            Response::RemovedFromPlaylist {
                playlist_id,
                result,
            }
        }
        Request::SetFollowing { artist_id, follow } => {
            let result = client.set_following(&artist_id, follow);
            Response::FollowingSet {
                artist_id,
                follow,
                result,
            }
        }
        Request::Search {
            serial,
            query,
            filter,
        } => Response::Search {
            serial,
            result: client.search(&query, filter),
        },
    }
}
