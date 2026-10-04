//! Sharing: the public YouTube Music link for something, for the clipboard.
//!
//! The link is YouTube Music's, never this app's: whoever receives it may
//! not have the app at all. Each kind has its own shape, and the ids the
//! catalogue hands out are not always the ones those links take.

/// What a link is to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Track,
    Album,
    Playlist,
    Artist,
    Podcast,
}

impl Kind {
    /// What the confirming message calls it.
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Track => "Song",
            Kind::Album => "Album",
            Kind::Playlist => "Playlist",
            Kind::Artist => "Artist",
            Kind::Podcast => "Podcast",
        }
    }
}

const BASE: &str = "https://music.youtube.com";

pub fn url(kind: Kind, id: &str) -> String {
    match kind {
        Kind::Track => format!("{BASE}/watch?v={id}"),
        // A playlist's browse id wears a prefix its link does not.
        Kind::Playlist => format!(
            "{BASE}/playlist?list={}",
            id.strip_prefix("VL").unwrap_or(id)
        ),
        // An album is a release page or, for some, a playlist.
        Kind::Album if id.starts_with("OLAK") => format!("{BASE}/playlist?list={id}"),
        Kind::Album | Kind::Podcast => format!("{BASE}/browse/{id}"),
        // The library wraps an artist's channel id; the link wants the
        // channel itself.
        Kind::Artist => {
            let wrapped = id
                .strip_prefix("MPLA")
                .filter(|rest| rest.starts_with("UC"));
            format!("{BASE}/channel/{}", wrapped.unwrap_or(id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_its_own_link() {
        assert_eq!(
            url(Kind::Track, "abc"),
            "https://music.youtube.com/watch?v=abc"
        );
        assert_eq!(
            url(Kind::Album, "MPREb_1"),
            "https://music.youtube.com/browse/MPREb_1"
        );
        assert_eq!(
            url(Kind::Album, "OLAK5uy_x"),
            "https://music.youtube.com/playlist?list=OLAK5uy_x"
        );
        assert_eq!(
            url(Kind::Podcast, "MPSP1"),
            "https://music.youtube.com/browse/MPSP1"
        );
    }

    #[test]
    fn a_link_takes_the_id_without_the_wrapping_the_catalogue_gives_it() {
        assert_eq!(
            url(Kind::Playlist, "VLPL123"),
            "https://music.youtube.com/playlist?list=PL123"
        );
        assert_eq!(
            url(Kind::Artist, "MPLAUCabc"),
            "https://music.youtube.com/channel/UCabc"
        );
        assert_eq!(
            url(Kind::Artist, "UCabc"),
            "https://music.youtube.com/channel/UCabc"
        );
    }
}
