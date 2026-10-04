//! The tray flyout, the accounts in the menu and in Settings, and the
//! problem report.

use spotified_client::models::Account;

use super::*;
use crate::accounts::{self, Accounts, SavedChannel};
use crate::report;

/// The flyout alone, in a window of its own size.
fn flyout_harness(state: State) -> Harness<'static, Fixture> {
    let fixture = Fixture {
        state,
        actions: Vec::new(),
        themed: false,
    };
    let mut harness = Harness::builder()
        .with_size(eframe::egui::Vec2::from(crate::views::flyout::SIZE))
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(16)
        .build_ui_state(
            |ui, fixture: &mut Fixture| {
                if !fixture.themed {
                    theme::install(ui.ctx(), &fixture.state.palette);
                    fixture.themed = true;
                    return;
                }
                crate::views::flyout::show(&fixture.state, ui, &mut fixture.actions);
            },
            fixture,
        );
    harness.run();
    harness
}

/// Ada in use, with a channel besides her own, and Grace saved.
fn with_accounts(mut state: State) -> (State, String) {
    let mut ada = accounts::new_account("Ada");
    ada.channels = vec![
        SavedChannel {
            id: String::new(),
            name: "Ada".into(),
            handle: "@ada".into(),
            ..SavedChannel::default()
        },
        SavedChannel {
            id: "1".into(),
            name: "Engines".into(),
            handle: "@engines".into(),
            ..SavedChannel::default()
        },
    ];
    let grace = accounts::new_account("Grace");
    let grace_id = grace.id.clone();
    let mut list = Accounts::default();
    list.add(grace);
    list.add(ada);
    state.accounts = list;
    state.account = Some(Account {
        name: "Ada".into(),
        ..Account::default()
    });
    (state, grace_id)
}

#[test]
fn the_flyout_has_the_transport_for_what_is_playing() {
    let mut harness = flyout_harness(playing(state()));
    for (label, wanted) in [
        ("Pause", "TogglePlay"),
        ("Next track", "Next"),
        ("Previous track", "Previous"),
        ("Shuffle", "ToggleShuffle"),
        ("Repeat: off", "CycleRepeat"),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert!(
            asked(&harness, |action| format!("{action:?}") == wanted),
            "{label} asks for {wanted}"
        );
    }
}

#[test]
fn the_flyout_offers_a_like_only_to_someone_signed_in() {
    let harness = flyout_harness(playing(state()));
    assert!(harness.query_by_label("Add to Liked Music").is_none());

    let (state, _) = with_accounts(playing(state()));
    let mut harness = flyout_harness(state);
    harness.get_by_label("Add to Liked Music").click();
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::ToggleLike(track) if track.id == "a")
    ));
}

#[test]
fn with_nothing_playing_the_flyout_leads_to_the_app() {
    let mut harness = flyout_harness(state());
    assert!(harness.query_by_label("Pause").is_none());
    assert!(harness.query_by_label("Play").is_none());
    harness.get_by_label("Open app").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ShowMainWindow
    )));
}

#[test]
fn escape_puts_the_flyout_away() {
    let mut harness = flyout_harness(playing(state()));
    harness.key_press(eframe::egui::Key::Escape);
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::HideFlyout
    )));
}

#[test]
fn the_account_menu_switches_account_and_channel() {
    let (state, grace) = with_accounts(state());
    let mut harness = harness(state);
    harness.get_by_label("Account").click();
    harness.run();
    harness.get_by_label("Account: Grace").click();
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::SwitchAccount(id) if *id == grace)
    ));

    harness.get_by_label("Account").click();
    harness.run();
    harness.get_by_label("Channel: Engines").click();
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::SwitchChannel(id) if id == "1")
    ));

    harness.get_by_label("Account").click();
    harness.run();
    harness.get_by_label("Add Google account").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::SignIn)));
}

#[test]
fn settings_lists_the_saved_accounts_with_what_can_be_done_with_each() {
    let (mut state, grace) = with_accounts(state());
    state.nav.open(Page::Settings);
    let mut harness = harness(state);
    harness.get_by_label("Use Grace").click();
    harness.run();
    assert!(asked(
        &harness,
        |action| matches!(action, Action::SwitchAccount(id) if *id == grace)
    ));
    harness.get_by_label("Remove Ada").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::AskRemoveAccount(_)
    )));
    // The account in use has no "use" of its own.
    assert!(harness.query_by_label("Use Ada").is_none());
    harness.get_by_label("Refresh channels").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::RefreshChannels
    )));
}

#[test]
fn signing_out_is_a_question_with_its_own_answer() {
    let (mut state, _) = with_accounts(state());
    state.dialog = Some(crate::state::Dialog::SignOut);
    let mut harness = harness(state);
    harness.get_by_label("Sign out").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::ConfirmDialog
    )));
}

#[test]
fn a_problem_report_is_saved_from_settings_and_says_where_it_went() {
    let mut on_settings = state();
    on_settings.nav.open(Page::Settings);
    on_settings.report = report::Status::Saved("C:/Downloads/ytms-diagnostics.zip".into());
    let mut harness = harness(on_settings);
    // It sits below the fold of the test window.
    harness.get_by_label("Save report").scroll_to_me();
    harness.run();
    assert!(
        harness
            .query_by_label_contains("Saved to C:/Downloads/ytms-diagnostics.zip")
            .is_some()
    );
    harness.get_by_label("Save report").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::SaveReport
    )));
    harness.get_by_label("Open log folder").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(action, Action::OpenLogs)));
}
