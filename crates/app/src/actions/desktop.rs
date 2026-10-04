//! What the desktop around the window asks for: the flyout by the tray
//! icon, the problem report, and starting with Windows.

use std::time::{Duration, Instant};

use super::{Action, Effect};
use crate::report;
use crate::state::{Flyout, State};

/// A click on the tray icon while the flyout is open takes the flyout's
/// focus first, which closes it, and then arrives here as a click that
/// would open it straight back up. A click this soon after it closed was
/// aimed at closing it.
const REOPEN_AFTER: Duration = Duration::from_millis(250);

pub(super) fn desktop(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::ToggleFlyout { position, now } => {
            let just_closed = state
                .flyout_closed
                .is_some_and(|closed| now.saturating_duration_since(closed) < REOPEN_AFTER);
            if state.flyout.is_some() {
                hide_flyout(state);
            } else if !just_closed {
                state.flyout = Some(Flyout {
                    position,
                    opened: now,
                });
            }
            Vec::new()
        }
        Action::HideFlyout => {
            hide_flyout(state);
            Vec::new()
        }
        Action::ShowMainWindow => {
            // The window is what was wanted; the flyout has done its part.
            hide_flyout(state);
            vec![Effect::ShowMainWindow]
        }
        Action::SaveReport => {
            if state.report == report::Status::Working {
                return Vec::new();
            }
            state.report = report::Status::Working;
            vec![Effect::SaveReport]
        }
        Action::ReportSaved(result) => {
            state.report = match result {
                Ok(zip) => report::Status::Saved(zip),
                Err(reason) => report::Status::Failed(reason),
            };
            Vec::new()
        }
        Action::StartAtLoginKnown(on) => {
            state.starts_at_login = on;
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn hide_flyout(state: &mut State) {
    if state.flyout.take().is_some() {
        state.flyout_closed = Some(Instant::now());
    }
}
