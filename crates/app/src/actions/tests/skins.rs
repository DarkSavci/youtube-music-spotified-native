//! Winamp skins for the mini player: choosing one, and its windows.

use std::path::PathBuf;

use super::*;
use crate::skin::layout;
use crate::skins::{Ask, BUILT_IN};

fn ask(state: &mut State, ask: Ask) -> Vec<Effect> {
    apply(state, Action::Skin(ask))
}

#[test]
fn choosing_a_skin_opens_the_mini_player_in_it() {
    let mut state = state();
    state.settings.mini_position = Some([40.0, 60.0]);
    let effects = ask(&mut state, Ask::Wear(Some(BUILT_IN.to_owned())));
    assert_eq!(effects, [Effect::SaveSettings]);
    assert_eq!(state.settings.mini_skin.as_deref(), Some(BUILT_IN));
    assert!(state.mini_player);
    assert_eq!(state.mini_opened.position, Some([40.0, 60.0]));
    // Not read yet: until it is, the mini player is the app's own.
    assert!(state.worn_skin().is_none());

    // Taking the skin off leaves the mini player open, as itself.
    ask(&mut state, Ask::Wear(None));
    assert_eq!(state.settings.mini_skin, None);
    assert!(state.mini_player);
}

#[test]
fn a_skin_that_cannot_be_read_is_given_up_and_said_to_be() {
    let mut state = state();
    ask(&mut state, Ask::Wear(Some("Gone.wsz".to_owned())));
    let effects = ask(&mut state, Ask::Failed("not a zip archive".to_owned()));
    assert_eq!(effects, [Effect::SaveSettings]);
    assert_eq!(state.settings.mini_skin, None);
    let said = &state.toasts.last().expect("a toast").text;
    assert!(said.contains("not a zip archive"), "{said}");
}

#[test]
fn skin_files_are_installed_by_the_app_and_none_is_nothing_to_do() {
    let mut state = state();
    assert!(ask(&mut state, Ask::Install(Vec::new())).is_empty());
    let files = vec![PathBuf::from("Fresh.wsz")];
    assert_eq!(
        ask(&mut state, Ask::Install(files.clone())),
        [Effect::InstallSkins(files)]
    );
    assert_eq!(ask(&mut state, Ask::Pick), [Effect::PickSkins]);
    assert_eq!(ask(&mut state, Ask::OpenFolder), [Effect::OpenSkinsFolder]);
    assert_eq!(ask(&mut state, Ask::Reload), [Effect::ReloadSkins]);
    assert_eq!(
        ask(&mut state, Ask::OpenMuseum),
        [Effect::OpenUrl("https://skins.webamp.org")]
    );
}

#[test]
fn a_skin_is_drawn_at_one_of_four_sizes() {
    let mut state = state();
    assert_eq!(state.settings.skin_scale, 2);
    assert!(ask(&mut state, Ask::Scale(2)).is_empty());
    assert_eq!(ask(&mut state, Ask::Scale(4)), [Effect::SaveSettings]);
    assert_eq!(state.settings.skin_scale, 4);
    assert!(ask(&mut state, Ask::Scale(9)).is_empty());
    assert!(ask(&mut state, Ask::Scale(0)).is_empty());
    assert_eq!(state.settings.skin_scale, 4);
}

#[test]
fn a_skins_windows_open_shut_and_roll_up() {
    let mut state = state();
    for (toggle, read) in [
        (
            Ask::ToggleShade,
            (|settings| settings.skin_shaded) as fn(&Settings) -> bool,
        ),
        (Ask::ToggleEqualizer, |settings| settings.skin_equalizer),
        (Ask::ToggleEqualizerShade, |settings| {
            settings.skin_equalizer_shaded
        }),
        (Ask::TogglePlaylist, |settings| settings.skin_playlist),
        (Ask::TogglePlaylistShade, |settings| {
            settings.skin_playlist_shaded
        }),
    ] {
        assert!(!read(&state.settings));
        assert_eq!(ask(&mut state, toggle.clone()), [Effect::SaveSettings]);
        assert!(read(&state.settings));
        ask(&mut state, toggle);
        assert!(!read(&state.settings));
    }
}

#[test]
fn the_skins_display_goes_from_spectrum_to_wave_to_nothing_and_round() {
    let mut state = state();
    let looks = |state: &State| (state.settings.skin_analyser, state.settings.skin_scope);
    assert_eq!(looks(&state), (true, false));
    assert_eq!(ask(&mut state, Ask::CycleAnalyser), [Effect::SaveSettings]);
    assert_eq!(looks(&state), (true, true));
    ask(&mut state, Ask::CycleAnalyser);
    assert_eq!(looks(&state), (false, false));
    ask(&mut state, Ask::CycleAnalyser);
    assert_eq!(looks(&state), (true, false));
}

#[test]
fn the_balance_is_heard_as_it_is_dragged_and_kept_when_let_go() {
    let mut state = state();
    ask(&mut state, Ask::Wear(Some(BUILT_IN.to_owned())));
    assert_eq!(ask(&mut state, Ask::Balance(-0.4)), [Effect::ApplyBalance]);
    assert_eq!(state.settings.balance, -0.4);
    // Where it already is: nothing to tell the engine.
    assert!(ask(&mut state, Ask::Balance(-0.4)).is_empty());
    assert_eq!(ask(&mut state, Ask::Keep), [Effect::SaveSettings]);
    ask(&mut state, Ask::Balance(7.0));
    assert_eq!(state.settings.balance, 1.0);
    ask(&mut state, Ask::Balance(f32::NAN));
    assert_eq!(state.settings.balance, 0.0);
}

