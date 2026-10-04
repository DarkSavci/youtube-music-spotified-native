//! What `--open` can ask for: a page to start on, a track to play, a room
//! to enter. Mostly for looking at the app without clicking through it.

use spotified_client::models::Track;

use crate::actions::Action;
use crate::state::{Page, Surface};
use crate::together;

/// The action that opens what `--open` named, if it names anything known.
pub(super) fn opening_action(spec: &str) -> Option<Action> {
    // A page with nothing to name, like `settings`, has no colon.
    let (kind, value) = spec.split_once(':').unwrap_or((spec, ""));
    let value = value.to_owned();
    Some(match kind {
        "album" => Action::Open(Page::Album(value)),
        "artist" => Action::Open(Page::Artist(value)),
        "playlist" => Action::Open(Page::Playlist(value)),
        "podcast" => Action::Open(Page::Podcast(value)),
        "settings" => Action::Open(Page::Settings),
        "stats" => Action::Open(Page::Stats),
        "mini" => Action::ToggleMiniPlayer,
        "together" => Action::Open(Page::Together),
        // A room on the server in the settings: made, or joined by its PIN.
        "together-create" => Action::TogetherCreate,
        "together-join" => Action::TogetherField(together::Field::Pin, value),
        "together-enter" => Action::TogetherJoin,
        "whatsnew" => Action::Open(Page::Changelog),
        "lyrics" => Action::ToggleLyrics,
        "lyrics-full" => Action::SetLyricsFullscreen(true),
        "history" => Action::Open(Page::History),
        "explore" => Action::Open(Page::Browse(Surface::explore())),
        "moods" => Action::Open(Page::Browse(Surface::moods())),
        "search" if value.is_empty() => Action::Open(Page::Search),
        "search" => Action::SetSearchQuery(value),
        // One id, or several with commas between to queue the rest. Known
        // only by their ids until the core says more.
        "track" => Action::Play {
            tracks: value
                .split(',')
                .map(|id| Track {
                    title: id.to_owned(),
                    id: id.to_owned(),
                    playable: true,
                    ..Track::default()
                })
                .collect(),
            index: 0,
            origin: String::new(),
        },
        "jump" => Action::JumpTo(value.parse().ok()?),
        _ => return None,
    })
}
