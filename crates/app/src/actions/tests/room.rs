//! Listen Together, inside a room: the leader's settings, requests, the
//! ready check, who is in the room, the saved servers, and leaving.

use serde_json::json;
use spotified_client::session::Command;

use super::together::{hear, in_room, room};
use super::*;
use crate::state::Dialog;
use crate::together::protocol::{
    Entry, Happening, Member, Mode, Person, Policy, Room, RoomTrack, SongRequest,
};
use crate::together::{Ask, Event, Phase, Setting, Tab, Transport};

pub(super) fn ask(state: &mut State, ask: Ask) -> Vec<Effect> {
    apply(state, Action::Room(ask))
}

fn heard(state: &mut State, room: Room) -> Vec<Effect> {
    let event = Event::Room {
        room: Box::new(room),
        offset_ms: 0.0,
    };
    hear(state, event)
}

/// In a room led by "me", with a guest called Bob.
pub(super) fn leading() -> State {
    let mut state = in_room();
    heard(&mut state, with_guest(room(1)));
    state.toasts.clear();
    state
}

/// In the same room as its guest, Bob: "bob" is this listener.
fn as_guest(mode: Mode) -> State {
    let mut state = leading();
    state.together.me = "bob".into();
    let mut room = with_guest(room(2));
    room.mode = mode;
    heard(&mut state, room);
    state.toasts.clear();
    state
}

fn with_guest(mut room: Room) -> Room {
    room.members.push(Member {
        id: "bob".into(),
        name: "Bob".into(),
        connected: true,
        role: "listener".into(),
        ..Member::default()
    });
    room
}

fn request(id: &str, title: &str, by: &str) -> SongRequest {
    SongRequest {
        id: id.into(),
        track: RoomTrack {
            id: "zyxwvutsrqp".into(),
            title: title.into(),
            ..RoomTrack::default()
        },
        by: Person {
            id: by.into(),
            name: "Bob".into(),
            ..Person::default()
        },
    }
}

fn said(state: &State) -> Vec<&str> {
    state
        .toasts
        .iter()
        .map(|toast| toast.text.as_str())
        .collect()
}

fn command(kind: &'static str, fields: serde_json::Value) -> [Effect; 1] {
    [Effect::TogetherCommand(kind, fields)]
}

#[test]
fn the_leaders_settings_go_to_the_room_one_at_a_time() {
    let mut state = leading();
    let set = |state: &mut State, setting| ask(state, Ask::Set(setting));
    assert_eq!(
        set(&mut state, Setting::Mode(Mode::Contributions)),
        command("settings", json!({ "mode": "contributions" }))
    );
    assert_eq!(
        set(&mut state, Setting::Policy(Policy::Turns)),
        command("settings", json!({ "policy": "turns" }))
    );
    assert_eq!(
        set(&mut state, Setting::JoinApproval(true)),
        command("settings", json!({ "joinApproval": true }))
    );
    assert_eq!(
        set(&mut state, Setting::Locked(true)),
        command("settings", json!({ "locked": true }))
    );
    assert_eq!(
        set(&mut state, Setting::AutoAccept(true)),
        command("settings", json!({ "autoAccept": true }))
    );
    assert_eq!(
        set(&mut state, Setting::Duplicates(false)),
        command("settings", json!({ "duplicates": false }))
    );
    assert_eq!(
        set(&mut state, Setting::VoteSkip(true)),
        command("settings", json!({ "voteSkip": true }))
    );
    assert_eq!(
        ask(&mut state, Ask::RotatePin),
        command("rotate", json!({}))
    );
}

#[test]
fn a_guest_cannot_change_the_rooms_settings() {
    let mut state = as_guest(Mode::Collaborative);
    assert!(ask(&mut state, Ask::Set(Setting::Locked(true))).is_empty());
    assert_eq!(said(&state), ["Only the leader can change room settings."]);
    assert!(ask(&mut state, Ask::EndRoom).is_empty());
}

