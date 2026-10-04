//! What each request comes to: the calls to the core, made on a worker.

use spotified_client::Client;
use spotified_client::models::SearchFilter;

use super::{Request, Response, artist};

/// How many songs a search from inside a Listen Together room offers.
const ROOM_SEARCH_SHOWN: usize = 12;

pub(super) fn answer(client: &Client, request: Request) -> Response {
    match request {
        Request::Home(mood) => {
            let result = client.home(&mood);
            Response::Home(mood, result)
        }
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
            let result = client.playlist_page(&id, "");
            Response::Playlist(id, result)
        }
        Request::PlaylistMore { id, token } => {
            let result = client.playlist_page(&id, &token);
            Response::PlaylistMore { id, token, result }
        }
        Request::PlaylistRest { id, token } => {
            let result = client.playlist_rest(&id, &token);
            Response::PlaylistRest { id, token, result }
        }
        Request::HomeMore(mood, token) => {
            let result = client.home_more(&token);
            Response::HomeMore(mood, token, result)
        }
        Request::Affinity(id) => {
            let result = client.affinity(&id);
            Response::Affinity(id, result)
        }
        Request::PlayArtist {
            device_id,
            artist_id,
            known,
            shuffle,
        } => Response::ArtistQueue {
            shuffle,
            result: artist::play(client, &device_id, &artist_id, known, shuffle),
        },
        Request::SongsPage {
            artist_id,
            songs_id,
            token,
        } => {
            let result = client.playlist_page(&songs_id, &token);
            Response::SongsPage {
                artist_id,
                token,
                result,
            }
        }
        Request::Discography {
            artist_id,
            albums,
            singles,
        } => Response::Discography {
            albums: artist::albums_of(client, albums),
            singles: artist::albums_of(client, singles),
            artist_id,
        },
        Request::Release {
            artist_id,
            album_id,
        } => {
            let result = client.album(&album_id);
            Response::Release {
                artist_id,
                album_id,
                result,
            }
        }
        Request::ArtistPhoto(id) => {
            let photo = client
                .artist(&id)
                .map(|artist| artist.artwork)
                .unwrap_or_default();
            Response::ArtistPhoto(id, photo)
        }
        Request::ForgetSearches { tokens, all } => Response::SearchesForgotten {
            all,
            result: client.forget_searches(&tokens),
        },
        Request::RemoteQueue => Response::RemoteQueue(client.remote_queue()),
        Request::StatsLookup(text) => {
            let result = client.stats_lookup(&text);
            Response::StatsLookup(text, result)
        }
        Request::StatDetail(kind, id) => {
            let result = client.stats_detail(kind, &id);
            Response::StatDetail(kind, id, result)
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
        Request::SearchHistory => Response::SearchHistory(client.search_history()),
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
        Request::RoomSearch { serial, query } => Response::RoomSearch {
            serial,
            result: client
                .search(&query, SearchFilter::Songs)
                .map(|found| found.songs(ROOM_SEARCH_SHOWN)),
        },
        Request::Radio(seed) => {
            let result = client.radio_of(&seed);
            Response::Radio(seed, result)
        }
        Request::Versions(track_id) => {
            let result = client.versions(&track_id);
            Response::Versions { track_id, result }
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
