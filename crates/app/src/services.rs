//! Other music services: a link to the same song, album or artist on
//! Spotify, Apple Music or Deezer, and what a link from one of them, or
//! from SoundCloud, is to, so that it can be found here.
//!
//! None of it needs an account or a key. Spotify's own catalogue is shut
//! to anyone without a Premium account, so it is reached around: a song
//! through ListenBrainz's index of Spotify, an artist through Wikidata,
//! and an album through one of its songs, whose public page names the
//! album it is on. Apple Music and Deezer are searched as they offer.
//!
//! Every call here waits on the network, and is made off the UI thread.

use serde_json::Value;
use ureq::{Agent, ResponseExt};

/// Who is asking, as ListenBrainz and Wikidata ask to be told.
const USER_AGENT: &str = concat!(
    "spotified-native/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/DarkSavci/youtube-music-spotified-native )"
);

/// A service a link can be found on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Spotify,
    AppleMusic,
    Deezer,
}

impl Service {
    /// In the order the share menu offers them.
    pub const EVERY: [Service; 3] = [Service::Spotify, Service::AppleMusic, Service::Deezer];

    pub fn name(self) -> &'static str {
        match self {
            Service::Spotify => "Spotify",
            Service::AppleMusic => "Apple Music",
            Service::Deezer => "Deezer",
        }
    }
}

/// What a link is looked for to, known by its names.
#[derive(Debug, Clone, PartialEq)]
pub enum Wanted {
    /// `album` may be empty.
    Song {
        title: String,
        artist: String,
        album: String,
    },
    /// `songs` are some of the titles on it, first first.
    Album {
        title: String,
        artist: String,
        songs: Vec<String>,
    },
    /// `channel` is the artist's id here, a YouTube channel's.
    Artist { name: String, channel: String },
}

impl Wanted {
    /// What the message that follows calls it.
    pub fn noun(&self) -> &'static str {
        match self {
            Wanted::Song { .. } => "song",
            Wanted::Album { .. } => "album",
            Wanted::Artist { .. } => "artist",
        }
    }
}

/// The link to `wanted` on `service`; `None` if it is not found there.
pub fn find(agent: &Agent, service: Service, wanted: &Wanted) -> Result<Option<String>, String> {
    match service {
        Service::Spotify => spotify::find(agent, wanted),
        Service::AppleMusic => apple::find(agent, wanted),
        Service::Deezer => deezer::find(agent, wanted),
    }
}

/// What a link from another service is to.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub kind: FoundKind,
    pub title: String,
    /// Empty for an artist, and for a playlist.
    pub artist: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundKind {
    Song,
    Album,
    Artist,
    Playlist,
}

impl Found {
    /// What to search for here to find it.
    pub fn query(&self) -> String {
        format!("{} {}", self.title, self.artist).trim().to_owned()
    }
}

/// Whether `text` is a link [`read`] can tell the meaning of.
pub fn is_link(text: &str) -> bool {
    source(text).is_some()
}

/// What the link `text` is to.
pub fn read(agent: &Agent, text: &str) -> Result<Found, String> {
    let text = text.trim();
    match source(text).ok_or("That is not a link this app can read")? {
        Source::Spotify => spotify::read(agent, &spotify::web_link(text)),
        Source::AppleMusic => apple::read(agent, text),
        Source::Deezer => deezer::read(agent, text),
        Source::SoundCloud => soundcloud::read(agent, text),
    }
}

/// Where a pasted link is from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Spotify,
    AppleMusic,
    Deezer,
    SoundCloud,
}

fn source(text: &str) -> Option<Source> {
    let text = text.trim();
    if text.contains(char::is_whitespace) {
        return None;
    }
    if text.starts_with("spotify:") {
        return Some(Source::Spotify);
    }
    let host = host(text)?;
    match host.strip_prefix("www.").unwrap_or(&host) {
        "open.spotify.com" | "spotify.link" => Some(Source::Spotify),
        "music.apple.com" | "itunes.apple.com" => Some(Source::AppleMusic),
        "deezer.com" | "link.deezer.com" | "deezer.page.link" => Some(Source::Deezer),
        "soundcloud.com" | "m.soundcloud.com" | "on.soundcloud.com" => Some(Source::SoundCloud),
        _ => None,
    }
}

