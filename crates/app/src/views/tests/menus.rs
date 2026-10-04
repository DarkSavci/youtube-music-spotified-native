//! The menus, and the links to a song's artists.

use eframe::egui::{Key, pos2};
use egui_kittest::kittest::NodeT;

use super::*;

/// The menu of the playlist's first song, open.
fn first_songs_menu() -> Harness<'static, Fixture> {
    let mut harness = harness(on_playlist());
    harness.get_by_label("First song").click_secondary();
    harness.run();
    harness
}

fn press(harness: &mut Harness<'_, Fixture>, key: Key) {
    harness.key_press(key);
    harness.run();
}

#[test]
fn a_songs_menu_lists_what_the_old_one_did_in_its_order() {
    let harness = first_songs_menu();
    let entries = [
        "Add to queue",
        "Play next",
        "Add to playlist",
        "Go to song radio",
        "Go to The Artist",
        "Save to your library",
        "Share",
    ];
    let mut last = f32::NEG_INFINITY;
    for entry in entries {
        let top = harness.get_by_label(entry).rect().top();
        assert!(top > last, "{entry} is out of order");
        last = top;
    }
}

#[test]
fn enter_chooses_the_entry_that_is_lit_which_is_the_first_as_a_menu_opens() {
    let mut harness = first_songs_menu();
    press(&mut harness, Key::Enter);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(tracks) if tracks[0].id == "a"
    )));
    // And the choice shut the menu.
    assert!(harness.query_by_label("Add to queue").is_none());
}

#[test]
fn the_arrows_move_through_a_menu_and_round_its_ends() {
    let mut harness = first_songs_menu();
    press(&mut harness, Key::ArrowDown);
    press(&mut harness, Key::Enter);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::PlayNext(_)
    )));

    // Up from the first entry is the last.
    let mut harness = first_songs_menu();
    press(&mut harness, Key::ArrowUp);
    press(&mut harness, Key::Enter);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Share { .. }
    )));
}

#[test]
fn the_keys_of_the_app_are_the_menus_while_one_is_open() {
    let mut harness = first_songs_menu();
    // Space pauses the music anywhere else; here it chooses.
    press(&mut harness, Key::Space);
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AddToQueue(_)
    )));
}

#[test]
fn right_opens_the_further_menu_and_left_comes_back_from_it() {
    let mut harness = first_songs_menu();
    press(&mut harness, Key::ArrowDown);
    press(&mut harness, Key::ArrowDown);
    assert!(harness.query_by_label("New playlist…").is_none());
    press(&mut harness, Key::ArrowRight);
    assert!(harness.query_by_label("New playlist…").is_some());
    press(&mut harness, Key::ArrowLeft);
    assert!(harness.query_by_label("New playlist…").is_none());
    // The first menu is still there, and the keys are its again.
    assert!(harness.query_by_label("Add to queue").is_some());

    press(&mut harness, Key::ArrowRight);
    press(&mut harness, Key::Enter);
    assert!(asked(&harness, |action| matches!(
        action,
        Action::NewPlaylist { name, track_ids }
            if name == "First song" && track_ids == &["a".to_owned()]
    )));
}

#[test]
fn escape_shuts_a_menu_and_asks_for_nothing() {
    let mut harness = first_songs_menu();
    press(&mut harness, Key::Escape);
    assert!(harness.query_by_label("Add to queue").is_none());
    assert!(harness.state().actions.is_empty());
}

#[test]
fn an_entry_that_cannot_be_chosen_says_so_and_is_stepped_over() {
    let mut state = state();
    state.account = Some(spotified_client::models::Account {
        name: "Ada".into(),
        ..Default::default()
    });
    state.signing_in = true;
    state.account_menu_asks = 1;
    let harness = harness(state);
    let add = harness.get_by_label("Finish in your browser…");
    assert!(add.accesskit_node().is_disabled());
}

/// The playlist, with its first song by `artists`.
fn first_song_by(artists: Vec<ArtistRef>) -> State {
    let mut state = on_playlist();
    let mut first = track("a", "First song");
    first.artists = artists;
    let playlist = Playlist {
        id: "pl".into(),
        title: "Road trip".into(),
        tracks: vec![first, track("b", "Second song")],
        ..Playlist::default()
    };
    state
        .playlists
        .insert("pl".into(), Loadable::Loaded(playlist));
    state
}

#[test]
fn a_song_by_two_offers_the_page_of_each() {
    let artist = |id: &str, name: &str| ArtistRef {
        id: id.into(),
        name: name.into(),
    };
    let state = first_song_by(vec![artist("one", "Ay"), artist("two", "Bee")]);
    let mut harness = harness(state);
    harness.get_by_label("First song").click_secondary();
    harness.run();
    assert!(harness.query_by_label("Go to Ay").is_some());
    harness.get_by_label("Go to Bee").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Artist(id)) if id == "two"
    )));
}

/// The line of artists alone, and what a click on it came to.
struct Line {
    artists: Vec<ArtistRef>,
    clicked: Option<String>,
    /// How wide a name and the comma between two came out.
    name: f32,
    gap: f32,
    themed: bool,
}

const LINE_AT: eframe::egui::Pos2 = pos2(20.0, 20.0);
const NAME: &str = "MMMMMMMMMM";

