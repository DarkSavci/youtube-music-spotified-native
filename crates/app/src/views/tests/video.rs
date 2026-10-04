//! The video button, the surface the picture is drawn on, and the setting
//! that keeps music videos out of the shelves.

use eframe::egui::vec2;
use spotified_client::models::{BrowsePage, Item, Shelf};

use super::*;
use crate::actions::VideoAsk;
use crate::state::video::Availability;

fn set_video(harness: &Harness<'_, Fixture>, on: bool) -> bool {
    asked(
        harness,
        |action| matches!(action, Action::Video(VideoAsk::Set(asked)) if *asked == on),
    )
}

/// Playing a song, as the player reported it: what is known of its video
/// is about that song.
fn listening() -> State {
    let mut state = playing(state());
    state.video.about = "a".into();
    state
}

/// Playing a video, with the picture switched on.
fn watching() -> State {
    let mut state = playing(state());
    if let Some(playback) = &mut state.playback {
        playback.session.queue.items[0].is_video = true;
    }
    state.video.enabled = true;
    state
}

#[test]
fn the_video_button_asks_for_the_songs_video() {
    let mut harness = harness(listening());
    harness.get_by_label("Watch music video").click();
    harness.run();
    assert!(set_video(&harness, true));
}

#[test]
fn reaching_the_video_button_finds_out_whether_there_is_a_video() {
    let mut harness = harness(listening());
    harness.get_by_label("Watch music video").hover();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Video(VideoAsk::Check)
    )));
    assert!(!set_video(&harness, true));
}

#[test]
fn the_video_button_does_not_answer_for_a_song_with_no_video() {
    let mut state = listening();
    state.video.availability = Availability::Unavailable;
    let mut harness = harness(state);
    let said = "No matching music video is available for this song.";
    harness.get_by_label(said).click();
    harness.run();
    assert!(harness.state().actions.is_empty());
}

#[test]
fn there_is_no_video_button_with_nothing_playing() {
    let harness = harness(state());
    assert!(
        harness
            .query_by_label("Play a song to watch its video.")
            .is_none()
    );
    assert!(harness.query_by_label("Watch music video").is_none());
}

#[test]
fn the_lit_video_button_goes_back_to_the_song() {
    let mut harness = harness(watching());
    harness.get_by_label("Switch to song").click();
    harness.run();
    assert!(set_video(&harness, false));
}

#[test]
fn the_picture_above_the_page_can_be_given_the_screen() {
    let mut harness = harness(watching());
    // Drawn, the surface says so, which is what keeps the decoder going.
    assert!(harness.state().state.video.watched.get().is_some());
    harness.get_by_label("Watch video full screen").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetFullscreenPlayer(true)
    )));
}

#[test]
fn no_surface_is_drawn_while_the_video_is_off() {
    let harness = harness(playing(state()));
    assert!(harness.state().state.video.watched.get().is_none());
    assert!(harness.query_by_label("Watch video full screen").is_none());
}

#[test]
fn a_picture_that_failed_offers_another_try() {
    let mut state = watching();
    state.video.error = Some(crate::actions::video::LOAD_FAILED.into());
    let mut harness = harness(state);
    harness.get_by_label("Retry video").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Video(VideoAsk::Retry)
    )));
}

#[test]
fn the_full_screen_player_has_the_video_button_too() {
    let mut state = watching();
    state.fullscreen_player = true;
    let mut harness = harness(state);
    assert!(harness.state().state.video.watched.get().is_some());
    harness.get_by_label("Switch to song").click();
    harness.run();
    assert!(set_video(&harness, false));
}

#[test]
fn the_mini_player_has_the_video_button_and_shows_the_picture() {
    let mut harness = mini_harness(watching(), vec2(360.0, 580.0));
    assert!(harness.state().state.video.watched.get().is_some());
    harness.get_by_label("Switch to song").click();
    harness.run();
    assert!(set_video(&harness, false));
    // A strip has only a thumbnail, which stays the cover.
    let strip = mini_harness(watching(), vec2(420.0, 80.0));
    assert!(strip.state().state.video.watched.get().is_none());
}

#[test]
fn the_setting_for_music_videos_asks_for_the_other_state() {
    let mut state = state();
    state.nav.open(Page::Settings);
    let mut harness = harness(state);
    switch(&harness, "Show music videos").scroll_to_me();
    harness.run();
    switch(&harness, "Show music videos").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SetShowMusicVideos(true)
    )));
}

/// Home, with a shelf of one song and one music video.
fn home_with_a_video(show: bool) -> State {
    let mut state = state();
    state.settings.show_music_videos = show;
    let mut clip = track("v", "The video of it");
    clip.is_video = true;
    let shelf = Shelf {
        title: "Quick picks".into(),
        items: vec![
            Item::Track(track("s", "The song itself")),
            Item::Track(clip),
        ],
        ..Shelf::default()
    };
    let page = BrowsePage {
        shelves: vec![shelf],
        ..BrowsePage::default()
    };
    state.home = Loadable::Loaded(page);
    state
}

#[test]
fn a_shelf_leaves_its_music_videos_out_unless_they_are_asked_for() {
    let harness = harness(home_with_a_video(false));
    assert!(harness.query_by_label_contains("The song itself").is_some());
    assert!(harness.query_by_label_contains("The video of it").is_none());
    let harness = self::harness(home_with_a_video(true));
    assert!(harness.query_by_label_contains("The video of it").is_some());
}