/// The host of a web link, lower case; `None` if `text` is not one.
fn host(text: &str) -> Option<String> {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// The parts of a link's path, without its query.
fn path_parts(link: &str) -> Vec<&str> {
    let rest = link.split_once("://").map_or(link, |(_, rest)| rest);
    let path = rest.split(['?', '#']).next().unwrap_or_default();
    path.split('/')
        .skip(1)
        .filter(|part| !part.is_empty())
        .collect()
}

/// A parameter of a link's query.
fn query_value<'a>(link: &'a str, key: &str) -> Option<&'a str> {
    let query = link.split_once('?')?.1.split('#').next()?;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == key)
        .map(|(_, value)| value)
}

fn get_json(agent: &Agent, url: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let mut request = agent.get(url).header("User-Agent", USER_AGENT).header(
        "Accept",
        "application/json, application/sparql-results+json",
    );
    for (name, value) in query {
        request = request.query(*name, *value);
    }
    let mut response = request.call().map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let host = host(url).unwrap_or_default();
        return Err(format!("{host} answered {status}"));
    }
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&body).map_err(|error| error.to_string())
}

/// A web page: where it ended up after any redirects, and its text.
fn get_page(agent: &Agent, url: &str) -> Result<(String, String), String> {
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "text/html")
        .call()
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let host = host(url).unwrap_or_default();
        return Err(format!("{host} answered {status}"));
    }
    let landed = response.get_uri().to_string();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| error.to_string())?;
    Ok((landed, body))
}

/// The `content` of the page's `<meta>` whose `property` or `name` is
/// `key`.
fn meta(page: &str, key: &str) -> Option<String> {
    page.split("<meta ").skip(1).find_map(|rest| {
        let tag = rest.split('>').next()?;
        let named = attribute(tag, "property").or_else(|| attribute(tag, "name"));
        (named.as_deref() == Some(key))
            .then(|| attribute(tag, "content"))
            .flatten()
    })
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let value = &tag[start..];
    let end = value.find('"')?;
    Some(unescape(&value[..end]))
}

