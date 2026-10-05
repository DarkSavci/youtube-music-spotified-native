//! The skin's main window, whole and rolled up to its title bar.

use eframe::egui::{self, Sense, ViewportCommand};
use spotified_client::session::Repeat;

use super::super::super::{format, volume};
use super::display::{analyser, marquee, rates, status, time_display};
use super::{Slid, View, balance_of, menu, options_menu, play, stop, stopped, times};
use crate::actions::Action;
use crate::skin::layout::{self, Area};
use crate::skin::sprites;
use crate::skins::Ask;
use crate::state::{Page, Playback, State};

const WHOLE: Area = Area::new(0, 0, layout::WINDOW_WIDTH, layout::WINDOW_HEIGHT);
const SHADE: Area = Area::new(0, 0, layout::WINDOW_WIDTH, layout::SHADE_HEIGHT);

/// The main window as it usually is. Returns whether the analyser is still
/// moving.
pub(super) fn full_window(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) -> bool {
    view.sprite(sprites::MAIN_BACKGROUND, WHOLE);
    title_bar(state, view, actions, focused);
    clutter_bar(state, view, actions, playback);
    status(view, playback);
    time_display(state, view, actions, playback);
    let moving = analyser(state, view, actions, playback);
    marquee(state, view, playback);
    rates(state, view, playback);
    sliders(state, view, actions, playback);
    window_buttons(state, view, actions);
    transport(view, actions, playback);
    shuffle_repeat(view, actions, playback);
    if view
        .interact(layout::ABOUT, "Open app", Sense::click())
        .on_hover_text("Open app")
        .clicked()
    {
        actions.push(Action::ShowMainWindow);
    }
    moving
}

/// The four buttons every title bar has, whole or rolled up.
fn title_buttons(state: &State, view: &mut View<'_>, actions: &mut Vec<Action>, shaded: bool) {
    let logo = view.button(
        layout::OPTIONS_BUTTON,
        sprites::OPTIONS_BUTTON,
        sprites::OPTIONS_BUTTON_PRESSED,
        "Options",
    );
    let unit = view.unit;
    if !shaded {
        menu(egui::Popup::menu(&logo), view.skin(), unit, |ui| {
            options_menu(state, ui, actions, unit);
        });
    } else if logo.clicked() {
        // Rolled up there is no room for a menu: the window comes down.
        actions.push(Action::Skin(Ask::ToggleShade));
    }
    if view
        .button(
            layout::MINIMIZE_BUTTON,
            sprites::MINIMIZE_BUTTON,
            sprites::MINIMIZE_BUTTON_PRESSED,
            "Minimise",
        )
        .clicked()
    {
        let minimise = ViewportCommand::Minimized(true);
        view.ui.ctx().send_viewport_cmd(minimise);
    }
    let (normal, pressed, name) = if shaded {
        (
            sprites::UNSHADE_BUTTON,
            sprites::UNSHADE_BUTTON_PRESSED,
            "Roll the window down",
        )
    } else {
        (
            sprites::SHADE_BUTTON,
            sprites::SHADE_BUTTON_PRESSED,
            "Roll the window up",
        )
    };
    if view
        .button(layout::SHADE_BUTTON, normal, pressed, name)
        .on_hover_text(name)
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleShade));
    }
    if view
        .button(
            layout::CLOSE_BUTTON,
            sprites::CLOSE_BUTTON,
            sprites::CLOSE_BUTTON_PRESSED,
            "Close mini player",
        )
        .on_hover_text("Close mini player")
        .clicked()
    {
        actions.push(Action::ToggleMiniPlayer);
    }
}

fn title_bar(state: &State, view: &mut View<'_>, actions: &mut Vec<Action>, focused: bool) {
    let bar = if focused {
        sprites::TITLE_BAR_ACTIVE
    } else {
        sprites::TITLE_BAR_INACTIVE
    };
    view.sprite(bar, layout::TITLE_BAR);
    let title = view.title_bar(layout::TITLE_BAR, "Title bar");
    if title.double_clicked() {
        actions.push(Action::Skin(Ask::ToggleShade));
    }
    let unit = view.unit;
    menu(egui::Popup::context_menu(&title), view.skin(), unit, |ui| {
        options_menu(state, ui, actions, unit);
    });
    title_buttons(state, view, actions, false);
}

