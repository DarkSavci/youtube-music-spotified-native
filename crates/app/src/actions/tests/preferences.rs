//! The settings: what each switch changes, and who is told.

use spotified_client::models::RemoteQueue;

use super::*;
use crate::settings::VolumeLevel;
use crate::state::{LaunchPickup, Notice};
use crate::together::Phase;

fn loaded(state: &mut State, response: Response) -> Vec<Effect> {
    apply(state, Action::Loaded(Box::new(response)))
}

const HEARD: [Effect; 2] = [Effect::SaveSettings, Effect::ApplyAudioSettings];

#[test]
fn a_speed_is_kept_and_the_engine_told() {
    let mut state = playing();
    assert_eq!(apply(&mut state, Action::SetSpeed(1.5)), HEARD);
    assert_eq!(state.settings.playback_speed, 1.5);
    assert_eq!(state.speed(), 1.5);
    // The bar carries on at the new speed from where it was.
    assert_eq!(
        state.playback.as_ref().map(|playback| playback.speed),
        Some(1.5)
    );
}

#[test]
fn a_speed_is_held_to_what_the_engine_plays() {
    let mut state = playing();
    apply(&mut state, Action::SetSpeed(7.0));
    assert_eq!(state.settings.playback_speed, 3.0);
    apply(&mut state, Action::SetSpeed(1.26));
    assert_eq!(state.settings.playback_speed, 1.25);
    // The speed already in force is not saved again.
    assert!(apply(&mut state, Action::SetSpeed(1.25)).is_empty());
}

#[test]
fn a_room_holds_everyone_at_normal_speed_and_says_so() {
    let mut state = playing();
    apply(&mut state, Action::SetSpeed(2.0));
    state.together.phase = Phase::Joined;
    assert_eq!(state.speed(), 1.0);
    assert!(apply(&mut state, Action::SetSpeed(1.5)).is_empty());
    assert_eq!(state.settings.playback_speed, 2.0);
    assert_eq!(
        state.toasts.last().map(|toast| toast.text.as_str()),
        Some("Listen Together keeps everyone at normal speed.")
    );
    // Out of the room, the speed chosen before it is back.
    state.together.phase = Phase::Idle;
    assert_eq!(state.speed(), 2.0);
}

#[test]
fn a_new_report_from_the_core_moves_at_the_speed_in_force() {
    let mut state = playing();
    apply(&mut state, Action::SetSpeed(2.0));
    apply(&mut state, Action::SessionChanged(Box::default()));
    assert_eq!(
        state.playback.as_ref().map(|playback| playback.speed),
        Some(2.0)
    );
}

#[test]
fn the_volume_stops_at_full_without_boost_and_at_double_with_it() {
    let mut state = playing();
    apply(&mut state, Action::SetVolume(1.6));
    assert_eq!(session(&state).volume, 1.0);
    assert_eq!(
        apply(&mut state, Action::SetVolumeBoost(true)),
        [Effect::SaveSettings]
    );
    assert_eq!(
        apply(&mut state, Action::SetVolume(1.6)),
        [Effect::Command(Command::SetVolume(1.6))]
    );
    apply(&mut state, Action::SetVolume(5.0));
    assert_eq!(session(&state).volume, 2.0);
}

#[test]
fn turning_boost_off_brings_a_boosted_level_back_to_full() {
    let mut state = playing();
    apply(&mut state, Action::SetVolumeBoost(true));
    apply(&mut state, Action::SetVolume(1.5));
    assert_eq!(
        apply(&mut state, Action::SetVolumeBoost(false)),
        [
            Effect::SaveSettings,
            Effect::Command(Command::SetVolume(1.0))
        ]
    );
    assert_eq!(session(&state).volume, 1.0);
    // A level under full is left where it is.
    apply(&mut state, Action::SetVolumeBoost(true));
    apply(&mut state, Action::SetVolume(0.4));
    assert_eq!(
        apply(&mut state, Action::SetVolumeBoost(false)),
        [Effect::SaveSettings]
    );
}

#[test]
fn the_settings_the_engine_and_the_core_act_on_are_passed_on() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::SetVolumeLevel(VolumeLevel::Loud)),
        HEARD
    );
    assert_eq!(state.settings.volume_level, VolumeLevel::Loud);
    assert_eq!(apply(&mut state, Action::SetGapless(false)), HEARD);
    assert!(!state.settings.gapless);
    assert_eq!(apply(&mut state, Action::SetAutoplay(false)), HEARD);
    assert!(!state.settings.autoplay);
    assert_eq!(apply(&mut state, Action::SetResumeOnLaunch(false)), HEARD);
    assert!(!state.settings.resume_on_launch);
    assert_eq!(apply(&mut state, Action::SetReportToYouTube(false)), HEARD);
    assert!(!state.settings.report_to_youtube);
    assert_eq!(apply(&mut state, Action::SetCacheSize(5120)), HEARD);
    assert_eq!(state.settings.cache_max_mb, 5120);
}

