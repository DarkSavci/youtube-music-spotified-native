//! Talks to the Go core over HTTP on localhost.
//!
//! Calls block; the app makes them from worker threads. Errors are typed so
//! the app can tell a signed-out account from an outage from a rate limit,
//! and each has a sentence fit to show.

pub mod migrate;
pub mod models;
pub mod session;
mod stats;

use std::fmt;
use std::io::Read;
use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use models::{
    Account, Album, Artist, BrowsePage, CacheUsage, Channel, Folder, LibraryItem, LibraryKind,
    Lyrics, Mix, Playlist, PlaylistPage, Podcast, RemoteQueue, SearchFilter, SearchHistoryEntry,
    SearchResults, Track,
};

/// The most pages a playlist is read through: the bound the core itself
/// uses for a whole playlist.
const PLAYLIST_PAGES_MOST: usize = 60;

/// The core answers from cache in milliseconds, but a miss waits on YouTube
/// behind its rate governor.
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// The core could not be reached at all.
    Unreachable(String),
    /// This needs an account and nobody is signed in.
    SignedOut,
    /// YouTube is refusing requests for now.
    RateLimited,
    /// The core answered with an error of its own.
    Status { code: u16, message: String },
    /// The answer was not what this build expects.
    Decode(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Unreachable(_) => write!(f, "The playback service is not answering."),
            ApiError::SignedOut => write!(f, "Sign in to see this."),
            ApiError::RateLimited => write!(
                f,
                "YouTube Music is limiting requests. Try again in a little while."
            ),
            ApiError::Status { message, .. } if !message.is_empty() => write!(f, "{message}"),
            ApiError::Status { code, .. } => write!(f, "Something went wrong (error {code})."),
            ApiError::Decode(_) => write!(f, "The answer could not be read."),
        }
    }
}

impl std::error::Error for ApiError {}

/// The body the core sends with an error status.
#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct ErrorBody {
    error: String,
    reauth: bool,
    rate_limited: bool,
}

#[derive(Clone)]
pub struct Client {
    origin: String,
    agent: ureq::Agent,
    /// For the event stream, which stays open: the same agent without the
    /// overall time limit.
    streaming: ureq::Agent,
}