/// The main window rolled up: the bar with the time in the small font, a
/// little transport, and a little seek bar, as Winamp's shade mode had.
pub(super) fn shade_bar(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    focused: bool,
    playback: Option<&Playback>,
) {
    let bar = if focused {
        sprites::SHADE_BAR_ACTIVE
    } else {
        sprites::SHADE_BAR_INACTIVE
    };
    view.sprite(bar, SHADE);
    let title = view.title_bar(SHADE, "Title bar");
    if title.double_clicked() {
        actions.push(Action::Skin(Ask::ToggleShade));
    }
    title_buttons(state, view, actions, true);

    // The time, in the small font; a click counts down instead.
    if view
        .interact(layout::SHADE_TIME, "Time", Sense::click())
        .clicked()
    {
        actions.push(Action::ToggleRemainingTime);
    }
    let sounding = playback.filter(|_| !stopped(playback));
    if let Some(playback) = sounding {
        let (position, duration) = times(view, playback);
        let remaining = state.settings.remaining_time && duration > 0;
        let (sign, shown) = if remaining {
            ('-', duration.saturating_sub(position))
        } else {
            (' ', position)
        };
        let text = format!("{sign}{}", format::duration(shown));
        view.text(&text, layout::SHADE_TIME);
    }

    // The little transport is painted into the bar, so these only listen.
    type Press = fn(&mut Vec<Action>, Option<&Playback>);
    let little: [(&str, Area, Press); 6] = [
        ("Previous", layout::SHADE_PREVIOUS, |actions, _| {
            actions.push(Action::Previous);
        }),
        ("Play", layout::SHADE_PLAY, play),
        ("Pause", layout::SHADE_PAUSE, |actions, playback| {
            if playback.is_some() {
                actions.push(Action::TogglePlay);
            }
        }),
        ("Stop", layout::SHADE_STOP, stop),
        ("Next", layout::SHADE_NEXT, |actions, _| {
            actions.push(Action::Next)
        }),
        ("Open app", layout::SHADE_EJECT, |actions, _| {
            actions.push(Action::ShowMainWindow);
        }),
    ];
    for (name, area, press) in little {
        if view.interact(area, name, Sense::click()).clicked() {
            press(actions, playback);
        }
    }

    // The little seek bar.
    view.sprite(sprites::SHADE_POSITION_TRACK, layout::SHADE_POSITION);
    let Some(playback) = sounding else {
        return;
    };
    let duration = playback.current().map_or(0, |track| track.duration_ms);
    if duration == 0 {
        return;
    }
    let area = layout::SHADE_POSITION;
    let (_, slid) = view.slider(area, "Seek, rolled up", 3);
    if let Slid::Released(fraction) = slid {
        actions.push(Action::Seek((fraction * duration as f32) as u64));
    }
    let (position, _) = times(view, playback);
    let fraction = (position as f32 / duration as f32).clamp(0.0, 1.0);
    let thumb = if fraction < 1.0 / 3.0 {
        sprites::SHADE_POSITION_THUMB_LEFT
    } else if fraction < 2.0 / 3.0 {
        sprites::SHADE_POSITION_THUMB
    } else {
        sprites::SHADE_POSITION_THUMB_RIGHT
    };
    let x = area.x + (fraction * (area.width - 3) as f32).round() as u32;
    view.sprite_at(thumb, x, area.y);
}

/// The O A I D V strip: options, always on top, info, double size, and the
/// analyser. Each lights while held; A, D and V stay lit while on.
fn clutter_bar(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
) {
    let settings = &state.settings;
    view.sprite(sprites::CLUTTER_BAR, layout::CLUTTER_BAR);
    let options = view.lamp_button(layout::CLUTTER_O, sprites::CLUTTER_O_LIT, false, "Menu");
    let unit = view.unit;
    menu(egui::Popup::menu(&options), view.skin(), unit, |ui| {
        options_menu(state, ui, actions, unit);
    });
    let on_top = settings.mini_on_top;
    if view
        .lamp_button(
            layout::CLUTTER_A,
            sprites::CLUTTER_A_LIT,
            on_top,
            "Always on top",
        )
        .on_hover_text("Always on top")
        .clicked()
    {
        actions.push(Action::SetMiniOnTop(!on_top));
    }
    if view
        .lamp_button(
            layout::CLUTTER_I,
            sprites::CLUTTER_I_LIT,
            false,
            "Open the album",
        )
        .on_hover_text("Open the album in the app")
        .clicked()
    {
        let album = playback
            .and_then(Playback::current)
            .and_then(|track| track.album.as_ref())
            .filter(|album| !album.id.is_empty());
        if let Some(album) = album {
            actions.push(Action::Open(Page::Album(album.id.clone())));
        }
        actions.push(Action::ShowMainWindow);
    }
    // D was Winamp's double size: here it is between the skin's own size
    // and twice it, with the larger ones in the menu.
    let doubled = settings.skin_scale > 1;
    if view
        .lamp_button(
            layout::CLUTTER_D,
            sprites::CLUTTER_D_LIT,
            doubled,
            "Double size",
        )
        .on_hover_text("Double size")
        .clicked()
    {
        let scale = if doubled { 1 } else { 2 };
        actions.push(Action::Skin(Ask::Scale(scale)));
    }
    if view
        .lamp_button(
            layout::CLUTTER_V,
            sprites::CLUTTER_V_LIT,
            settings.skin_analyser,
            "Spectrum analyser",
        )
        .on_hover_text("Spectrum analyser")
        .clicked()
    {
        actions.push(Action::Skin(Ask::CycleAnalyser));
    }
}

