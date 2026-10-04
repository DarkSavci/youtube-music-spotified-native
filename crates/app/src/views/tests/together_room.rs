//! A Listen Together room's page: what it offers the leader, and a guest.

use eframe::egui::accesskit::Role;

use super::together::{asked_room, entry, person, room, seated, song, type_into};
use super::*;
use crate::together::protocol::{Countdown, Happening, Knock, Mode, Room, SongRequest};
use crate::together::{Ask, Setting, Tab, Transport};

#[test]
fn the_rooms_head_copies_the_pin_and_leads_to_leaving_and_settings() {
    let mut harness = harness(seated(room(), "ada", 0.0));
    harness.get_by_label("Ada’s room");
    harness.get_by_label("Copy room PIN").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::CopyText { text, .. } if text == "01234567"
    )));
    harness.get_by_label("Leave room").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::OpenLeave));
    harness.get_by_label("Room settings").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::ToggleSettings));
    // A guest has no settings to open.
    let harness = super::harness(seated(room(), "bob", 0.0));
    assert!(harness.query_by_label("Room settings").is_none());
}

#[test]
fn the_leader_leaving_chooses_who_leads_next_or_ends_the_room() {
    let mut state = seated(room(), "ada", 0.0);
    state.together.leaving = Some(String::new());
    let mut harness = harness(state);
    harness.get_by_label("Keep the music going.");
    harness
        .get_by_role_and_label(Role::Button, "Next leader")
        .click();
    harness.run();
    harness.get_by_label("Bob").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::NextLeader(member) if member == "bob"
    )));
    for (label, wanted) in [
        ("Leave the room now", Ask::Leave),
        ("Stay", Ask::Stay),
        ("End room for everyone", Ask::EndRoom),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert!(asked_room(&harness, |ask| *ask == wanted), "{label}");
    }
}

#[test]
fn a_guest_leaving_is_only_asked_whether_to() {
    let mut state = seated(room(), "bob", 0.0);
    state.together.leaving = Some(String::new());
    let harness = harness(state);
    harness.get_by_label("Leave this room?");
    assert!(harness.query_by_label("Next leader").is_none());
    assert!(harness.query_by_label("End room for everyone").is_none());
}

#[test]
fn the_leaders_settings_change_the_room() {
    let mut state = seated(room(), "ada", 0.0);
    state.together.settings_open = true;
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::Button, "Who controls playback?")
        .click();
    harness.run();
    harness.get_by_label("Just listen").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask
        == Ask::Set(Setting::Mode(Mode::Listen))));
    harness
        .get_by_role_and_label(Role::Button, "Queue order")
        .click();
    harness.run();
    harness.get_by_label("Take turns").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Set(Setting::Policy(_))
    )));
    let ticks = [
        ("Approve new listeners", Setting::JoinApproval(true)),
        ("Lock new joins", Setting::Locked(true)),
        ("Allow duplicates", Setting::Duplicates(false)),
        ("Vote to skip", Setting::VoteSkip(true)),
    ];
    for (label, wanted) in ticks {
        harness.get_by_role_and_label(Role::CheckBox, label).click();
        harness.run();
        assert!(
            asked_room(&harness, |ask| *ask == Ask::Set(wanted)),
            "{label}"
        );
    }
    // Only a room that takes requests can take them without asking.
    assert!(
        harness
            .query_by_label("Add requests without asking me")
            .is_none()
    );
    harness.get_by_label("Rotate PIN").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::RotatePin));
    harness.get_by_label("Start together / ready check").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::ReadyCheck));
    type_into(&mut harness, "Songs per guest", "7");
    assert!(asked_room(&harness, |ask| matches!(ask, Ask::LimitText(_))));
}

#[test]
fn a_room_that_takes_requests_can_be_set_to_take_them_without_asking() {
    let mut requests = room();
    requests.mode = Mode::Contributions;
    let mut state = seated(requests, "ada", 0.0);
    state.together.settings_open = true;
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::CheckBox, "Add requests without asking me")
        .click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::Set(Setting::AutoAccept(true))));
}

