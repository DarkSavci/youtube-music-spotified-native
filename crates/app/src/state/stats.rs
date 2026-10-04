//! What the listening page holds beyond its figures: the lookup, and the
//! song, artist or album whose own figures are open.
//!
//! The figures name songs and covers by what the play log kept of them.
//! What the page lists is made from those once, as they arrive, so the
//! page does not make it again on every frame.

use spotified_client::models::{
    Artwork, LookupResults, StatDetail, StatKind, Stats, Track, TrackStat, cover,
};

use super::Loadable;

/// How many of the most played songs the page lists.
const TOP_SHOWN: usize = 25;

/// The song, artist or album whose figures are open.
#[derive(Debug, Clone, PartialEq)]
pub struct StatSelection {
    pub kind: StatKind,
    pub id: String,
    pub detail: Loadable<StatDetail>,
    /// Its most played songs, as tracks to list and play.
    pub tracks: Vec<Track>,
}

impl StatSelection {
    pub fn arrived(&mut self, detail: Loadable<StatDetail>) {
        self.tracks = match &detail {
            Loadable::Loaded(detail) => tracks_of(&detail.top_tracks),
            _ => Vec::new(),
        };
        self.detail = detail;
    }
}

fn tracks_of(stats: &[TrackStat]) -> Vec<Track> {
    stats.iter().map(TrackStat::track).collect()
}

#[derive(Debug, Default)]
pub struct StatsPage {
    /// What is typed in the lookup field.
    pub lookup_text: String,
    /// The matches are showing under the field.
    pub lookup_open: bool,
    /// The matches, and the text they are for.
    pub lookup: Option<(String, LookupResults)>,
    pub selected: Option<StatSelection>,
    /// The period's most played songs, and those on repeat, as tracks.
    pub top: Vec<Track>,
    pub on_repeat: Vec<Track>,
    /// A cover for each of the period's artists and albums, in their order.
    pub artist_covers: Vec<Vec<Artwork>>,
    pub album_covers: Vec<Vec<Artwork>>,
}

impl StatsPage {
    /// The period's figures have come: what the page lists is made from
    /// them.
    pub fn arrived(&mut self, stats: &Stats) {
        let shown = &stats.tracks[..stats.tracks.len().min(TOP_SHOWN)];
        self.top = tracks_of(shown);
        self.on_repeat = tracks_of(&stats.on_repeat);
        let covers = |urls: &mut dyn Iterator<Item = &String>| urls.map(|url| cover(url)).collect();
        self.artist_covers = covers(&mut stats.artists.iter().map(|artist| &artist.artwork));
        self.album_covers = covers(&mut stats.albums.iter().map(|album| &album.artwork));
    }
}
