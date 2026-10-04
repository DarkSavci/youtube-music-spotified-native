//! The recent searches, the listening page, and the page of all an
//! artist's songs.

use spotified_client::models::{
    Album, AlbumRef, Artist, ArtistRef, BrowseLink, LookupResults, Playlist, PlaylistPage,
    SearchHistoryEntry, StatDetail, StatKind, TrackStat,
};

use super::*;
use crate::state::SongOrder;
use crate::state::artist_songs::RELEASE_BATCH;

fn loaded(state: &mut State, response: Response) -> Vec<Effect> {
    apply(state, Action::Loaded(Box::new(response)))
}

fn entry(query: &str, token: &str) -> SearchHistoryEntry {
    SearchHistoryEntry {
        query: query.into(),
        token: token.into(),
    }
}

fn recent(state: &State) -> Vec<&str> {
    state
        .search
        .recent
        .iter()
        .map(|row| row.query.as_str())
        .collect()
}

#[test]
fn a_search_that_is_sent_is_remembered_here() {
    let mut state = ready();
    apply(&mut state, Action::SetSearchQuery("bonobo".into()));
    let effects = apply(&mut state, Action::RunSearch);
    assert!(effects.contains(&Effect::SaveSettings));
    assert_eq!(state.settings.recent_searches, ["bonobo"]);
    assert_eq!(recent(&state), ["bonobo"]);
    // Narrowing it is not another search to remember.
    let effects = apply(&mut state, Action::SetSearchFilter(SearchFilter::Videos));
    assert!(!effects.contains(&Effect::SaveSettings));
}

#[test]
fn the_accounts_searches_and_this_computers_are_shown_as_one_list() {
    let mut state = ready();
    state.settings.recent_searches = vec!["air".into()];
    let history = vec![entry("bonobo", "t1")];
    loaded(&mut state, Response::SearchHistory(Ok(history)));
    assert_eq!(recent(&state), ["air", "bonobo"]);
}

#[test]
fn removing_a_search_the_account_holds_removes_it_there_too() {
    let mut state = ready();
    state.settings.recent_searches = vec!["Bonobo".into(), "air".into()];
    let history = vec![entry("bonobo", "t1")];
    loaded(&mut state, Response::SearchHistory(Ok(history)));
    let effects = apply(&mut state, Action::ForgetSearch("bonobo".into()));
    assert_eq!(
        effects,
        [
            Effect::SaveSettings,
            Effect::Fetch(Request::ForgetSearches {
                tokens: vec!["t1".into()],
                all: false,
            })
        ]
    );
    assert_eq!(recent(&state), ["air"]);
    // One only this computer knows is forgotten here and nowhere else.
    assert_eq!(
        apply(&mut state, Action::ForgetSearch("air".into())),
        [Effect::SaveSettings]
    );
    assert!(state.search.recent.is_empty());
}

#[test]
fn clearing_the_searches_clears_the_accounts_as_well() {
    let mut state = ready();
    state.settings.recent_searches = vec!["air".into()];
    let history = vec![entry("bonobo", "t1"), entry("moby", "t2")];
    loaded(&mut state, Response::SearchHistory(Ok(history)));
    let effects = apply(&mut state, Action::ClearSearches);
    assert_eq!(
        effects,
        [
            Effect::SaveSettings,
            Effect::Fetch(Request::ForgetSearches {
                tokens: vec!["t1".into(), "t2".into()],
                all: true,
            })
        ]
    );
    assert!(state.search.recent.is_empty());
    assert!(state.settings.recent_searches.is_empty());
}