#[test]
fn a_ready_check_is_answered_and_the_leader_may_start_anyway() {
    let mut checking = room();
    checking.countdown = Some(Countdown {
        expires: f64::MAX,
        start_at: None,
    });
    checking.members[0].ready = true;
    let mut harness = harness(seated(checking.clone(), "ada", 0.0));
    harness.get_by_label("Ready for a shared start?");
    harness.get_by_label("1 of 2 ready");
    harness.get_by_label("I’m ready").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::Ready));
    harness.get_by_label("Start in 3 seconds").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::StartSoon));
    let harness = super::harness(seated(checking, "bob", 0.0));
    assert!(harness.query_by_label("Start in 3 seconds").is_none());
}

#[test]
fn the_rooms_own_buttons_steer_it_and_a_vote_skips() {
    let mut voting = room();
    voting.vote_skip = true;
    voting.votes = vec!["bob".into()];
    let mut harness = harness(seated(voting, "ada", 0.0));
    for (label, wanted) in [
        ("Previous song", Transport::Previous),
        ("Pause room", Transport::Toggle),
        ("Next song", Transport::Next),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert!(
            asked_room(&harness, |ask| *ask == Ask::Transport(wanted)),
            "{label}"
        );
    }
    harness.get_by_label("Vote to skip · 1").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::VoteSkip));
}

#[test]
fn a_guest_in_a_listening_room_cannot_use_its_buttons_or_add() {
    let mut listening = room();
    listening.mode = Mode::Listen;
    let mut harness = harness(seated(listening, "bob", 300.0));
    harness.get_by_label("Next song").click();
    harness.run();
    assert!(!asked_room(&harness, |ask| matches!(
        ask,
        Ask::Transport(_)
    )));
    assert!(harness.query_by_label("Find songs to add").is_none());
    assert!(harness.query_by_label("Play Second song").is_none());
}

#[test]
fn the_queue_plays_and_removes_its_songs() {
    let mut harness = harness(seated(room(), "ada", 300.0));
    harness.get_by_label("Queue · 2");
    harness.get_by_label("Play Second song").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Jump(entry) if entry == "e2"
    )));
    harness.get_by_label("Remove Second song").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Remove(entry) if entry == "e2"
    )));
    // The song that is playing is skipped, not removed.
    assert!(harness.query_by_label("Remove First song").is_none());
}

#[test]
fn an_edit_that_can_be_taken_back_is_offered_to_who_made_it() {
    let mut edited = room();
    edited.undo = Some(crate::together::protocol::Undo {
        revision: 3,
        expires: f64::MAX,
        by: "ada".into(),
    });
    let mut harness = harness(seated(edited.clone(), "ada", 300.0));
    harness.get_by_label("Undo edit").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::Undo));
    let harness = super::harness(seated(edited, "bob", 300.0));
    assert!(harness.query_by_label("Undo edit").is_none());
}

#[test]
fn the_tabs_show_the_history_and_what_has_happened() {
    let mut played = room();
    played.history = vec![entry("e0", "Earlier song", "bob")];
    played.activity = vec![Happening {
        id: "a1".into(),
        text: "Bob joined.".into(),
        at: 0.0,
    }];
    let mut harness = harness(seated(played.clone(), "ada", 300.0));
    harness.get_by_label("History").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::ShowTab(Tab::History)));

    let mut state = seated(played.clone(), "ada", 300.0);
    state.together.tab = Tab::History;
    let mut harness = super::harness(state);
    harness.get_by_label("Save as playlist").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::SaveHistory));
    harness.get_by_label("Add Earlier song to queue").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Add(track) if track.id == "e0"
    )));

    let mut state = seated(played, "ada", 300.0);
    state.together.tab = Tab::Activity;
    let harness = super::harness(state);
    harness.get_by_label("Bob joined.");
}

fn requested() -> Room {
    let mut room = room();
    room.mode = Mode::Contributions;
    let wanted = |id: &str, title: &str| SongRequest {
        id: id.into(),
        track: song("zyxwvutsrqp", title),
        by: person("bob", "Bob"),
    };
    room.requests = vec![wanted("q1", "Wanted"), wanted("q2", "Also wanted")];
    room
}

