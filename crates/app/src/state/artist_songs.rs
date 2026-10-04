//! The page of all an artist's songs: YouTube's most played, newest first,
//! or album by album.
//!
//! The most-played list is the playlist the artist's page links to, and
//! YouTube stops it at about a hundred and fifty. The dated orders complete
//! it from the artist's releases, which are opened only once one of those
//! orders is chosen, so those orders really are every song.

use std::collections::HashMap;
use std::ops::Range;

use spotified_client::models::{Album, Artist, ArtistRef, Artwork, Track};

use super::more::Tail;
use crate::artistsongs::{
    Dates, ReleasePlan, by_album, by_plays, editions, missing_albums, newest_first, release_plan,
    with_releases, years_by_album,
};

/// How many releases open without asking. Each is a request to YouTube, so
/// the first batch covers the artist's own albums and the likeliest
/// singles, and a long discography opens this many more each time the
/// listener asks.
pub const RELEASE_BATCH: usize = 20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SongOrder {
    #[default]
    Popular,
    Newest,
    Album,
}

impl SongOrder {
    pub const EVERY: [SongOrder; 3] = [SongOrder::Popular, SongOrder::Newest, SongOrder::Album];

    pub fn label(self) -> &'static str {
        match self {
            SongOrder::Popular => "Popular",
            SongOrder::Newest => "Newest",
            SongOrder::Album => "By album",
        }
    }

    /// Whether the order needs to know when each song came out.
    pub fn dated(self) -> bool {
        self != SongOrder::Popular
    }
}

/// The heading of one album's songs in the album-by-album order.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupHead {
    /// The album's id; empty for songs on no album.
    pub id: String,
    pub title: String,
    /// "2001 · 14 songs".
    pub detail: String,
    pub art: Vec<Artwork>,
    /// Which of the songs shown are this album's.
    pub songs: Range<usize>,
}

#[derive(Debug, Default)]
pub struct ArtistSongs {
    pub artist_id: String,
    pub order: SongOrder,
    /// YouTube's list of the artist's songs, as much of it as has come.
    pub listed: Vec<Track>,
    /// `None` until the first page of the list has been asked for.
    pub list: Option<Tail>,
    /// Their albums and their singles beyond what their page shows; `None`
    /// until a dated order has asked and been answered.
    pub discography: Option<(Vec<Album>, Vec<Album>)>,
    pub discography_asked: bool,
    /// The releases opened so far, in the order they came; `None` for one
    /// that could not be opened.
    pub opened: Vec<(String, Option<Album>)>,
    pub opening: Vec<String>,
    /// How many releases may be opened.
    pub limit: usize,
    /// YouTube refused: nothing more is opened until the listener asks.
    pub limited: bool,
    /// The releases worth opening, in the order to open them.
    pub plan: ReleasePlan,
    /// What is on the page, in the order chosen: what Play plays.
    pub shown: Vec<Track>,
    /// The album headings, for the album-by-album order.
    pub groups: Vec<GroupHead>,
    /// The line under the buttons that says how much is here.
    pub status: String,
    /// How many releases "Open more" would open; none when it is not on
    /// offer.
    pub can_open: usize,
    /// Whether what is on offer is only another go at those that failed.
    pub only_retry: bool,
}

impl ArtistSongs {
    pub fn of(artist_id: String) -> Self {
        Self {
            artist_id,
            limit: RELEASE_BATCH,
            ..Self::default()
        }
    }

    /// The whole of YouTube's list is here.
    pub fn complete(&self) -> bool {
        self.list
            .as_ref()
            .is_some_and(|tail| !tail.more() && !tail.loading && tail.failed.is_none())
    }

    /// Whether the releases may be opened: a dated order is chosen, and
    /// the list and the discography that say which to open are here.
    pub fn ready(&self) -> bool {
        self.order.dated() && self.complete() && self.discography.is_some()
    }

    pub fn is_opened(&self, id: &str) -> bool {
        self.opened.iter().any(|(opened, _)| opened == id)
    }

    fn failed(&self) -> usize {
        self.opened
            .iter()
            .filter(|(_, album)| album.is_none())
            .count()
    }

    /// The next release to open, if another may be opened now.
    pub fn next_release(&self) -> Option<&str> {
        if !self.ready() || self.limited || self.opened.len() + self.opening.len() >= self.limit {
            return None;
        }
        self.plan
            .order
            .iter()
            .find(|id| !self.is_opened(id) && !self.opening.contains(id))
            .map(String::as_str)
    }

    /// Lets the failures be tried again, and that many more opened beside
    /// them.
    pub fn open_more(&mut self) {
        self.limit = self.opened.len() - self.failed() + RELEASE_BATCH;
        self.opened.retain(|(_, album)| album.is_some());
        self.limited = false;
    }

