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
