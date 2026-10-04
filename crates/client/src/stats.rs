//! Your listening: what the core counts from the plays recorded here. None
//! of it goes to YouTube; it is read from the core's own database.

use crate::models::{
    Affinity, AlbumStat, ArtistStat, LookupResults, StatDetail, StatKind, Stats, TrackStat,
};
use crate::{ApiError, Client, encode};

/// How many of each kind the listening page asks for.
const TOP_TRACKS: usize = 50;
const TOP_ARTISTS: usize = 20;
const TOP_ALBUMS: usize = 20;
const ON_REPEAT: usize = 20;
/// How many of each kind a lookup offers.
const LOOKUP: usize = 6;

impl Client {
    /// The totals and the most played of the last `days` days, and what
    /// has been on repeat lately.
    pub fn stats(&self, days: u32) -> Result<Stats, ApiError> {
        let top =
            |what: &str, limit: usize| format!("/v1/me/stats/{what}?days={days}&limit={limit}");
        let tracks: Option<Vec<TrackStat>> = self.get(&top("tracks", TOP_TRACKS))?;
        let artists: Option<Vec<ArtistStat>> = self.get(&top("artists", TOP_ARTISTS))?;
        let albums: Option<Vec<AlbumStat>> = self.get(&top("albums", TOP_ALBUMS))?;
        let on_repeat: Option<Vec<TrackStat>> =
            self.get(&format!("/v1/me/stats/on-repeat?limit={ON_REPEAT}"))?;
        Ok(Stats {
            summary: self.get(&format!("/v1/me/stats/summary?days={days}"))?,
            tracks: tracks.unwrap_or_default(),
            artists: artists.unwrap_or_default(),
            albums: albums.unwrap_or_default(),
            on_repeat: on_repeat.unwrap_or_default(),
        })
    }

    /// The songs, artists and albums of the listener's own history whose
    /// names hold `text`. Something never played is simply not found.
    pub fn stats_lookup(&self, text: &str) -> Result<LookupResults, ApiError> {
        self.get(&format!(
            "/v1/me/stats/lookup?q={}&limit={LOOKUP}",
            encode(text)
        ))
    }

    /// The listener's figures for one song, artist or album.
    pub fn stats_detail(&self, kind: StatKind, id: &str) -> Result<StatDetail, ApiError> {
        let path = format!("/v1/me/stats/detail?kind={}&id={}", kind.wire(), encode(id));
        self.get(&path)
    }

    /// The listener's history with an artist.
    pub fn affinity(&self, artist_id: &str) -> Result<Affinity, ApiError> {
        self.get(&format!("/v1/artists/{}/affinity", encode(artist_id)))
    }
}