#[test]
fn a_cache_size_that_is_not_on_offer_is_not_taken() {
    let mut state = ready();
    assert!(apply(&mut state, Action::SetCacheSize(3)).is_empty());
    assert_eq!(state.settings.cache_max_mb, 2048);
}

#[test]
fn the_settings_that_are_only_this_windows_are_only_saved() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::SetReduceMotion(true)),
        [Effect::SaveSettings]
    );
    assert!(state.settings.reduce_motion);
    assert_eq!(
        apply(&mut state, Action::SetContinueFromYouTubeMusic(true)),
        [Effect::SaveSettings]
    );
    assert!(state.settings.continue_from_youtube_music);
    assert_eq!(
        apply(&mut state, Action::ToggleRemainingTime),
        [Effect::SaveSettings]
    );
    assert!(state.settings.remaining_time);
    apply(&mut state, Action::ToggleRemainingTime);
    assert!(!state.settings.remaining_time);
}

#[test]
fn a_reset_puts_the_page_back_and_tells_everyone_who_acts_on_it() {
    let mut state = playing();
    apply(&mut state, Action::SetSpeed(2.0));
    apply(&mut state, Action::SetVolumeBoost(true));
    apply(&mut state, Action::SetVolume(1.8));
    apply(&mut state, Action::SetSystemTitleBar(true));
    apply(&mut state, Action::SetCrossfade(0));
    let effects = apply(&mut state, Action::ResetPreferences);
    assert_eq!(
        effects,
        [
            Effect::SaveSettings,
            Effect::ApplyAudioSettings,
            Effect::SetDecorations(false),
            Effect::Command(Command::SetVolume(1.0)),
        ]
    );
    assert_eq!(state.settings.playback_speed, 1.0);
    assert_eq!(state.settings.crossfade_seconds, 6);
    assert!(!state.settings.volume_boost);
    assert_eq!(
        state.playback.as_ref().map(|playback| playback.speed),
        Some(1.0)
    );
}

#[test]
fn the_full_screen_player_needs_something_playing() {
    let mut state = ready();
    assert!(apply(&mut state, Action::ToggleFullscreenPlayer).is_empty());
    assert!(!state.fullscreen_player);

    let mut state = playing();
    assert_eq!(
        apply(&mut state, Action::ToggleFullscreenPlayer),
        [Effect::SetFullscreen(true)]
    );
    assert!(state.fullscreen_player);
    assert_eq!(
        apply(&mut state, Action::SetFullscreenPlayer(false)),
        [Effect::SetFullscreen(false)]
    );
    // Closing what is already closed asks nothing of the window.
    assert!(apply(&mut state, Action::SetFullscreenPlayer(false)).is_empty());
}

#[test]
fn the_player_closed_over_full_screen_lyrics_leaves_them_the_screen() {
    let mut state = playing();
    apply(&mut state, Action::SetLyricsFullscreen(true));
    apply(&mut state, Action::SetFullscreenPlayer(true));
    assert_eq!(
        apply(&mut state, Action::SetFullscreenPlayer(false)),
        [Effect::SetFullscreen(true)]
    );
    assert!(state.lyrics_fullscreen);
}

#[test]
fn saving_the_current_track_likes_it_and_does_nothing_with_none() {
    let mut state = ready();
    assert!(apply(&mut state, Action::SaveCurrent).is_empty());

    let mut state = playing();
    assert_eq!(
        apply(&mut state, Action::SaveCurrent),
        [Effect::Fetch(Request::SetLiked {
            track_id: "a".into(),
            liked: true,
        })]
    );
    assert!(state.likes.is_liked("a"));
}

#[test]
fn focusing_the_search_opens_its_page_and_asks_for_the_caret() {
    let mut state = ready();
    apply(&mut state, Action::FocusSearch);
    assert_eq!(state.nav.page(), &Page::Search);
    assert_eq!(state.search_focus, 1);
    // Already there, only the caret is asked for again.
    apply(&mut state, Action::FocusSearch);
    assert_eq!(state.search_focus, 2);
}

#[test]
fn a_rate_limit_is_said_until_it_is_dismissed() {
    let mut state = playing();
    apply(&mut state, Action::Notify(Notice::RateLimited));
    assert_eq!(state.notice, Some(Notice::RateLimited));
    apply(&mut state, Action::DismissNotice);
    assert_eq!(state.notice, None);
}

fn report(offline: bool, playing: bool) -> Action {
    let mut projection = Projection {
        offline,
        ..Projection::default()
    };
    projection.state.state = if playing {
        PlayState::Playing
    } else {
        PlayState::Paused
    };
    Action::SessionChanged(Box::new(projection))
}

