//! The core's JSON, as Rust types.
//!
//! These mirror `core/internal/domain/types.go`. Two habits of Go's encoder
//! shape them: an empty value is often left out, and a nil slice is written
//! as `null`. So every field has a default and every list reads `null` as
//! empty.

use serde::{Deserialize, Deserializer, Serialize};

mod stats;

pub use stats::{
    Affinity, AlbumStat, ArtistStat, LookupResults, MonthPlays, StatDetail, StatKind, Stats,
    Summary, TrackStat, channel_id, cover,
};

/// Reads `null` as the type's default, as Go writes a nil slice.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Artwork {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

/// The address of an image at least `min_width` wide.
///
/// Google's image hosts take the size in the URL (`=w226-h226…` or `=s226`),
/// so one is rewritten to ask for exactly what will be drawn rather than
/// taking a listed size that may be far larger.
pub fn artwork_url(set: &[Artwork], min_width: u32) -> Option<String> {
    let chosen = set
        .iter()
        .find(|art| art.width >= min_width)
        .or_else(|| set.last())?;
    Some(resized(&chosen.url, min_width).unwrap_or_else(|| chosen.url.clone()))
}

fn resized(url: &str, size: u32) -> Option<String> {
    let (base, spec) = url.rsplit_once('=')?;
    let number = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    let mut parts = spec.splitn(3, '-');
    let width = parts.next()?.strip_prefix('w');
    let height = parts.next().and_then(|part| part.strip_prefix('h'));
    if let (Some(width), Some(height)) = (width, height)
        && number(width)
        && number(height)
    {
        // What follows the size says how the picture is cut and packed:
        // an artist's is cropped square, and stays so at the new size.
        let rest = parts.next().unwrap_or("l90-rj");
        Some(format!("{base}=w{size}-h{size}-{rest}"))
    } else if spec.starts_with('s') {
        Some(format!("{base}=s{size}"))
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumRef {
    pub id: String,
    pub name: String,
}

/// Sent back to the core as well as read from it: a play command carries
/// the tracks to queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub title: String,
    #[serde(deserialize_with = "null_as_default")]
    pub artists: Vec<ArtistRef>,
    pub album: Option<AlbumRef>,
    pub duration_ms: u64,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    pub explicit: bool,
    pub is_video: bool,
    pub playable: bool,
    pub play_count: String,
    /// Which entry of a playlist this is, for removing it later.
    pub playlist_item_id: String,
    /// How long it has been listened to here, on a row of the listener's
    /// own figures. Never from the core's catalogue, nor sent back to it.
    #[serde(skip)]
    pub listened_ms: Option<u64>,
}

impl Track {
    pub fn artist_names(&self) -> String {
        join_names(&self.artists)
    }
}

pub fn join_names(artists: &[ArtistRef]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub title: String,
    #[serde(deserialize_with = "null_as_default")]
    pub artists: Vec<ArtistRef>,
    pub year: String,
    pub track_count: u32,
    pub duration_ms: u64,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    pub explicit: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub tracks: Vec<Track>,
    /// "Album", "Single" or "EP", as YouTube Music labels it.
    #[serde(rename = "type")]
    pub kind: String,
    /// The first artist's picture, for the page's byline.
    #[serde(deserialize_with = "null_as_default")]
    pub artist_artwork: Vec<Artwork>,
    /// What is said of the release, where YouTube Music says anything.
    pub description: String,
    /// The rows under the songs, such as "Releases for you".
    #[serde(deserialize_with = "null_as_default")]
    pub shelves: Vec<Shelf>,
}

/// The address of a surface: its id, and the params some need to be told
/// apart by.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct BrowseLink {
    pub id: String,
    pub params: String,
}

/// One of YouTube's generated queues: the list, the song it starts from,
/// and the params that say how it behaves.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MixSeed {
    pub playlist_id: String,
    pub video_id: String,
    pub params: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    pub subscribers: String,
    pub monthly_listeners: String,
    pub following: bool,
    /// What the artist says of themselves, or what is said of them.
    pub description: String,
    /// Where the description came from, usually Wikipedia; empty when it
    /// names no source.
    pub description_url: String,
    /// The artist's mix, their music and music like it, and the song it
    /// starts from. YouTube makes neither list without its song.
    pub radio_id: String,
    pub radio_seed: String,
    pub radio_params: String,
    /// A shuffle of the artist's own songs, and the song it starts from.
    pub shuffle_id: String,
    pub shuffle_seed: String,
    pub shuffle_params: String,
    /// The playlist of all the artist's songs; empty when there is none.
    pub songs_id: String,
    /// Where the whole of the discography is, when the page holds a part.
    pub albums_more: Option<BrowseLink>,
    pub singles_more: Option<BrowseLink>,
    #[serde(deserialize_with = "null_as_default")]
    pub top_tracks: Vec<Track>,
    #[serde(deserialize_with = "null_as_default")]
    pub albums: Vec<Album>,
    #[serde(deserialize_with = "null_as_default")]
    pub singles: Vec<Album>,
    #[serde(deserialize_with = "null_as_default")]
    pub related: Vec<Artist>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub description: String,
    pub owner: String,
    pub track_count: u32,
    pub duration_ms: u64,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    pub editable: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub tracks: Vec<Track>,
}

