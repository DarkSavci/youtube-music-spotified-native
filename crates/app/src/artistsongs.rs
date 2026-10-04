//! Ordering an artist's songs by release.
//!
//! The list of all songs YouTube gives an artist is ordered by plays, stops
//! at about a hundred and fifty, and its rows carry no year, only the album
//! each song is on. So the year is looked up by album: the artist's page
//! supplies most of them, and the rest come as the releases are opened. A
//! song whose year is still unknown is not guessed at: it goes last.

use std::collections::{HashMap, HashSet};

use spotified_client::models::{Album, AlbumRef, ArtistRef, Track};

#[cfg(test)]
mod tests;

/// "1.9B plays" as a number; nothing when the text is not a count.
pub fn play_count(text: &str) -> f64 {
    let text = text.trim();
    let digits = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(text.len());
    let Ok(number) = text[..digits].replace(',', "").parse::<f64>() else {
        return 0.0;
    };
    let scale = match text[digits..].trim_start().chars().next() {
        Some('B') => 1e9,
        Some('M') => 1e6,
        Some('K') => 1e3,
        _ => 1.0,
    };
    number * scale
}

/// Most played first, by the counts the rows show. YouTube's own order is
/// not quite that, and a list labelled with counts reads as wrong when it
/// disagrees with them. Left alone when any row has no count.
pub fn by_plays(tracks: Vec<Track>) -> Vec<Track> {
    let counted = |track: &Track| {
        track
            .play_count
            .trim_start()
            .starts_with(|c: char| c.is_ascii_digit())
    };
    if !tracks.iter().all(counted) {
        return tracks;
    }
    let mut keyed: Vec<(f64, Track)> = tracks
        .into_iter()
        .map(|track| (play_count(&track.play_count), track))
        .collect();
    // Stable: songs with the same count keep YouTube's order.
    keyed.sort_by(|a, b| b.0.total_cmp(&a.0));
    keyed.into_iter().map(|(_, track)| track).collect()
}

/// The list in a random order. `random` gives numbers from nought up to,
/// not including, one.
pub fn shuffled<T>(mut items: Vec<T>, mut random: impl FnMut() -> f64) -> Vec<T> {
    for last in (1..items.len()).rev() {
        let other = (random() * (last + 1) as f64) as usize;
        items.swap(last, other.min(last));
    }
    items
}

