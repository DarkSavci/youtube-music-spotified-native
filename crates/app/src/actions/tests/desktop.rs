//! The flyout by the tray icon, the problem report, and the core going away.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::*;
use crate::report;

fn click(state: &mut State, now: Instant) {
    let position = [100.0, 200.0];
    apply(state, Action::ToggleFlyout { position, now });
}

#[test]
fn a_click_on_the_tray_icon_opens_the_flyout_and_another_closes_it() {
    let mut state = state();
    let now = Instant::now();
    click(&mut state, now);
    assert_eq!(
        state.flyout.map(|flyout| flyout.position),
        Some([100.0, 200.0])
    );
    click(&mut state, now + Duration::from_secs(2));
    assert_eq!(state.flyout, None);
}

#[test]
fn the_click_that_closed_the_flyout_does_not_open_it_again() {
    let mut state = state();
    click(&mut state, Instant::now());
    // Clicking the icon takes the flyout's focus, which closes it, and
    // then the click itself arrives.
    apply(&mut state, Action::HideFlyout);
    click(&mut state, Instant::now());
    assert_eq!(state.flyout, None);
    // A click a moment later was meant to open it.
    click(&mut state, Instant::now() + Duration::from_secs(1));
    assert!(state.flyout.is_some());
}

#[test]
fn opening_the_app_from_the_flyout_puts_the_flyout_away() {
    let mut state = state();
    click(&mut state, Instant::now());
    assert_eq!(
        apply(&mut state, Action::ShowMainWindow),
        [Effect::ShowMainWindow]
    );
    assert_eq!(state.flyout, None);
}

#[test]
fn a_report_is_made_once_at_a_time_and_says_where_it_went() {
    let mut state = state();
    assert_eq!(apply(&mut state, Action::SaveReport), [Effect::SaveReport]);
    assert_eq!(state.report, report::Status::Working);
    assert!(apply(&mut state, Action::SaveReport).is_empty());

    let zip = PathBuf::from("report.zip");
    apply(&mut state, Action::ReportSaved(Ok(zip.clone())));
    assert_eq!(state.report, report::Status::Saved(zip));
    // Another can be made after it.
    assert_eq!(apply(&mut state, Action::SaveReport), [Effect::SaveReport]);
    apply(
        &mut state,
        Action::ReportSaved(Err("tar ended with 1".into())),
    );
    assert_eq!(
        state.report,
        report::Status::Failed("tar ended with 1".into())
    );
}

#[test]
fn the_switch_shows_what_the_system_says_of_starting_with_it() {
    let mut state = state();
    apply(&mut state, Action::StartAtLoginKnown(true));
    assert!(state.starts_at_login);
}

#[test]
fn a_core_that_stops_while_in_use_is_said_so() {
    let mut state = ready();
    let stopped = CoreStatus::Failed("The playback service stopped.".into());
    assert!(apply(&mut state, Action::CoreChanged(stopped)).is_empty());
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
    // One that never started says so on the page, not in a toast as well.
    let mut state = super::state();
    let failed = CoreStatus::Failed("It could not start.".into());
    apply(&mut state, Action::CoreChanged(failed));
    assert!(state.toasts.is_empty());
}
