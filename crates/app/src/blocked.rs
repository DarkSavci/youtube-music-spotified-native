//! What the listener never wants played: songs, artists and albums.
//!
//! The list is kept with the settings and told to the core, which is what
//! steps over a blocked song as the queue moves. Here it is only looked
//! up, to say which a menu's entry would do and to dim what is blocked.

use serde::{Deserialize, Serialize};
use spotified_client::models::Track;
use spotified_client::session;

/// What a block is of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Song,
    Artist,
    Album,
}

impl Kind {
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Song => "Song",
            Kind::Artist => "Artist",
            Kind::Album => "Album",
        }
    }
}

/// One thing blocked: its id, and the name to show it by.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub name: String,
    /// An album's songs, by id, once its page has been read: each is
    /// blocked while the album is, since a song does not always say what
    /// album it is on.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Blocked {
    pub songs: Vec<Entry>,
    pub artists: Vec<Entry>,
    pub albums: Vec<Entry>,
}

/// The channel an artist's id names. One from the library arrives wrapped
/// ("MPLA" before the channel's id), and is the same artist.
fn key(kind: Kind, id: &str) -> &str {
    match kind {
        Kind::Artist => crate::views::widgets::artist_page_id(id),
        Kind::Song | Kind::Album => id,
    }
}

impl Blocked {
    pub fn of(&self, kind: Kind) -> &[Entry] {
        match kind {
            Kind::Song => &self.songs,
            Kind::Artist => &self.artists,
            Kind::Album => &self.albums,
        }
    }

    fn of_mut(&mut self, kind: Kind) -> &mut Vec<Entry> {
        match kind {
            Kind::Song => &mut self.songs,
            Kind::Artist => &mut self.artists,
            Kind::Album => &mut self.albums,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty() && self.artists.is_empty() && self.albums.is_empty()
    }

    /// Whether this very song, artist or album is blocked.
    pub fn has(&self, kind: Kind, id: &str) -> bool {
        let id = key(kind, id);
        // An entry written by hand may name an artist as the library does.
        !id.is_empty() && self.of(kind).iter().any(|entry| key(kind, &entry.id) == id)
    }

    /// Whether a song is kept from playing: blocked itself, or by its album
    /// or one of its artists.
    pub fn stops(&self, track: &Track) -> bool {
        if self.is_empty() {
            return false;
        }
        self.has(Kind::Song, &track.id)
            || self.on_blocked_album(&track.id)
            || track
                .album
                .as_ref()
                .is_some_and(|album| self.has(Kind::Album, &album.id))
            || track
                .artists
                .iter()
                .any(|artist| self.has(Kind::Artist, &artist.id))
    }

    /// Blocks it, or lets it play again. Returns whether anything changed.
    pub fn set(&mut self, kind: Kind, id: &str, name: &str, blocked: bool) -> bool {
        let id = key(kind, id).to_owned();
        if id.is_empty() || self.has(kind, &id) == blocked {
            return false;
        }
        let list = self.of_mut(kind);
        if blocked {
            list.push(Entry {
                id,
                name: name.to_owned(),
                songs: Vec::new(),
            });
        } else {
            list.retain(|entry| key(kind, &entry.id) != id);
        }
        true
    }

    /// Whether a song is one of those a blocked album was found to hold.
    fn on_blocked_album(&self, track_id: &str) -> bool {
        self.albums
            .iter()
            .any(|album| album.songs.iter().any(|song| song == track_id))
    }

    /// Notes the songs a blocked album holds. Returns whether that changed
    /// anything: it does not for an album that is not blocked, one that
    /// came back empty, or songs already known.
    pub fn learn_album(&mut self, id: &str, songs: Vec<String>) -> bool {
        let Some(album) = self.albums.iter_mut().find(|album| album.id == id) else {
            return false;
        };
        if songs.is_empty() || album.songs == songs {
            return false;
        }
        album.songs = songs;
        true
    }

    /// The blocked albums whose songs are not known yet.
    pub fn albums_unread(&self) -> Vec<String> {
        let unread = self.albums.iter().filter(|album| album.songs.is_empty());
        unread.map(|album| album.id.clone()).collect()
    }

    /// The list as the core takes it: ids alone, a blocked album's songs
    /// among the songs.
    pub fn for_core(&self) -> session::Blocked {
        let ids = |list: &[Entry]| list.iter().map(|entry| entry.id.clone()).collect();
        let mut tracks: Vec<String> = ids(&self.songs);
        tracks.extend(self.albums.iter().flat_map(|album| album.songs.clone()));
        session::Blocked {
            tracks,
            artists: ids(&self.artists),
            albums: ids(&self.albums),
        }
    }
}

#[cfg(test)]
mod tests {
    use spotified_client::models::{AlbumRef, ArtistRef};