#[test]
fn taking_the_skin_off_puts_the_balance_back_in_the_middle() {
    let mut state = state();
    ask(&mut state, Ask::Wear(Some(BUILT_IN.to_owned())));
    ask(&mut state, Ask::Balance(1.0));
    let effects = ask(&mut state, Ask::Wear(None));
    assert_eq!(effects, [Effect::SaveSettings, Effect::ApplyBalance]);
    assert_eq!(state.settings.balance, 0.0);

    ask(&mut state, Ask::Wear(Some("Gone.wsz".to_owned())));
    ask(&mut state, Ask::Balance(-1.0));
    let effects = ask(&mut state, Ask::Failed("unreadable".to_owned()));
    assert_eq!(effects, [Effect::SaveSettings, Effect::ApplyBalance]);
    assert_eq!(state.settings.balance, 0.0);
}

#[test]
fn the_skins_playlist_stretches_a_row_of_tiles_at_a_time() {
    let mut state = state();
    let least = layout::PLAYLIST_MIN_HEIGHT;
    assert!(ask(&mut state, Ask::PlaylistHeight(least + 31)).is_empty());
    assert_eq!(state.settings.skin_playlist_height, least + 29);
    ask(&mut state, Ask::PlaylistHeight(3));
    assert_eq!(state.settings.skin_playlist_height, least);
    ask(&mut state, Ask::PlaylistHeight(100_000));
    assert_eq!(
        state.settings.skin_playlist_height,
        layout::PLAYLIST_MAX_HEIGHT
    );
}

#[test]
fn a_skinned_mini_player_remembers_its_place_and_not_the_skins_size() {
    let mut state = state();
    state.settings.mini_size = [640.0, 280.0];
    ask(&mut state, Ask::Wear(Some(BUILT_IN.to_owned())));
    let moved = Action::MiniMoved {
        position: [10.0, 20.0],
        size: [550.0, 232.0],
    };
    apply(&mut state, moved);
    assert_eq!(state.settings.mini_position, Some([10.0, 20.0]));
    assert_eq!(state.settings.mini_size, [640.0, 280.0]);
}

#[test]
fn the_sound_is_watched_for_a_skins_analyser_only_while_it_shows() {
    let mut state = state();
    assert!(!state.watches_sound());
    ask(&mut state, Ask::Wear(Some(BUILT_IN.to_owned())));
    assert!(state.watches_sound());
    // The wave is drawn from the sound as the spectrum is.
    ask(&mut state, Ask::CycleAnalyser);
    assert!(state.watches_sound());
    ask(&mut state, Ask::CycleAnalyser);
    assert!(!state.watches_sound());
    ask(&mut state, Ask::CycleAnalyser);
    apply(&mut state, Action::ToggleMiniPlayer);
    assert!(!state.watches_sound());
    state.settings.visualizer = true;
    assert!(state.watches_sound());
}

#[test]
fn milkdrop_opens_at_once_and_closes_when_it_fails_or_is_closed() {
    use crate::milkdrop::Ask as MilkDrop;
    let mut state = state();
    let ask = |state: &mut State, ask: MilkDrop| apply(state, Action::MilkDrop(ask));
    assert_eq!(ask(&mut state, MilkDrop::Toggle), [Effect::OpenMilkDrop]);
    assert!(state.milkdrop.open);
    // With no presets it still opens, and says where to get some.
    assert!(
        state
            .toasts
            .last()
            .expect("a toast")
            .text
            .contains("presets")
    );
    assert_eq!(ask(&mut state, MilkDrop::Toggle), [Effect::CloseMilkDrop]);
    assert!(!state.milkdrop.open);

    ask(&mut state, MilkDrop::Toggle);
    assert!(ask(&mut state, MilkDrop::Closed).is_empty());
    assert!(!state.milkdrop.open);
    ask(&mut state, MilkDrop::Toggle);
    ask(&mut state, MilkDrop::Failed("no library".into()));
    assert!(!state.milkdrop.open);
    assert!(
        state
            .toasts
            .last()
            .expect("a toast")
            .text
            .contains("no library")
    );
}

#[test]
fn one_pack_of_presets_is_fetched_at_a_time() {
    use crate::milkdrop::Ask as MilkDrop;
    let mut state = state();
    let ask = |state: &mut State, ask: MilkDrop| apply(state, Action::MilkDrop(ask));
    assert_eq!(
        ask(&mut state, MilkDrop::GetPresets(0)),
        [Effect::FetchPresets(0)]
    );
    assert!(state.milkdrop.fetching);
    assert!(ask(&mut state, MilkDrop::GetPresets(1)).is_empty());
    assert!(ask(&mut state, MilkDrop::Fetched(Ok(550))).is_empty());
    assert!(!state.milkdrop.fetching);
    assert!(
        state
            .toasts
            .last()
            .expect("a toast")
            .text
            .starts_with("550")
    );
    // A pack that is not there is not asked for.
    assert!(ask(&mut state, MilkDrop::GetPresets(99)).is_empty());
    assert!(!state.milkdrop.fetching);
    ask(&mut state, MilkDrop::GetPresets(1));
    ask(&mut state, MilkDrop::Fetched(Err("offline".into())));
    assert!(!state.milkdrop.fetching);
    assert_eq!(
        ask(&mut state, MilkDrop::OpenFolder),
        [Effect::OpenMilkDropFolder]
    );
}
