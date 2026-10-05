//! The mini player in a Winamp skin, and choosing one in Settings.

use super::*;
use crate::skins::{self, Ask};

/// The mini player wearing the skin the app comes with, at the skin's own
/// size, with whatever of its windows `open` opens.
fn skinned(mut state: State, open: impl FnOnce(&mut Settings)) -> Harness<'static, Fixture> {
    state.mini_player = true;
    state.settings.mini_skin = Some(skins::BUILT_IN.to_owned());
    state.settings.skin_scale = 1;
    open(&mut state.settings);
    // The harness keeps a margin of eight points round what it is given.
    let size = super::super::mini::skinned::size(&state.settings, 1.0) + vec2(16.0, 16.0);
    let fixture = Fixture {
        state,
        actions: Vec::new(),
        themed: false,
    };
    let mut harness = Harness::builder()
        .with_size(size)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(16)
        .build_ui_state(
            |ui, fixture: &mut Fixture| {
                if !fixture.themed {
                    theme::install(ui.ctx(), &fixture.state.palette);
                    let folder = std::path::Path::new("");
                    fixture.state.skin = skins::wear(ui.ctx(), skins::BUILT_IN, folder).ok();
                    fixture.themed = true;
                    return;
                }
                super::super::mini::show(&fixture.state, ui, &mut fixture.actions);
            },
            fixture,
        );
    harness.run();
    harness
}

#[test]
fn a_skinned_mini_player_is_the_size_of_its_windows() {
    let mut settings = Settings {
        skin_scale: 2,
        ..Settings::default()
    };
    let size = |settings: &Settings| super::super::mini::skinned::size(settings, 1.0);
    assert_eq!(size(&settings), vec2(550.0, 232.0));
    settings.skin_equalizer = true;
    settings.skin_playlist = true;
    assert_eq!(size(&settings), vec2(550.0, 696.0));
    settings.skin_shaded = true;
    settings.skin_equalizer_shaded = true;
    assert_eq!(size(&settings), vec2(550.0, 288.0));
    settings.skin_playlist_shaded = true;
    assert_eq!(size(&settings), vec2(550.0, 84.0));
    // On a display half as dense again, twice is three pixels to one.
    let unit = super::super::mini::skinned::unit(&settings, 1.5);
    assert_eq!(unit, 2.0);
}

#[test]
fn the_skins_transport_steers_playback_as_winamps_did() {
    let mut harness = skinned(playing(on_playlist()), |_| {});
    harness.get_by_label("Next").click();
    harness.get_by_label("Previous").click();
    harness.get_by_label("Pause").click();
    harness.get_by_label("Shuffle").click();
    harness.get_by_label("Repeat").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::Next)));
    assert!(asked(&harness, |action| matches!(action, Action::Previous)));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogglePlay
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleShuffle
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CycleRepeat
    )));
}

#[test]
fn play_on_a_playing_song_starts_it_over_and_stop_rewinds() {
    let mut harness = skinned(playing(on_playlist()), |_| {});
    harness.get_by_label("Play").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::Seek(0))));
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::SetPlaying(_)
    )));

    let mut harness = skinned(playing(on_playlist()), |_| {});
    harness.get_by_label("Stop").click();
    harness.run();
    let asked_for: Vec<&Action> = harness.state().actions.iter().collect();
    assert!(matches!(
        asked_for[..],
        [Action::SetPlaying(false), Action::Seek(0)]
    ));
}

#[test]
fn eject_and_the_title_bar_lead_out_of_the_skin() {
    let mut harness = skinned(playing(on_playlist()), |_| {});
    harness.get_by_label("Eject").click();
    harness.get_by_label("Roll the window up").click();
    harness.get_by_label("Close mini player").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ShowMainWindow
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::ToggleShade)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleMiniPlayer
    )));
}