/// Three artists of one name each: two with pages either side of one
/// without.
fn line_of_three() -> Harness<'static, Line> {
    let artists = ["first", "", "third"].into_iter().map(|id| ArtistRef {
        id: id.into(),
        name: NAME.into(),
    });
    let line = Line {
        artists: artists.collect(),
        clicked: None,
        name: 0.0,
        gap: 0.0,
        themed: false,
    };
    let mut harness = Harness::builder()
        .with_size(vec2(600.0, 60.0))
        .build_ui_state(
            |ui, line: &mut Line| {
                if !line.themed {
                    theme::install(ui.ctx(), &theme::DARK);
                    line.themed = true;
                    return;
                }
                let font = theme::regular(12.0);
                let ink = eframe::egui::Color32::WHITE;
                let width = |text: &str| {
                    let text = text.to_owned();
                    ui.painter()
                        .layout_no_wrap(text, font.clone(), ink)
                        .size()
                        .x
                };
                (line.name, line.gap) = (width(NAME), width(", "));
                let artists = crate::views::widgets::Artists {
                    artists: &line.artists,
                    font: font.clone(),
                    color: ink,
                    width: 560.0,
                };
                let id = eframe::egui::Id::new("line");
                if let (Some(artist), _) = artists.show(ui, id, LINE_AT) {
                    line.clicked = Some(artist);
                }
            },
            line,
        );
    harness.run();
    harness
}

/// Clicks the middle of the `nth` name.
fn click_name(harness: &mut Harness<'_, Line>, nth: usize) {
    let line = harness.state();
    let along = (line.name + line.gap) * nth as f32 + line.name / 2.0;
    let at = LINE_AT + vec2(along, 6.0);
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.drop_at(at);
    harness.run();
}

#[test]
fn each_artist_of_a_song_leads_to_their_own_page() {
    let mut harness = line_of_three();
    click_name(&mut harness, 0);
    assert_eq!(harness.state().clicked.as_deref(), Some("first"));
    click_name(&mut harness, 2);
    assert_eq!(harness.state().clicked.as_deref(), Some("third"));
}

#[test]
fn an_artist_with_no_page_is_plain_text_among_the_links() {
    let mut harness = line_of_three();
    click_name(&mut harness, 1);
    assert_eq!(harness.state().clicked, None);
}

#[test]
fn an_artist_from_the_library_opens_the_channel_the_page_wants() {
    use crate::views::widgets::artist_page_id;
    assert_eq!(artist_page_id("MPLAUCabc"), "UCabc");
    assert_eq!(artist_page_id("UCabc"), "UCabc");
    assert_eq!(artist_page_id("MPLAxyz"), "MPLAxyz");
}

#[test]
fn a_songs_menu_blocks_the_song_its_artist_or_its_album() {
    use crate::blocked::Kind;

    let mut harness = first_songs_menu();
    harness.get_by_label("Block this song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetBlocked { kind: Kind::Song, id, blocked: true, .. } if id == "a"
    )));

    let mut harness = first_songs_menu();
    harness.get_by_label("Block The Artist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetBlocked { kind: Kind::Artist, id, name, blocked: true }
            if id == "artist-1" && name == "The Artist"
    )));
}

#[test]
fn what_is_blocked_is_offered_back() {
    use crate::blocked::Kind;

    let mut state = on_playlist();
    let blocked = &mut state.settings.blocked;
    blocked.set(Kind::Song, "a", "First song", true);
    let mut harness = harness(state);
    harness.get_by_label("First song").click_secondary();
    harness.run();
    assert!(harness.query_by_label("Block this song").is_none());
    harness.get_by_label("Unblock this song").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetBlocked {
            kind: Kind::Song,
            blocked: false,
            ..
        }
    )));
}

/// A right click on the player bar's song (`true`) or on its artist, found
/// from the heart that follows them.
fn right_click_now_playing(harness: &mut Harness<'_, Fixture>, title: bool) {
    let heart = harness.get_by_label("Save to Liked Songs").rect().center();
    let at = heart + vec2(-60.0, if title { -9.0 } else { 9.0 });
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(eframe::egui::Event::PointerButton {
            pos: at,
            button: eframe::egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        });
    }
    harness.run();
}

#[test]
fn a_right_click_on_the_playing_song_brings_its_menu() {
    let mut harness = harness(playing(state()));
    right_click_now_playing(&mut harness, true);
    for entry in ["Add to queue", "Go to song radio", "Block this song"] {
        assert!(harness.query_by_label(entry).is_some(), "{entry}");
    }
}

#[test]
fn a_right_click_on_the_playing_artist_brings_theirs() {
    let mut harness = harness(playing(state()));
    right_click_now_playing(&mut harness, false);
    assert!(harness.query_by_label("Add to queue").is_none());
    harness.get_by_label("Block The Artist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetBlocked { kind: crate::blocked::Kind::Artist, id, blocked: true, .. }
            if id == "artist-1"
    )));
}

#[test]
fn the_playing_songs_album_is_named_after_its_artists_and_leads_to_it() {
    use spotified_client::models::AlbumRef;

    let mut state = playing(state());
    if let Some(playback) = &mut state.playback {
        playback.session.queue.items[0].album = Some(AlbumRef {
            id: "MPREb1".into(),
            name: "A long enough album name".into(),
        });
    }
    let mut harness = harness(state);
    // The line ends with the album, a little before the heart.
    let heart = harness.get_by_label("Save to Liked Songs").rect().center();
    let at = heart + vec2(-48.0, 9.0);
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.drop_at(at);
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Open(Page::Album(id)) if id == "MPREb1"
    )));
}
