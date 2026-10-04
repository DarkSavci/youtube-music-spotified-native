//! How the core's JSON is read.

use super::*;

#[test]
fn the_line_being_sung_is_the_last_that_has_started() {
    let lyrics = Lyrics {
        synced: true,
        lines: [1000, 4000, 9000]
            .into_iter()
            .map(|at_ms| LyricLine {
                at_ms,
                text: String::new(),
            })
            .collect(),
        ..Lyrics::default()
    };
    assert_eq!(lyrics.line_at(500), None);
    assert_eq!(lyrics.line_at(1000), Some(0));
    assert_eq!(lyrics.line_at(8999), Some(1));
    assert_eq!(lyrics.line_at(60_000), Some(2));
}

#[test]
fn untimed_lyrics_have_no_line_being_sung_and_are_cut_into_lines() {
    let lyrics = Lyrics {
        plain: "one\ntwo".into(),
        ..Lyrics::default()
    }
    .with_lines();
    assert_eq!(lyrics.lines.len(), 2);
    assert_eq!(lyrics.line_at(5000), None);
}

#[test]
fn a_nil_slice_reads_as_empty() {
    let track: Track =
        serde_json::from_str(r#"{"id":"a","title":"T","artists":null,"artwork":null}"#)
            .expect("a track with null lists");
    assert!(track.artists.is_empty());
    assert!(track.artwork.is_empty());
}

#[test]
fn a_shelf_keeps_the_items_it_understands() {
    let shelf: Shelf = serde_json::from_str(
        r#"{"title":"Mixed","items":[
            {"kind":"album","album":{"id":"al","title":"A"}},
            {"kind":"episode","episode":{"id":"e","publishedText":"3 days ago"}},
            {"kind":"audiobook","audiobook":{"id":"b"}},
            {"kind":"track"},
            {"kind":"artist","artist":{"id":"ar","name":"N"}}
        ]}"#,
    )
    .expect("a shelf");
    let kinds: Vec<&str> = shelf
        .items
        .iter()
        .map(|item| match item {
            Item::Track(_) => "track",
            Item::Album(_) => "album",
            Item::Artist(_) => "artist",
            Item::Playlist(_) => "playlist",
            Item::Podcast(_) => "podcast",
            Item::Episode(_) => "episode",
        })
        .collect();
    assert_eq!(kinds, ["album", "episode", "artist"]);
}

#[test]
fn a_page_reads_its_mood_tiles_and_where_a_shelf_continues() {
    let page: BrowsePage = serde_json::from_str(
        r##"{"shelves":[{"title":"New","items":null,"showAllId":"FEmusic_new_releases_albums","showAllParams":"p"}],
            "moods":[{"id":"FEmusic_moods_and_genres_category","params":"ggM","title":"Chill","color":"#3366AA"}]}"##,
    )
    .expect("a page");
    assert_eq!(page.shelves[0].show_all_id, "FEmusic_new_releases_albums");
    assert_eq!(page.shelves[0].show_all_params, "p");
    assert_eq!(page.moods[0].title, "Chill");
}

#[test]
fn home_reads_its_row_of_moods() {
    let page: BrowsePage = serde_json::from_str(
        r#"{"shelves":null,"chips":[{"title":"Energize","params":"ggM"},{"title":"Relax","params":"ggN","selected":true}]}"#,
    )
    .expect("a page");
    assert_eq!(page.chips.len(), 2);
    assert_eq!(page.chips[0].params, "ggM");
    assert!(page.chips[1].selected);
    // A page without the row has none, not an error.
    let plain: BrowsePage = serde_json::from_str(r#"{"shelves":[]}"#).expect("a page");
    assert!(plain.chips.is_empty());
}

#[test]
fn a_search_reads_what_is_inside_its_top_result() {
    let results: SearchResults = serde_json::from_str(
        r#"{"query":"daft","topResult":{"kind":"artist","artist":{"id":"UC1","name":"Daft Punk"}},
            "topResultItems":[{"kind":"track","track":{"id":"t1","title":"Get Lucky"}}],"shelves":null}"#,
    )
    .expect("results");
    assert!(matches!(results.top_result, Some(Item::Artist(_))));
    assert!(matches!(&results.top_result_items[..], [Item::Track(track)] if track.id == "t1"));
}

