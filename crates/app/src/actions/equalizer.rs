//! What the equalizer's panel asks for: the switch, the sliders, the
//! curves to start from and the ones saved under a name.

use super::Effect;
use crate::equalizer::{self, Ask, MOST_SAVED, Naming, Saved, stepped};
use crate::state::State;

/// The equalizer changed in a way that is heard, and is to be kept.
fn heard() -> Vec<Effect> {
    vec![Effect::SaveSettings, Effect::ApplyEqualizer]
}

pub(super) fn asked(state: &mut State, ask: Ask) -> Vec<Effect> {
    let settings = &mut state.settings;
    match ask {
        Ask::On(on) => {
            settings.equalizer_on = on;
            heard()
        }
        // A drag sends one of these for every step it passes. They are
        // heard as they come and written down once, when the hand lets go.
        Ask::Band(band, decibels) => {
            let Some(gain) = settings.equalizer.get_mut(band) else {
                return Vec::new();
            };
            *gain = stepped(decibels);
            // Moving a slider is a wish to hear it.
            settings.equalizer_on = true;
            vec![Effect::ApplyEqualizer]
        }
        Ask::Preamp(decibels) => {
            settings.equalizer_preamp = stepped(decibels);
            settings.equalizer_on = true;
            vec![Effect::ApplyEqualizer]
        }
        Ask::Keep => vec![Effect::SaveSettings],
        Ask::Headroom(on) => {
            settings.equalizer_headroom = on;
            heard()
        }
        Ask::Curve(gains) => {
            settings.equalizer = gains.map(stepped);
            settings.equalizer_on = true;
            heard()
        }
        // The switch is left as it is: a reset is not a wish for silence
        // from the equalizer, nor to hear a flat one.
        Ask::Reset => {
            settings.equalizer = [0.0; 10];
            settings.equalizer_preamp = 0.0;
            heard()
        }
        Ask::Name => {
            state.equalizer_naming = Some(Naming::default());
            Vec::new()
        }
        Ask::Rename(name) => {
            state.equalizer_naming = Some(Naming {
                renaming: Some(name.clone()),
                name,
            });
            Vec::new()
        }
        Ask::Typed(name) => {
            if let Some(naming) = &mut state.equalizer_naming {
                naming.name = name.chars().take(equalizer::NAME_LENGTH).collect();
            }
            Vec::new()
        }
        Ask::Cancel => {
            state.equalizer_naming = None;
            Vec::new()
        }
        Ask::Save => save(state),
        Ask::Delete(name) => {
            let saved = &mut settings.equalizer_presets;
            let before = saved.len();
            saved.retain(|saved| saved.name != name);
            if saved.len() == before {
                return Vec::new();
            }
            // The sliders stay where they are: the curve is still heard,
            // and is now nobody's.
            state.toast(format!("Deleted “{name}”"));
            vec![Effect::SaveSettings]
        }
    }
}

/// Saves under the name typed: the curve as it stands, or the saved curve
/// being renamed. A name that cannot be used is said to be, and the field
/// stays for another.
fn save(state: &mut State) -> Vec<Effect> {
    let Some(naming) = state.equalizer_naming.clone() else {
        return Vec::new();
    };
    let name = equalizer::tidy_name(&naming.name);
    // Nothing typed yet: the field stays for a name.
    if name.is_empty() {
        return Vec::new();
    }
    if equalizer::is_built_in(&name) {
        state.toast_error(format!(
            "“{name}” is a built-in preset. Choose another name."
        ));
        return Vec::new();
    }
    let curve = state.settings.equalizer;
    let saved = &mut state.settings.equalizer_presets;
    let taken = saved.iter().position(|saved| saved.name == name);
    match naming.renaming {
        Some(old) if old == name => {}
        Some(old) => {
            if taken.is_some() {
                state.toast_error(format!("There is already a preset called “{name}”."));
                return Vec::new();
            }
            if let Some(renamed) = saved.iter_mut().find(|saved| saved.name == old) {
                renamed.name = name;
            }
        }
        // Saving under a name already used is how a saved curve is
        // changed: it takes the sliders as they now are.
        None => match taken {
            Some(index) => saved[index].gains = curve,
            None if saved.len() >= MOST_SAVED => {
                state.toast_error("That is as many presets as can be saved. Delete one first.");
                return Vec::new();
            }
            None => {
                saved.push(Saved {
                    name: name.clone(),
                    gains: curve,
                });
                state.toast(format!("Saved “{name}”"));
            }
        },
    }
    state.equalizer_naming = None;
    vec![Effect::SaveSettings]
}