    use super::*;

    fn song() -> Track {
        Track {
            id: "s".into(),
            artists: vec![
                ArtistRef {
                    id: "UCa".into(),
                    name: "A".into(),
                },
                ArtistRef {
                    id: "UCb".into(),
                    name: "B".into(),
                },
            ],
            album: Some(AlbumRef {
                id: "MPREb".into(),
                name: "Album".into(),
            }),
            ..Track::default()
        }
    }

    #[test]
    fn a_song_is_stopped_by_itself_its_album_or_any_of_its_artists() {
        for (kind, id) in [
            (Kind::Song, "s"),
            (Kind::Album, "MPREb"),
            (Kind::Artist, "UCb"),
        ] {
            let mut blocked = Blocked::default();
            assert!(!blocked.stops(&song()));
            assert!(blocked.set(kind, id, "name", true));
            assert!(blocked.stops(&song()), "{kind:?}");
            assert!(blocked.set(kind, id, "name", false));
            assert!(!blocked.stops(&song()), "{kind:?}");
        }
    }

    #[test]
    fn an_artist_from_the_library_is_the_same_artist() {
        let mut blocked = Blocked::default();
        blocked.set(Kind::Artist, "MPLAUCa", "A", true);
        assert!(blocked.has(Kind::Artist, "UCa"));
        assert_eq!(blocked.for_core().artists, ["UCa"]);
        // Blocked once, however it is named.
        assert!(!blocked.set(Kind::Artist, "UCa", "A", true));
    }

    #[test]
    fn a_blocked_albums_songs_are_stopped_wherever_they_turn_up() {
        let mut blocked = Blocked::default();
        // A song as a Home shelf gives it: no album named.
        let bare = Track {
            album: None,
            ..song()
        };
        blocked.set(Kind::Album, "MPREb", "Album", true);
        assert!(!blocked.stops(&bare));
        assert_eq!(blocked.albums_unread(), ["MPREb"]);

        assert!(blocked.learn_album("MPREb", vec!["s".into(), "t".into()]));
        assert!(blocked.stops(&bare));
        assert!(blocked.albums_unread().is_empty());
        let core = blocked.for_core();
        assert_eq!(core.tracks, ["s", "t"]);
        assert_eq!(core.albums, ["MPREb"]);
        // Read again with the same songs, nothing changes.
        assert!(!blocked.learn_album("MPREb", vec!["s".into(), "t".into()]));
        // An album that is not blocked teaches nothing.
        assert!(!blocked.learn_album("MPREother", vec!["u".into()]));

        // Unblocked, its songs play again.
        blocked.set(Kind::Album, "MPREb", "Album", false);
        assert!(!blocked.stops(&bare));
        assert!(blocked.for_core().tracks.is_empty());
    }

    #[test]
    fn nothing_without_an_id_is_blocked() {
        let mut blocked = Blocked::default();
        assert!(!blocked.set(Kind::Artist, "", "Somebody", true));
        assert!(blocked.is_empty());
    }
}