#[test]
fn a_library_item_reads_when_it_was_added_and_last_played() {
    let item: LibraryItem = serde_json::from_str(
        r#"{"id":"a","kind":"album","title":"Discovery","addedAt":"2026-06-01T00:00:00Z","lastPlayedAt":"2026-10-04T09:00:00.5Z"}"#,
    )
    .expect("an item");
    assert_eq!(item.added_at.as_deref(), Some("2026-06-01T00:00:00Z"));
    assert_eq!(
        item.last_played_at.as_deref(),
        Some("2026-10-04T09:00:00.5Z")
    );
    let bare: LibraryItem = serde_json::from_str(r#"{"id":"a"}"#).expect("an item");
    assert_eq!(bare.added_at, None);
}

#[test]
fn a_resized_picture_keeps_how_it_was_cut() {
    let set = [Artwork {
        url: "https://lh3.googleusercontent.com/abc=w120-h120-p-l90-rj".into(),
        width: 120,
        height: 120,
    }];
    assert_eq!(
        artwork_url(&set, 256).as_deref(),
        Some("https://lh3.googleusercontent.com/abc=w256-h256-p-l90-rj")
    );
}

#[test]
fn artwork_is_asked_for_at_the_size_drawn() {
    let set = [Artwork {
        url: "https://lh3.googleusercontent.com/abc=w60-h60-l90-rj".into(),
        width: 60,
        height: 60,
    }];
    assert_eq!(
        artwork_url(&set, 148).as_deref(),
        Some("https://lh3.googleusercontent.com/abc=w148-h148-l90-rj")
    );
}

#[test]
fn an_address_without_a_size_is_left_alone() {
    let set = [Artwork {
        url: "https://i.ytimg.com/vi/abc/hq720.jpg".into(),
        width: 720,
        height: 405,
    }];
    assert_eq!(artwork_url(&set, 148).as_deref(), Some(set[0].url.as_str()));
    assert_eq!(artwork_url(&[], 148), None);
}

#[test]
fn a_page_of_a_playlist_names_the_token_for_the_next() {
    let page: PlaylistPage = serde_json::from_str(
        r#"{"playlist":{"id":"pl","title":"Road trip","tracks":[{"id":"a"}]},"next":"t1"}"#,
    )
    .expect("a page with more behind it");
    assert_eq!(page.next, "t1");
    assert_eq!(page.playlist.tracks.len(), 1);
    // The last page says nothing of a next.
    let last: PlaylistPage =
        serde_json::from_str(r#"{"playlist":{"id":"pl","tracks":null}}"#).expect("a last page");
    assert!(last.next.is_empty());
}

#[test]
fn a_search_can_be_narrowed_to_every_kind_the_core_knows() {
    let wires: Vec<&str> = SearchFilter::EVERY
        .into_iter()
        .map(SearchFilter::wire)
        .collect();
    assert_eq!(
        wires,
        [
            "",
            "songs",
            "albums",
            "artists",
            "playlists",
            "videos",
            "podcasts",
            "episodes"
        ]
    );
}

#[test]
fn the_figures_for_one_artist_are_read_with_their_months_and_songs() {
    let detail: StatDetail = serde_json::from_str(
        r#"{"kind":"artist","id":"UCRr1xG_2WIDs18a6cIiCxeA","name":"Daft Punk","plays":12,
            "plays30d":4,"totalMs":3600000,"distinctTracks":3,"rank":1,
            "firstPlayedAt":"2026-01-02T10:00:00Z",
            "months":[{"month":"2026-09","plays":4,"totalMs":900000}],
            "topTracks":[{"trackId":"a","title":"One More Time","artist":"Daft Punk",
                          "artistId":"UCRr1xG_2WIDs18a6cIiCxeA","plays":7,"totalMs":1}]}"#,
    )
    .expect("an artist's figures");
    assert_eq!(detail.kind, StatKind::Artist);
    assert_eq!(detail.plays30d, 4);
    assert_eq!(detail.months[0].month, "2026-09");
    assert_eq!(detail.last_played_at, None);
    // A row of the play log, as a track to list and play: its plays stand
    // where a length would, and its artist leads to their page.
    let track = detail.top_tracks[0].track();
    assert_eq!(track.play_count, "7");
    assert_eq!(track.artists[0].id, "UCRr1xG_2WIDs18a6cIiCxeA");
    assert!(track.playable);
}

#[test]
fn a_name_standing_in_for_a_channel_is_told_from_a_channel() {
    assert_eq!(
        channel_id("UCRr1xG_2WIDs18a6cIiCxeA"),
        Some("UCRr1xG_2WIDs18a6cIiCxeA")
    );
    assert_eq!(channel_id("Daft Punk"), None);
    assert_eq!(channel_id("UC short"), None);
    // Such a row's artist has no page to lead to.
    let stat = TrackStat {
        artist_id: "Daft Punk".into(),
        ..TrackStat::default()
    };
    assert!(stat.track().artists[0].id.is_empty());
}

#[test]
fn your_history_with_an_artist_is_read_by_the_cores_names() {
    let affinity: Affinity = serde_json::from_str(
        r#"{"artistId":"x","plays30d":3,"playsAllTime":40,"rankAmongYourArtists":2,"totalMs":9}"#,
    )
    .expect("an affinity");
    assert_eq!(affinity.plays30d, 3);
    assert_eq!(affinity.plays_all_time, 40);
    assert_eq!(affinity.rank_among_your_artists, 2);
}

#[test]
fn the_songs_of_a_search_are_each_given_once_and_no_more_than_asked() {
    let song = |id: &str| {
        Item::Track(Track {
            id: id.into(),
            ..Track::default()
        })
    };
    let results = SearchResults {
        shelves: vec![
            Shelf {
                items: vec![song("a"), song("b"), Item::Album(Album::default())],
                ..Shelf::default()
            },
            Shelf {
                items: vec![song("a"), song("c"), song("d")],
                ..Shelf::default()
            },
        ],
        ..SearchResults::default()
    };
    let ids: Vec<String> = results.songs(3).into_iter().map(|track| track.id).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}
