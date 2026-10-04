//! Several accounts: which is in use, switching, and signing one out.

use spotified_client::models::{Account, Channel};

use super::*;
use crate::accounts::{self, Accounts};
use crate::state::Dialog;

/// Two saved accounts, the second in use. Returns their ids.
fn two_accounts(state: &mut State) -> (String, String) {
    let (ada, grace) = (accounts::new_account("Ada"), accounts::new_account("Grace"));
    let ids = (ada.id.clone(), grace.id.clone());
    let mut list = Accounts::default();
    list.add(ada);
    list.add(grace);
    apply(state, Action::AccountsChanged(Box::new(list)));
    ids
}

#[test]
fn switching_to_another_saved_account_restarts_on_it() {
    let mut state = ready();
    let (ada, _) = two_accounts(&mut state);
    assert_eq!(
        apply(&mut state, Action::SwitchAccount(ada.clone())),
        [Effect::SwitchAccount(ada)]
    );
}

#[test]
fn switching_to_the_account_in_use_or_to_nobody_changes_nothing() {
    let mut state = ready();
    let (_, grace) = two_accounts(&mut state);
    assert!(apply(&mut state, Action::SwitchAccount(grace)).is_empty());
    assert!(apply(&mut state, Action::SwitchAccount("nobody".into())).is_empty());
}

#[test]
fn one_account_change_is_finished_before_another_begins() {
    let mut state = ready();
    let (ada, _) = two_accounts(&mut state);
    // A sign-in is open in the browser.
    apply(&mut state, Action::SignIn);
    assert!(apply(&mut state, Action::SwitchAccount(ada.clone())).is_empty());
    apply(&mut state, Action::SignInFailed("closed".into()));
    // The core is starting again on another account.
    apply(&mut state, Action::AccountChanged);
    assert!(apply(&mut state, Action::SwitchAccount(ada.clone())).is_empty());
    assert!(account_busy(&state));
}

#[test]
fn signing_out_asks_first_and_then_removes_the_account_in_use() {
    let mut state = ready();
    let (_, grace) = two_accounts(&mut state);
    assert!(apply(&mut state, Action::SignOut).is_empty());
    assert_eq!(state.dialog, Some(Dialog::SignOut));
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        [Effect::RemoveAccount(grace)]
    );
    assert_eq!(state.dialog, None);
}

#[test]
fn never_mind_leaves_the_account_signed_in() {
    let mut state = ready();
    two_accounts(&mut state);
    apply(&mut state, Action::SignOut);
    assert!(apply(&mut state, Action::CloseDialog).is_empty());
    assert_eq!(state.dialog, None);
}

#[test]
fn with_nobody_signed_in_there_is_nobody_to_sign_out() {
    let mut state = ready();
    assert!(apply(&mut state, Action::SignOut).is_empty());
    assert_eq!(state.dialog, None);
}

#[test]
fn removing_a_saved_account_asks_first_by_its_name() {
    let mut state = ready();
    let (ada, _) = two_accounts(&mut state);
    apply(&mut state, Action::AskRemoveAccount(ada.clone()));
    assert_eq!(
        state.dialog,
        Some(Dialog::RemoveAccount {
            id: ada.clone(),
            name: "Ada".into()
        })
    );
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        [Effect::RemoveAccount(ada)]
    );
    // One that is not saved is not asked about.
    apply(&mut state, Action::AskRemoveAccount("nobody".into()));
    assert_eq!(state.dialog, None);
}

#[test]
fn the_channel_in_use_follows_the_account_in_use() {
    let mut state = ready();
    let mut ada = accounts::new_account("Ada");
    ada.channel = "123".into();
    let mut list = Accounts::default();
    list.add(ada);
    apply(&mut state, Action::AccountsChanged(Box::new(list)));
    assert_eq!(state.channel_id, "123");
    apply(&mut state, Action::AccountsChanged(Box::default()));
    assert_eq!(state.channel_id, "");
}

#[test]
fn what_the_core_says_of_the_account_is_noted_for_the_list() {
    let mut state = ready();
    let account = Account {
        name: "Ada".into(),
        avatar_url: "https://example.test/ada.jpg".into(),
        ..Account::default()
    };
    let effects = apply(
        &mut state,
        Action::Loaded(Box::new(Response::Account(Ok(Some(account))))),
    );
    assert!(effects.contains(&Effect::RememberAccount {
        name: "Ada".into(),
        avatar_url: "https://example.test/ada.jpg".into(),
    }));

    let channels = vec![Channel {
        id: "1".into(),
        name: "Band".into(),
        handle: "@band".into(),
        avatar_url: String::new(),
    }];
    let answer = Response::Channels(Ok(channels.clone()));
    assert_eq!(
        apply(&mut state, Action::Loaded(Box::new(answer))),
        [Effect::RememberChannels(channels)]
    );
    // A list that could not be read is not taken for an empty one.
    let failed = Response::Channels(Err(ApiError::Unreachable("offline".into())));
    assert!(apply(&mut state, Action::Loaded(Box::new(failed))).is_empty());
}

#[test]
fn channels_are_asked_for_again_only_of_a_signed_in_account() {
    let mut state = ready();
    assert!(apply(&mut state, Action::RefreshChannels).is_empty());
    state.account = Some(Account::default());
    assert_eq!(
        apply(&mut state, Action::RefreshChannels),
        [Effect::Fetch(Request::Channels)]
    );
}