#[test]
fn the_skin_opens_its_equalizer_and_playlist_under_it() {
    let mut harness = skinned(playing(on_playlist()), |_| {});
    harness.get_by_label("Equalizer").click();
    harness.get_by_label("Playlist").click();
    harness.get_by_label("Spectrum analyser").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::ToggleEqualizer)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::TogglePlaylist)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::CycleAnalyser)
    )));
}

#[test]
fn the_skins_equalizer_is_the_apps_own() {
    let mut harness = skinned(playing(on_playlist()), |settings| {
        settings.skin_equalizer = true;
    });
    harness.get_by_label("Equalizer on").click();
    harness.get_by_label("Reset the equalizer").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Equalizer(crate::equalizer::Ask::On(true))
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Equalizer(crate::equalizer::Ask::Reset)
    )));
}

#[test]
fn a_double_click_in_the_skins_playlist_plays_from_that_song() {
    let mut state = playing(on_playlist());
    if let Some(playback) = &mut state.playback {
        playback.session.queue.items = vec![
            track("a", "First song"),
            track("b", "Second song"),
            track("c", "Third song"),
        ];
    }
    let mut harness = skinned(state, |settings| settings.skin_playlist = true);
    let row = harness.get_by_label("2. Second song");
    row.click();
    row.click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::JumpTo(1)
    )));
}

#[test]
fn a_rolled_up_skin_keeps_a_little_transport() {
    let mut harness = skinned(playing(on_playlist()), |settings| {
        settings.skin_shaded = true;
    });
    harness.get_by_label("Next").click();
    harness.get_by_label("Roll the window down").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::Next)));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::ToggleShade)
    )));
}

#[test]
fn a_skin_with_nothing_playing_still_draws_and_leads_back() {
    let mut harness = skinned(state(), |settings| {
        settings.skin_equalizer = true;
        settings.skin_playlist = true;
    });
    harness.get_by_label("Stop").click();
    harness.get_by_label("Open app").click();
    harness.run();
    let asked_for: Vec<&Action> = harness.state().actions.iter().collect();
    assert!(matches!(asked_for[..], [Action::ShowMainWindow]));
}

#[test]
fn settings_offer_the_skins_and_the_ways_to_get_more() {
    let mut state = state();
    state.nav.open(Page::Settings);
    // The section is far down the page: held where it can be reached.
    state.held_scroll = Some(1500.0);
    state.skins = vec![skins::Choice {
        file: "Base 2.91.wsz".into(),
    }];
    let mut harness = harness(state);
    harness.get_by_label("Add a skin…").click();
    harness.get_by_label("Get skins").click();
    harness.get_by_label("Classic").click();
    harness.get_by_label("Base 2.91").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::Wear(Some(file))) if file == "Base 2.91.wsz"
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::Wear(Some(file))) if file == skins::BUILT_IN
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::Pick)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::OpenMuseum)
    )));
}

#[test]
fn a_skin_that_is_not_a_rectangle_gives_its_window_a_shape() {
    use super::super::mini::skinned::shape;
    use crate::skin::Skin;

    let image = image::RgbImage::from_pixel(275, 116, image::Rgb([1, 2, 3]));
    let mut main = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut main, image::ImageFormat::Png)
        .expect("a picture");
    // Everything but a strip ten pixels wide down the left edge.
    let region = b"[Normal]\nNumPoints=4\nPointList=10,0, 275,0, 275,116, 10,116\n";
    let archive = crate::skin::zip::write(&[
        ("main.bmp", main.get_ref(), true),
        ("region.txt", region, false),
    ]);
    let skin = Skin::from_archive("shaped", &archive).expect("a skin");

    let mut settings = Settings::default();
    let boxes = shape(&settings, &skin).expect("a shape");
    assert_eq!(boxes.len(), 116);
    assert_eq!(boxes[0], [10, 0, 275, 1]);
    assert_eq!(boxes[115], [10, 115, 275, 116]);
    // The playlist under it is a whole rectangle, further down.
    settings.skin_playlist = true;
    let boxes = shape(&settings, &skin).expect("a shape");
    assert_eq!(boxes.last(), Some(&[0, 116, 275, 232]));
    // Rolled up, this skin says nothing of the bar's shape: a rectangle.
    settings.skin_shaded = true;
    assert_eq!(shape(&settings, &skin), None);
    assert_eq!(shape(&Settings::default(), &Skin::builtin()), None);
}

