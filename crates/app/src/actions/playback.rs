//! Playing, and the controls that act on what is playing.

use std::time::Instant;

use spotified_client::models::Track;
use spotified_client::session::{Command, PlayState, Projection};

use super::{Action, Effect};
use crate::backend::Request;
use crate::settings::RightPanel;
use crate::state::{Loadable, MiniPanel, Notice, Playback, State, TrackLyrics};

/// Asks for the lyrics of what is playing, if the lyrics panel is open and
/// they are not already here or on their way.
pub(super) fn want_lyrics(state: &mut State) -> Vec<Effect> {
    let in_mini = state.mini_player && state.mini_panel == MiniPanel::Lyrics;
    if state.settings.panel != RightPanel::Lyrics && !state.lyrics_fullscreen && !in_mini {
        return Vec::new();
    }
    let Some(track) = state.playback.as_ref().and_then(Playback::current) else {
        return Vec::new();
    };
    if state.lyrics.track_id == track.id && !state.lyrics.words.needs_fetch() {
        return Vec::new();
    }
    state.lyrics = TrackLyrics {
        track_id: track.id.clone(),
        words: Loadable::Loading,
    };
    vec![Effect::Fetch(Request::Lyrics(Box::new(track.clone())))]
}

/// Adds tracks to the queue, after what is playing or at the end, and says
/// so. With nothing queued there is nothing to add to, so they play.
pub(super) fn enqueue(state: &mut State, tracks: Vec<Track>, next: bool) -> Vec<Effect> {
    let Some(first) = tracks.first() else {
        return Vec::new();
    };
    let queue = state
        .playback
        .as_ref()
        .map(|playback| &playback.session.queue)
        .filter(|queue| !queue.items.is_empty());
    let Some(queue) = queue else {
        return vec![Effect::Command(Command::Play {
            origin: first.title.clone(),
            tracks,
            start_index: 0,
        })];
    };
    let at = next.then_some(queue.index + 1);
    let what = match tracks.len() {
        1 => first.title.clone(),
        count => format!("{count} songs"),
    };
    state.toast(if next {
        format!("{what} will play next")
    } else {
        format!("{what} added to queue")
    });
    vec![Effect::Command(Command::Enqueue { tracks, at })]
}

/// The controls that act on the session as it stands. Each shows its result
/// at once, ahead of the core's answer, so nothing waits on a round trip;
/// the answer then replaces the guess.
pub(super) fn control(state: &mut State, action: Action) -> Vec<Effect> {
    let loudest = state.settings.max_volume();
    let Some(playback) = &mut state.playback else {
        return Vec::new();
    };
    let now = Instant::now();
    // The relative forms are worked out against the session as it stands,
    // then handled as their absolute ones.
    let action = match action {
        Action::SetPlaying(playing) if playing == playback.wants_to_play() => return Vec::new(),
        Action::SetPlaying(_) => Action::TogglePlay,
        Action::SeekBy(delta_ms) => {
            let duration = playback.current().map_or(0, |track| track.duration_ms);
            let target = playback.position_ms(now).saturating_add_signed(delta_ms);
            Action::Seek(if duration > 0 {
                target.min(duration)
            } else {
                target
            })
        }
        Action::VolumeBy(delta) => Action::SetVolume(playback.session.volume + delta),
        other => other,
    };
    let command = match action {
        Action::TogglePlay => {
            // Hold the position where it is, so the bar neither jumps back
            // nor runs on while the answer is on its way.
            playback.session.position_ms = playback.position_ms(now);
            playback.received = now;
            playback.session.state = if playback.wants_to_play() {
                PlayState::Paused
            } else {
                PlayState::Playing
            };
            Command::Toggle
        }
        Action::Seek(position_ms) => {
            playback.session.position_ms = position_ms;
            playback.received = now;
            Command::Seek(position_ms)
        }
        Action::SetVolume(volume) => {
            // Past 100% only with boost on.
            let volume = volume.clamp(0.0, loudest);
            playback.session.volume = volume;
            Command::SetVolume(volume)
        }
        Action::ToggleMute => {
            let volume = if playback.session.volume > 0.0 {
                state.volume_before_mute = playback.session.volume;
                0.0
            } else {
                state.volume_before_mute.max(0.05)
            };
            playback.session.volume = volume;
            Command::SetVolume(volume)
        }
        Action::ToggleShuffle => {
            playback.session.shuffle = !playback.session.shuffle;
            Command::SetShuffle(playback.session.shuffle)
        }
        Action::CycleRepeat => {
            playback.session.repeat = playback.next_repeat();
            Command::SetRepeat(playback.session.repeat)
        }
        _ => return Vec::new(),
    };
    vec![Effect::Command(command)]
}

