//! What `--open` can ask for: a page to start on, a track to play, a room
//! to enter. Mostly for looking at the app without clicking through it.

mod input;
mod room;

use std::path::PathBuf;
use std::time::Duration;

use spotified_client::models::{
    Account, Artwork, LibraryItem, LibraryKind, SearchFilter, StatKind, Track,
};

use spotified_client::session::Command;

use crate::accounts::{self, Accounts, SavedChannel};
use crate::actions::{Action, VideoAsk};
use crate::backend::Response;
use crate::state::{Page, SongOrder, State, Surface};
use crate::together;

/// Whether what `spec` asks for waits until the app has settled: a room
/// is entered with the music that is playing, a library put in by hand
/// must come after the core's own answer, or that would replace it, and
/// what is asked of a page can only be asked once the page is there.
pub(super) fn waits(spec: &str) -> bool {
    let kind = spec.split_once(':').map_or(spec, |(kind, _)| kind);
    spec.starts_with("together-")
        || room::knows(kind)
        || input::knows(kind)
        || matches!(
            kind,
            "wait"
                | "shot"
                | "scroll"
                | "radio"
                | "library-demo"
                | "stat"
                | "stats-lookup"
                | "songs-order"
                | "search-filter"
                | "fullscreen"
                | "speed"
                | "eq"
                | "video"
                | "seek"
                | "playing"
                | "flyout"
                | "report"
                | "accounts-demo"
                | "account-menu"
                | "switch-account"
                | "remove-account"
                | "confirm"
                | "migrate"
                | "migrate-start"
                | "migrate-close"
        )
}

/// One step of what waits: something to do, a pause, or a picture.
pub(super) enum Step {
    Do(Vec<Action>),
    /// `wait:<milliseconds>`: let the room, or the relay, catch up.
    Wait(Duration),
    /// `shot:<file.png>`: a picture of the window as it then is, saved
    /// without closing it.
    Shot(PathBuf),
    /// `flyout`: a click on the tray icon.
    Flyout,
    /// What the pointer or a key does, as the window is told it.
    Input(Vec<eframe::egui::Event>),
}

/// What a step that waited comes to, in the app as it stands when its
/// turn comes.
pub(super) fn step(spec: &str, state: &State) -> Step {
    let (kind, value) = spec.split_once(':').unwrap_or((spec, ""));
    match kind {
        "wait" => Step::Wait(Duration::from_millis(value.parse().unwrap_or(0))),
        "shot" => Step::Shot(PathBuf::from(value)),
        "flyout" => Step::Flyout,
        "accounts-demo" => Step::Do(demo_accounts()),
        _ if input::knows(kind) => Step::Input(input::events(kind, value)),
        _ if room::knows(kind) => Step::Do(room::actions(kind, value, state)),
        _ => Step::Do(opening_action(spec).into_iter().collect()),
    }
}

/// A library to look at where there is no account to have one: a demo is
/// signed out. A few things of each kind, with covers YouTube serves to
/// anyone.
fn demo_library() -> Vec<LibraryItem> {
    const HOST: &str = "https://lh3.googleusercontent.com/";
    let entry = |kind, id: &str, title: &str, subtitle: &str, cover: &str| LibraryItem {
        id: id.to_owned(),
        kind,
        title: title.to_owned(),
        subtitle: subtitle.to_owned(),
        artwork: vec![Artwork {
            url: format!("{HOST}{cover}=w120-h120-l90-rj"),
            width: 120,
            height: 120,
        }],
        ..LibraryItem::default()
    };
    let covers = [
        "NuHosKA4eblVMc1FikgVycO4XdVii6yFQPjqC25_kzFAgusSHWHqcqedYnT1xXqitofx6t4asRCz52z_",
        "kVv3kgz7SnyPdBt5_wV7hA0KgEUwoCp2mp0Xw2LnuyPkwMSMc2LKBfy_UQMfmVWQivvhTKfapIek7Kdo",
        "onR0ZnuFE6PwBeNwMiaTQHtz5vIbEIV8GJwDj8nEmOHuOvUj0efwFdAtXpfCOnpm-4GBl1IMgmtUNw",
        "DfowsipT1GE6GGzFrj4vbg6J6T199WrDgWGSBqQsYwCiaVIze-GN0EfMaXP1pVGnX8bE3CJmisceRW8",
        "qLhu6Py_4_xoBsoubKQsXlhOQGqU9YU1ZRAbFusF0LlrPkXbbpu7bEh-k_ZtE4JwLgubvucAQqcK1hRk",
    ];
    let mut liked = entry(
        LibraryKind::Playlist,
        "LM",
        "Liked Music",
        "Auto playlist",
        covers[0],
    );
    liked.pinned = true;
    vec![
        liked,
        entry(
            LibraryKind::Playlist,
            "x",
            "Daft punk radio",
            "25 songs",
            covers[1],
        ),
        entry(LibraryKind::Album, "x", "Discovery", "Daft Punk", covers[2]),
        entry(
            LibraryKind::Artist,
            "x",
            "Daft Punk",
            "7.17M subscribers",
            covers[4],
        ),
        entry(
            LibraryKind::Album,
            "y",
            "Random Access Memories",
            "Daft Punk",
            covers[3],
        ),
        entry(
            LibraryKind::Playlist,
            "z",
            "Road trip",
            "12 songs",
            covers[2],
        ),
    ]
}

