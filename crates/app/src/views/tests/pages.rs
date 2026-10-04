//! What the pages ask for: songs that start a radio, pages read a part at
//! a time, the listening page, the release notes over the page.

use spotified_client::models::{AlbumStat, Stats, TrackStat};

use super::*;
use crate::state::{Dialog, SongOrder, StatSelection, Tail, Whole};

fn song_shelf() -> BrowsePage {
    BrowsePage {
        shelves: vec![Shelf {
            title: "Quick picks".into(),
            items: vec![Item::Track(track("a", "First song"))],
            ..Shelf::default()
        }],
        ..BrowsePage::default()
    }
}

#[test]
fn a_song_card_starts_a_radio_from_the_song() {
    let mut state = state();
    state.home = Loadable::Loaded(song_shelf());
    let mut harness = harness(state);
    harness.get_by_label("First song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartRadio(track) if track.id == "a"
    )));
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::Play { .. }
    )));
}

fn searching() -> State {
    let mut state = state();
    state.search.query = "song".into();
    state.search.results = Loadable::Loaded(SearchResults {
        query: "song".into(),
        shelves: vec![Shelf {
            title: "Songs".into(),
            items: vec![
                Item::Track(track("a", "First song")),
                Item::Track(track("b", "Second song")),
            ],
            ..Shelf::default()
        }],
        ..SearchResults::default()
    });
    state.nav.open(Page::Search);
    state
}

#[test]
fn a_song_found_by_a_search_plays_as_a_radio_not_as_the_list() {
    let mut harness = harness(searching());
    harness.get_by_label("Second song").click();
    harness.step();
    harness.get_by_label("Second song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartRadio(track) if track.id == "b"
    )));
}

#[test]
fn a_search_can_be_narrowed_to_videos_podcasts_and_episodes() {
    let mut harness = harness(searching());
    for (label, filter) in [
        ("Videos", SearchFilter::Videos),
        ("Podcasts", SearchFilter::Podcasts),
        ("Episodes", SearchFilter::Episodes),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert!(asked(&harness, |action| matches!(
            action,
            Action::SetSearchFilter(chosen) if *chosen == filter
        )));
    }
}

fn with_recent_searches() -> State {
    let mut state = state();
    state.nav.open(Page::Search);
    state.search.recent = vec![RecentSearch {
        query: "bonobo".into(),
        token: "t1".into(),
    }];
    state.surfaces.insert(
        Surface::moods().key(),
        Loadable::Loaded(BrowsePage::default()),
    );
    state
}

#[test]
fn a_recent_search_is_removed_by_the_cross_that_shows_under_the_pointer() {
    let mut harness = harness(with_recent_searches());
    let label = "Remove bonobo from recent searches";
    assert!(harness.query_by_label(label).is_none());
    harness.get_by_label("bonobo").hover();
    harness.run();
    harness.get_by_label(label).click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ForgetSearch(query) if query == "bonobo"
    )));
    // The cross is not the row: nothing is searched for.
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::Search(_)
    )));
}

#[test]
fn the_recent_searches_can_be_cleared() {
    let mut harness = harness(with_recent_searches());
    harness.get_by_label("Clear").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ClearSearches
    )));
}

#[test]
fn the_end_of_home_coming_into_view_asks_for_more_of_it() {
    let mut state = state();
    state.home = Loadable::Loaded(song_shelf());
    state.home_more.tail = Tail::after("next".into());
    let harness = harness(state);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MoreHome { retry: false }
    )));
}

#[test]
fn more_of_home_that_failed_is_asked_for_again_only_by_hand() {
    let mut state = state();
    state.home = Loadable::Loaded(song_shelf());
    state.home_more.tail = Tail::after("next".into());
    state
        .home_more
        .tail
        .failed("Couldn't load more of Home.".into());
    let mut harness = harness(state);
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::MoreHome { .. }
    )));
    harness.get_by_label("Try again").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MoreHome { retry: true }
    )));
}

#[test]
fn a_playlist_with_more_to_read_asks_for_it_and_says_how_much_is_here() {
    let mut state = on_playlist();
    state
        .playlist_tails
        .insert("pl".into(), Tail::after("next".into()));
    let mut harness = harness(state);
    // The end of what is here is in sight, so the next page is asked for.
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MorePlaylist { id, retry: false } if id == "pl"
    )));
    harness.get_by_label("3 songs loaded");
    harness.get_by_label("Load more songs").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MorePlaylist { id, retry: true } if id == "pl"
    )));
}

#[test]
fn an_artist_is_played_as_all_their_songs() {
    let mut harness = harness(on_artist());
    harness.get_by_label("Play Bonobo").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayArtist { artist_id, shuffle: false } if artist_id == "ar"
    )));
}

#[test]
fn an_artist_youtube_names_no_shuffle_for_can_still_be_shuffled() {
    let mut state = on_artist();
    if let Some(artist) = state.artists.loaded_mut(&"ar".to_owned()) {
        artist.shuffle_id.clear();
        artist.shuffle_seed.clear();
    }
    let mut harness = harness(state);
    harness.get_by_label("Shuffle Bonobo").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayArtist { artist_id, shuffle: true } if artist_id == "ar"
    )));
}

