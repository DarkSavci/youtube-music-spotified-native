//! What is asked for on the room's page: the leader's settings, requests
//! and their answers, the ready check, who is in the room, and leaving it.

use serde_json::{Value, json};

use super::super::Effect;
use super::{leave, radio, servers};
use crate::state::{Dialog, State};
use crate::together::sync::{self, LEADER_PLAYS, RESTART_AFTER_MS, Routed};
use crate::together::{Ask, Phase, Room, Setting, Transport};

/// The room this listener is seated in with the line up, which is what a
/// command needs; otherwise the reason to wait is said.
fn seated(state: &mut State) -> Option<&Room> {
    if state.together.phase != Phase::Joined {
        state.toast("Wait for the room to reconnect.");
        return None;
    }
    state.together.room.as_ref()
}

/// One command for the room, if this listener is seated in it.
fn command(state: &mut State, kind: &'static str, fields: Value) -> Vec<Effect> {
    match seated(state) {
        Some(_) => vec![Effect::TogetherCommand(kind, fields)],
        None => Vec::new(),
    }
}

/// As [`command`], for what only the leader may ask.
fn leader_command(state: &mut State, kind: &'static str, fields: Value) -> Vec<Effect> {
    let me = state.together.me.clone();
    match seated(state) {
        Some(room) if room.leads(&me) => vec![Effect::TogetherCommand(kind, fields)],
        Some(_) => {
            state.toast_error("Only the leader can change room settings.");
            Vec::new()
        }
        None => Vec::new(),
    }
}

/// Carries out what routing a command came to.
pub(super) fn routed(state: &mut State, routed: Routed) -> Vec<Effect> {
    match routed {
        Routed::Room(kind, fields) => vec![Effect::TogetherCommand(kind, fields)],
        Routed::Refused(why) => {
            state.toast_error(why);
            Vec::new()
        }
        Routed::Core => Vec::new(),
    }
}

