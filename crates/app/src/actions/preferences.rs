//! The Settings page's switches, and the few settings changed from
//! elsewhere: the speed from the player bar, the time shown at the end of
//! the seek bar.

use std::time::Instant;

use spotified_audio::eq::RANGE_DB as EQ_RANGE_DB;
use spotified_client::session::Command;

use super::{Action, Effect, MAX_CROSSFADE_SECONDS};
use crate::settings::{self, CACHE_SIZES_MB};
use crate::state::State;

/// What changing a setting the engine or the core acts on calls for.
fn heard() -> Vec<Effect> {
    vec![Effect::SaveSettings, Effect::ApplyAudioSettings]
}

pub(super) fn preferences(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::SetEqualizerOn(on) => {
            state.settings.equalizer_on = on;
            heard()
        }
        Action::SetEqualizerBand(band, decibels) => {
            let Some(gain) = state.settings.equalizer.get_mut(band) else {
                return Vec::new();
            };
            *gain = decibels.clamp(-EQ_RANGE_DB, EQ_RANGE_DB);
            // Moving a slider is a wish to hear it.
            state.settings.equalizer_on = true;
            heard()
        }
        Action::SetEqualizer(gains) => {
            state.settings.equalizer = gains;
            state.settings.equalizer_on = true;
            heard()
        }
        Action::SetVisualizer(on) => {
            state.settings.visualizer = on;
            heard()
        }
        Action::SetCrossfade(seconds) => {
            state.settings.crossfade_seconds = seconds.min(MAX_CROSSFADE_SECONDS);
            heard()
        }
        Action::SetNormaliseVolume(on) => {
            state.settings.normalise_volume = on;
            heard()
        }
        Action::SetVolumeLevel(level) => {
            state.settings.volume_level = level;
            heard()
        }
        Action::SetGapless(on) => {
            state.settings.gapless = on;
            heard()
        }
        Action::SetAutoplay(on) => {
            state.settings.autoplay = on;
            heard()
        }
        Action::SetResumeOnLaunch(on) => {
            state.settings.resume_on_launch = on;
            heard()
        }
        Action::SetReportToYouTube(on) => {
            state.settings.report_to_youtube = on;
            heard()
        }
        Action::SetCacheSize(megabytes) => {
            // Only the sizes on offer: the core is not to be told of another.
            if !CACHE_SIZES_MB.contains(&megabytes) {
                return Vec::new();
            }
            state.settings.cache_max_mb = megabytes;
            heard()
        }
        Action::SetVolumeBoost(on) => {
            state.settings.volume_boost = on;
            let mut effects = vec![Effect::SaveSettings];
            // Turning it off brings a boosted level back to 100%.
            if let Some(playback) = &mut state.playback
                && !on
                && playback.session.volume > 1.0
            {
                playback.session.volume = 1.0;
                effects.push(Effect::Command(Command::SetVolume(1.0)));
            }
            effects
        }
        // Read at launch only: turning it on later does not reach back to
        // a launch that has passed.
        Action::SetContinueFromYouTubeMusic(on) => {
            state.settings.continue_from_youtube_music = on;
            vec![Effect::SaveSettings]
        }
        Action::SetReduceMotion(on) => {
            state.settings.reduce_motion = on;
            vec![Effect::SaveSettings]
        }
        Action::ToggleRemainingTime => {
            state.settings.remaining_time = !state.settings.remaining_time;
            vec![Effect::SaveSettings]
        }
        Action::SetSpeed(speed) => set_speed(state, speed),
        Action::ResetPreferences => {
            let framed = state.settings.system_title_bar;
            state.settings.reset_preferences();
            let mut effects = heard();
            if framed != state.settings.system_title_bar {
                effects.push(Effect::SetDecorations(state.settings.system_title_bar));
            }
            // A boosted level has no boost to stand on any more.
            if let Some(playback) = &mut state.playback
                && playback.session.volume > 1.0
            {
                playback.session.volume = 1.0;
                effects.push(Effect::Command(Command::SetVolume(1.0)));
            }
            hold_position(state);
            state.toast("Preferences reset");
            effects
        }
        _ => Vec::new(),
    }
}

/// Chooses how fast playback runs. A Listen Together room keeps everyone on
/// one clock, so in one the choice is refused, and said to be.
fn set_speed(state: &mut State, speed: f32) -> Vec<Effect> {
    if state.speed_pinned() {
        state.toast("Listen Together keeps everyone at normal speed.");
        return Vec::new();
    }
    let speed = settings::clamp_speed(speed);
    if speed == state.settings.playback_speed {
        return Vec::new();
    }
    state.settings.playback_speed = speed;
    hold_position(state);
    heard()
}

/// Takes the position again at the speed now in force, so the progress bar
/// carries on from where it is and does not jump.
pub(super) fn hold_position(state: &mut State) {
    let speed = state.speed();
    if let Some(playback) = &mut state.playback {
        let now = Instant::now();
        playback.session.position_ms = playback.position_ms(now);
        playback.received = now;
        playback.speed = speed;
    }
}