/// Some of a playlist's songs, and the token that fetches the next of
/// them; empty once there are no more.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PlaylistPage {
    pub playlist: Playlist,
    pub next: String,
}

/// A show. Its episodes are read as tracks: they queue and play as songs
/// do, and what an episode has beyond a track is not shown.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Podcast {
    pub id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    #[serde(deserialize_with = "null_as_default")]
    pub episodes: Vec<Track>,
}

/// Something a shelf or a search can hold.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Track(Track),
    Album(Album),
    /// Boxed: an artist carries far more than the other kinds.
    Artist(Box<Artist>),
    Playlist(Playlist),
    Podcast(Podcast),
    /// One instalment of a show.
    Episode(Track),
}

impl Item {
    /// The picture that stands for the item, whatever kind it is.
    pub fn artwork(&self) -> &[Artwork] {
        match self {
            Item::Track(track) | Item::Episode(track) => &track.artwork,
            Item::Album(album) => &album.artwork,
            Item::Artist(artist) => &artist.artwork,
            Item::Playlist(playlist) => &playlist.artwork,
            Item::Podcast(podcast) => &podcast.artwork,
        }
    }
}

impl BrowsePage {
    /// A picture to stand for the whole page: the first its shelves hold.
    pub fn cover(&self) -> &[Artwork] {
        self.shelves
            .iter()
            .flat_map(|shelf| &shelf.items)
            .map(Item::artwork)
            .find(|artwork| !artwork.is_empty())
            .unwrap_or_default()
    }
}

/// How the core writes an item: a kind, and the one field that kind names.
#[derive(Deserialize)]
#[serde(default)]
#[derive(Default)]
struct WireItem {
    kind: String,
    track: Option<Track>,
    album: Option<Album>,
    artist: Option<Artist>,
    playlist: Option<Playlist>,
    podcast: Option<Podcast>,
    episode: Option<Track>,
}

impl WireItem {
    /// `None` for a kind this client does not know and for an item whose
    /// body is missing. One odd item must not cost
    /// the page it sits on.
    fn into_item(self) -> Option<Item> {
        match self.kind.as_str() {
            "track" => self.track.map(Item::Track),
            "album" => self.album.map(Item::Album),
            "artist" => self.artist.map(|artist| Item::Artist(Box::new(artist))),
            "playlist" => self.playlist.map(Item::Playlist),
            "podcast" => self.podcast.map(Item::Podcast),
            "episode" => self.episode.map(Item::Episode),
            _ => None,
        }
    }
}

fn items<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Item>, D::Error> {
    let wire: Vec<WireItem> = null_as_default(deserializer)?;
    Ok(wire.into_iter().filter_map(WireItem::into_item).collect())
}

fn item<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Item>, D::Error> {
    let wire: Option<WireItem> = Option::deserialize(deserializer)?;
    Ok(wire.and_then(WireItem::into_item))
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shelf {
    pub title: String,
    #[serde(deserialize_with = "items")]
    pub items: Vec<Item>,
    /// The surface that shows the whole of this shelf; empty when the
    /// shelf is all there is.
    pub show_all_id: String,
    /// Goes with `show_all_id`: some surfaces share an id and are told
    /// apart only by this.
    pub show_all_params: String,
}

/// A mood or a genre, as a tile. The colour is YouTube's own for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct MoodChip {
    pub id: String,
    pub params: String,
    pub title: String,
    /// `#RRGGBB`, or empty.
    pub color: String,
}

/// One of the moods across the top of Home ("Energize", "Relax"). Each
/// reads Home again through its params.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HomeChip {
    pub title: String,
    pub params: String,
    /// The one the page was read through.
    pub selected: bool,
}

/// A surface made of shelves, of mood tiles, or of both.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BrowsePage {
    pub title: String,
    #[serde(deserialize_with = "null_as_default")]
    pub shelves: Vec<Shelf>,
    #[serde(deserialize_with = "null_as_default")]
    pub moods: Vec<MoodChip>,
    /// Home's row of moods; other surfaces have none.
    #[serde(deserialize_with = "null_as_default")]
    pub chips: Vec<HomeChip>,
    /// Fetches the next few shelves of the page; empty at its end.
    pub continuation: String,
}

/// One of the account's earlier searches. The token removes it from the
/// account's history; a search with none cannot be removed there.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SearchHistoryEntry {
    pub query: String,
    pub token: String,
}

/// The queue the account has on another device: the phone, the website.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RemoteQueue {
    #[serde(deserialize_with = "null_as_default")]
    pub tracks: Vec<Track>,
    /// The entry that device was on.
    pub index: usize,
    /// What the queue plays from, when YouTube names it.
    pub title: String,
}

