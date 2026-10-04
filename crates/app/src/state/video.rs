//! Whether what plays is shown as its music video, and what is known of
//! the song's other edit.
//!
//! YouTube keeps a song and its music video as two tracks. Watching the
//! video is switching the queue's entry to the video's track; going back
//! is switching it to the song's. The picture itself is the app's to
//! fetch: here is only what the views need to draw it and to say why not.

use std::cell::Cell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use eframe::egui::{TextureId, Vec2};
use spotified_client::models::Track;

/// How long a switch to the other edit may go unanswered before the
/// button is given back.
const SWITCH_PATIENCE: Duration = Duration::from_secs(8);
/// How many songs' pairs are remembered.
const PAIRS_KEPT: usize = 100;

/// What is known of whether the song playing has a music video.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Availability {
    /// Not asked: most listening never needs to know, so YouTube is asked
    /// only once the pointer reaches the button, or the video is on.
    #[default]
    Unknown,
    Checking,
    Available,
    Unavailable,
    /// The asking failed; pressing the button asks again.
    Error,
}

/// A press of the video button that waits for the song's pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanting {
    pub track_id: String,
    pub enabled: bool,
}

/// A switch that has been sent and not yet seen to happen.
#[derive(Debug, Clone, PartialEq)]
pub struct Switching {
    /// The track the queue's entry is to become.
    pub to: String,
    pub enabled: bool,
    pub since: Instant,
}

#[derive(Default)]
pub struct Video {
    /// The video is shown rather than the cover.
    pub enabled: bool,
    /// A switch between the song and its video is under way.
    pub busy: bool,
    /// The picture is on its way: found, opened, or caught up after a jump.
    pub loading: bool,
    pub error: Option<String>,
    /// Counts fresh starts of the picture, so a retry fetches it again.
    pub revision: u64,
    /// The track `availability` is about.
    pub about: String,
    pub availability: Availability,
    pub wanting: Option<Wanting>,
    pub switching: Option<Switching>,
    /// The song and video of each track asked about, under both their ids.
    pairs: HashMap<String, Vec<Track>>,
    /// The picture to draw and its size, when there is one.
    pub picture: Option<(TextureId, Vec2)>,
    /// How tall, in the screen's pixels, the tallest surface drawn this
    /// frame was. The views say; the app reads it and clears it.
    pub watched: Cell<Option<u32>>,
    /// The track last seen playing, to tell when another takes its place.
    pub playing: Option<String>,
    /// A Listen Together room is being followed.
    pub in_room: bool,
    /// Whether the video was on before the room was joined, to put it back
    /// so on leaving.
    pub before_room: bool,
    /// The last of the room's display changes that has been acted on.
    pub room_seen: u64,
}

impl Video {
    /// Whether `track` is the video edit. Its own flag can be missing, in
    /// a queue an older version saved or a list that does not report it,
    /// and then the pair YouTube gave for it settles it.
    pub fn is_video(&self, track: &Track) -> bool {
        track.is_video
            || self
                .pairs
                .get(&track.id)
                .is_some_and(|pair| pair.iter().any(|edit| edit.id == track.id && edit.is_video))
    }

    pub fn pair(&self, track_id: &str) -> Option<&[Track]> {
        self.pairs.get(track_id).map(Vec::as_slice)
    }

    /// Remembers a pair under the track asked about and under each edit.
    pub fn remember(&mut self, track_id: &str, pair: Vec<Track>) {
        if self.pairs.len() > PAIRS_KEPT {
            self.pairs.clear();
        }
        for edit in &pair {
            self.pairs.insert(edit.id.clone(), pair.clone());
        }
        self.pairs.insert(track_id.to_owned(), pair);
    }

    /// What picture is wanted: the track's id and the count of fresh
    /// starts, or `None` when the cover is what shows.
    pub fn key(&self, track: Option<&Track>) -> Option<(String, u64)> {
        let track = track?;
        (self.enabled && !self.busy && self.is_video(track))
            .then(|| (track.id.clone(), self.revision))
    }

    /// Says that a surface `height` pixels tall was drawn this frame.
    pub fn watch(&self, height: u32) {
        let tallest = self.watched.get().map_or(height, |seen| seen.max(height));
        self.watched.set(Some(tallest));
    }

    /// Whether a switch has gone unanswered for too long.
    pub fn switch_overdue(&self, now: Instant) -> bool {
        self.switching
            .as_ref()
            .is_some_and(|switching| now.duration_since(switching.since) > SWITCH_PATIENCE)
    }
}

/// What the video button says and whether it answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    pub label: &'static str,
    /// Pressing does nothing: drawn dim.
    pub blocked: bool,
}

/// How the video button stands. `controls_room` is whether, in a Listen
/// Together room, this listener may steer it.
pub fn control(
    video: &Video,
    track: Option<&Track>,
    following: bool,
    controls_room: bool,
) -> Control {
    if video.enabled {
        // In a room the video only hides for this listener; alone, it
        // switches back to the song.
        let label = if following {
            "Hide music video"
        } else {
            "Switch to song"
        };
        return Control {
            label,
            blocked: video.busy,
        };
    }
    let Some(track) = track else {
        return Control {
            label: "Play a song to watch its video.",
            blocked: true,
        };
    };
    let is_video = video.is_video(track);
    let status = if is_video {
        Availability::Available
    } else if video.about == track.id {
        video.availability
    } else {
        Availability::Checking
    };
    let led = following && !controls_room && !is_video;
    let label = if video.busy {
        "Switching playback format…"
    } else if led {
        "The room leader chooses the media version."
    } else {
        match status {
            Availability::Checking => "Checking for a music video…",
            Availability::Unavailable => "No matching music video is available for this song.",
            Availability::Error => "Could not check video availability. Click to retry.",
            // Not asked yet: pressing the button finds out.
            Availability::Unknown | Availability::Available => "Watch music video",
        }
    };
    // Still checking is no reason to refuse a press: it waits for the
    // answer and then switches.
    Control {
        label,
        blocked: video.busy || led || status == Availability::Unavailable,
    }
}