#[test]
fn a_search_the_account_would_not_forget_is_shown_again() {
    let mut state = ready();
    let refused = Response::SearchesForgotten {
        all: false,
        result: Err(ApiError::RateLimited),
    };
    assert_eq!(
        loaded(&mut state, refused),
        [Effect::Fetch(Request::SearchHistory)]
    );
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
fn the_lookup_asks_once_the_typing_pauses_and_keeps_only_the_answer_to_what_is_typed() {
    let mut state = ready();
    // Each keystroke only waits; the pause is what asks.
    assert_eq!(
        apply(&mut state, Action::SetStatsLookup(" d".into())),
        [Effect::DebounceStatsLookup]
    );
    assert_eq!(
        apply(&mut state, Action::SetStatsLookup(" da".into())),
        [Effect::DebounceStatsLookup]
    );
    assert_eq!(
        apply(&mut state, Action::RunStatsLookup),
        [Effect::Fetch(Request::StatsLookup("da".into()))]
    );
    apply(&mut state, Action::SetStatsLookup("daft".into()));
    let late = Response::StatsLookup("da".into(), Ok(LookupResults::default()));
    loaded(&mut state, late);
    assert_eq!(state.stats_page.lookup, None);
    let current = Response::StatsLookup("daft".into(), Ok(LookupResults::default()));
    loaded(&mut state, current);
    assert!(state.stats_page.lookup.is_some());
    // A pause on text already answered asks nothing more.
    assert!(apply(&mut state, Action::RunStatsLookup).is_empty());
    // Emptied, it asks nothing and shows nothing.
    assert!(apply(&mut state, Action::SetStatsLookup(String::new())).is_empty());
    assert_eq!(state.stats_page.lookup, None);
    assert!(apply(&mut state, Action::RunStatsLookup).is_empty());
}

#[test]
fn choosing_a_match_opens_your_figures_for_it_and_puts_the_matches_away() {
    let mut state = ready();
    apply(&mut state, Action::SetStatsLookup("daft".into()));
    let open = Action::OpenStat {
        kind: StatKind::Artist,
        id: "UC1".into(),
    };
    assert_eq!(
        apply(&mut state, open),
        [Effect::Fetch(Request::StatDetail(
            StatKind::Artist,
            "UC1".into()
        ))]
    );
    assert!(!state.stats_page.lookup_open);
    let detail = StatDetail {
        plays: 3,
        top_tracks: vec![TrackStat {
            track_id: "a".into(),
            plays: 3,
            ..TrackStat::default()
        }],
        ..StatDetail::default()
    };
    let answer = Response::StatDetail(StatKind::Artist, "UC1".into(), Ok(detail));
    loaded(&mut state, answer);
    let selected = state.stats_page.selected.as_ref().expect("a selection");
    // Its songs are ready to list and play: no length, so plays are shown.
    assert_eq!(selected.tracks.len(), 1);
    assert_eq!(selected.tracks[0].play_count, "3");
    apply(&mut state, Action::CloseStat);
    assert!(state.stats_page.selected.is_none());
}

#[test]
fn artists_pictures_are_fetched_a_couple_at_a_time() {
    let mut state = ready();
    let want = |id: &str| Action::WantArtistPhoto(id.into());
    assert_eq!(apply(&mut state, want("UC1")).len(), 1);
    assert_eq!(apply(&mut state, want("UC2")).len(), 1);
    assert!(apply(&mut state, want("UC3")).is_empty());
    assert!(apply(&mut state, want("UC1")).is_empty());
    loaded(&mut state, Response::ArtistPhoto("UC1".into(), Vec::new()));
    assert_eq!(apply(&mut state, want("UC3")).len(), 1);
}

fn release(id: &str, title: &str, year: &str) -> Album {
    Album {
        id: id.into(),
        title: title.into(),
        year: year.into(),
        ..Album::default()
    }
}

fn artist() -> Artist {
    Artist {
        id: "ar".into(),
        name: "Bonobo".into(),
        songs_id: "OLAK".into(),
        albums: vec![release("al1", "Migration", "2017")],
        albums_more: Some(BrowseLink {
            id: "MPAD".into(),
            params: "p".into(),
        }),
        ..Artist::default()
    }
}

fn song(id: &str, title: &str, album: &str) -> Track {
    Track {
        title: title.into(),
        artists: vec![ArtistRef {
            id: "ar".into(),
            name: "Bonobo".into(),
        }],
        album: Some(AlbumRef {
            id: album.into(),
            name: album.into(),
        }),
        ..track(id)
    }
}

fn songs_page(tracks: Vec<Track>, next: &str) -> PlaylistPage {
    PlaylistPage {
        playlist: Playlist {
            tracks,
            ..Playlist::default()
        },
        next: next.into(),
    }
}

/// The songs page of an artist whose page is here, with its list read.
fn on_songs() -> State {
    let mut state = ready();
    apply(&mut state, Action::Open(Page::ArtistSongs("ar".into())));
    loaded(
        &mut state,
        Response::Artist("ar".into(), Ok(Box::new(artist()))),
    );
    let page = Response::SongsPage {
        artist_id: "ar".into(),
        token: String::new(),
        result: Ok(songs_page(vec![song("a", "Kerala", "al1")], "")),
    };
    loaded(&mut state, page);
    state
}

fn songs(state: &State) -> &crate::state::ArtistSongs {
    state.artist_songs.as_ref().expect("a songs page")
}

#[test]
fn the_songs_page_reads_the_artist_then_their_list_to_its_end() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::Open(Page::ArtistSongs("ar".into()))),
        [Effect::Fetch(Request::Artist("ar".into()))]
    );
    let page = |token: &str| {
        Effect::Fetch(Request::SongsPage {
            artist_id: "ar".into(),
            songs_id: "OLAK".into(),
            token: token.into(),
        })
    };
    let arrived = Response::Artist("ar".into(), Ok(Box::new(artist())));
    assert_eq!(loaded(&mut state, arrived), [page("")]);
    // Ordering needs the whole list, so the next page is asked for at once.
    let first = Response::SongsPage {
        artist_id: "ar".into(),
        token: String::new(),
        result: Ok(songs_page(vec![song("a", "Kerala", "al1")], "t1")),
    };
    assert_eq!(loaded(&mut state, first), [page("t1")]);
    assert!(!songs(&state).complete());
    let last = Response::SongsPage {
        artist_id: "ar".into(),
        token: "t1".into(),
        result: Ok(songs_page(vec![song("b", "Cirrus", "al2")], "")),
    };
    assert!(loaded(&mut state, last).is_empty());
    assert!(songs(&state).complete());
    assert_eq!(songs(&state).shown.len(), 2);
    assert!(songs(&state).status.starts_with("2 songs"));
}

