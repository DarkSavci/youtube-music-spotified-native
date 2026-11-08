//! The Settings page's switches, and the few settings changed from
//! elsewhere: the speed from the player bar, the time shown at the end of
//! the seek bar.

use std::time::Instant;

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
        Action::SetNormaliseAlbums(on) => {
            state.settings.normalise_albums = on;
            heard()
        }
        Action::SetSoundPreset(preset) => {
            if state.speed_pinned() && state.settings.playback_speed != 1.0 {
                // A room's speed is the room's; the rest still applies.
                state.toast("Listen Together keeps everyone at normal speed.");
            }
            preset.apply(&mut state.settings);
            hold_position(state);
            let mut effects = heard();
            effects.push(Effect::ApplyEqualizer);
            effects
        }
        Action::SetStreamQuality(quality) => {
            state.settings.stream_quality = quality;
            heard()
        }
        Action::SetExclusiveOutput(on) => {
            state.settings.exclusive_output = on;
            heard()
        }
        Action::SetHeadphonesOn(on) => {
            state.settings.headphones_on = on;
            vec![Effect::SaveSettings, Effect::ApplyEqualizer]
        }
        Action::ForgetHeadphones => {
            state.settings.headphones = None;
            vec![Effect::SaveSettings, Effect::ApplyEqualizer]
        }
        Action::SearchHeadphones(query) => {
            state.headphones.query = query;
            // The list is fetched the first time anyone looks.
            if state.headphones.list.is_none() && !state.headphones.listing {
                state.headphones.listing = true;
                return vec![Effect::ListHeadphones];
            }
            Vec::new()
        }
        Action::HeadphonesListed(listed) => {
            state.headphones.listing = false;
            state.headphones.list = Some(listed);
            Vec::new()
        }
        Action::ChooseHeadphones(headphone) => {
            state.headphones.fetching = Some(headphone.name.clone());
            vec![Effect::FetchHeadphones(headphone)]
        }
        Action::HeadphonesFetched(fetched) => {
            state.headphones.fetching = None;
            match fetched {
                Ok(correction) => {
                    state.toast(format!("Correcting for {}", correction.name));
                    state.settings.headphones = Some(correction);
                    state.settings.headphones_on = true;
                    state.headphones.query.clear();
                    vec![Effect::SaveSettings, Effect::ApplyEqualizer]
                }
                Err(why) => {
                    state.toast_error(why);
                    Vec::new()
                }
            }
        }
        Action::SetOutputDevice(device) => {
            state.settings.output_device = device;
            heard()
        }
        Action::ListOutputDevices => vec![Effect::ListOutputDevices],
        Action::OutputDevicesListed(devices) => {
            state.output_devices = devices;
            Vec::new()
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
        Action::SetShowMusicVideos(on) => {
            state.settings.show_music_videos = on;
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