fn sliders(
    state: &State,
    view: &mut View<'_>,
    actions: &mut Vec<Action>,
    playback: Option<&Playback>,
) {
    // Volume: the track is drawn at the level, the thumb rides on it. It
    // follows the drag as it happens: it is heard, not just seen.
    let most = state.settings.max_volume();
    let volume = playback.map_or(0.0, |playback| playback.session.volume);
    let level = (volume / most).clamp(0.0, 1.0);
    let (response, slid) = view.slider(layout::VOLUME, "Volume", 14);
    if let Slid::Dragging(to) | Slid::Released(to) = slid
        && (to - level).abs() >= 0.005
    {
        actions.push(Action::SetVolume(to * most));
    }
    if playback.is_some()
        && response.hovered()
        && let Some(step) = volume::wheel(view.ui)
    {
        actions.push(Action::VolumeBy(step));
    }
    let frame = (level * (sprites::SLIDER_FRAMES - 1) as f32).round() as u32;
    view.sprite(sprites::volume_frame(frame), layout::VOLUME);
    let thumb = if response.dragged() || response.is_pointer_button_down_on() {
        sprites::VOLUME_THUMB_PRESSED
    } else {
        sprites::VOLUME_THUMB
    };
    let x = layout::VOLUME.x + (level * layout::VOLUME_TRAVEL as f32).round() as u32;
    view.sprite_at(thumb, x, layout::VOLUME.y + 1);

    // Balance: one side turned down in the sound's path, with Winamp's
    // snap to the middle.
    let balance = state.settings.balance;
    let (response, slid) = view.slider(layout::BALANCE, "Balance", 14);
    match slid {
        Slid::Dragging(to) => actions.push(Action::Skin(Ask::Balance(balance_of(to)))),
        Slid::Released(to) => {
            actions.push(Action::Skin(Ask::Balance(balance_of(to))));
            actions.push(Action::Skin(Ask::Keep));
        }
        Slid::No => {}
    }
    let frame = (balance.abs() * (sprites::SLIDER_FRAMES - 1) as f32).round() as u32;
    view.sprite(sprites::balance_frame(frame), layout::BALANCE);
    let thumb = if response.dragged() || response.is_pointer_button_down_on() {
        sprites::BALANCE_THUMB_PRESSED
    } else {
        sprites::BALANCE_THUMB
    };
    let along = (balance + 1.0) / 2.0 * layout::BALANCE_TRAVEL as f32;
    let x = layout::BALANCE.x + along.round() as u32;
    view.sprite_at(thumb, x, layout::BALANCE.y + 1);

    // The seek bar. The thumb only exists while something plays, as in
    // Winamp, so an empty or stopped player has nothing to drag.
    view.sprite(sprites::POSITION_TRACK, layout::POSITION);
    let Some(playback) = playback.filter(|_| !stopped(playback)) else {
        return;
    };
    let duration = playback.current().map_or(0, |track| track.duration_ms);
    if duration == 0 {
        return;
    }
    let (response, slid) = view.slider(layout::POSITION, "Seek", 29);
    if let Slid::Released(fraction) = slid {
        actions.push(Action::Seek((fraction * duration as f32) as u64));
    }
    let (position, _) = times(view, playback);
    let fraction = (position as f32 / duration as f32).clamp(0.0, 1.0);
    let thumb = if response.dragged() || response.is_pointer_button_down_on() {
        sprites::POSITION_THUMB_PRESSED
    } else {
        sprites::POSITION_THUMB
    };
    let x = layout::POSITION.x + (fraction * layout::POSITION_TRAVEL as f32).round() as u32;
    view.sprite_at(thumb, x, layout::POSITION.y);
}