#[test]
fn losing_the_connection_is_said_for_as_long_as_it_lasts() {
    let mut state = playing();
    apply(&mut state, report(true, true));
    assert_eq!(state.notice, Some(Notice::Offline { paused: false }));
    // Paused, nothing starts again by itself, and the wording says so.
    apply(&mut state, report(true, false));
    assert_eq!(state.notice, Some(Notice::Offline { paused: true }));
    apply(&mut state, report(false, true));
    assert_eq!(state.notice, None);
}

#[test]
fn a_dismissed_offline_notice_stays_away_for_that_outage_only() {
    let mut state = playing();
    apply(&mut state, report(true, true));
    apply(&mut state, Action::DismissNotice);
    apply(&mut state, report(true, true));
    assert_eq!(state.notice, None);
    // Back, and gone again: a new outage, said anew.
    apply(&mut state, report(false, true));
    apply(&mut state, report(true, true));
    assert_eq!(state.notice, Some(Notice::Offline { paused: false }));
}

#[test]
fn the_offline_notice_never_takes_the_place_of_another() {
    let mut state = playing();
    apply(&mut state, Action::Notify(Notice::RateLimited));
    apply(&mut state, report(true, true));
    assert_eq!(state.notice, Some(Notice::RateLimited));
    apply(&mut state, report(false, true));
    assert_eq!(state.notice, Some(Notice::RateLimited));
}

fn account(signed_in: bool) -> Response {
    Response::Account(Ok(
        signed_in.then(spotified_client::models::Account::default)
    ))
}

fn remote(ids: &[&str]) -> RemoteQueue {
    RemoteQueue {
        tracks: ids
            .iter()
            .map(|id| Track {
                title: format!("Song {id}"),
                ..track(id)
            })
            .collect(),
        index: 1,
        ..RemoteQueue::default()
    }
}

#[test]
fn a_launch_picks_up_the_other_devices_queue_paused_when_asked_to() {
    let mut state = ready();
    state.settings.continue_from_youtube_music = true;
    let effects = loaded(&mut state, account(true));
    assert!(effects.contains(&Effect::Fetch(Request::RemoteQueue)));
    assert_eq!(state.launch_pickup, LaunchPickup::Reading);

    let effects = loaded(&mut state, Response::RemoteQueue(Ok(remote(&["x", "y"]))));
    assert!(matches!(
        &effects[..],
        [Effect::Command(Command::Load { tracks, start_index: 1, .. })] if tracks.len() == 2
    ));
    assert_eq!(
        state.toasts.last().map(|toast| toast.text.as_str()),
        Some("Picked up your queue from YouTube Music: Song y")
    );
    // Once in a run: a later answer about the account does not ask again.
    assert_eq!(state.launch_pickup, LaunchPickup::Done);
    let effects = loaded(&mut state, account(true));
    assert!(!effects.contains(&Effect::Fetch(Request::RemoteQueue)));
}

#[test]
fn a_launch_asks_nothing_with_the_setting_off_or_nobody_signed_in() {
    let mut state = ready();
    let effects = loaded(&mut state, account(true));
    assert!(!effects.contains(&Effect::Fetch(Request::RemoteQueue)));
    assert_eq!(state.launch_pickup, LaunchPickup::Done);

    let mut state = ready();
    state.settings.continue_from_youtube_music = true;
    assert!(loaded(&mut state, account(false)).is_empty());
    assert_eq!(state.launch_pickup, LaunchPickup::Done);
}

#[test]
fn a_launch_leaves_alone_what_is_already_playing_here() {
    let mut state = playing();
    state.settings.continue_from_youtube_music = true;
    let effects = loaded(&mut state, account(true));
    assert!(!effects.contains(&Effect::Fetch(Request::RemoteQueue)));
}

#[test]
fn a_queue_read_at_launch_is_dropped_if_music_started_meanwhile() {
    let mut state = ready();
    state.settings.continue_from_youtube_music = true;
    loaded(&mut state, account(true));
    apply(&mut state, report(false, true));
    let effects = loaded(&mut state, Response::RemoteQueue(Ok(remote(&["x", "y"]))));
    assert!(effects.is_empty());
    assert!(state.toasts.is_empty());
}

#[test]
fn a_launch_that_finds_this_device_on_the_same_song_changes_nothing() {
    let mut state = playing();
    apply(&mut state, Action::SetPlaying(false));
    state.settings.continue_from_youtube_music = true;
    loaded(&mut state, account(true));
    assert_eq!(state.launch_pickup, LaunchPickup::Reading);
    // The other device is on "a" too, which is what is loaded here.
    let effects = loaded(&mut state, Response::RemoteQueue(Ok(remote(&["z", "a"]))));
    assert!(effects.is_empty());
}

#[test]
fn a_failed_or_empty_read_at_launch_says_nothing() {
    let mut state = ready();
    state.settings.continue_from_youtube_music = true;
    loaded(&mut state, account(true));
    let failed = Response::RemoteQueue(Err(ApiError::RateLimited));
    assert!(loaded(&mut state, failed).is_empty());
    assert!(state.toasts.is_empty());
}