/// The few entities a page's attributes are written with.
fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// A name as it is compared: lower case, letters and digits, one space
/// between words. Turkish's dotless i is an i: written in capitals it
/// comes back lower case with the dot.
fn plain(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'ı' => 'i',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a title found is the one wanted, perhaps with more after it:
/// "One More Time (Radio Edit)" for "One More Time" is the same song to a
/// listener, and the store's choice of which to keep.
fn alike(found: &str, wanted: &str) -> bool {
    let (found, wanted) = (plain(found), plain(wanted));
    !wanted.is_empty()
        && (found == wanted
            || found.starts_with(&format!("{wanted} "))
            || wanted.starts_with(&format!("{found} ")))
}

/// Whether the artist found is the one wanted, or one of them.
fn same_artist(found: &str, wanted: &str) -> bool {
    let (found, wanted) = (plain(found), plain(wanted));
    !found.is_empty() && !wanted.is_empty() && (found.contains(&wanted) || wanted.contains(&found))
}

/// An album's name without what a store adds to say what sort it is.
fn album_title(title: &str) -> &str {
    [" - Single", " - EP", " - Album"]
        .into_iter()
        .find_map(|suffix| title.strip_suffix(suffix))
        .unwrap_or(title)
}

/// The YouTube channel an artist's id here is, if it is one.
fn channel_of(id: &str) -> Option<&str> {
    let channel = id.strip_prefix("MPLA").unwrap_or(id);
    channel.starts_with("UC").then_some(channel)
}

mod spotify {
    use super::*;

    const LISTENBRAINZ: &str = "https://labs.api.listenbrainz.org/spotify-id-from-metadata/json";
    const WIKIDATA: &str = "https://query.wikidata.org/sparql";
    const WEB: &str = "https://open.spotify.com";
    /// How many of a song's matches, and of an album's songs, are tried
    /// in looking for the album; each try is two pages.
    const ALBUM_SONGS: usize = 3;
    const MATCHES_PER_SONG: usize = 4;

    pub(super) fn find(agent: &Agent, wanted: &Wanted) -> Result<Option<String>, String> {
        match wanted {
            Wanted::Song {
                title,
                artist,
                album,
            } => {
                let ids = track_ids(agent, artist, album, title)?;
                Ok(ids.first().map(|id| format!("{WEB}/track/{id}")))
            }
            Wanted::Album {
                title,
                artist,
                songs,
            } => album(agent, title, artist, songs),
            Wanted::Artist { name, channel } => artist(agent, name, channel),
        }
    }

    /// The Spotify songs ListenBrainz takes these names to be, the likeliest
    /// first.
    fn track_ids(
        agent: &Agent,
        artist: &str,
        album: &str,
        title: &str,
    ) -> Result<Vec<String>, String> {
        let query = [
            ("artist_name", artist),
            ("release_name", album),
            ("track_name", title),
        ];
        let answer = get_json(agent, LISTENBRAINZ, &query)?;
        Ok(answer[0]["spotify_track_ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect())
    }

    /// An album is found through its songs: each song Spotify has is on
    /// some album, which its page names. A song is on singles and
    /// compilations too, so the album must be called what is wanted.
    fn album(
        agent: &Agent,
        title: &str,
        artist: &str,
        songs: &[String],
    ) -> Result<Option<String>, String> {
        let mut seen = Vec::new();
        for song in songs.iter().take(ALBUM_SONGS) {
            let ids = track_ids(agent, artist, title, song)?;
            for id in ids.iter().take(MATCHES_PER_SONG) {
                let (_, page) = get_page(agent, &format!("{WEB}/track/{id}"))?;
                let Some(link) = meta(&page, "music:album") else {
                    continue;
                };
                if seen.contains(&link) {
                    continue;
                }
                let (_, album_page) = get_page(agent, &link)?;
                let name = meta(&album_page, "og:title").map(|title| album_name(&title));
                if name.is_some_and(|name| alike(&name, title)) {
                    return Ok(Some(link));
                }
                seen.push(link);
            }
        }
        Ok(None)
    }

    /// An album page's title is "Listen to Discovery by Daft Punk |
    /// Spotify"; the name is what is between.
    pub(super) fn album_name(title: &str) -> String {
        let title = title.strip_prefix("Listen to ").unwrap_or(title);
        let title = title.strip_suffix(" | Spotify").unwrap_or(title);
        let title = title.rsplit_once(" by ").map_or(title, |(name, _)| name);
        album_title(title).to_owned()
    }

    /// An artist is found in Wikidata, which keeps Spotify's id for many:
    /// by the YouTube channel when it keeps that too, which is certain,
    /// and otherwise by the name, when only one artist has it.
    fn artist(agent: &Agent, name: &str, channel: &str) -> Result<Option<String>, String> {
        let by_channel = channel_of(channel).map(|channel| {
            format!(
                "SELECT DISTINCT ?id WHERE {{ ?artist wdt:P2397 \"{}\"; wdt:P1902 ?id. }}",
                sparql_text(channel)
            )
        });
        let name = sparql_text(name);
        let by_name = format!(
            "SELECT DISTINCT ?id WHERE {{ VALUES ?label {{ \"{name}\"@en \"{name}\"@mul }} \
             ?artist rdfs:label ?label; wdt:P1902 ?id. }}"
        );
        for query in by_channel.into_iter().chain([by_name]) {
            let answer = get_json(agent, WIKIDATA, &[("query", &query), ("format", "json")])?;
            let ids: Vec<&str> = answer["results"]["bindings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| row["id"]["value"].as_str())
                .collect();
            // Two artists of one name: neither is surely the one.
            if let [id] = ids.as_slice() {
                return Ok(Some(format!("{WEB}/artist/{id}")));
            }
        }
        Ok(None)
    }

    /// Text put between quotes in a query.
    fn sparql_text(text: &str) -> String {
        text.replace('\\', "\\\\").replace('"', "\\\"")
    }

    /// A Spotify URI as the web link to the same thing.
    pub(super) fn web_link(text: &str) -> String {
        match text.strip_prefix("spotify:") {
            Some(rest) => format!("{WEB}/{}", rest.replace(':', "/")),
            None => text.to_owned(),
        }
    }

    /// What a Spotify page is, from what it says of itself to anything
    /// that shows a preview of it.
    pub(super) fn read(agent: &Agent, link: &str) -> Result<Found, String> {
        let (landed, page) = get_page(agent, link)?;
        let title = meta(&page, "og:title").ok_or("Spotify did not say what that link is to")?;
        // "Daft Punk · One More Time · Song · 2000"
        let byline = meta(&page, "og:description")
            .and_then(|text| text.split(" · ").next().map(str::to_owned))
            .unwrap_or_default();
        let found = |kind, title: String, artist: String| Found {
            kind,
            title,
            artist,
        };
        match meta(&page, "og:type").as_deref() {
            Some("music.song") => {
                let artist = meta(&page, "music:musician_description").unwrap_or(byline);
                Ok(found(FoundKind::Song, title, artist))
            }
            Some("music.album") => Ok(found(FoundKind::Album, album_name(&title), byline)),
            Some("music.playlist") => Ok(found(FoundKind::Playlist, title, String::new())),
            _ if path_parts(&landed).contains(&"artist") => {
                Ok(found(FoundKind::Artist, title, String::new()))
            }
            _ => Err("That Spotify link is not to a song, album, artist or playlist".to_owned()),
        }
    }
}

mod apple {
    use super::*;

    const SEARCH: &str = "https://itunes.apple.com/search";
    const LOOKUP: &str = "https://itunes.apple.com/lookup";
    /// Where a store is chosen and the person's own has nothing.
    const FALLBACK_STORE: &str = "US";

    /// Apple Music is a store a country, and a song may be sold in one and
    /// not another: the person's own is searched first.
    pub(super) fn find(agent: &Agent, wanted: &Wanted) -> Result<Option<String>, String> {
        let own = crate::platform::region::country();
        let stores = own
            .iter()
            .map(String::as_str)
            .chain([FALLBACK_STORE])
            .collect::<Vec<_>>();
        let mut tried = Vec::new();
        for store in stores {
            if tried.contains(&store) {
                continue;
            }
            tried.push(store);
            if let Some(link) = find_in(agent, wanted, store)? {
                return Ok(Some(link));
            }
        }
        Ok(None)
    }

    fn find_in(agent: &Agent, wanted: &Wanted, store: &str) -> Result<Option<String>, String> {
        let (term, entity) = match wanted {
            Wanted::Song { title, artist, .. } => (format!("{artist} {title}"), "song"),
            Wanted::Album { title, artist, .. } => (format!("{artist} {title}"), "album"),
            Wanted::Artist { name, .. } => (name.clone(), "musicArtist"),
        };
        let query = [
            ("term", term.as_str()),
            ("entity", entity),
            ("country", store),
            ("limit", "10"),
        ];
        let answer = get_json(agent, SEARCH, &query)?;
        let results = answer["results"].as_array().cloned().unwrap_or_default();
        let text = |result: &Value, key: &str| result[key].as_str().unwrap_or_default().to_owned();
        let link = results.iter().find_map(|result| {
            let (matches, link) = match wanted {
                Wanted::Song { title, artist, .. } => (
                    alike(&text(result, "trackName"), title)
                        && same_artist(&text(result, "artistName"), artist),
                    text(result, "trackViewUrl"),
                ),
                Wanted::Album { title, artist, .. } => (
                    alike(album_title(&text(result, "collectionName")), title)
                        && same_artist(&text(result, "artistName"), artist),
                    text(result, "collectionViewUrl"),
                ),
                Wanted::Artist { name, .. } => (
                    plain(&text(result, "artistName")) == plain(name),
                    text(result, "artistLinkUrl"),
                ),
            };
            (matches && !link.is_empty()).then_some(link)
        });
        Ok(link.map(|link| without_tracking(&link)))
    }

    /// The store's links carry a parameter that says where they were got.
    fn without_tracking(link: &str) -> String {
        link.replace("?uo=4", "").replace("&uo=4", "")
    }

    /// `music.apple.com/tr/album/discovery/697194953?i=697195462` is a
    /// song on an album; without `i`, the album.
    pub(super) fn read(agent: &Agent, link: &str) -> Result<Found, String> {
        let parts = path_parts(link);
        let store = parts
            .first()
            .filter(|part| part.len() == 2)
            .copied()
            .unwrap_or(FALLBACK_STORE);
        if parts.contains(&"playlist") {
            let (_, page) = get_page(agent, link)?;
            let title = meta(&page, "og:title").ok_or("Apple Music did not name that playlist")?;
            let title = title.strip_suffix(" on Apple Music").unwrap_or(&title);
            return Ok(Found {
                kind: FoundKind::Playlist,
                title: title.to_owned(),
                artist: String::new(),
            });
        }
        let id = query_value(link, "i")
            .or_else(|| parts.last().copied())
            .filter(|id| id.chars().all(|c| c.is_ascii_digit()))
            .ok_or("That Apple Music link has no id")?;
        let answer = get_json(agent, LOOKUP, &[("id", id), ("country", store)])?;
        let result = &answer["results"][0];
        let text = |key: &str| result[key].as_str().unwrap_or_default().to_owned();
        let (kind, title, artist) = match text("wrapperType").as_str() {
            "track" => (FoundKind::Song, text("trackName"), text("artistName")),
            "collection" => (
                FoundKind::Album,
                album_title(&text("collectionName")).to_owned(),
                text("artistName"),
            ),
            "artist" => (FoundKind::Artist, text("artistName"), String::new()),
            _ => return Err("Apple Music did not say what that link is to".to_owned()),
        };
        Ok(Found {
            kind,
            title,
            artist,
        })
    }
}

mod deezer {
    use super::*;

    const API: &str = "https://api.deezer.com";

    pub(super) fn find(agent: &Agent, wanted: &Wanted) -> Result<Option<String>, String> {
        let (path, query) = match wanted {
            Wanted::Song { title, artist, .. } => ("search", format!("{artist} {title}")),
            Wanted::Album { title, artist, .. } => ("search/album", format!("{artist} {title}")),
            Wanted::Artist { name, .. } => ("search/artist", name.clone()),
        };
        let answer = get_json(agent, &format!("{API}/{path}"), &[("q", &query)])?;
        let results = answer["data"].as_array().cloned().unwrap_or_default();
        let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
        let mut matching = results.iter().filter(|result| match wanted {
            Wanted::Song { title, artist, .. } | Wanted::Album { title, artist, .. } => {
                alike(&text(&result["title"]), title)
                    && same_artist(&text(&result["artist"]["name"]), artist)
            }
            Wanted::Artist { name, .. } => plain(&text(&result["name"])) == plain(name),
        });
        // Many artists share a name, and the one meant is the one listened
        // to; songs and albums come likeliest first already.
        let best = match wanted {
            Wanted::Artist { .. } => matching.max_by_key(|result| result["nb_fan"].as_u64()),
            Wanted::Song { .. } | Wanted::Album { .. } => matching.next(),
        };
        Ok(best
            .map(|result| text(&result["link"]))
            .filter(|link| !link.is_empty()))
    }

    /// `deezer.com/en/track/3135556`; a short link is followed to one.
    pub(super) fn read(agent: &Agent, link: &str) -> Result<Found, String> {
        let landed = if host(link).is_some_and(|host| host.ends_with("deezer.com")) {
            link.to_owned()
        } else {
            get_page(agent, link)?.0
        };
        let parts = path_parts(&landed);
        let (kind, id) = parts
            .windows(2)
            .find_map(|pair| match pair[0] {
                "track" => Some((FoundKind::Song, pair[1])),
                "album" => Some((FoundKind::Album, pair[1])),
                "artist" => Some((FoundKind::Artist, pair[1])),
                "playlist" => Some((FoundKind::Playlist, pair[1])),
                _ => None,
            })
            .ok_or("That Deezer link is not to a song, album, artist or playlist")?;
        let path = match kind {
            FoundKind::Song => "track",
            FoundKind::Album => "album",
            FoundKind::Artist => "artist",
            FoundKind::Playlist => "playlist",
        };
        let answer = get_json(agent, &format!("{API}/{path}/{id}"), &[])?;
        if let Some(message) = answer["error"]["message"].as_str() {
            return Err(format!("Deezer: {message}"));
        }
        let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
        let (title, artist) = match kind {
            FoundKind::Artist => (text(&answer["name"]), String::new()),
            FoundKind::Playlist => (text(&answer["title"]), String::new()),
            FoundKind::Song | FoundKind::Album => {
                (text(&answer["title"]), text(&answer["artist"]["name"]))
            }
        };
        Ok(Found {
            kind,
            title,
            artist,
        })
    }
}

mod soundcloud {
    use super::*;

    const OEMBED: &str = "https://soundcloud.com/oembed";

    /// SoundCloud says what a link is to in its preview of it: the title
    /// "Flickermood by Forss", and the account it is on.
    pub(super) fn read(agent: &Agent, link: &str) -> Result<Found, String> {
        let answer = get_json(agent, OEMBED, &[("format", "json"), ("url", link)])?;
        let author = answer["author_name"].as_str().unwrap_or_default();
        let title = answer["title"]
            .as_str()
            .ok_or("SoundCloud did not name that link")?;
        let title = title
            .strip_suffix(&format!(" by {author}"))
            .unwrap_or(title)
            .to_owned();
        let kind = if path_parts(link).contains(&"sets") {
            FoundKind::Playlist
        } else {
            FoundKind::Song
        };
        Ok(Found {
            kind,
            title,
            artist: author.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_from_other_services_are_told_apart_from_anything_else() {
        assert!(is_link(
            "https://open.spotify.com/track/5F0fiSkDxZL5V3SlnTNhKh"
        ));
        assert!(is_link("https://open.spotify.com/intl-tr/album/0bH?si=x"));
        assert!(is_link("spotify:track:5F0fiSkDxZL5V3SlnTNhKh"));
        assert!(is_link("https://music.apple.com/tr/album/x/1?i=2"));
        assert!(is_link("https://www.deezer.com/en/track/3135556"));
        assert!(is_link("https://soundcloud.com/forss/flickermood"));
        assert!(!is_link("daft punk"));
        assert!(!is_link("https://music.youtube.com/watch?v=abc"));
        assert!(!is_link("https://open.spotify.com/track/x and more"));
    }

    #[test]
    fn a_spotify_uri_is_read_as_its_web_link() {
        assert_eq!(
            spotify::web_link("spotify:track:abc"),
            "https://open.spotify.com/track/abc"
        );
    }

    #[test]
    fn an_album_pages_title_gives_its_name() {
        assert_eq!(
            spotify::album_name("Listen to One More Time - Single by Daft Punk | Spotify"),
            "One More Time"
        );
        assert_eq!(
            spotify::album_name("Listen to Discovery - Album by Daft Punk | Spotify"),
            "Discovery"
        );
    }

    #[test]
    fn titles_match_with_what_a_store_adds_after_them() {
        assert!(alike("One More Time (Radio Edit)", "One More Time"));
        assert!(alike("ŞIMARIK", "Şımarık"));
        assert!(!alike("One More Time Again Tonight", "Once More"));
        assert!(!alike("Anything", ""));
        assert!(same_artist("Daft Punk & Pharrell Williams", "Daft Punk"));
        assert!(!same_artist("Daft Punk", ""));
    }

    #[test]
    fn a_pages_meta_tags_are_read_whichever_way_they_are_named() {
        let page = r#"<head><meta property="og:title" content="Rock &amp; Roll"/>
            <meta name="music:album" content="https://open.spotify.com/album/1"/></head>"#;
        assert_eq!(meta(page, "og:title").as_deref(), Some("Rock & Roll"));
        assert_eq!(
            meta(page, "music:album").as_deref(),
            Some("https://open.spotify.com/album/1")
        );
        assert_eq!(meta(page, "og:type"), None);
    }

    #[test]
    fn a_links_path_and_query_are_read_apart() {
        let link = "https://music.apple.com/tr/album/discovery/697194953?i=697195462&l=en";
        assert_eq!(path_parts(link), ["tr", "album", "discovery", "697194953"]);
        assert_eq!(query_value(link, "i"), Some("697195462"));
        assert_eq!(query_value(link, "x"), None);
    }

    /// Asks the services themselves: run by hand, with `--ignored`, to
    /// see that they still answer as they are read.
    #[test]
    #[ignore = "asks the services over the network"]
    fn every_service_answers() {
        let agent = crate::update::agent();
        let song = Wanted::Song {
            title: "One More Time".into(),
            artist: "Daft Punk".into(),
            album: "Discovery".into(),
        };
        let album = Wanted::Album {
            title: "Discovery".into(),
            artist: "Daft Punk".into(),
            songs: vec!["One More Time".into(), "Aerodynamic".into()],
        };
        let artist = Wanted::Artist {
            name: "Tarkan".into(),
            channel: String::new(),
        };
        for service in Service::EVERY {
            for wanted in [&song, &album, &artist] {
                let found = find(&agent, service, wanted);
                println!("{} {}: {found:?}", service.name(), wanted.noun());
                assert!(matches!(found, Ok(Some(_))), "{found:?}");
            }
        }
        for link in [
            "https://open.spotify.com/track/5F0fiSkDxZL5V3SlnTNhKh",
            "spotify:album:0bHuuhpKn42ktYbyTIeumE",
            "https://open.spotify.com/artist/4tZwfgrHOc3mvqYlEYSvVi",
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
            "https://music.apple.com/us/album/discovery/697194953?i=697195462",
            "https://music.apple.com/us/album/discovery/697194953",
            "https://music.apple.com/us/artist/daft-punk/5468295",
            "https://www.deezer.com/en/track/3135556",
            "https://www.deezer.com/album/302127",
            "https://soundcloud.com/forss/flickermood",
        ] {
            let read = read(&agent, link);
            println!("{link}: {read:?}");
            assert!(read.is_ok(), "{read:?}");
        }
    }
}
