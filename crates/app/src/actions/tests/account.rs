//! The account: who is signed in, which channel is in use, and upkeep.

use spotified_client::models::Account;

use super::*;

#[test]
fn a_signed_in_account_is_asked_for_its_channels() {
    let mut state = ready();
    let account = Account {
        name: "Ada".into(),
        ..Account::default()
    };
    let answer = Response::Account(Ok(Some(account)));
    assert_eq!(
        apply(&mut state, Action::Loaded(Box::new(answer))),
        [Effect::Fetch(Request::Channels)]
    );
    assert!(state.account.is_some());

    let signed_out = Response::Account(Ok(None));
    assert!(apply(&mut state, Action::Loaded(Box::new(signed_out))).is_empty());
    assert!(state.account.is_none());
}

#[test]
fn choosing_the_channel_in_use_changes_nothing() {
    let mut state = ready();
    state.channel_id = "123".into();
    assert!(apply(&mut state, Action::SwitchChannel("123".into())).is_empty());
    assert_eq!(
        apply(&mut state, Action::SwitchChannel(String::new())),
        [Effect::SwitchChannel(String::new())]
    );
    assert_eq!(state.channel_id, "");
}

#[test]
fn the_resolver_is_updated_once_at_a_time_and_says_how_it_went() {
    let mut state = ready();
    assert_eq!(
        apply(&mut state, Action::UpdateResolver),
        [Effect::UpdateResolver]
    );
    assert!(apply(&mut state, Action::UpdateResolver).is_empty());

    apply(
        &mut state,
        Action::ResolverUpdated(Ok("yt-dlp is up to date".into())),
    );
    assert!(!state.updating_resolver);
    assert!(state.toasts.last().is_some_and(|toast| !toast.error));

    apply(&mut state, Action::UpdateResolver);
    apply(
        &mut state,
        Action::ResolverUpdated(Err("no network".into())),
    );
    assert!(state.toasts.last().is_some_and(|toast| toast.error));
}