#[test]
fn songs_per_guest_is_sent_only_when_it_is_a_new_and_sensible_limit() {
    let mut state = leading();
    let mut limited = with_guest(room(2));
    limited.limit = 50;
    heard(&mut state, limited);
    ask(&mut state, Ask::LimitText("7 songs".into()));
    assert_eq!(state.together.limit_text.as_deref(), Some("7"));
    assert_eq!(
        ask(&mut state, Ask::CommitLimit),
        command("settings", json!({ "limit": 7 }))
    );
    // Tabbing through, or a figure out of range, sends nothing.
    ask(&mut state, Ask::LimitText("50".into()));
    assert!(ask(&mut state, Ask::CommitLimit).is_empty());
    ask(&mut state, Ask::LimitText("500".into()));
    assert!(ask(&mut state, Ask::CommitLimit).is_empty());
    assert!(state.together.limit_text.is_none());
}

#[test]
fn a_ready_check_is_started_answered_and_forced() {
    let mut state = leading();
    assert_eq!(
        ask(&mut state, Ask::ReadyCheck),
        command("countdown", json!({}))
    );
    assert_eq!(
        ask(&mut state, Ask::Ready),
        command("ready", json!({ "ready": true }))
    );
    assert_eq!(
        ask(&mut state, Ask::StartSoon),
        command("countdown", json!({ "force": true }))
    );
    // A guest answers but does not start one.
    let mut state = as_guest(Mode::Collaborative);
    assert!(ask(&mut state, Ask::ReadyCheck).is_empty());
    assert_eq!(ask(&mut state, Ask::Ready).len(), 1);
}

#[test]
fn anyone_votes_to_skip_and_only_those_who_steer_use_the_rooms_buttons() {
    let mut state = as_guest(Mode::Listen);
    assert_eq!(ask(&mut state, Ask::VoteSkip), command("vote", json!({})));
    assert!(ask(&mut state, Ask::Transport(Transport::Next)).is_empty());
    assert_eq!(said(&state), ["The leader controls playback in this room."]);
    assert!(ask(&mut state, Ask::Jump("e1".into())).is_empty());

    let mut state = leading();
    // The room plays: its button pauses.
    assert_eq!(
        ask(&mut state, Ask::Transport(Transport::Toggle)),
        command("pause", json!({}))
    );
    assert_eq!(
        ask(&mut state, Ask::Transport(Transport::Next)),
        command("next", json!({}))
    );
    assert!(matches!(
        &ask(&mut state, Ask::Transport(Transport::Previous))[..],
        [Effect::TogetherCommand("previous", fields)] if fields["restart"].is_boolean()
    ));
    assert_eq!(
        ask(&mut state, Ask::Jump("e1".into())),
        command("jump", json!({ "entry": "e1" }))
    );
}

#[test]
fn the_leader_makes_djs_hands_over_and_removes_after_being_asked() {
    let mut state = leading();
    let role = Ask::Role {
        member: "bob".into(),
        dj: true,
    };
    assert_eq!(
        ask(&mut state, role),
        command("role", json!({ "member": "bob", "role": "dj" }))
    );
    let role = Ask::Role {
        member: "bob".into(),
        dj: false,
    };
    assert_eq!(
        ask(&mut state, role),
        command("role", json!({ "member": "bob", "role": "listener" }))
    );
    assert_eq!(
        ask(&mut state, Ask::MakeLeader("bob".into())),
        command("transfer", json!({ "member": "bob" }))
    );
    let remove = Ask::RemoveListener {
        member: "bob".into(),
        name: "Bob".into(),
    };
    assert!(ask(&mut state, remove).is_empty());
    assert!(matches!(&state.dialog, Some(Dialog::RemoveListener { .. })));
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        command("kick", json!({ "member": "bob" }))
    );
}

#[test]
fn those_waiting_to_join_are_let_in_or_turned_away() {
    let mut state = leading();
    assert_eq!(
        ask(&mut state, Ask::Admit("p1".into())),
        command("approve", json!({ "request": "p1" }))
    );
    assert_eq!(
        ask(&mut state, Ask::TurnAway("p1".into())),
        command("deny", json!({ "request": "p1" }))
    );
}

