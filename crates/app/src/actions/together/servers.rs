//! The saved servers: choosing one, and the form that adds, renames,
//! tests and removes them.

use super::super::Effect;
use crate::state::{Dialog, State};
use crate::together::protocol::checked_address;
use crate::together::servers::{self, CHECKED, CHECKING};
use crate::together::{Ask, Phase, ServerForm};

pub(super) fn asked(state: &mut State, ask: Ask) -> Vec<Effect> {
    // The server is not changed under a room, nor on the way into one.
    let idle = state.together.phase == Phase::Idle;
    match ask {
        Ask::SelectServer(id) if idle => {
            state.settings.together_selected = id;
            return vec![Effect::SaveSettings];
        }
        Ask::ToggleManage if idle => {
            state.together.manage = match state.together.manage {
                Some(_) => None,
                None => Some(ServerForm::on(state.settings.together_server())),
            };
        }
        Ask::ServerName(name) => {
            if let Some(form) = &mut state.together.manage {
                form.name = name.chars().take(80).collect();
            }
        }
        Ask::ServerAddress(url) => {
            if let Some(form) = &mut state.together.manage {
                form.url = url.chars().take(300).collect();
            }
        }
        Ask::SaveServer => {
            let Some(form) = &state.together.manage else {
                return Vec::new();
            };
            return match servers::save(&mut state.settings.together_servers, form) {
                Ok(id) => {
                    state.settings.together_selected = id;
                    state.together.manage = None;
                    state.together.error = None;
                    vec![Effect::SaveSettings]
                }
                Err(why) => {
                    state.together.error = Some(why.to_owned());
                    Vec::new()
                }
            };
        }
        Ask::TestServer => {
            let Some(form) = &mut state.together.manage else {
                return Vec::new();
            };
            if form.check == CHECKING {
                return Vec::new();
            }
            return match checked_address(&form.url) {
                Ok(url) => {
                    form.check = CHECKING.to_owned();
                    vec![Effect::TogetherProbe(url)]
                }
                Err(why) => {
                    form.check = why.to_owned();
                    Vec::new()
                }
            };
        }
        Ask::Tested(result) => {
            if let Some(form) = &mut state.together.manage {
                form.check = match result {
                    Ok(()) => CHECKED.to_owned(),
                    Err(why) => why,
                };
            }
        }
        Ask::AddAnother => {
            if state.together.manage.is_some() {
                state.together.manage = Some(ServerForm::default());
            }
        }
        Ask::RemoveServer => {
            let editing = state.together.manage.as_ref().and_then(|form| {
                let id = form.editing.as_ref()?;
                let servers = &state.settings.together_servers;
                servers.iter().find(|server| server.id == *id)
            });
            if let Some(server) = editing {
                state.dialog = Some(Dialog::RemoveServer {
                    id: server.id.clone(),
                    name: server.name.clone(),
                });
            }
        }
        _ => {}
    }
    Vec::new()
}

/// Forgets a saved server. None is then chosen.
pub(super) fn remove(state: &mut State, id: &str) -> Vec<Effect> {
    state
        .settings
        .together_servers
        .retain(|server| server.id != id);
    state.settings.together_selected.clear();
    state.together.manage = None;
    vec![Effect::SaveSettings]
}
