//! Playback: the controls, the queue, radios and lyrics.

use super::*;

#[test]
fn the_controls_do_nothing_before_there_is_a_session() {
    let mut state = ready();
    assert!(apply(&mut state, Action::TogglePlay).is_empty());
    assert!(apply(&mut state, Action::Seek(1000)).is_empty());
}

#[test]
fn pausing_shows_at_once_and_tells_the_core() {
    let mut state = playing();
    let effects = apply(&mut state, Action::TogglePlay);
    assert_eq!(session(&state).state, PlayState::Paused);
    assert_eq!(effects, [Effect::Command(Command::Toggle)]);
}

#[test]
fn a_seek_moves_the_bar_before_the_core_answers() {
    let mut state = playing();
    let effects = apply(&mut state, Action::Seek(90_000));
    assert_eq!(session(&state).position_ms, 90_000);
    assert_eq!(effects, [Effect::Command(Command::Seek(90_000))]);
}

#[test]
fn seeking_by_a_step_stays_inside_the_track() {
    let mut state = playing();
    apply(&mut state, Action::Seek(195_000));
    apply(&mut state, Action::SetPlaying(false));
    let effects = apply(&mut state, Action::SeekBy(10_000));
    assert_eq!(effects, [Effect::Command(Command::Seek(200_000))]);
    apply(&mut state, Action::Seek(4000));
    let effects = apply(&mut state, Action::SeekBy(-10_000));
    assert_eq!(effects, [Effect::Command(Command::Seek(0))]);
}

#[test]
fn asking_for_the_state_already_in_force_does_nothing() {
    let mut state = playing();
    assert!(apply(&mut state, Action::SetPlaying(true)).is_empty());
    assert_eq!(
        apply(&mut state, Action::SetPlaying(false)),
        [Effect::Command(Command::Toggle)]
    );
}

#[test]
fn a_volume_step_stops_at_the_ends() {
    let mut state = playing();
    for _ in 0..10 {
        apply(&mut state, Action::VolumeBy(0.05));
    }
    assert_eq!(session(&state).volume, 1.0);
}

#[test]
fn play_next_goes_right_after_what_is_playing() {
    let mut state = playing();
    let track = Track {
        title: "Next".into(),
        ..Track::default()
    };
    let effects = apply(&mut state, Action::PlayNext(vec![track.clone()]));
    assert_eq!(
        effects,
        [Effect::Command(Command::Enqueue {
            tracks: vec![track],
            at: Some(1)
        })]
    );
    assert_eq!(
        state.toasts.last().map(|t| t.text.as_str()),
        Some("Next will play next")
    );
}

#[test]
fn adding_to_an_empty_queue_plays() {
    let mut state = ready();
    let effects = apply(&mut state, Action::AddToQueue(vec![Track::default()]));
    assert!(matches!(
        effects.as_slice(),
        [Effect::Command(Command::Play { .. })]
    ));
}

#[test]
fn opening_the_lyrics_fetches_them_once_for_the_track() {
    let mut state = playing();
    let effects = apply(&mut state, Action::ToggleLyrics);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Fetch(Request::Lyrics(track)) if track.id == "a"
    )));
    // Another snapshot of the same track asks for nothing more.
    let mut projection = Projection::default();
    projection.state.queue.items = vec![Track {
        id: "a".into(),
        ..Track::default()
    }];
    let effects = apply(&mut state, Action::SessionChanged(Box::new(projection)));
    assert!(effects.is_empty());
}

#[test]
fn lyrics_for_a_track_no_longer_playing_are_dropped() {
    let mut state = playing();
    apply(&mut state, Action::ToggleLyrics);
    let late = Response::Lyrics {
        track_id: "another".into(),
        result: Ok(None),
    };
    apply(&mut state, Action::Loaded(Box::new(late)));
    assert_eq!(state.lyrics.words, Loadable::Loading);
}

#[test]
fn the_queue_and_the_lyrics_take_turns_on_the_right() {
    let mut state = ready();
    apply(&mut state, Action::ToggleQueue);
    assert_eq!(state.settings.panel, RightPanel::Queue);
    apply(&mut state, Action::ToggleLyrics);
    assert_eq!(state.settings.panel, RightPanel::Lyrics);
    apply(&mut state, Action::ToggleLyrics);
    assert_eq!(state.settings.panel, RightPanel::Closed);
}