#[test]
fn requests_are_accepted_next_or_last_declined_and_withdrawn() {
    let mut state = leading();
    let accept = Ask::Accept {
        requests: vec!["q1".into(), "q2".into()],
        next: false,
    };
    assert_eq!(
        ask(&mut state, accept),
        command(
            "acceptRequest",
            json!({ "requests": ["q1", "q2"], "placement": "end" })
        )
    );
    let accept = Ask::Accept {
        requests: vec!["q1".into()],
        next: true,
    };
    assert_eq!(
        ask(&mut state, accept),
        command(
            "acceptRequest",
            json!({ "requests": ["q1"], "placement": "next" })
        )
    );
    assert_eq!(
        ask(&mut state, Ask::Decline(vec!["q1".into()])),
        command("declineRequest", json!({ "requests": ["q1"] }))
    );
    assert_eq!(
        ask(&mut state, Ask::Withdraw("q1".into())),
        command("cancelRequest", json!({ "request": "q1" }))
    );
    assert_eq!(state.together.withdrawn, ["q1"]);
}

#[test]
fn a_song_added_from_the_room_is_queued_or_requested_as_the_room_has_it() {
    let song = || Box::new(track("abcdefghijk"));
    let mut state = leading();
    assert!(matches!(
        &ask(&mut state, Ask::Add(song()))[..],
        [Effect::TogetherCommand("enqueue", fields)] if fields["tracks"][0]["id"] == "abcdefghijk"
    ));
    let mut state = as_guest(Mode::Contributions);
    assert!(matches!(
        &ask(&mut state, Ask::Add(song()))[..],
        [Effect::TogetherCommand("request", _)]
    ));
    let mut state = as_guest(Mode::Listen);
    assert!(ask(&mut state, Ask::Add(song())).is_empty());
    assert_eq!(said(&state), ["This room is listen only."]);
}

#[test]
fn the_leader_hears_of_a_request_and_the_guest_of_its_answer() {
    let mut state = leading();
    let mut asked = with_guest(room(1));
    asked.requests = vec![request("q1", "Wanted", "bob")];
    heard(&mut state, asked.clone());
    assert_eq!(said(&state), ["Bob requested “Wanted”."]);

    let mut state = as_guest(Mode::Contributions);
    asked.mode = Mode::Contributions;
    asked.revision = 2;
    heard(&mut state, asked.clone());
    assert_eq!(
        said(&state),
        ["Request sent. The leader decides what plays."]
    );
    // Declined: the request is gone and no entry carries it.
    state.toasts.clear();
    asked.requests.clear();
    heard(&mut state, asked);
    assert_eq!(said(&state), ["Your request for “Wanted” wasn’t added."]);
}

#[test]
fn what_happens_in_the_room_is_said_only_to_those_who_asked_to_hear_it() {
    let happened = |revision| {
        let mut room = with_guest(room(revision));
        room.activity = vec![Happening {
            id: format!("a{revision}"),
            text: "Bob joined.".into(),
            at: 0.0,
        }];
        room
    };
    let mut state = leading();
    heard(&mut state, happened(2));
    assert!(said(&state).is_empty());
    assert_eq!(
        ask(&mut state, Ask::Notifications(true)),
        [Effect::SaveSettings]
    );
    heard(&mut state, happened(3));
    assert_eq!(said(&state), ["Bob joined."]);
}

#[test]
fn an_edit_is_taken_back_and_an_entry_removed_by_asking_the_room() {
    let mut state = leading();
    assert_eq!(ask(&mut state, Ask::Undo), command("undo", json!({})));
    assert_eq!(
        ask(&mut state, Ask::Remove("e1".into())),
        command("remove", json!({ "entry": "e1" }))
    );
}