#[test]
fn the_leader_answers_requests_one_by_one_or_all_at_once() {
    let mut harness = harness(seated(requested(), "ada", 300.0));
    harness.get_by_label("Requests");
    harness.get_by_label("Play Wanted next").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Accept { requests, next: true } if requests == &["q1"]
    )));
    harness.get_by_label("Add Wanted to queue").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Accept { requests, next: false } if requests == &["q1"]
    )));
    harness.get_by_label("Decline Wanted from Bob").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Decline(requests) if requests == &["q1"]
    )));
    harness.get_by_label("Accept all").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Accept { requests, next: false } if requests.len() == 2
    )));
}

#[test]
fn a_guest_sees_their_own_requests_and_may_withdraw_them() {
    let mut harness = harness(seated(requested(), "bob", 300.0));
    harness.get_by_label("Your requests");
    assert!(harness.query_by_label("Accept all").is_none());
    harness
        .get_by_label("Cancel your request for Wanted")
        .click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Withdraw(request) if request == "q1"
    )));
    // What they find is asked for, not added.
    harness.get_by_role_and_label(Role::TextInput, "Find songs to request");
}

#[test]
fn the_rooms_search_adds_what_it_finds_or_plays_its_radio() {
    let mut state = seated(room(), "ada", 300.0);
    state.together.search.query = "daft".into();
    state.together.search.results = vec![track("abcdefghijk", "Found song")];
    let mut harness = harness(state);
    harness.get_by_label("Add Found song").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Add(track) if track.id == "abcdefghijk"
    )));
    harness.get_by_label("Play Found song radio").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(ask, Ask::Radio(_))));
    harness.get_by_label("Clear search").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Search(text) if text.is_empty()
    )));
    type_into(&mut harness, "Find songs to add", "x");
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Search(text) if !text.is_empty()
    )));
}

#[test]
fn a_search_says_when_it_is_looking_and_when_it_found_nothing() {
    let mut state = seated(room(), "ada", 300.0);
    state.together.search.query = "daft".into();
    state.together.search.searching = true;
    let harness = harness(state);
    harness.get_by_label("Searching…");

    let mut state = seated(room(), "ada", 300.0);
    state.together.search.query = "daft".into();
    let harness = super::harness(state);
    harness.get_by_label("No songs found. Try another search.");
}

#[test]
fn the_leader_manages_listeners_and_lets_in_those_who_wait() {
    let mut waiting = room();
    waiting.pending = vec![Knock {
        id: "p1".into(),
        name: "Cy".into(),
        avatar: Vec::new(),
    }];
    let mut harness = harness(seated(waiting, "ada", 150.0));
    harness.get_by_label("Waiting to join");
    harness.get_by_label("Accept Cy").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Admit(request) if request == "p1"
    )));
    harness.get_by_label("Decline Cy").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::TurnAway(request) if request == "p1"
    )));
    harness.get_by_label("Ada (you), Leader · listening");
    harness.get_by_label("Manage Bob").click();
    harness.run();
    harness.get_by_label("Make DJ").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::Role { member, dj: true } if member == "bob"
    )));
    harness.get_by_label("Manage Bob").click();
    harness.run();
    harness.get_by_label("Make leader").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::MakeLeader(member) if member == "bob"
    )));
    harness.get_by_label("Manage Bob").click();
    harness.run();
    harness.get_by_label("Remove & rotate PIN").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::RemoveListener { name, .. } if name == "Bob"
    )));
}

#[test]
fn a_listeners_own_preferences_are_theirs_to_set() {
    let mut harness = harness(seated(room(), "bob", 150.0));
    assert!(harness.query_by_label("Manage Ada").is_none());
    harness
        .get_by_role_and_label(Role::CheckBox, "Show room activity notifications")
        .click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::Notifications(true)));
    harness.get_by_label("Resync me").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::Resync));
}