impl Client {
    /// `origin` is the core's address, for example `http://127.0.0.1:51234`.
    pub fn new(origin: impl Into<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            // Error statuses carry a JSON body that says what went wrong.
            .http_status_as_error(false)
            .build()
            .new_agent();
        let streaming = ureq::Agent::config_builder()
            .timeout_connect(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .new_agent();
        Self {
            origin: origin.into(),
            agent,
            streaming,
        }
    }

    /// Home, or Home read through one of its mood chips when `mood` is
    /// that chip's params.
    pub fn home(&self, mood: &str) -> Result<BrowsePage, ApiError> {
        if mood.is_empty() {
            self.get("/v1/home")
        } else {
            self.get(&format!("/v1/home?mood={}", encode(mood)))
        }
    }

    /// The next few shelves of Home, from the token its last page ended
    /// with.
    pub fn home_more(&self, continuation: &str) -> Result<BrowsePage, ApiError> {
        self.get(&format!("/v1/home?continuation={}", encode(continuation)))
    }

    /// Any other surface: explore, the moods, one mood's page, the whole
    /// of a shelf.
    pub fn browse(&self, id: &str, params: &str) -> Result<BrowsePage, ApiError> {
        let path = format!("/v1/browse/{}?params={}", encode(id), encode(params));
        self.get(&path)
    }

    /// What the account has played, newest first, on any device.
    pub fn history(&self) -> Result<Vec<Track>, ApiError> {
        self.get("/v1/me/history")
    }

    /// Ways the text typed so far might go on.
    pub fn suggest(&self, query: &str) -> Result<Vec<String>, ApiError> {
        self.get(&format!("/v1/suggest?q={}", encode(query)))
    }

    /// What the account searched for lately, newest first. Empty when
    /// signed out.
    pub fn search_history(&self) -> Result<Vec<SearchHistoryEntry>, ApiError> {
        let entries: Option<Vec<SearchHistoryEntry>> = self.get("/v1/me/search-history")?;
        Ok(entries.unwrap_or_default())
    }

    /// Removes earlier searches from the account, by the tokens they came
    /// with.
    pub fn forget_searches(&self, tokens: &[String]) -> Result<(), ApiError> {
        self.post_empty("/v1/me/search-history/forget", &json!({ "tokens": tokens }))
    }

    /// The queue the account has on its other devices; empty signed out.
    pub fn remote_queue(&self) -> Result<RemoteQueue, ApiError> {
        self.get("/v1/me/remote-queue")
    }

    pub fn cache_usage(&self) -> Result<CacheUsage, ApiError> {
        self.get("/v1/cache")
    }

    /// Deletes every kept song, and says what is left.
    pub fn clear_cache(&self) -> Result<CacheUsage, ApiError> {
        let url = format!("{}/v1/cache", self.origin);
        let response = self.agent.delete(&url).call().map_err(unreachable)?;
        let body = successful_body(response)?;
        serde_json::from_str(&body).map_err(|error| ApiError::Decode(error.to_string()))
    }

    /// Who is signed in; `None` when nobody is, or it cannot be told.
    pub fn account(&self) -> Result<Option<Account>, ApiError> {
        #[derive(Deserialize)]
        struct Me {
            account: Option<Account>,
        }
        let me: Me = self.get("/v1/me")?;
        Ok(me.account)
    }

    /// The channels the signed-in account can act as.
    pub fn channels(&self) -> Result<Vec<Channel>, ApiError> {
        let channels: Option<Vec<Channel>> = self.get("/v1/me/channels")?;
        Ok(channels.unwrap_or_default())
    }

    pub fn album(&self, id: &str) -> Result<Album, ApiError> {
        self.get(&format!("/v1/albums/{}", encode(id)))
    }

    pub fn artist(&self, id: &str) -> Result<Artist, ApiError> {
        self.get(&format!("/v1/artists/{}", encode(id)))
    }

    pub fn playlist(&self, id: &str) -> Result<Playlist, ApiError> {
        self.get(&format!("/v1/playlists/{}", encode(id)))
    }

    /// Some of a playlist's songs: its first page, or the one a token
    /// from the page before fetches.
    pub fn playlist_page(&self, id: &str, continuation: &str) -> Result<PlaylistPage, ApiError> {
        let mut path = format!("/v1/playlists/{}?paged=1", encode(id));
        if !continuation.is_empty() {
            path.push_str(&format!("&continuation={}", encode(continuation)));
        }
        self.get(&path)
    }

    /// The rest of a playlist, read page by page from `next` on. A token
    /// seen before would go round for ever; the list ends there.
    pub fn playlist_rest(&self, id: &str, next: &str) -> Result<Vec<Track>, ApiError> {
        let mut tracks = Vec::new();
        let mut seen = vec![next.to_owned()];
        let mut cursor = next.to_owned();
        for _ in 0..PLAYLIST_PAGES_MOST {
            if cursor.is_empty() {
                break;
            }
            let page = self.playlist_page(id, &cursor)?;
            tracks.extend(page.playlist.tracks);
            if seen.contains(&page.next) {
                break;
            }
            seen.push(page.next.clone());
            cursor = page.next;
        }
        Ok(tracks)
    }

    /// A whole playlist, however many pages it takes.
    pub fn complete_playlist(&self, id: &str) -> Result<Playlist, ApiError> {
        let first = self.playlist_page(id, "")?;
        let mut playlist = first.playlist;
        playlist.tracks.extend(self.playlist_rest(id, &first.next)?);
        Ok(playlist)
    }

    pub fn podcast(&self, id: &str) -> Result<Podcast, ApiError> {
        self.get(&format!("/v1/podcasts/{}", encode(id)))
    }

    /// Songs like this one, as its radio would play them.
    pub fn radio_of(&self, track_id: &str) -> Result<Vec<Track>, ApiError> {
        let tracks: Option<Vec<Track>> = self.get(&format!("/v1/radio/{}", encode(track_id)))?;
        Ok(tracks.unwrap_or_default())
    }

    /// The song and its music video, as YouTube pairs them: both, when it
    /// names a pair, and otherwise the one that was asked about or nothing.
    pub fn versions(&self, track_id: &str) -> Result<Vec<Track>, ApiError> {
        let tracks: Option<Vec<Track>> =
            self.get(&format!("/v1/tracks/{}/versions", encode(track_id)))?;
        Ok(tracks.unwrap_or_default())
    }

    /// Where the core serves a video's picture, without its sound.
    pub fn video_stream_url(origin: &str, track_id: &str) -> String {
        format!(
            "{}/v1/video-stream/{}",
            origin.trim_end_matches('/'),
            encode(track_id)
        )
    }

    pub fn search(&self, query: &str, filter: SearchFilter) -> Result<SearchResults, ApiError> {
        let path = format!("/v1/search?q={}&filter={}", encode(query), filter.wire());
        self.get(&path)
    }

    pub fn library(&self) -> Result<Vec<LibraryItem>, ApiError> {
        self.get("/v1/me/library")
    }

    pub fn folders(&self) -> Result<Vec<Folder>, ApiError> {
        let folders: Option<Vec<Folder>> = self.get("/v1/me/folders")?;
        Ok(folders.unwrap_or_default())
    }

    pub fn create_folder(&self, name: &str) -> Result<(), ApiError> {
        self.post_empty("/v1/me/folders", &json!({ "name": name }))
    }

    /// Deletes a folder. What was in it goes back to the top of the library.
    pub fn delete_folder(&self, id: &str) -> Result<(), ApiError> {
        let url = format!("{}/v1/me/folders/{}", self.origin, encode(id));
        let response = self.agent.delete(&url).call().map_err(unreachable)?;
        successful_body(response).map(drop)
    }

    /// Pins a library item or files it in a folder, or both. `None` leaves
    /// that as it is; an empty folder id takes the item out of its folder.
    pub fn organise(
        &self,
        kind: LibraryKind,
        item_id: &str,
        pinned: Option<bool>,
        folder_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut body = json!({ "kind": kind.wire(), "itemId": item_id });
        if let Some(pinned) = pinned {
            body["pinned"] = json!(pinned);
        }
        if let Some(folder_id) = folder_id {
            body["folderId"] = json!(folder_id);
        }
        self.post_empty("/v1/me/library/organise", &body)
    }

    /// The mixes made from what has been played here. Empty until there
    /// is enough history to make them from.
    pub fn mixes(&self) -> Result<Vec<Mix>, ApiError> {
        self.get("/v1/me/mixes")
    }

    /// Every liked song, as the playlist YouTube Music keeps them in.
    pub fn liked(&self) -> Result<Playlist, ApiError> {
        self.get("/v1/me/liked")
    }

    /// The lyrics of a track, timed when a source has them timed. `None`
    /// when no source has any.
    pub fn lyrics(&self, track: &Track) -> Result<Option<Lyrics>, ApiError> {
        let album = track.album.as_ref().map_or("", |album| album.name.as_str());
        // A video's title and length seldom match a lyrics source; said to
        // be one, the core looks for the words of the song it is paired with.
        let video = if track.is_video { "&video=1" } else { "" };
        let path = format!(
            "/v1/tracks/{}/lyrics?title={}&artist={}&album={}&durationMs={}&timed=1{video}",
            encode(&track.id),
            encode(&track.title),
            encode(&track.artist_names()),
            encode(album),
            track.duration_ms,
        );
        match self.get::<Lyrics>(&path) {
            Ok(lyrics) => Ok(Some(lyrics.with_lines())),
            Err(ApiError::Status { code: 404, .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Likes a track, or takes the like back.
    pub fn set_liked(&self, track_id: &str, liked: bool) -> Result<(), ApiError> {
        let rating = if liked { "like" } else { "none" };
        let path = format!("/v1/me/tracks/{}/rating", encode(track_id));
        self.post_empty(&path, &json!({ "rating": rating }))
    }

    pub fn add_to_playlist(&self, playlist_id: &str, track_ids: &[String]) -> Result<(), ApiError> {
        let path = format!("/v1/me/playlists/{}/tracks", encode(playlist_id));
        self.post_empty(&path, &json!({ "trackIds": track_ids }))
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), ApiError> {
        let url = format!("{}/v1/me/playlists/{}", self.origin, encode(playlist_id));
        let response = self.agent.delete(&url).call().map_err(unreachable)?;
        successful_body(response).map(drop)
    }

    /// Removes entries from a playlist. Each is a track id with the id of
    /// its place in the playlist, since a song can be in one twice.
    pub fn remove_from_playlist(
        &self,
        playlist_id: &str,
        items: &[(String, String)],
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/v1/me/playlists/{}/tracks",
            self.origin,
            encode(playlist_id)
        );
        let items: Vec<Value> = items
            .iter()
            .map(|(track_id, item_id)| json!({ "trackId": track_id, "itemId": item_id }))
            .collect();
        let response = self
            .agent
            .delete(&url)
            .force_send_body()
            .send_json(json!({ "items": items }))
            .map_err(unreachable)?;
        successful_body(response).map(drop)
    }

    /// Follows an artist, or stops.
    pub fn set_following(&self, artist_id: &str, follow: bool) -> Result<(), ApiError> {
        let path = format!("/v1/me/artists/{}/follow", encode(artist_id));
        self.post_empty(&path, &json!({ "follow": follow }))
    }

    /// Creates a private playlist and returns its id.
    pub fn create_playlist(&self, title: &str) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let body = json!({ "title": title, "description": "", "public": false });
        let created: Created = self.post("/v1/me/playlists", &body)?;
        Ok(created.id)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let url = format!("{}{path}", self.origin);
        let response = self.agent.get(&url).call().map_err(unreachable)?;
        let body = successful_body(response)?;
        serde_json::from_str(&body).map_err(|error| ApiError::Decode(error.to_string()))
    }

    fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T, ApiError> {
        let body = self.post_text(path, body)?;
        serde_json::from_str(&body).map_err(|error| ApiError::Decode(error.to_string()))
    }

    /// A post whose answer has no body worth reading.
    fn post_empty(&self, path: &str, body: &Value) -> Result<(), ApiError> {
        self.post_text(path, body).map(drop)
    }

    fn post_text(&self, path: &str, body: &Value) -> Result<String, ApiError> {
        let url = format!("{}{path}", self.origin);
        let response = self.agent.post(&url).send_json(body).map_err(unreachable)?;
        successful_body(response)
    }

    /// Opens a response that is read for as long as it lasts.
    fn stream(&self, path: &str) -> Result<impl Read + use<>, ApiError> {
        let url = format!("{}{path}", self.origin);
        let response = self.streaming.get(&url).call().map_err(unreachable)?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            Ok(response.into_body().into_reader())
        } else {
            Err(error_for(status, ""))
        }
    }
}

fn unreachable(error: ureq::Error) -> ApiError {
    ApiError::Unreachable(error.to_string())
}

/// The body of a successful answer, or the error an unsuccessful one names.
fn successful_body(mut response: ureq::http::Response<ureq::Body>) -> Result<String, ApiError> {
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(unreachable)?;
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(error_for(status, &body))
    }
}

fn error_for(status: u16, body: &str) -> ApiError {
    let body: ErrorBody = serde_json::from_str(body).unwrap_or_default();
    if status == 401 || body.reauth {
        ApiError::SignedOut
    } else if status == 429 || body.rate_limited {
        ApiError::RateLimited
    } else {
        ApiError::Status {
            code: status,
            message: body.error,
        }
    }
}

/// Percent-encodes a path segment or query value.
pub(crate) fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_out_answer_is_told_apart() {
        assert_eq!(
            error_for(401, r#"{"error":"signed out"}"#),
            ApiError::SignedOut
        );
        assert_eq!(
            error_for(502, r#"{"error":"x","reauth":true}"#),
            ApiError::SignedOut
        );
    }

    #[test]
    fn a_rate_limit_is_told_apart() {
        assert_eq!(error_for(429, "{}"), ApiError::RateLimited);
    }

    #[test]
    fn other_errors_keep_the_cores_words() {
        let error = error_for(502, r#"{"error":"upstream said no"}"#);
        assert_eq!(error.to_string(), "upstream said no");
        assert_eq!(
            error_for(500, "not json").to_string(),
            "Something went wrong (error 500)."
        );
    }

    #[test]
    fn queries_are_percent_encoded() {
        assert_eq!(encode("daft punk & co"), "daft%20punk%20%26%20co");
        assert_eq!(encode("şarkı"), "%C5%9Fark%C4%B1");
    }
}
