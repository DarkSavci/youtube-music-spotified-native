//! What `--open` can ask of the Listen Together page: the saved servers,
//! and everything a room's page offers, named as the page names it. Who and
//! what is meant is looked up in the room as it stands when the step is
//! taken: a listener by name, a song by its place.

use spotified_client::models::Track;

use crate::actions::Action;
use crate::state::State;
use crate::together::protocol::{Mode, Policy};
use crate::together::{Ask, Room, Setting, Tab, Transport};

/// Whether `kind` is one of this file's.
pub(super) fn knows(kind: &str) -> bool {
    kind.starts_with("room-") || kind.starts_with("server-") || kind == "confirm"
}

fn on(value: &str) -> bool {
    value != "off"
}

fn setting(text: &str) -> Option<Ask> {
    let (name, value) = text.split_once('=')?;
    Some(match name {
        "mode" => Ask::Set(Setting::Mode(match value {
            "contributions" => Mode::Contributions,
            "listen" => Mode::Listen,
            _ => Mode::Collaborative,
        })),
        "policy" => Ask::Set(Setting::Policy(match value {
            "turns" => Policy::Turns,
            _ => Policy::Fifo,
        })),
        "approval" => Ask::Set(Setting::JoinApproval(on(value))),
        "locked" => Ask::Set(Setting::Locked(on(value))),
        "auto" => Ask::Set(Setting::AutoAccept(on(value))),
        "duplicates" => Ask::Set(Setting::Duplicates(on(value))),
        "voteskip" => Ask::Set(Setting::VoteSkip(on(value))),
        _ => return None,
    })
}

/// The id of the member called `name`.
fn member(room: &Room, name: &str) -> Option<String> {
    let found = room.members.iter().find(|member| member.name == name);
    found.map(|member| member.id.clone())
}

/// The song at `place` among what the room's search found.
fn found(state: &State, place: &str) -> Option<Box<Track>> {
    let results = &state.together.search.results;
    results
        .get(place.parse::<usize>().ok()?)
        .cloned()
        .map(Box::new)
}

/// The actions a step of this file's comes to; none if it names nothing
/// that is there.
pub(super) fn actions(kind: &str, value: &str, state: &State) -> Vec<Action> {
    let asks = asks(kind, value, state).unwrap_or_default();
    let mut actions: Vec<Action> = asks.into_iter().map(Action::Room).collect();
    if kind == "confirm" {
        actions.push(Action::ConfirmDialog);
    }
    actions
}

fn asks(kind: &str, value: &str, state: &State) -> Option<Vec<Ask>> {
    let together = &state.together;
    let room = together.room.as_ref();
    let me = together.me.as_str();
    let text = || value.to_owned();
    let ask = match kind {
        "server-manage" => Ask::ToggleManage,
        // A server to save: `server-form:<name>=<address>`.
        "server-form" => {
            let (name, url) = value.split_once('=')?;
            let fill = [Ask::ServerName(name.into()), Ask::ServerAddress(url.into())];
            return Some(fill.into());
        }
        "server-save" => Ask::SaveServer,
        "server-test" => Ask::TestServer,
        "server-another" => Ask::AddAnother,
        "server-remove" => Ask::RemoveServer,
        "server-select" => {
            let servers = &state.settings.together_servers;
            Ask::SelectServer(
                servers
                    .iter()
                    .find(|server| server.name == value)?
                    .id
                    .clone(),
            )
        }
        "room-tab" => Ask::ShowTab(match value {
            "history" => Tab::History,
            "activity" => Tab::Activity,
            _ => Tab::Queue,
        }),
        "room-settings" => Ask::ToggleSettings,
        "room-set" => setting(value)?,
        "room-limit" => return Some(vec![Ask::LimitText(text()), Ask::CommitLimit]),
        "room-rotate" => Ask::RotatePin,
        "room-readycheck" => Ask::ReadyCheck,
        "room-startsoon" => Ask::StartSoon,
        "room-ready" => Ask::Ready,
        "room-vote" => Ask::VoteSkip,
        "room-toggle" => Ask::Transport(Transport::Toggle),
        "room-skip" => Ask::Transport(Transport::Next),
        "room-back" => Ask::Transport(Transport::Previous),
        "room-dj" | "room-listener" => Ask::Role {
            member: member(room?, value)?,
            dj: kind == "room-dj",
        },
        "room-leader" => Ask::MakeLeader(member(room?, value)?),
        "room-kick" => Ask::RemoveListener {
            member: member(room?, value)?,
            name: text(),
        },
        "room-admit" => Ask::Admit(room?.pending.first()?.id.clone()),
        "room-turnaway" => Ask::TurnAway(room?.pending.first()?.id.clone()),
        // The first request waiting: `next`, `end`, or `all` of them.
        "room-accept" => {
            let room = room?;
            let most = if value == "all" { usize::MAX } else { 1 };
            let requests = room.requests.iter().take(most);
            Ask::Accept {
                requests: requests.map(|request| request.id.clone()).collect(),
                next: value == "next",
            }
        }
        "room-decline" => Ask::Decline(vec![room?.requests.first()?.id.clone()]),
        "room-withdraw" => Ask::Withdraw(room?.requests_for(me).next()?.id.clone()),
        "room-search" => Ask::Search(text()),
        "room-add" => Ask::Add(found(state, value)?),
        "room-radio" => Ask::Radio(found(state, value)?),
        // A song the room has played, by its place in the history.
        "room-again" => {
            let entry = room?.history.get(value.parse::<usize>().ok()?)?;
            Ask::Add(Box::new(entry.track.to_track()))
        }
        "room-jump" => Ask::Jump(room?.queue.get(value.parse::<usize>().ok()?)?.id.clone()),
        "room-remove" => Ask::Remove(room?.queue.get(value.parse::<usize>().ok()?)?.id.clone()),
        "room-undo" => Ask::Undo,
        "room-save" => Ask::SaveHistory,
        "room-resync" => Ask::Resync,
        "room-notify" => Ask::Notifications(on(value)),
        "room-leave" => Ask::OpenLeave,
        "room-stay" => Ask::Stay,
        "room-next-leader" => Ask::NextLeader(member(room?, value)?),
        "room-leave-now" => Ask::Leave,
        "room-end" => Ask::EndRoom,
        _ => return None,
    };
    Some(vec![ask])
}