/// Numbers from nought to one, random enough to shuffle with.
pub fn random() -> impl FnMut() -> f64 {
    use std::hash::{BuildHasher, Hasher};
    // The standard library seeds each hasher state from the system's
    // randomness; an xorshift carries on from there.
    let mut state = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
        | 1;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A name as it is compared: without case, and with its spaces evened out.
fn norm(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a credit list names the artist: by id, or by name when either
/// side has no id.
pub fn credits(artists: &[ArtistRef], who: &ArtistRef) -> bool {
    artists.iter().any(|artist| {
        if !artist.id.is_empty() && !who.id.is_empty() {
            artist.id == who.id
        } else {
            norm(&artist.name) == norm(&who.name)
        }
    })
}

/// One song across editions, videos and releases: its title. Every song
/// here credits the artist already, and the lead credit is not a safe
/// second key: a single and its album can credit one song differently.
fn song_key(track: &Track) -> String {
    norm(&track.title)
}

fn album_id(track: &Track) -> &str {
    track.album.as_ref().map_or("", |album| album.id.as_str())
}

/// The songs list completed from the artist's releases.
///
/// The releases' songs are added after the list: only those that credit
/// the artist, since a compilation or someone else's album they feature on
/// is mostly other people's songs. Each song appears once: a second video
/// of it, or its copy on another edition, single or compilation, is
/// dropped, from the list too. The first, most played, stays.
pub fn with_releases(tracks: &[Track], releases: &[&Album], who: &ArtistRef) -> Vec<Track> {
    let mut ids: HashSet<&str> = HashSet::new();
    let mut songs: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Track> = Vec::new();
    let on_compilation =
        |track: &Track| is_compilation(track.album.as_ref().map_or("", |album| &album.name));
    for track in tracks {
        let key = song_key(track);
        if let Some(&at) = songs.get(&key) {
            // The copy on the album it came from, not on a greatest hits,
            // is the one to place: in the better-played one's slot.
            if on_compilation(&out[at]) && !on_compilation(track) {
                out[at] = track.clone();
            }
            continue;
        }
        if !ids.insert(&track.id) {
            continue;
        }
        songs.insert(key, out.len());
        out.push(track.clone());
    }
    for release in releases {
        for track in &release.tracks {
            let artists = if track.artists.is_empty() {
                &release.artists
            } else {
                &track.artists
            };
            if !credits(artists, who) {
                continue;
            }
            let key = song_key(track);
            if ids.contains(track.id.as_str()) || songs.contains_key(&key) {
                continue;
            }
            ids.insert(&track.id);
            songs.insert(key, out.len());
            let mut song = track.clone();
            song.artists.clone_from(artists);
            if album_id(&song).is_empty() {
                song.album = Some(AlbumRef {
                    id: release.id.clone(),
                    name: release.title.clone(),
                });
            }
            out.push(song);
        }
    }
    out
}

/// Whether `text` holds `word` whole: not as part of a longer word.
fn holds_word(text: &str, word: &str) -> bool {
    let boundary = |c: Option<char>| c.is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    text.match_indices(word).any(|(at, found)| {
        boundary(text[..at].chars().next_back())
            && boundary(text[at + found.len()..].chars().next())
    })
}

/// What an edition adds to a release's name, in brackets.
const EDITION_WORDS: [&str; 13] = [
    "deluxe",
    "edition",
    "expanded",
    "remaster",
    "remastered",
    "anniversary",
    "bonus",
    "complete",
    "special",
    "super",
    "version",
    "alternate",
    "original motion picture",
];
/// And after a dash: "- Expanded Edition".
const EDITION_AFTER_DASH: [&str; 4] = ["deluxe", "expanded", "remastered", "special"];

/// The title with one edition taken off its end, if it ends with one.
fn without_edition(title: &str) -> Option<&str> {
    let trimmed = title.trim_end();
    if trimmed.ends_with([')', ']']) {
        let body = &trimmed[..trimmed.len() - 1];
        // The bracket that opens the last pair: nothing closes after it.
        let from = body.rfind([')', ']']).map_or(0, |at| at + 1);
        let open = from + body[from..].find(['(', '['])?;
        let inside = body[open + 1..].to_lowercase();
        return EDITION_WORDS
            .iter()
            .any(|word| holds_word(&inside, word))
            .then(|| trimmed[..open].trim_end());
    }
    trimmed.match_indices('-').find_map(|(at, _)| {
        let after = trimmed[at + 1..].trim_start().to_lowercase();
        EDITION_AFTER_DASH
            .iter()
            .any(|word| {
                after.starts_with(word)
                    && after[word.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
            })
            .then(|| trimmed[..at].trim_end())
    })
}

/// A release's name without its editions: "(Deluxe)", "[Remastered 2011]",
/// "- Expanded Edition".
pub fn edition_title(title: &str) -> &str {
    let mut plain = title;
    while let Some(shorter) = without_edition(plain) {
        plain = shorter;
    }
    if plain.trim().is_empty() {
        title.trim()
    } else {
        plain.trim()
    }
}

/// The release that stands for all of its editions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
}

fn year_of_album(album: &Album) -> Option<u32> {
    album.year.trim().parse().ok().filter(|year| *year > 0)
}

/// Every edition mapped to the one that stands for it: the earliest, or
/// the plainest-named of those from the same year. A deluxe edition's
/// bonus songs then sit with the album they extend, under its name.
pub fn editions<'a>(albums: impl IntoIterator<Item = &'a Album>) -> HashMap<String, Release> {
    let mut by_name: Vec<(String, Vec<&Album>)> = Vec::new();
    for album in albums {
        if album.id.is_empty() {
            continue;
        }
        let key = norm(edition_title(&album.title));
        match by_name.iter_mut().find(|(name, _)| *name == key) {
            Some((_, list)) => list.push(album),
            None => by_name.push((key, vec![album])),
        }
    }
    let mut out = HashMap::new();
    for (_, list) in by_name {
        let Some(main) = list
            .iter()
            .min_by_key(|album| (year_of_album(album).unwrap_or(u32::MAX), album.title.len()))
        else {
            continue;
        };
        let release = Release {
            id: main.id.clone(),
            title: edition_title(&main.title).to_owned(),
            year: year_of_album(main),
        };
        for album in &list {
            out.insert(album.id.clone(), release.clone());
        }
    }
    out
}

/// Releases that are mostly other people's songs, or the artist's again.
const COMPILATION_WORDS: [&str; 12] = [
    "greatest hits",
    "best of",
    "the best",
    "the highlights",
    "collection",
    "anthology",
    "essentials",
    "en iyileri",
    "karaoke",
    "tribute",
    "various artists",
    "compilation",
];

/// Whether a release's title says it gathers songs rather than releasing
/// them.
pub fn is_compilation(title: &str) -> bool {
    // Without the marks lowering leaves behind, so "En İyileri" reads as
    // "en iyileri".
    let plain: String = title
        .to_lowercase()
        .chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect();
    COMPILATION_WORDS
        .iter()
        .any(|word| holds_word(&plain, word))
}

/// Album id to release year, from every album at hand. The first to name
/// a year for an album is believed.
pub fn years_by_album<'a>(albums: impl IntoIterator<Item = &'a Album>) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    for album in albums {
        if let Some(year) = year_of_album(album)
            && !album.id.is_empty()
        {
            out.entry(album.id.clone()).or_insert(year);
        }
    }
    out
}