#[test]
fn the_skins_playlist_rolls_up_and_its_buttons_open_menus() {
    let mut harness = skinned(playing(on_playlist()), |settings| {
        settings.skin_playlist = true;
    });
    harness.get_by_label("Roll the playlist up").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::TogglePlaylistShade)
    )));
    // Every button along the bottom is there to be pressed.
    for menu in ["Add", "Remove", "Select", "Miscellaneous", "List options"] {
        harness.get_by_label(menu);
    }

    let mut harness = skinned(playing(on_playlist()), |settings| {
        settings.skin_playlist = true;
        settings.skin_playlist_shaded = true;
    });
    harness.get_by_label("Roll the playlist down").click();
    harness.get_by_label("Close the playlist").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::TogglePlaylistShade)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::Skin(Ask::TogglePlaylist)
    )));
}

#[test]
fn the_skins_display_draws_the_wave_as_well_as_the_spectrum() {
    // Nothing to assert of the picture: that each look draws at all, with
    // and without a song, is what could go wrong.
    for (analyser, scope) in [(true, false), (true, true), (false, false)] {
        for state in [state(), playing(on_playlist())] {
            let mut harness = skinned(state, |settings| {
                settings.skin_analyser = analyser;
                settings.skin_scope = scope;
            });
            harness.get_by_label("Analyser display").click();
            harness.run();
            assert!(asked(&harness, |action| matches!(
                action,
                Action::Skin(Ask::CycleAnalyser)
            )));
        }
    }
}

#[test]
fn settings_open_milkdrop_and_fetch_its_presets() {
    use crate::milkdrop::Ask as MilkDrop;
    let mut state = state();
    state.nav.open(Page::Settings);
    state.held_scroll = Some(1700.0);
    let mut harness = harness(state);
    harness.get_by_label("Open MilkDrop").click();
    harness.get_by_label("Get presets").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MilkDrop(MilkDrop::Toggle)
    )));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::MilkDrop(MilkDrop::GetPresets(0))
    )));
}

#[test]
fn a_modern_skin_is_drawn_and_its_standard_controls_steer_playback() {
    let dir = std::env::temp_dir().join(format!("spotified-modern-view-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    std::fs::write(dir.join("Small.wal"), crate::skin::modern::tests::skin()).expect("a skin");
    let mut state = playing(on_playlist());
    state.mini_player = true;
    state.settings.mini_skin = Some("Small.wal".to_owned());
    let fixture = Fixture {
        state,
        actions: Vec::new(),
        themed: false,
    };
    let folder = dir.clone();
    let mut harness = Harness::builder()
        // The skin's own two hundred by eighty, and the harness's margin.
        .with_size(vec2(216.0, 96.0))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(16)
        .build_ui_state(
            move |ui, fixture: &mut Fixture| {
                if !fixture.themed {
                    theme::install(ui.ctx(), &fixture.state.palette);
                    fixture.state.skin = skins::wear(ui.ctx(), "Small.wal", &folder).ok();
                    fixture.themed = true;
                    return;
                }
                super::super::mini::show(&fixture.state, ui, &mut fixture.actions);
            },
            fixture,
        );
    harness.run();
    let worn = harness.state().state.worn_skin().expect("the skin, worn");
    let size = super::super::mini::skinned::window_size(worn, &Settings::default(), 1.0);
    assert_eq!(size, vec2(200.0, 80.0));
    harness.get_by_label("Play").click();
    harness.get_by_label("Shuffle").click();
    harness.run();
    std::fs::remove_dir_all(&dir).expect("removed");
    // Play on a song that is playing starts it over, as in a classic skin.
    assert!(asked(&harness, |action| matches!(action, Action::Seek(0))));
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ToggleShuffle
    )));
}
