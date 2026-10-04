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
        !id.is_empty() && self.of(kind).iter().any(|entry| entry.id == id)
    }

    /// Whether a song is kept from playing: blocked itself, or by its album
    /// or one of its artists.
    pub fn stops(&self, track: &Track) -> bool {
        if self.is_empty() {
            return false;
        }
        self.has(Kind::Song, &track.id)
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
            let name = name.to_owned();
            list.push(Entry { id, name });
        } else {
            list.retain(|entry| entry.id != id);
        }
        true
    }

    /// The list as the core takes it: ids alone.
    pub fn for_core(&self) -> session::Blocked {
        let ids = |list: &[Entry]| list.iter().map(|entry| entry.id.clone()).collect();
        session::Blocked {
            tracks: ids(&self.songs),
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
    fn nothing_without_an_id_is_blocked() {
        let mut blocked = Blocked::default();
        assert!(!blocked.set(Kind::Artist, "", "Somebody", true));
        assert!(blocked.is_empty());
    }
}