/// Accounts to look at where nobody is signed in: two saved, the one in
/// use with a channel besides its own.
fn demo_accounts() -> Vec<Action> {
    let channel = |id: &str, name: &str, handle: &str| SavedChannel {
        id: id.to_owned(),
        name: name.to_owned(),
        handle: handle.to_owned(),
        ..SavedChannel::default()
    };
    let mut ada = accounts::new_account("Ada Lovelace");
    ada.channels = vec![
        channel("", "Ada Lovelace", "@ada"),
        channel("1", "Analytical Engines", "@engines"),
    ];
    let grace = accounts::new_account("Grace Hopper");
    let mut list = Accounts::default();
    list.add(grace);
    list.add(ada);
    let account = Account {
        name: "Ada Lovelace".to_owned(),
        handle: "@ada".to_owned(),
        ..Account::default()
    };
    vec![
        Action::Loaded(Box::new(Response::Account(Ok(Some(account))))),
        Action::AccountsChanged(Box::new(list)),
    ]
}

/// What `--open` asks of the player before anything else is sent to it:
/// `volume:0` has a track that is about to be played start in silence, for
/// looking at the app without being heard.
pub(super) fn opening_commands(specs: &[String]) -> Vec<Command> {
    specs
        .iter()
        .filter_map(|spec| spec.strip_prefix("volume:")?.parse::<f32>().ok())
        .map(|volume| Command::SetVolume(volume.clamp(0.0, 1.0)))
        .collect()
}

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
        // The lower part of a page: how far down it, in points.
        "scroll" => Action::HoldScroll(value.parse().ok()?),
        // The newest notes, over whatever page is open.
        "whatsnew-dialog" => Action::ShowWhatsNew,
        // Every song of an artist's, and the order they are shown in.
        "artist-songs" => Action::Open(Page::ArtistSongs(value)),
        "songs-order" => Action::SetSongOrder(match value.as_str() {
            "newest" => SongOrder::Newest,
            "album" => SongOrder::Album,
            _ => SongOrder::Popular,
        }),
        // The listener's figures for one thing: `stat:artist:<id>`.
        "stat" => {
            let (kind, id) = value.split_once(':')?;
            Action::OpenStat {
                kind: match kind {
                    "artist" => StatKind::Artist,
                    "album" => StatKind::Album,
                    _ => StatKind::Track,
                },
                id: id.to_owned(),
            }
        }
        "stats-lookup" => Action::SetStatsLookup(value),
        // A search narrowed to one kind, by the kind's name.
        "search-filter" => Action::SetSearchFilter(
            SearchFilter::EVERY
                .into_iter()
                .find(|filter| filter.label().eq_ignore_ascii_case(&value))?,
        ),
        // The sidebar's shapes, and something to show in them.
        // Home read through one of its mood chips, by the chip's params.
        "mood" => Action::ChooseMood(value),
        "sidebar-rail" => Action::ToggleSidebar,
        "library-grid" => Action::ToggleLibraryGrid,
        "library-wide" => Action::ToggleLibraryExpanded,
        "library-demo" => Action::Loaded(Box::new(Response::Library(Ok(demo_library())))),
        "lyrics" => Action::ToggleLyrics,
        "queue" => Action::ToggleQueue,
        // The built-in theme to look at it in.
        "theme" => Action::SetTheme(match value.as_str() {
            "light" => crate::themes::Choice::Light,
            _ => crate::themes::Choice::Dark,
        }),
        "account-menu" => Action::OpenAccountMenu,
        // A saved account by its id: used, or signed out once the
        // question that follows is answered with `confirm`.
        "switch-account" => Action::SwitchAccount(value),
        "remove-account" => Action::AskRemoveAccount(value),
        "confirm" => Action::ConfirmDialog,
        // The move from the Electron app: its choices, going ahead, and
        // putting the dialog away.
        "migrate" => Action::OpenMigration,
        "migrate-start" => Action::StartMigration,
        "migrate-close" => Action::CloseDialog,
        // A problem report, saved as the button in Settings saves one.
        "report" => Action::SaveReport,
        "lyrics-full" => Action::SetLyricsFullscreen(true),
        // What is playing, given the screen; it needs something playing.
        "fullscreen" => Action::SetFullscreenPlayer(true),
        "speed" => Action::SetSpeed(value.parse().ok()?),
        // The equalizer: `eq:off`, `eq:on`, or a built-in curve by name.
        "eq" => Action::Equalizer(match value.as_str() {
            "off" => crate::equalizer::Ask::On(false),
            "on" => crate::equalizer::Ask::On(true),
            name => crate::equalizer::PRESETS
                .into_iter()
                .find(|(preset, _)| preset.eq_ignore_ascii_case(name))
                .map(|(_, gains)| crate::equalizer::Ask::Curve(gains))?,
        }),
        // `video` shows the music video of what plays; `video:off` goes
        // back to the song.
        "video" => Action::Video(VideoAsk::Set(value != "off")),
        "seek" => Action::Seek(value.parse().ok()?),
        "playing" => Action::SetPlaying(value != "off"),
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
        // One song with a title and artists of its own, to see how they
        // are written: `song:<id>|<title>|<artist>,<artist>`.
        "song" => {
            let mut parts = value.split('|');
            let id = parts.next()?.to_owned();
            let title = parts.next().unwrap_or(&id).to_owned();
            let artists = parts.next().unwrap_or_default().split(',');
            Action::Play {
                tracks: vec![Track {
                    title,
                    id,
                    playable: true,
                    artists: artists
                        .filter(|name| !name.is_empty())
                        .enumerate()
                        .map(|(index, name)| spotified_client::models::ArtistRef {
                            id: format!("UC{index}"),
                            name: name.to_owned(),
                        })
                        .collect(),
                    ..Track::default()
                }],
                index: 0,
                origin: String::new(),
            }
        }
        // A song by its id, then songs like it; in a room, the room's.
        "radio" => Action::StartRadio(Track {
            title: value.clone(),
            id: value,
            playable: true,
            ..Track::default()
        }),
        "jump" => Action::JumpTo(value.parse().ok()?),
        _ => return None,
    })
}