#[test]
fn nothing_is_sent_while_the_line_is_down() {
    let mut state = leading();
    hear(&mut state, Event::Reconnecting);
    assert!(ask(&mut state, Ask::VoteSkip).is_empty());
    assert_eq!(said(&state), ["Wait for the room to reconnect."]);
}

#[test]
fn leaving_as_the_leader_names_who_leads_next() {
    let mut state = leading();
    ask(&mut state, Ask::OpenLeave);
    assert_eq!(state.together.leaving.as_deref(), Some(""));
    ask(&mut state, Ask::NextLeader("bob".into()));
    assert_eq!(
        ask(&mut state, Ask::Leave),
        [
            Effect::TogetherHandOver("bob".into()),
            Effect::TogetherDisconnect,
            Effect::Command(Command::LeaveRoom { keep_queue: true })
        ]
    );
    assert_eq!(state.together.phase, Phase::Idle);
}

#[test]
fn leaving_without_a_choice_lets_the_relay_pick_and_staying_closes_the_panel() {
    let mut state = leading();
    ask(&mut state, Ask::OpenLeave);
    ask(&mut state, Ask::Stay);
    assert!(state.together.leaving.is_none());
    ask(&mut state, Ask::OpenLeave);
    assert_eq!(
        ask(&mut state, Ask::Leave),
        [
            Effect::TogetherDisconnect,
            Effect::Command(Command::LeaveRoom { keep_queue: true })
        ]
    );
}

#[test]
fn the_leader_can_end_the_room_for_everyone_and_is_told_why_it_ended() {
    let mut state = leading();
    assert_eq!(ask(&mut state, Ask::EndRoom), command("end", json!({})));
    hear(
        &mut state,
        Event::Ended("The leader ended the room.".into()),
    );
    assert_eq!(state.together.phase, Phase::Idle);
    assert_eq!(
        state.together.error.as_deref(),
        Some("The leader ended the room.")
    );
}

#[test]
fn the_rooms_history_is_saved_as_a_playlist_under_a_name() {
    let mut state = leading();
    // Nothing played yet: nothing to save.
    ask(&mut state, Ask::SaveHistory);
    assert!(state.dialog.is_none());
    let mut played = with_guest(room(2));
    played.history = vec![Entry {
        id: "e0".into(),
        track: RoomTrack {
            id: "lmnopqrstuv".into(),
            ..RoomTrack::default()
        },
        ..Entry::default()
    }];
    heard(&mut state, played);
    ask(&mut state, Ask::SaveHistory);
    assert!(matches!(
        &state.dialog,
        Some(Dialog::SaveRoomHistory { name, track_ids })
            if name == "Listen Together" && track_ids == &["lmnopqrstuv"]
    ));
    apply(&mut state, Action::SetDialogText("Friday".into()));
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        [Effect::Fetch(Request::CreatePlaylist {
            title: "Friday".into(),
            track_ids: vec!["lmnopqrstuv".into()],
        })]
    );
    let created = Response::PlaylistCreated {
        title: "Friday".into(),
        result: Ok("PL1".into()),
    };
    apply(&mut state, Action::Loaded(Box::new(created)));
    assert_eq!(said(&state), ["Room history saved to your library."]);
}

#[test]
fn the_pages_tabs_and_panels_open_and_close() {
    let mut state = leading();
    ask(&mut state, Ask::ShowTab(Tab::Activity));
    assert_eq!(state.together.tab, Tab::Activity);
    ask(&mut state, Ask::ToggleSettings);
    assert!(state.together.settings_open);
    state.together.error = Some("Something.".into());
    ask(&mut state, Ask::DismissError);
    assert!(state.together.error.is_none());
}

#[test]
fn resyncing_brings_the_player_to_the_room_again() {
    let mut state = leading();
    assert!(state.together.applied_entry.is_some());
    let effects = ask(&mut state, Ask::Resync);
    assert!(matches!(
        &effects[0],
        Effect::Command(Command::FollowRoom { entry, .. }) if entry == "e1"
    ));
}
