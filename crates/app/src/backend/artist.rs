//! Playing an artist: more than one call to the core, and choices between
//! them, so it is done here on a worker rather than a step at a time.

use spotified_client::models::{Album, Artist, Item, MixSeed, Track};
use spotified_client::session::MixStart;
use spotified_client::{ApiError, Client};

use crate::artistsongs::{by_plays, random, shuffled};

/// Fewer songs than this in YouTube's shuffle of an artist, and their own
/// song list is shuffled instead.
const SHUFFLE_LEAST: usize = 20;

/// What of an artist's page playing them needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistSeed {
    pub name: String,
    /// The playlist of all their songs; empty when there is none.
    pub songs_id: String,
    /// The few songs their page shows: what is played failing all else.
    pub top_tracks: Vec<Track>,
    /// YouTube's own shuffle of them, when it names one.
    pub shuffle: Option<MixSeed>,
}

impl ArtistSeed {
    pub fn of(artist: &Artist) -> Self {
        let named = !artist.shuffle_id.is_empty() && !artist.shuffle_seed.is_empty();
        Self {
            name: artist.name.clone(),
            songs_id: artist.songs_id.clone(),
            top_tracks: artist.top_tracks.clone(),
            shuffle: named.then(|| MixSeed {
                playlist_id: artist.shuffle_id.clone(),
                video_id: artist.shuffle_seed.clone(),
                params: artist.shuffle_params.clone(),
            }),
        }
    }
}

/// What playing an artist came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtistQueue {
    /// The core is already playing one of YouTube's queues.
    Started,
    /// These are to be played, under this name. None when the artist has
    /// nothing that can be.
    Songs { tracks: Vec<Track>, origin: String },
}

/// Every song of theirs that can be played, most played first.
///
/// Playing only the few on the page looped them under repeat and never
/// reached the albums. Those few are what is left when the artist has no
/// list of all their songs, or it cannot be read.
fn songs(client: &Client, seed: &ArtistSeed) -> Vec<Track> {
    let playable = |tracks: Vec<Track>| -> Vec<Track> {
        tracks.into_iter().filter(|track| track.playable).collect()
    };
    let top = || playable(seed.top_tracks.clone());
    if seed.songs_id.is_empty() {
        return top();
    }
    match client.complete_playlist(&seed.songs_id) {
        Ok(playlist) if playlist.tracks.iter().any(|track| track.playable) => {
            by_plays(playable(playlist.tracks))
        }
        _ => top(),
    }
}

pub(super) fn play(
    client: &Client,
    device_id: &str,
    artist_id: &str,
    known: Option<ArtistSeed>,
    shuffle: bool,
) -> Result<ArtistQueue, ApiError> {
    // Played from a card, the artist's page has not been read yet.
    let seed = match known {
        Some(seed) => seed,
        None => ArtistSeed::of(&client.artist(artist_id)?),
    };
    if !shuffle {
        return Ok(ArtistQueue::Songs {
            tracks: songs(client, &seed),
            origin: seed.name,
        });
    }
    // Its own name: Play and Shuffle can start on the same song, and a
    // queue is known by its name and first song.
    let origin = format!("{} · Shuffle", seed.name);
    // YouTube's shuffle keeps going for as long as the artist has songs.
    // A small artist's can be three long, after which autoplay drifts to
    // other artists; so a short one gives way to a shuffle of the artist's
    // own song list, when that list is the longer of the two.
    let short = match &seed.shuffle {
        Some(mix) => match client.start_mix_of_at_least(device_id, mix, &origin, SHUFFLE_LEAST) {
            Ok(MixStart::Playing) => return Ok(ArtistQueue::Started),
            Ok(MixStart::Short(tracks)) => Some(tracks),
            Err(_) => None,
        },
        None => None,
    };
    let tracks = shuffled(songs(client, &seed), random());
    if let (Some(mix), Some(short)) = (&seed.shuffle, short)
        && tracks.len() < short
        && client.start_mix(device_id, mix, &origin).is_ok()
    {
        return Ok(ArtistQueue::Started);
    }
    Ok(ArtistQueue::Songs { tracks, origin })
}

/// The albums on a surface: the whole of an artist's albums, or of their
/// singles. None when there is no such surface or it cannot be read; the
/// songs page then makes do with what the artist's own page shows.
pub(super) fn albums_of(client: &Client, link: Option<(String, String)>) -> Vec<Album> {
    let Some((id, params)) = link else {
        return Vec::new();
    };
    let Ok(page) = client.browse(&id, &params) else {
        return Vec::new();
    };
    page.shelves
        .into_iter()
        .flat_map(|shelf| shelf.items)
        .filter_map(|item| match item {
            Item::Album(album) => Some(album),
            _ => None,
        })
        .collect()
}
