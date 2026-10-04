//! The accounts: signing in, which of those saved is in use, which channel
//! it acts as, and signing out.

use super::{Action, Effect};
use crate::backend::Request;
use crate::sidecar::CoreStatus;
use crate::state::{Dialog, State};

/// Whether an account change is already under way: a sign-in in the
/// browser, the core starting again on another account, or things being
/// brought over from the Electron app into the accounts as they stand. A
/// second change on top of any of them would race it.
pub fn busy(state: &State) -> bool {
    state.signing_in || state.core == CoreStatus::Starting || state.migration.running.is_some()
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
            if state.signing_in {
                return Vec::new();
            }
            state.signing_in = true;
            state.import_error = None;
            vec![Effect::SignIn]
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
            state.signing_in = false;
            state.forget_account_data();
            Vec::new()
        }
        Action::SignInFailed(message) => {
            state.signing_in = false;
            state.import_error = Some(message);
            Vec::new()
        }
        _ => Vec::new(),
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