/// The EQ and PL toggles, each lit while its window hangs below.
fn window_buttons(state: &State, view: &mut View<'_>, actions: &mut Vec<Action>) {
    let (normal, pressed) = if state.settings.skin_equalizer {
        (sprites::EQ_ON, sprites::EQ_ON_PRESSED)
    } else {
        (sprites::EQ_OFF, sprites::EQ_OFF_PRESSED)
    };
    if view
        .button(layout::EQ_BUTTON, normal, pressed, "Equalizer")
        .clicked()
    {
        actions.push(Action::Skin(Ask::ToggleEqualizer));
    }
    let (normal, pressed) = if state.settings.skin_playlist {
        (sprites::PLAYLIST_ON, sprites::PLAYLIST_ON_PRESSED)
    } else {
        (sprites::PLAYLIST_OFF, sprites::PLAYLIST_OFF_PRESSED)
    };
    if view
        .button(layout::PLAYLIST_BUTTON, normal, pressed, "Playlist")
        .clicked()
    {
        actions.push(Action::Skin(Ask::TogglePlaylist));
    }
}

fn transport(view: &mut View<'_>, actions: &mut Vec<Action>, playback: Option<&Playback>) {
    let playing = playback.is_some_and(Playback::wants_to_play);
    if view
        .button(
            layout::PREVIOUS,
            sprites::PREVIOUS,
            sprites::PREVIOUS_PRESSED,
            "Previous",
        )
        .clicked()
    {
        actions.push(Action::Previous);
    }
    // Play sits pressed in while the music plays, pause while it waits.
    let face = if playing {
        sprites::PLAY_PRESSED
    } else {
        sprites::PLAY
    };
    if view
        .button(layout::PLAY, face, sprites::PLAY_PRESSED, "Play")
        .clicked()
    {
        play(actions, playback);
    }
    let face = if playback.is_some() && !playing && !stopped(playback) {
        sprites::PAUSE_PRESSED
    } else {
        sprites::PAUSE
    };
    if view
        .button(layout::PAUSE, face, sprites::PAUSE_PRESSED, "Pause")
        .clicked()
        && playback.is_some()
    {
        actions.push(Action::TogglePlay);
    }
    let face = if stopped(playback) {
        sprites::STOP_PRESSED
    } else {
        sprites::STOP
    };
    if view
        .button(layout::STOP, face, sprites::STOP_PRESSED, "Stop")
        .clicked()
    {
        stop(actions, playback);
    }
    if view
        .button(layout::NEXT, sprites::NEXT, sprites::NEXT_PRESSED, "Next")
        .clicked()
    {
        actions.push(Action::Next);
    }
    if view
        .button(
            layout::EJECT,
            sprites::EJECT,
            sprites::EJECT_PRESSED,
            "Eject",
        )
        .on_hover_text("Open app")
        .clicked()
    {
        actions.push(Action::ShowMainWindow);
    }
}

fn shuffle_repeat(view: &mut View<'_>, actions: &mut Vec<Action>, playback: Option<&Playback>) {
    let session = playback.map(|playback| &playback.session);
    let (normal, pressed) = if session.is_some_and(|session| session.shuffle) {
        (sprites::SHUFFLE_ON, sprites::SHUFFLE_ON_PRESSED)
    } else {
        (sprites::SHUFFLE_OFF, sprites::SHUFFLE_OFF_PRESSED)
    };
    if view
        .button(layout::SHUFFLE, normal, pressed, "Shuffle")
        .clicked()
    {
        actions.push(Action::ToggleShuffle);
    }
    // Winamp's repeat is on or off; here the lamp is lit for either kind,
    // and each press goes on to the next as it does everywhere else.
    let repeat = session.map_or(Repeat::Off, |session| session.repeat);
    let (normal, pressed) = if repeat != Repeat::Off {
        (sprites::REPEAT_ON, sprites::REPEAT_ON_PRESSED)
    } else {
        (sprites::REPEAT_OFF, sprites::REPEAT_OFF_PRESSED)
    };
    let hint = match repeat {
        Repeat::Off => "Repeat: off",
        Repeat::All => "Repeat: everything",
        Repeat::One => "Repeat: this song",
    };
    if view
        .button(layout::REPEAT, normal, pressed, "Repeat")
        .on_hover_text(hint)
        .clicked()
    {
        actions.push(Action::CycleRepeat);
    }
}
