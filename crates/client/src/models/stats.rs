//! What the core counts from the plays recorded on this computer.
//!
//! These mirror `core/internal/control/stats.go` and `insights.go`. Times
//! arrive as RFC 3339 text and are kept as text: they are only shown.

use serde::Deserialize;

use super::{AlbumRef, ArtistRef, Artwork, Track, null_as_default};

/// How often a song was played.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackStat {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    /// The artist's channel; empty for a play recorded without one.
    pub artist_id: String,
    pub plays: u32,
    pub total_ms: u64,
    /// The address of a cover, when the play log has one.
    pub artwork: String,
}

impl TrackStat {
    /// The row as a track, so it can be listed, queued and played like any
    /// other. It has no length: a table of these shows the plays instead.
    pub fn track(&self) -> Track {
        Track {
            id: self.track_id.clone(),
            title: self.title.clone(),
            artists: vec![ArtistRef {
                id: channel_id(&self.artist_id).unwrap_or_default().to_owned(),
                name: self.artist.clone(),
            }],
            album: None::<AlbumRef>,
            artwork: cover(&self.artwork),
            playable: true,
            play_count: self.plays.to_string(),
            listened_ms: Some(self.total_ms),
            ..Track::default()
        }
    }
}

/// A cover known only by its address, as the size the play log keeps.
pub fn cover(url: &str) -> Vec<Artwork> {
    if url.is_empty() {
        return Vec::new();
    }
    vec![Artwork {
        url: url.to_owned(),
        width: 226,
        height: 226,
    }]
}

/// The id, when it is a real YouTube channel and not a name standing in
/// for one: plays recorded without a channel are grouped by name.
pub fn channel_id(id: &str) -> Option<&str> {
    let rest = id.strip_prefix("UC")?;
    let fits = rest.len() >= 10
        && rest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    fits.then_some(id)
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ArtistStat {
    /// A channel id, or the artist's name for plays recorded without one.
    pub artist_id: String,
    pub artist: String,
    pub plays: u32,
    pub distinct_tracks: u32,
    pub total_ms: u64,
    /// A cover from one of their songs.
    pub artwork: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AlbumStat {
    /// What the album's detail is asked for by: its id, or its name.
    pub key: String,
    pub album_id: String,
    pub album: String,
    pub artist: String,
    pub artist_id: String,
    pub plays: u32,
    pub distinct_tracks: u32,
    pub total_ms: u64,
    pub artwork: String,
}

/// A period's totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Summary {
    pub plays: u32,
    pub total_ms: u64,
    pub distinct_tracks: u32,
    pub distinct_artists: u32,
    pub distinct_albums: u32,
}

/// Everything the listening page shows for a period.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    pub summary: Summary,
    pub tracks: Vec<TrackStat>,
    pub artists: Vec<ArtistStat>,
    pub albums: Vec<AlbumStat>,
    /// Songs heard again and again in the last thirty days, whatever the
    /// period chosen.
    pub on_repeat: Vec<TrackStat>,
}

/// What of the listener's own history matches some text.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LookupResults {
    #[serde(deserialize_with = "null_as_default")]
    pub tracks: Vec<TrackStat>,
    #[serde(deserialize_with = "null_as_default")]
    pub artists: Vec<ArtistStat>,
    #[serde(deserialize_with = "null_as_default")]
    pub albums: Vec<AlbumStat>,
}

impl LookupResults {
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty() && self.artists.is_empty() && self.albums.is_empty()
    }
}

/// What a detail is of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatKind {
    #[default]
    Track,
    Artist,
    Album,
}

impl StatKind {
    /// What the core calls it.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            StatKind::Track => "track",
            StatKind::Artist => "artist",
            StatKind::Album => "album",
        }
    }
}

/// One month of plays: `month` is `2026-09`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MonthPlays {
    pub month: String,
    pub plays: u32,
    pub total_ms: u64,
}

/// The listener's own figures for one song, artist or album.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StatDetail {
    pub kind: StatKind,
    pub id: String,
    pub name: String,
    pub artist: String,
    pub artist_id: String,
    pub album_id: String,
    pub artwork: String,
    pub plays: u32,
    pub plays30d: u32,
    pub total_ms: u64,
    pub distinct_tracks: u32,
    pub first_played_at: Option<String>,
    pub last_played_at: Option<String>,
    /// The place among the listener's own songs, artists or albums.
    pub rank: u32,
    /// The last twelve months, oldest first, the empty ones included.
    #[serde(deserialize_with = "null_as_default")]
    pub months: Vec<MonthPlays>,
    #[serde(deserialize_with = "null_as_default")]
    pub top_tracks: Vec<TrackStat>,
}

/// The listener's history with one artist, shown on the artist's page
/// where a count of strangers would otherwise go.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Affinity {
    pub plays30d: u32,
    pub plays_all_time: u32,
    pub rank_among_your_artists: u32,
}