/// The albums the songs are on whose year is not known yet, in the order
/// they first appear.
pub fn missing_albums(tracks: &[Track], years: &HashMap<String, u32>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for track in tracks {
        let id = album_id(track);
        if !id.is_empty() && !years.contains_key(id) && !out.iter().any(|seen| seen == id) {
            out.push(id.to_owned());
        }
    }
    out
}

/// What is known of when releases came out.
pub struct Dates<'a> {
    pub years: &'a HashMap<String, u32>,
    pub canon: &'a HashMap<String, Release>,
}

impl Dates<'_> {
    /// The year a song counts as from, through the release that stands for
    /// its edition.
    pub fn year(&self, track: &Track) -> Option<u32> {
        let id = album_id(track);
        if id.is_empty() {
            return None;
        }
        self.canon
            .get(id)
            .and_then(|release| release.year)
            .or_else(|| self.years.get(id).copied())
    }
}

/// Newest first. Songs from the same year keep their order, the most
/// played first; songs with no known year go last, also in their order.
pub fn newest_first(tracks: &[Track], dates: &Dates) -> Vec<Track> {
    let mut keyed: Vec<(u32, &Track)> = tracks
        .iter()
        .map(|track| (dates.year(track).unwrap_or(0), track))
        .collect();
    keyed.sort_by_key(|(year, _)| std::cmp::Reverse(*year));
    keyed.into_iter().map(|(_, track)| track.clone()).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlbumGroup {
    /// The album's id; empty for songs on no album.
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub tracks: Vec<Track>,
}

/// The songs grouped album by album, newest album first, the editions of
/// one release together under it. Albums of unknown year follow the dated
/// ones, and songs on no album at all come last under one heading.
pub fn by_album(tracks: &[Track], dates: &Dates) -> Vec<AlbumGroup> {
    let mut groups: Vec<AlbumGroup> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for track in tracks {
        let own = album_id(track);
        let release = dates.canon.get(own);
        let id = release.map_or(own, |release| release.id.as_str());
        let index = *at.entry(id.to_owned()).or_insert_with(|| {
            let title = if id.is_empty() {
                "Other songs"
            } else {
                release
                    .map(|release| release.title.as_str())
                    .or(track.album.as_ref().map(|album| album.name.as_str()))
                    .filter(|title| !title.is_empty())
                    .unwrap_or("Unknown album")
            };
            groups.push(AlbumGroup {
                id: id.to_owned(),
                title: title.to_owned(),
                year: dates.year(track),
                tracks: Vec::new(),
            });
            groups.len() - 1
        });
        groups[index].tracks.push(track.clone());
    }
    let rank = |group: &AlbumGroup| match (group.id.is_empty(), group.year) {
        (true, _) => 2,
        (false, Some(_)) => 0,
        (false, None) => 1,
    };
    // Stable: albums of one year keep the order their songs came in.
    groups.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then(b.year.unwrap_or(0).cmp(&a.year.unwrap_or(0)))
    });
    groups
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleasePlan {
    /// Every release worth opening, the most useful first.
    pub order: Vec<String>,
    /// Of those, singles whose song is already known: opened last, if at
    /// all.
    pub covered: HashSet<String>,
}

/// The order to open an artist's releases in, for the dated orders.
///
/// First the releases listed songs are on whose year nothing else gives,
/// so the list itself can be placed. Then the artist's albums and EPs,
/// which hold most of the songs the list lacks. Then singles, except those
/// whose song is already known, which go last: a single is usually one
/// song, and opening it would only repeat what is there. Compilations are
/// left out; their songs are on the releases they came from.
pub fn release_plan(
    undated_listed: &[String],
    albums: &[&Album],
    singles: &[&Album],
    known: &[Track],
) -> ReleasePlan {
    let songs: HashSet<String> = known.iter().map(song_key).collect();
    let own = |list: &[&Album]| -> Vec<String> {
        list.iter()
            .filter(|album| !album.id.is_empty() && !is_compilation(&album.title))
            .map(|album| album.id.clone())
            .collect()
    };
    let (last, open): (Vec<&&Album>, Vec<&&Album>) = singles
        .iter()
        .filter(|album| !album.id.is_empty() && !is_compilation(&album.title))
        .partition(|single| songs.contains(&norm(&single.title)));
    let ids = |list: Vec<&&Album>| -> Vec<String> {
        list.into_iter().map(|album| album.id.clone()).collect()
    };
    let mut order: Vec<String> = Vec::new();
    let early = undated_listed
        .iter()
        .cloned()
        .chain(own(albums))
        .chain(ids(open));
    for id in early {
        if !order.contains(&id) {
            order.push(id);
        }
    }
    let mut covered = HashSet::new();
    for id in ids(last) {
        if !order.contains(&id) {
            covered.insert(id.clone());
            order.push(id);
        }
    }
    ReleasePlan { order, covered }
}