/// One of the YouTube channels a Google account holds. Each has its own
/// library. The id is empty for the account's own channel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub handle: String,
    /// The address of the channel's picture; empty when it has none.
    #[serde(rename = "avatarUrl")]
    pub avatar_url: String,
}

/// How much the core has kept of the songs played, to start them at once
/// and to play them offline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct CacheUsage {
    pub bytes: u64,
    pub tracks: u32,
}

/// Who is signed in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub handle: String,
    /// The address of the account's picture; empty when it has none.
    pub avatar_url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchResults {
    pub query: String,
    #[serde(deserialize_with = "item")]
    pub top_result: Option<Item>,
    /// What YouTube shows inside the top result's card: an artist's top
    /// songs, an album's songs, other versions of a song.
    #[serde(deserialize_with = "items")]
    pub top_result_items: Vec<Item>,
    #[serde(deserialize_with = "null_as_default")]
    pub shelves: Vec<Shelf>,
}

impl SearchResults {
    /// The songs among the results, each once, and no more than `most`.
    pub fn songs(self, most: usize) -> Vec<Track> {
        let mut songs: Vec<Track> = Vec::new();
        let found = self.shelves.into_iter().flat_map(|shelf| shelf.items);
        for item in found {
            if let Item::Track(track) = item
                && songs.len() < most
                && !songs.iter().any(|song| song.id == track.id)
            {
                songs.push(track);
            }
        }
        songs
    }
}

/// What a search is narrowed to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchFilter {
    #[default]
    All,
    Songs,
    Albums,
    Artists,
    Playlists,
    Videos,
    Podcasts,
    Episodes,
}

impl SearchFilter {
    pub const EVERY: [SearchFilter; 8] = [
        SearchFilter::All,
        SearchFilter::Songs,
        SearchFilter::Albums,
        SearchFilter::Artists,
        SearchFilter::Playlists,
        SearchFilter::Videos,
        SearchFilter::Podcasts,
        SearchFilter::Episodes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchFilter::All => "All",
            SearchFilter::Songs => "Songs",
            SearchFilter::Albums => "Albums",
            SearchFilter::Artists => "Artists",
            SearchFilter::Playlists => "Playlists",
            SearchFilter::Videos => "Videos",
            SearchFilter::Podcasts => "Podcasts",
            SearchFilter::Episodes => "Episodes",
        }
    }

    /// What the core calls it; nothing for an unfiltered search.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            SearchFilter::All => "",
            SearchFilter::Songs => "songs",
            SearchFilter::Albums => "albums",
            SearchFilter::Artists => "artists",
            SearchFilter::Playlists => "playlists",
            SearchFilter::Videos => "videos",
            SearchFilter::Podcasts => "podcasts",
            SearchFilter::Episodes => "episodes",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LibraryKind {
    #[default]
    Playlist,
    Album,
    Artist,
    Podcast,
}

impl LibraryKind {
    /// What the core calls it.
    pub(crate) fn wire(self) -> &'static str {
        match self {
            LibraryKind::Playlist => "playlist",
            LibraryKind::Album => "album",
            LibraryKind::Artist => "artist",
            LibraryKind::Podcast => "podcast",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub kind: LibraryKind,
    pub title: String,
    pub subtitle: String,
    #[serde(deserialize_with = "null_as_default")]
    pub artwork: Vec<Artwork>,
    /// Kept at the top of the library. Ours: YouTube Music has no pins.
    pub pinned: bool,
    /// The folder it is filed in; empty for none. Ours as well.
    pub folder_id: String,
    /// When the core first saw it in the library, as RFC 3339. `None` for
    /// what was there before the core kept track.
    pub added_at: Option<String>,
    /// When something of it was last played here, from the play log.
    pub last_played_at: Option<String>,
}

/// A folder in the library, made here and kept by the core.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Folder {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LyricLine {
    /// When the line starts. Zero throughout when the lyrics are not timed.
    pub at_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Lyrics {
    #[serde(deserialize_with = "null_as_default")]
    pub lines: Vec<LyricLine>,
    /// Whether the lines carry times and can follow the song.
    pub synced: bool,
    /// The whole text, for lyrics that came without lines.
    pub plain: String,
    /// Where the words came from, which their providers ask to be said.
    pub source: String,
}

impl Lyrics {
    /// The line being sung at `position_ms`: the last one that has started.
    /// `None` before the first, and always for lyrics without times.
    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        self.lines
            .partition_point(|line| line.at_ms <= position_ms)
            .checked_sub(1)
    }

    /// Lyrics that came as one text are cut into lines to show.
    pub(crate) fn with_lines(mut self) -> Self {
        if self.lines.is_empty() {
            self.lines = self
                .plain
                .lines()
                .map(|text| LyricLine {
                    at_ms: 0,
                    text: text.to_owned(),
                })
                .collect();
            self.synced = false;
        }
        self
    }
}

/// A playlist the core makes from what has been played here: On Repeat,
/// the daily mixes, Discover.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Mix {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(deserialize_with = "null_as_default")]
    pub tracks: Vec<Track>,
}

#[cfg(test)]
mod tests;