#[test]
fn mute_remembers_the_volume_and_returns_to_it() {
    let mut state = playing();
    apply(&mut state, Action::ToggleMute);
    assert_eq!(session(&state).volume, 0.0);
    let effects = apply(&mut state, Action::ToggleMute);
    assert_eq!(session(&state).volume, 0.8);
    assert_eq!(effects, [Effect::Command(Command::SetVolume(0.8))]);
}

#[test]
fn playing_a_row_queues_the_list_from_that_row() {
    let mut state = ready();
    let tracks = vec![Track::default(), Track::default()];
    let effects = apply(
        &mut state,
        Action::Play {
            tracks: tracks.clone(),
            index: 1,
            origin: "Discovery".into(),
        },
    );
    assert_eq!(
        effects,
        [Effect::Command(Command::Play {
            tracks,
            start_index: 1,
            origin: "Discovery".into(),
        })]
    );
}

#[test]
fn a_collection_not_yet_fetched_plays_when_it_arrives() {
    use spotified_client::models::Album;

    let mut state = ready();
    let page = Page::Album("al".into());
    let effects = apply(&mut state, Action::PlayCollection(page));
    assert_eq!(effects, [Effect::Fetch(Request::Album("al".into()))]);

    let album = Album {
        title: "Discovery".into(),
        tracks: vec![
            Track::default(),
            Track {
                playable: true,
                ..Track::default()
            },
        ],
        ..Album::default()
    };
    let arrived = Response::Album("al".into(), Ok(album));
    let effects = apply(&mut state, Action::Loaded(Box::new(arrived)));
    assert!(matches!(
        effects.as_slice(),
        [Effect::Command(Command::Play { start_index: 1, origin, .. })] if origin == "Discovery"
    ));
    assert!(state.pending_play.is_none());
}

#[test]
fn a_radio_is_started_for_this_device() {
    let mut state = ready();
    state.settings.device_id = "native-1".into();
    let effects = apply(&mut state, Action::StartRadio(track("a")));
    assert_eq!(
        effects,
        [Effect::Fetch(Request::StartRadio {
            device_id: "native-1".into(),
            track: Box::new(track("a")),
        })]
    );
    let refused = Response::RadioStarted(Err(ApiError::RateLimited));
    apply(&mut state, Action::Loaded(Box::new(refused)));
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}

#[test]
fn a_show_asked_to_play_is_fetched_and_plays_from_its_first_episode() {
    let mut state = ready();
    let page = Page::Podcast("show".into());
    assert_eq!(
        apply(&mut state, Action::PlayCollection(page)),
        [Effect::Fetch(Request::Podcast("show".into()))]
    );
    let podcast = Podcast {
        id: "show".into(),
        title: "The Show".into(),
        episodes: vec![track("e1"), track("e2")],
        ..Podcast::default()
    };
    let answer = Response::Podcast("show".into(), Ok(podcast));
    let effects = apply(&mut state, Action::Loaded(Box::new(answer)));
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Play { tracks, start_index: 0, origin })]
            if tracks.len() == 2 && origin == "The Show"
    ));
}

#[test]
fn the_mini_player_opens_where_it_was_left_and_remembers_where_it_is_closed() {
    let mut state = ready();
    state.settings.mini_size = [640.0, 280.0];
    state.settings.mini_position = Some([40.0, 60.0]);
    assert!(apply(&mut state, Action::ToggleMiniPlayer).is_empty());
    assert!(state.mini_player);
    assert_eq!(state.mini_opened.size, [640.0, 280.0]);
    assert_eq!(state.mini_opened.position, Some([40.0, 60.0]));

    let moved = Action::MiniMoved {
        position: [10.0, 20.0],
        size: [360.0, 80.0],
    };
    assert!(apply(&mut state, moved).is_empty());
    assert_eq!(
        apply(&mut state, Action::ToggleMiniPlayer),
        [Effect::SaveSettings]
    );
    assert_eq!(state.settings.mini_size, [360.0, 80.0]);
}

#[test]
fn asking_the_mini_player_for_the_queue_makes_it_tall_enough() {
    let mut state = ready();
    apply(&mut state, Action::ToggleMiniPlayer);
    let effects = apply(&mut state, Action::SetMiniPanel(MiniPanel::Queue));
    assert_eq!(effects, [Effect::GrowMini([360.0, 580.0])]);
    assert!(apply(&mut state, Action::SetMiniPanel(MiniPanel::Art)).is_empty());
}
