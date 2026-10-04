//! Pages of shelves and tiles: home, explore, search, an artist.

use super::*;

#[test]
fn a_card_opens_its_page() {
    let mut state = state();
    state.home = Loadable::Loaded(BrowsePage {
        shelves: vec![Shelf {
            title: "New releases".into(),
            items: vec![Item::Album(Album {
                id: "album-1".into(),
                title: "Discovery".into(),
                ..Album::default()
            })],
            ..Shelf::default()
        }],
        ..BrowsePage::default()
    });
    let mut harness = harness(state);
    harness.get_by_label("Discovery").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Album(id)) if id == "album-1"
    )));
}

#[test]
fn the_play_button_on_a_card_plays_it_without_opening_it() {
    let mut state = state();
    state.home = Loadable::Loaded(BrowsePage {
        shelves: vec![Shelf {
            title: "New releases".into(),
            items: vec![Item::Album(Album {
                id: "album-1".into(),
                title: "Discovery".into(),
                ..Album::default()
            })],
            ..Shelf::default()
        }],
        ..BrowsePage::default()
    });
    let mut harness = harness(state);
    // The button is only there while the pointer is on the card.
    let card = harness.get_by_label("Discovery").rect();
    harness.hover_at(card.center());
    harness.run();
    harness.get_by_label("Play Discovery").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayCollection(Page::Album(id)) if id == "album-1"
    )));
    assert!(!asked(&harness, |action| matches!(action, Action::Open(_))));
}

#[test]
fn a_mood_tile_opens_that_moods_page() {
    let mut state = state();
    state.nav.open(Page::Search);
    let moods = BrowsePage {
        moods: vec![MoodChip {
            id: "FEmusic_moods_and_genres_category".into(),
            params: "chill".into(),
            title: "Chill".into(),
            color: "#3366AA".into(),
        }],
        ..BrowsePage::default()
    };
    state
        .surfaces
        .insert(Surface::moods().key(), Loadable::Loaded(moods));
    let mut harness = harness(state);
    harness.get_by_label("Chill").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Browse(surface)) if surface.params == "chill" && surface.title == "Chill"
    )));
}

#[test]
fn an_earlier_search_is_searched_for_again_with_a_click() {
    let mut state = state();
    state.nav.open(Page::Search);
    state.search.recent = vec!["bonobo".into()];
    state.surfaces.insert(
        Surface::moods().key(),
        Loadable::Loaded(BrowsePage::default()),
    );
    let mut harness = harness(state);
    harness.get_by_label("bonobo").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Search(query) if query == "bonobo"
    )));
}

#[test]
fn a_shelf_with_more_behind_it_leads_to_the_whole() {
    let mut state = state();
    state.home = Loadable::Loaded(BrowsePage {
        shelves: vec![Shelf {
            title: "New releases".into(),
            items: vec![Item::Album(Album {
                id: "album-1".into(),
                title: "Discovery".into(),
                ..Album::default()
            })],
            show_all_id: "FEmusic_new_releases_albums".into(),
            ..Shelf::default()
        }],
        ..BrowsePage::default()
    });
    let mut harness = harness(state);
    harness.get_by_label("Show all").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Browse(surface))
            if surface.id == "FEmusic_new_releases_albums" && surface.title == "New releases"
    )));
}

#[test]
fn an_artists_page_offers_a_shuffle_of_their_songs() {
    let mut harness = harness(on_artist());
    harness.get_by_label("Shuffle songs").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::StartMix { seed, origin } if seed.playlist_id == "RDAO1" && origin == "Bonobo"
    )));
}

#[test]
fn an_artists_popular_songs_lead_to_all_of_them() {
    let mut harness = harness(on_artist());
    harness.get_by_label("Show all").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Playlist(id)) if id == "OLAK"
    )));
}

#[test]
fn a_podcast_card_opens_the_show() {
    let mut state = state();
    state.home = Loadable::Loaded(BrowsePage {
        shelves: vec![Shelf {
            title: "Shows".into(),
            items: vec![Item::Podcast(Podcast {
                id: "show".into(),
                title: "The Show".into(),
                ..Podcast::default()
            })],
            ..Shelf::default()
        }],
        ..BrowsePage::default()
    });
    let mut harness = harness(state);
    harness.get_by_label("The Show").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Podcast(id)) if id == "show"
    )));
}
