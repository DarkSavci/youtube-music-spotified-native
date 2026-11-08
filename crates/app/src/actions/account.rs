//! The accounts: signing in, which of those saved is in use, which channel
//! it acts as, and signing out.

use super::{Action, Effect};
use crate::backend::Request;
use crate::sidecar::CoreStatus;
use crate::state::{Dialog, SignInStage, State};

/// Whether an account change is already under way: a sign-in in the
/// browser, the core starting again on another account, or things being
/// brought over from the Electron app into the accounts as they stand. A
/// second change on top of any of them would race it.
pub fn busy(state: &State) -> bool {
    state.signing_in() || state.core == CoreStatus::Starting || state.migration.running.is_some()
}

pub(super) fn account(state: &mut State, action: Action) -> Vec<Effect> {
    match action {
        Action::SwitchChannel(channel_id) => {
            if channel_id == state.channel_id {
                return Vec::new();
            }
            state.channel_id.clone_from(&channel_id);
            vec![Effect::SwitchChannel(channel_id)]
        }
        Action::SignIn => {
            state.dialog = Some(Dialog::SignIn);
            Vec::new()
        }
        Action::StartSignIn => {
            // The core starting on another account, or a move from the old
            // app, would race the account this brings.
            if busy(state) {
                return Vec::new();
            }
            state.sign_in = SignInStage::Browser;
            state.import_error = None;
            state.dialog = Some(Dialog::SignIn);
            vec![Effect::SignIn]
        }
        Action::CancelSignIn => match state.sign_in {
            SignInStage::Browser | SignInStage::Reading => {
                state.sign_in = SignInStage::Idle;
                vec![Effect::CancelSignIn]
            }
            // Once the core is starting on the account it is too late.
            SignInStage::Idle | SignInStage::Starting => Vec::new(),
        },
        Action::ShowSignInBrowser => {
            if state.sign_in == SignInStage::Browser {
                vec![Effect::RaiseSignInBrowser]
            } else {
                Vec::new()
            }
        }
        Action::SignInReading => {
            if state.sign_in == SignInStage::Browser {
                state.sign_in = SignInStage::Reading;
            }
            Vec::new()
        }
        Action::SignOut => {
            if state.accounts.active.is_some() {
                state.dialog = Some(Dialog::SignOut);
            }
            Vec::new()
        }
        Action::SwitchAccount(id) => {
            let known = state.accounts.get(&id).is_some();
            if busy(state) || !known || state.accounts.is_active(&id) {
                return Vec::new();
            }
            state.import_error = None;
            vec![Effect::SwitchAccount(id)]
        }
        Action::AskRemoveAccount(id) => {
            if let Some(account) = state.accounts.get(&id) {
                let name = account.name.clone();
                state.dialog = Some(Dialog::RemoveAccount { id, name });
            }
            Vec::new()
        }
        Action::AccountsChanged(accounts) => {
            state.accounts = *accounts;
            // The channel in use is the one the account in use acts as.
            state.channel_id = state
                .accounts
                .active()
                .map(|account| account.channel.clone())
                .unwrap_or_default();
            Vec::new()
        }
        Action::RefreshChannels => {
            if state.core_ready() && state.account.is_some() {
                vec![Effect::Fetch(Request::Channels)]
            } else {
                Vec::new()
            }
        }
        Action::OpenAccountMenu => {
            state.account_menu_asks += 1;
            Vec::new()
        }
        Action::AccountChanged => {
            state.import_error = None;
            // A sign-in is done once the core says who it brought.
            if matches!(state.sign_in, SignInStage::Browser | SignInStage::Reading) {
                state.sign_in = SignInStage::Starting;
            }
            state.forget_account_data();
            Vec::new()
        }
        Action::SignInFailed(message) => {
            failed(state, message);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// A sign-in ended without an account: the dialog says why, shown again if
/// it had been put away.
fn failed(state: &mut State, message: String) {
    state.sign_in = SignInStage::Idle;
    state.import_error = Some(message);
    if state.dialog.is_none() {
        state.dialog = Some(Dialog::SignIn);
    }
}

/// The core could not start. If it was starting on a new sign-in, that
/// sign-in has failed with it.
pub(super) fn core_failed(state: &mut State, message: &str) {
    if state.sign_in == SignInStage::Starting {
        failed(state, message.to_owned());
    }
}

/// The core has said who is signed in. A sign-in waiting for that is
/// finished: well, or with a session YouTube would not take, which is no
/// use to keep.
pub(super) fn account_known<E>(
    state: &mut State,
    result: &Result<Option<spotified_client::models::Account>, E>,
) -> Vec<Effect> {
    if state.sign_in != SignInStage::Starting {
        return Vec::new();
    }
    state.sign_in = SignInStage::Idle;
    match result {
        Ok(Some(account)) => {
            if state.dialog == Some(Dialog::SignIn) {
                state.dialog = None;
            }
            let name = account.name.trim();
            state.toast(if name.is_empty() {
                "Signed in".to_owned()
            } else {
                format!("Signed in as {name}")
            });
            Vec::new()
        }
        Ok(None) => {
            failed(
                state,
                "Google signed you in, but YouTube Music did not accept the session.                  Try again; if it keeps happening, sign in at music.youtube.com in your                  browser once first."
                    .to_owned(),
            );
            state
                .accounts
                .active
                .clone()
                .map(Effect::RemoveAccount)
                .into_iter()
                .collect()
        }
        // Signed in, but the account could not be asked after just now;
        // the account is kept and the rest of the app says what failed.
        Err(_) => {
            if state.dialog == Some(Dialog::SignIn) {
                state.dialog = None;
            }
            state.toast("Signed in");
            Vec::new()
        }
    }
}

/// A question about an account was answered "yes".
pub(super) fn confirmed(state: &mut State, dialog: Dialog) -> Vec<Effect> {
    let id = match dialog {
        Dialog::SignOut => state.accounts.active.clone(),
        Dialog::RemoveAccount { id, .. } => Some(id),
        _ => None,
    };
    match id {
        // A change already under way is finished first.
        Some(id) if !busy(state) => vec![Effect::RemoveAccount(id)],
        _ => Vec::new(),
    }
}