    /// Works out what the page shows from what has arrived. Called after
    /// anything changes; the page itself only reads the result.
    pub fn refresh(&mut self, artist: &Artist) {
        let tracks = by_plays(self.listed.clone());
        let dated = self.order.dated();
        let releases: Vec<&Album> = self
            .opened
            .iter()
            .filter_map(|(_, album)| album.as_ref())
            .collect();
        let who = ArtistRef {
            id: artist.id.clone(),
            name: artist.name.clone(),
        };
        let all = if dated {
            with_releases(&tracks, &releases, &who)
        } else {
            tracks.clone()
        };
        let (more_albums, more_singles) = match &self.discography {
            Some((albums, singles)) => (albums.as_slice(), singles.as_slice()),
            None => (&[][..], &[][..]),
        };
        let albums_all: Vec<&Album> = artist.albums.iter().chain(more_albums).collect();
        let singles_all: Vec<&Album> = artist.singles.iter().chain(more_singles).collect();
        let listed_years = years_by_album(albums_all.iter().chain(&singles_all).copied());
        self.plan = if self.ready() {
            let undated = missing_albums(&tracks, &listed_years);
            release_plan(&undated, &albums_all, &singles_all, &all)
        } else {
            ReleasePlan::default()
        };
        // The year the artist's own lists give an album is believed before
        // the one an opened release gives itself; for everything else the
        // opened release, which says the most, comes first.
        let listed = albums_all.iter().chain(&singles_all).copied();
        let years = years_by_album(listed.clone().chain(releases.iter().copied()));
        let mut by_id: HashMap<&str, &Album> = HashMap::new();
        for album in releases.iter().copied().chain(listed) {
            by_id.entry(album.id.as_str()).or_insert(album);
        }
        // In a settled order, so which of two like editions stands for the
        // release does not change from one look to the next.
        let mut each: Vec<&Album> = by_id.values().copied().collect();
        each.sort_by(|a, b| a.id.cmp(&b.id));
        let canon = editions(each);
        let dates = Dates {
            years: &years,
            canon: &canon,
        };
        self.groups.clear();
        self.shown = match self.order {
            SongOrder::Popular => all,
            SongOrder::Newest => newest_first(&all, &dates),
            SongOrder::Album => {
                let mut shown = Vec::with_capacity(all.len());
                for group in by_album(&all, &dates) {
                    let cover = by_id
                        .get(group.id.as_str())
                        .map(|album| &album.artwork)
                        .filter(|art| !art.is_empty())
                        .or(group.tracks.first().map(|track| &track.artwork));
                    let year = match (group.year, group.id.is_empty()) {
                        (Some(year), _) => year.to_string(),
                        (None, false) => "Year unknown".to_owned(),
                        (None, true) => String::new(),
                    };
                    let songs = crate::views::format::songs(group.tracks.len());
                    self.groups.push(GroupHead {
                        detail: crate::views::format::middle_dotted([year.as_str(), &songs]),
                        art: cover.cloned().unwrap_or_default(),
                        songs: shown.len()..shown.len() + group.tracks.len(),
                        id: group.id,
                        title: group.title,
                    });
                    shown.extend(group.tracks);
                }
                shown
            }
        };
        let undated = if dated {
            self.shown
                .iter()
                .filter(|track| dates.year(track).is_none())
                .count()
        } else {
            0
        };
        self.say(tracks.len(), undated);
    }

    /// Writes the status line, and what "Open more" would do.
    fn say(&mut self, listed: usize, undated: usize) {
        let plural = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        self.can_open = 0;
        self.only_retry = false;
        let shown = self.shown.len();
        if self.list.as_ref().is_some_and(|tail| tail.failed.is_some()) {
            self.status = format!("{listed} songs · Some songs could not be loaded.");
            return;
        }
        if !self.complete() {
            self.status = format!("Loading songs… {listed} so far");
            return;
        }
        if !self.order.dated() {
            // YouTube's list stops at about 150; a shorter one is not cut
            // short.
            self.status = if listed >= 100 {
                format!(
                    "The {listed} most played · Newest and By album add the rest of the discography"
                )
            } else {
                format!("{listed} songs · Newest and By album add any the list misses")
            };
            return;
        }
        let unopened: Vec<&String> = self
            .plan
            .order
            .iter()
            .filter(|id| !self.is_opened(id))
            .collect();
        let opened = self
            .plan
            .order
            .iter()
            .filter(|id| {
                self.opened
                    .iter()
                    .any(|(opened, album)| opened == *id && album.is_some())
            })
            .count();
        // A refusal stops the run, so nothing is pending until asked again.
        let pending = !self.opening.is_empty()
            || (!self.limited && !unopened.is_empty() && self.opened.len() < self.limit);
        if self.discography.is_none() || pending {
            self.status = format!(
                "{shown} songs · opening releases {opened} of {}…",
                self.plan.order.len()
            );
            return;
        }
        let covered = unopened
            .iter()
            .filter(|id| self.plan.covered.contains(**id))
            .count();
        let failed = self.failed();
        let mut parts = vec![format!(
            "{shown} songs from {}",
            plural(opened, "release", "releases")
        )];
        if unopened.len() > covered {
            let left = unopened.len() - covered;
            parts.push(format!(
                "{} not opened yet",
                plural(left, "more release", "more releases")
            ));
        }
        if covered > 0 {
            let songs = if covered == 1 {
                "its title song is"
            } else {
                "their title songs are"
            };
            parts.push(format!(
                "{} not opened, as {songs} already here",
                plural(covered, "single", "singles")
            ));
        }
        if self.limited {
            parts.push("YouTube is limiting requests; try again in a bit".to_owned());
        } else if failed > 0 {
            parts.push(format!("{failed} could not be opened"));
        }
        if undated > 0 {
            parts.push(format!(
                "{undated} without a known release date, shown last"
            ));
        }
        self.status = parts.join(" · ");
        self.can_open = RELEASE_BATCH.min(unopened.len() + failed);
        self.only_retry = unopened.is_empty();
    }
}