/// Plays a song, then songs like it. The song already playing is not
/// started over: a click on it only sets it going again if it had stopped.
pub(super) fn start_radio(state: &mut State, track: Track) -> Vec<Effect> {
    // In a room the song and its radio are the room's, not this player's.
    if state.together.in_room() {
        return super::together::start_radio(state, track);
    }
    let current = state
        .playback
        .as_ref()
        .filter(|playback| playback.current().is_some_and(|now| now.id == track.id));
    if let Some(playback) = current {
        return if playback.wants_to_play() {
            Vec::new()
        } else {
            vec![Effect::Command(Command::Toggle)]
        };
    }
    vec![Effect::Fetch(Request::StartRadio {
        device_id: state.settings.device_id.clone(),
        track: Box::new(track),
    })]
}

/// The core's session changed: what is playing is replaced by what it says,
/// and what stands in the way of playback is said or unsaid.
pub(super) fn session_changed(state: &mut State, projection: Projection) -> Vec<Effect> {
    let was_offline = state
        .playback
        .as_ref()
        .is_some_and(|playback| playback.offline);
    let playback = Playback {
        session: projection.state,
        received: Instant::now(),
        offline: projection.offline,
        following_room: projection.following_room,
        room_length: projection
            .room
            .as_ref()
            .filter(|room| room.duration_ms > 0)
            .map(|room| (room.entry.clone(), room.duration_ms)),
        room_ended: projection
            .room
            .filter(|room| room.ended)
            .map(|room| room.entry),
        speed: state.speed(),
    };
    let paused = !playback.wants_to_play();
    let offline = playback.offline;
    state.playback = Some(playback);
    note_connection(state, was_offline, offline, paused);
    want_lyrics(state)
}

/// Says that the connection has gone for as long as it has, unless the
/// listener dismissed it, and never over a different notice.
fn note_connection(state: &mut State, was_offline: bool, offline: bool, paused: bool) {
    let about_connection = matches!(state.notice, None | Some(Notice::Offline { .. }));
    if offline {
        if !was_offline {
            state.offline_dismissed = false;
        }
        if !state.offline_dismissed && about_connection {
            state.notice = Some(Notice::Offline { paused });
        }
        return;
    }
    if was_offline {
        state.offline_dismissed = false;
        if matches!(state.notice, Some(Notice::Offline { .. })) {
            state.notice = None;
        }
    }
}

/// Gives the window, and the screen, to what is playing, or takes them
/// back. There must be something playing to give them to.
pub(super) fn fullscreen_player(state: &mut State, action: Action) -> Vec<Effect> {
    let on = match action {
        Action::SetFullscreenPlayer(on) => on,
        _ => !state.fullscreen_player,
    };
    let has_track = state
        .playback
        .as_ref()
        .and_then(Playback::current)
        .is_some();
    let on = on && has_track;
    if on == state.fullscreen_player {
        return Vec::new();
    }
    state.fullscreen_player = on;
    // Closed over full-screen lyrics, the screen stays theirs.
    vec![Effect::SetFullscreen(on || state.lyrics_fullscreen)]
}