pub(in crate::actions) fn asked(state: &mut State, ask: Ask) -> Vec<Effect> {
    let me = state.together.me.clone();
    match ask {
        Ask::SelectServer(_)
        | Ask::ToggleManage
        | Ask::ServerName(_)
        | Ask::ServerAddress(_)
        | Ask::SaveServer
        | Ask::TestServer
        | Ask::Tested(_)
        | Ask::AddAnother
        | Ask::RemoveServer => servers::asked(state, ask),
        Ask::SharePicture(on) => {
            state.settings.together_share_picture = on;
            vec![Effect::SaveSettings]
        }
        Ask::Notifications(on) => {
            state.settings.together_notifications = on;
            vec![Effect::SaveSettings]
        }
        Ask::DismissError => {
            state.together.error = None;
            Vec::new()
        }
        Ask::ShowTab(tab) => {
            state.together.tab = tab;
            Vec::new()
        }
        Ask::ToggleSettings => {
            state.together.settings_open = !state.together.settings_open;
            state.together.limit_text = None;
            Vec::new()
        }
        Ask::OpenLeave => {
            state.together.leaving = Some(String::new());
            Vec::new()
        }
        Ask::Stay => {
            state.together.leaving = None;
            Vec::new()
        }
        Ask::NextLeader(member) => {
            if let Some(next) = &mut state.together.leaving {
                *next = member;
            }
            Vec::new()
        }
        Ask::Leave => {
            // The room is told who leads next before the line is closed.
            let next = state.together.leaving.take().unwrap_or_default();
            let leads = state.together.room.as_ref().is_some_and(|r| r.leads(&me));
            let mut effects = Vec::new();
            if leads && !next.is_empty() {
                effects.push(Effect::TogetherHandOver(next));
            }
            effects.extend(leave(state, None));
            effects
        }
        Ask::RetryPlayback => {
            // Forgetting what was asked of the player is what asks again.
            state.together.applied_entry = None;
            state.together.corrected_at = None;
            super::keep_up(state)
        }
        Ask::EndRoom => {
            state.together.leaving = None;
            leader_command(state, "end", json!({}))
        }
        Ask::Set(setting) => {
            let fields = match setting {
                Setting::Mode(mode) => json!({ "mode": mode.wire() }),
                Setting::Policy(policy) => json!({ "policy": policy.wire() }),
                Setting::JoinApproval(on) => json!({ "joinApproval": on }),
                Setting::Locked(on) => json!({ "locked": on }),
                Setting::AutoAccept(on) => json!({ "autoAccept": on }),
                Setting::Duplicates(on) => json!({ "duplicates": on }),
                Setting::VoteSkip(on) => json!({ "voteSkip": on }),
            };
            leader_command(state, "settings", fields)
        }
        Ask::LimitText(text) => {
            let digits = text.chars().filter(char::is_ascii_digit).take(3);
            state.together.limit_text = Some(digits.collect());
            Vec::new()
        }
        Ask::CommitLimit => {
            // Only a real change is sent: leaving the field must not post
            // "updated room settings", nor make others' commands stale.
            let typed = state.together.limit_text.take();
            let limit = typed.and_then(|text| text.parse::<u32>().ok());
            let now = state.together.room.as_ref().map(|room| room.limit);
            match limit {
                Some(limit) if (1..=100).contains(&limit) && Some(limit) != now => {
                    leader_command(state, "settings", json!({ "limit": limit }))
                }
                _ => Vec::new(),
            }
        }
        Ask::RotatePin => leader_command(state, "rotate", json!({})),
        Ask::ReadyCheck => leader_command(state, "countdown", json!({})),
        Ask::StartSoon => leader_command(state, "countdown", json!({ "force": true })),
        Ask::Ready => command(state, "ready", json!({ "ready": true })),
        Ask::VoteSkip => command(state, "vote", json!({})),
        Ask::Transport(transport) => {
            let server_now = state.together.server_now();
            let Some(room) = seated(state) else {
                return Vec::new();
            };
            if !room.may_control(&me) {
                state.toast_error(LEADER_PLAYS);
                return Vec::new();
            }
            let (kind, fields) = match transport {
                Transport::Toggle if room.playing => ("pause", json!({})),
                Transport::Toggle => ("play", json!({})),
                Transport::Next => ("next", json!({})),
                // "Previous" means back to the start or back a song by
                // where the song was when it was pressed; saying which
                // keeps two presses at once from doing both.
                Transport::Previous => {
                    let restart = room.position_at(server_now) > RESTART_AFTER_MS;
                    ("previous", json!({ "restart": restart }))
                }
            };
            vec![Effect::TogetherCommand(kind, fields)]
        }
        Ask::Role { member, dj } => {
            let role = if dj { "dj" } else { "listener" };
            leader_command(state, "role", json!({ "member": member, "role": role }))
        }
        Ask::MakeLeader(member) => leader_command(state, "transfer", json!({ "member": member })),
        Ask::RemoveListener { member, name } => {
            state.dialog = Some(Dialog::RemoveListener { member, name });
            Vec::new()
        }
        Ask::Admit(request) => leader_command(state, "approve", json!({ "request": request })),
        Ask::TurnAway(request) => leader_command(state, "deny", json!({ "request": request })),
        Ask::Accept { requests, next } => {
            let placement = if next { "next" } else { "end" };
            let fields = json!({ "requests": requests, "placement": placement });
            command(state, "acceptRequest", fields)
        }
        Ask::Decline(requests) => command(state, "declineRequest", json!({ "requests": requests })),
        Ask::Withdraw(request) => {
            let effects = command(state, "cancelRequest", json!({ "request": request }));
            // Its going is then this listener's doing, not the leader's no.
            if !effects.is_empty() {
                state.together.withdrawn.push(request);
            }
            effects
        }
        Ask::Add(track) => {
            let Some(room) = seated(state) else {
                return Vec::new();
            };
            let add = sync::adding(room, &me, std::slice::from_ref(&*track), None);
            routed(state, add)
        }
        Ask::Radio(track) => {
            state.together.search = Default::default();
            radio::start_radio(state, *track)
        }
        Ask::Jump(entry) => {
            let Some(room) = seated(state) else {
                return Vec::new();
            };
            if !room.may_control(&me) {
                state.toast_error(LEADER_PLAYS);
                return Vec::new();
            }
            vec![Effect::TogetherCommand("jump", json!({ "entry": entry }))]
        }
        Ask::Remove(entry) => command(state, "remove", json!({ "entry": entry })),
        Ask::Undo => command(state, "undo", json!({})),
        Ask::Search(_) | Ask::RunSearch => radio::search(state, ask),
        Ask::SaveHistory => {
            let history = state.together.room.as_ref().map(|room| &room.history);
            let track_ids: Vec<String> = history
                .into_iter()
                .flatten()
                .map(|entry| entry.track.id.clone())
                .collect();
            if !track_ids.is_empty() {
                state.dialog = Some(Dialog::SaveRoomHistory {
                    name: "Listen Together".to_owned(),
                    track_ids,
                });
            }
            Vec::new()
        }
        Ask::Resync => {
            // As if the player had never been brought to the room.
            state.together.applied_entry = None;
            state.together.corrected_at = None;
            super::keep_up(state)
        }
    }
}

/// A question the page asked was answered with yes.
pub(in crate::actions) fn confirmed(state: &mut State, dialog: Dialog) -> Vec<Effect> {
    match dialog {
        Dialog::RemoveServer { id, .. } => servers::remove(state, &id),
        Dialog::RemoveListener { member, .. } => {
            leader_command(state, "kick", json!({ "member": member }))
        }
        Dialog::SaveRoomHistory { name, track_ids } if !name.trim().is_empty() => {
            let title = name.trim().to_owned();
            state.together.saving_history = Some(title.clone());
            vec![Effect::Fetch(crate::backend::Request::CreatePlaylist {
                title,
                track_ids,
            })]
        }
        // Nothing typed yet: the dialog stays for a name.
        unnamed => {
            state.dialog = Some(unnamed);
            Vec::new()
        }
    }
}
