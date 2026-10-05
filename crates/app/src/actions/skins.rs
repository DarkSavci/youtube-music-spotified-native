//! What is asked about the mini player's Winamp skins: which one it wears,
//! putting new ones in the folder, and how the skinned windows are laid out.

use super::Effect;
use crate::skin::layout;
use crate::skins::{Ask, SCALES};
use crate::state::{MiniOpened, State};

pub(super) fn asked(state: &mut State, ask: Ask) -> Vec<Effect> {
    let settings = &mut state.settings;
    match ask {
        // The skin itself is read on the next frame the mini player is
        // drawn, by the app, which has the folder and the window to hand.
        Ask::Wear(skin) => {
            let mut effects = vec![Effect::SaveSettings];
            // The balance is the skin's to set: with no skin there would
            // be no way to put it back.
            if skin.is_none() && settings.balance != 0.0 {
                settings.balance = 0.0;
                effects.push(Effect::ApplyBalance);
            }
            settings.mini_skin = skin;
            // Whoever chooses a skin wants to see it.
            if !state.mini_player {
                state.mini_player = true;
                state.mini_opened = MiniOpened {
                    size: state.settings.mini_size,
                    position: state.settings.mini_position,
                };
            }
            effects
        }
        Ask::Install(files) if files.is_empty() => Vec::new(),
        Ask::Install(files) => vec![Effect::InstallSkins(files)],
        Ask::Pick => vec![Effect::PickSkins],
        // A skin that cannot be read is not worn, so the mini player is
        // never left with nothing to draw.
        Ask::Failed(why) => {
            settings.mini_skin = None;
            let mut effects = vec![Effect::SaveSettings];
            if settings.balance != 0.0 {
                settings.balance = 0.0;
                effects.push(Effect::ApplyBalance);
            }
            state.toast_error(format!("That skin could not be used. {why}"));
            effects
        }
        Ask::OpenFolder => vec![Effect::OpenSkinsFolder],
        Ask::OpenMuseum => vec![Effect::OpenUrl(crate::skins::MUSEUM)],
        Ask::Reload => vec![Effect::ReloadSkins],
        Ask::Scale(scale) => {
            if !SCALES.contains(&scale) || settings.skin_scale == scale {
                return Vec::new();
            }
            settings.skin_scale = scale;
            vec![Effect::SaveSettings]
        }
        Ask::ToggleShade => {
            settings.skin_shaded = !settings.skin_shaded;
            vec![Effect::SaveSettings]
        }
        Ask::ToggleEqualizer => {
            settings.skin_equalizer = !settings.skin_equalizer;
            vec![Effect::SaveSettings]
        }
        Ask::ToggleEqualizerShade => {
            settings.skin_equalizer_shaded = !settings.skin_equalizer_shaded;
            vec![Effect::SaveSettings]
        }
        Ask::TogglePlaylist => {
            settings.skin_playlist = !settings.skin_playlist;
            vec![Effect::SaveSettings]
        }
        // Sent for every step of a drag, and written down by `Keep`.
        Ask::PlaylistHeight(height) => {
            settings.skin_playlist_height = layout::playlist_height(height);
            Vec::new()
        }
        Ask::TogglePlaylistShade => {
            settings.skin_playlist_shaded = !settings.skin_playlist_shaded;
            vec![Effect::SaveSettings]
        }
        // The spectrum, then the wave, then nothing, and round again.
        Ask::CycleAnalyser => {
            (settings.skin_analyser, settings.skin_scope) =
                match (settings.skin_analyser, settings.skin_scope) {
                    (true, false) => (true, true),
                    (true, true) => (false, false),
                    (false, _) => (true, false),
                };
            vec![Effect::SaveSettings]
        }
        // A drag sends one of these for every step it passes: heard as
        // they come, and written down once, by `Keep`.
        Ask::Balance(balance) => {
            let balance = crate::skins::held_balance(balance);
            if balance == settings.balance {
                return Vec::new();
            }
            settings.balance = balance;
            vec![Effect::ApplyBalance]
        }
        Ask::Keep => vec![Effect::SaveSettings],
    }
}

/// What is asked of MilkDrop: its window, and the presets it draws.
pub(super) fn milkdrop(state: &mut State, ask: crate::milkdrop::Ask) -> Vec<Effect> {
    use crate::milkdrop::{Ask, PACKS};
    match ask {
        // Shown as open at once; the window follows, or says why not.
        Ask::Toggle => {
            state.milkdrop.open = !state.milkdrop.open;
            if !state.milkdrop.open {
                return vec![Effect::CloseMilkDrop];
            }
            if state.milkdrop.presets == 0 && !state.milkdrop.fetching {
                state.toast("MilkDrop has no presets yet. Get some in Settings, under Appearance.");
            }
            vec![Effect::OpenMilkDrop]
        }
        Ask::Failed(why) => {
            state.milkdrop.open = false;
            state.toast_error(format!("MilkDrop could not open. {why}"));
            Vec::new()
        }
        Ask::Closed => {
            state.milkdrop.open = false;
            Vec::new()
        }
        Ask::GetPresets(pack) => {
            if state.milkdrop.fetching || pack >= PACKS.len() {
                return Vec::new();
            }
            state.milkdrop.fetching = true;
            vec![Effect::FetchPresets(pack)]
        }
        Ask::Fetched(result) => {
            state.milkdrop.fetching = false;
            match result {
                Ok(written) => state.toast(format!("{written} MilkDrop presets added")),
                Err(why) => state.toast_error(format!("The presets could not be fetched. {why}")),
            }
            Vec::new()
        }
        Ask::OpenFolder => vec![Effect::OpenMilkDropFolder],
    }
}