#[test]
fn a_dated_order_reads_the_discography_then_opens_releases_two_at_a_time() {
    let mut state = on_songs();
    assert_eq!(
        apply(&mut state, Action::SetSongOrder(SongOrder::Newest)),
        [Effect::Fetch(Request::Discography {
            artist_id: "ar".into(),
            albums: Some(("MPAD".into(), "p".into())),
            singles: None,
        })]
    );
    let discography = Response::Discography {
        artist_id: "ar".into(),
        albums: vec![
            release("al1", "Migration", "2017"),
            release("al2", "Black Sands", "2010"),
            release("al3", "Fragments", "2022"),
        ],
        singles: Vec::new(),
    };
    let open = |id: &str| {
        Effect::Fetch(Request::Release {
            artist_id: "ar".into(),
            album_id: id.into(),
        })
    };
    assert_eq!(loaded(&mut state, discography), [open("al1"), open("al2")]);
    assert!(songs(&state).status.contains("opening releases 0 of 3"));
    let mut opened = release("al1", "Migration", "2017");
    opened.artists = vec![ArtistRef {
        id: "ar".into(),
        name: "Bonobo".into(),
    }];
    opened.tracks = vec![song("a2", "Kerala", ""), song("c", "Outlier", "")];
    let answer = Response::Release {
        artist_id: "ar".into(),
        album_id: "al1".into(),
        result: Ok(opened),
    };
    // One has opened, so the next may.
    assert_eq!(loaded(&mut state, answer), [open("al3")]);
    // Its new song joins the list; the one already there is not repeated.
    let ids: Vec<&str> = songs(&state).shown.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["a", "c"]);
}

#[test]
fn a_refusal_stops_the_releases_until_more_are_asked_for() {
    let mut state = on_songs();
    apply(&mut state, Action::SetSongOrder(SongOrder::Album));
    let discography = Response::Discography {
        artist_id: "ar".into(),
        albums: vec![
            release("al1", "Migration", "2017"),
            release("al2", "Black Sands", "2010"),
            release("al3", "Fragments", "2022"),
        ],
        singles: Vec::new(),
    };
    loaded(&mut state, discography);
    for id in ["al1", "al2"] {
        let refused = Response::Release {
            artist_id: "ar".into(),
            album_id: id.into(),
            result: Err(ApiError::RateLimited),
        };
        assert!(loaded(&mut state, refused).is_empty());
    }
    assert!(songs(&state).limited);
    assert!(
        songs(&state)
            .status
            .contains("YouTube is limiting requests")
    );
    assert_eq!(songs(&state).can_open, 3);
    // Asked again, the failures are tried first.
    let effects = apply(&mut state, Action::OpenMoreReleases);
    assert_eq!(effects.len(), 2);
    assert_eq!(songs(&state).limit, RELEASE_BATCH);
    assert!(!songs(&state).limited);
}

#[test]
fn the_songs_are_grouped_by_album_under_headings() {
    let mut state = on_songs();
    apply(&mut state, Action::SetSongOrder(SongOrder::Album));
    let discography = Response::Discography {
        artist_id: "ar".into(),
        albums: Vec::new(),
        singles: Vec::new(),
    };
    loaded(&mut state, discography);
    let groups = &songs(&state).groups;
    assert_eq!(groups.len(), 1);
    // Named as the artist's own page names the album.
    assert_eq!(groups[0].title, "Migration");
    assert_eq!(groups[0].detail, "2017 · 1 song");
    assert_eq!(groups[0].songs, 0..1);
}

#[test]
fn nothing_more_is_asked_for_once_the_songs_page_is_left() {
    let mut state = on_songs();
    apply(&mut state, Action::SetSongOrder(SongOrder::Newest));
    apply(&mut state, Action::Open(Page::Home));
    let discography = Response::Discography {
        artist_id: "ar".into(),
        albums: vec![release("al2", "Black Sands", "2010")],
        singles: Vec::new(),
    };
    assert!(loaded(&mut state, discography).is_empty());
    // Coming back carries on from what was read.
    let effects = apply(&mut state, Action::Back);
    assert!(effects.contains(&Effect::Fetch(Request::Release {
        artist_id: "ar".into(),
        album_id: "al1".into(),
    })));
}