fn on_songs() -> State {
    let mut state = on_artist();
    let mut songs = crate::state::ArtistSongs::of("ar".into());
    songs.listed = vec![track("a", "Kerala"), track("b", "Cirrus")];
    songs.shown = songs.listed.clone();
    songs.list = Some(Tail::after(String::new()));
    songs.status = "2 songs".into();
    songs.can_open = 3;
    state.artist_songs = Some(songs);
    state.nav.open(Page::ArtistSongs("ar".into()));
    state
}

#[test]
fn the_songs_page_offers_its_orders_and_more_releases() {
    let mut harness = harness(on_songs());
    harness.get_by_label("By album").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetSongOrder(SongOrder::Album)
    )));
    harness.get_by_label("Open 3 more releases").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::OpenMoreReleases
    )));
    harness.get_by_label("Play Bonobo: all songs").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Play { tracks, origin, .. } if tracks.len() == 2 && origin == "Bonobo: all songs"
    )));
}

#[test]
fn a_long_text_about_an_album_is_clipped_until_asked_for_in_full() {
    let mut state = state();
    let album = Album {
        id: "al".into(),
        title: "Discovery".into(),
        tracks: vec![track("a", "First song")],
        description: "Words about the album. ".repeat(30),
        ..Album::default()
    };
    state.albums.insert("al".into(), Loadable::Loaded(album));
    state.nav.open(Page::Album("al".into()));
    let mut harness = harness(state);
    harness.get_by_label("Show all").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleAbout
    )));
}

fn with_stats() -> State {
    let mut state = state();
    let stats = Stats {
        tracks: vec![TrackStat {
            track_id: "a".into(),
            title: "First song".into(),
            artist: "The Artist".into(),
            plays: 4,
            ..TrackStat::default()
        }],
        albums: vec![AlbumStat {
            key: "album-key".into(),
            album: "Discovery".into(),
            artist: "The Artist".into(),
            plays: 9,
            ..AlbumStat::default()
        }],
        ..Stats::default()
    };
    state.stats_page.arrived(&stats);
    state.stats = Loadable::Loaded(stats);
    state.nav.open(Page::Stats);
    state
}

#[test]
fn an_album_in_your_listening_opens_your_figures_for_it() {
    let mut harness = harness(with_stats());
    harness.get_by_label("Your listening: Discovery").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::OpenStat { kind: StatKind::Album, id } if id == "album-key"
    )));
}

#[test]
fn the_top_tracks_can_all_be_played() {
    let mut harness = harness(with_stats());
    harness.get_by_label("Play all").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Play { tracks, index: 0, origin }
            if tracks.len() == 1 && origin == "Your top tracks"
    )));
}

#[test]
fn the_figures_for_one_thing_can_be_put_away() {
    let mut state = with_stats();
    state.stats_page.selected = Some(StatSelection {
        kind: StatKind::Album,
        id: "album-key".into(),
        detail: Loadable::Failed("no".into()),
        tracks: Vec::new(),
    });
    let mut harness = harness(state);
    harness.get_by_label("No plays of this in your history.");
    harness.get_by_label("Close").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseStat
    )));
}

#[test]
fn the_release_notes_over_the_page_lead_to_all_of_them() {
    let mut state = state();
    state.dialog = Some(Dialog::WhatsNew);
    let mut harness = harness(state);
    harness.get_by_label("View all releases").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Changelog)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CloseDialog
    )));
}

#[test]
fn a_toast_that_leads_somewhere_has_a_button_that_goes_there() {
    let mut state = state();
    state.toast_with_link("Updated to 9.9.9", "See what's new", Page::Changelog);
    let mut harness = harness(state);
    harness.get_by_label("See what's new").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Changelog)
    )));
}

#[test]
fn signed_out_the_pages_that_need_no_account_are_still_a_click_away() {
    let mut harness = harness(state());
    harness.get_by_label("Your listening and more").click();
    harness.run();
    harness.get_by_label("Your listening").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Stats)
    )));
}

#[test]
fn a_library_row_plays_from_its_cover_without_opening() {
    let mut harness = harness(with_library());
    assert!(harness.query_by_label("Play Road trip").is_none());
    harness.get_by_label("Road trip").hover();
    harness.run();
    harness.get_by_label("Play Road trip").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayCollection(Page::Playlist(id)) if id == "pl"
    )));
    assert!(!asked(&harness, |action| matches!(action, Action::Open(_))));
}

#[test]
fn the_menu_of_a_playlist_part_read_asks_for_the_whole_of_it() {
    let mut state = on_playlist();
    state
        .playlist_tails
        .insert("pl".into(), Tail::after("next".into()));
    let mut harness = harness(state);
    harness.get_by_label("More options for Road trip").click();
    harness.run();
    harness.get_by_label("Add to queue").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::WholePlaylist { id, then: Whole::Queue { next: false } } if id == "pl"
    )));
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(_)
    )));
}
