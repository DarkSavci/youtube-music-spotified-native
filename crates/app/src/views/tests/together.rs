//! The Listen Together page: the servers, the two ways into a room, and
//! what a room's page offers to the leader and to a guest.

use eframe::egui::accesskit::Role;

use super::*;
use crate::together::protocol::{Entry, Member, Mode, Person, Room, RoomTrack};
use crate::together::{Ask, Field, Phase, SavedServer, ServerForm};

pub(super) fn on_page() -> State {
    let mut state = state();
    state.nav.open(Page::Together);
    state.settings.together_servers = vec![SavedServer {
        id: "server-1".into(),
        name: "Ours".into(),
        url: "wss://listen.example.com".into(),
    }];
    state.settings.together_selected = "server-1".into();
    state
}

pub(super) fn member(id: &str, name: &str) -> Member {
    Member {
        id: id.into(),
        name: name.into(),
        connected: true,
        status: "listening".into(),
        role: "listener".into(),
        ..Member::default()
    }
}

pub(super) fn person(id: &str, name: &str) -> Person {
    Person {
        id: id.into(),
        name: name.into(),
        ..Person::default()
    }
}

pub(super) fn song(id: &str, title: &str) -> RoomTrack {
    RoomTrack {
        id: id.into(),
        title: title.into(),
        duration_ms: 200_000.0,
        ..RoomTrack::default()
    }
}

pub(super) fn entry(id: &str, title: &str, by: &str) -> Entry {
    Entry {
        id: id.into(),
        track: song(id, title),
        added_by: person(by, by),
        ..Entry::default()
    }
}

/// A room led by Ada with Bob in it, two songs queued and the first playing.
pub(super) fn room() -> Room {
    Room {
        pin: "01234567".into(),
        owner: "ada".into(),
        members: vec![member("ada", "Ada"), member("bob", "Bob")],
        queue: vec![
            entry("e1", "First song", "ada"),
            entry("e2", "Second song", "bob"),
        ],
        current: Some("e1".into()),
        playing: true,
        duplicates: true,
        limit: 50,
        revision: 3,
        ..Room::default()
    }
}

/// On the page, in `room`, as the member `me`, the page held `scrolled`
/// points down.
pub(super) fn seated(room: Room, me: &str, scrolled: f32) -> State {
    let mut state = on_page();
    state.together.phase = Phase::Joined;
    state.together.me = me.into();
    state.together.room = Some(room);
    state.held_scroll = Some(scrolled);
    state
}

/// Puts the caret in the field called `label` and types `text` there.
pub(super) fn type_into(harness: &mut Harness<'_, Fixture>, label: &str, text: &str) {
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .focus();
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, label)
        .type_text(text);
    harness.run();
}

pub(super) fn asked_room(harness: &Harness<'_, Fixture>, wanted: impl Fn(&Ask) -> bool) -> bool {
    asked(
        harness,
        |action| matches!(action, Action::Room(ask) if wanted(ask)),
    )
}

#[test]
fn the_start_page_names_itself_and_says_it_is_a_preview() {
    let harness = harness(on_page());
    harness.get_by_label("YOUR MUSIC, TOGETHER");
    harness.get_by_label("Preview · v2");
    harness.get_by_label("Start something good.");
    harness.get_by_label("Your friends are waiting.");
}

#[test]
fn the_server_is_chosen_from_those_saved() {
    let mut state = on_page();
    state.settings.together_servers.push(SavedServer {
        id: "server-2".into(),
        name: "Theirs".into(),
        url: "wss://other.example.com".into(),
    });
    let mut harness = harness(state);
    harness.get_by_label("Room server").click();
    harness.run();
    harness.get_by_label("Theirs").click();
    harness.run();
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::SelectServer(id) if id == "server-2"
    )));
}

#[test]
fn manage_opens_the_form_and_with_no_server_it_offers_to_add_one() {
    let mut harness = harness(on_page());
    harness.get_by_label("Manage").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::ToggleManage));

    let mut state = on_page();
    state.settings.together_selected.clear();
    let mut harness = super::harness(state);
    assert!(harness.query_by_label("Manage").is_none());
    harness.get_by_label("Add server").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::ToggleManage));
}

#[test]
fn the_servers_form_saves_tests_adds_another_and_removes() {
    let mut state = on_page();
    state.together.manage = Some(ServerForm {
        editing: Some("server-1".into()),
        name: "Ours".into(),
        url: "wss://listen.example.com".into(),
        check: "Ready for v2 rooms".into(),
    });
    let mut harness = harness(state);
    harness.get_by_label("Edit server");
    harness.get_by_label("Ready for v2 rooms");
    for (label, wanted) in [
        ("Save server", Ask::SaveServer),
        ("Test connection", Ask::TestServer),
        ("Add another", Ask::AddAnother),
        ("Remove saved server", Ask::RemoveServer),
        ("Cancel", Ask::ToggleManage),
    ] {
        harness.get_by_label(label).click();
        harness.run();
        assert!(asked_room(&harness, |ask| *ask == wanted), "{label}");
    }
    type_into(&mut harness, "Server address", "x");
    assert!(asked_room(&harness, |ask| matches!(
        ask,
        Ask::ServerAddress(_)
    )));
}

#[test]
fn a_room_is_made_as_the_chosen_kind() {
    let mut state = on_page();
    state.held_scroll = Some(260.0);
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::RadioButton, "Take requests")
        .click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogetherMode(Mode::Contributions)
    )));
    harness.get_by_label("Create room").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogetherCreate
    )));
}

#[test]
fn a_room_is_joined_only_with_a_whole_pin() {
    let mut harness = harness(on_page());
    harness.get_by_label("Join room").click();
    harness.run();
    assert!(!asked(&harness, |action| matches!(
        action,
        Action::TogetherJoin
    )));
    type_into(&mut harness, "Room PIN", "0123");
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogetherField(Field::Pin, typed) if typed == "0123"
    )));

    let mut state = on_page();
    state.together.form.pin = "01234567".into();
    let mut harness = super::harness(state);
    harness.get_by_label("Join room").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogetherJoin
    )));
}

#[test]
fn the_picture_is_offered_only_to_an_account_that_has_one() {
    let harness = harness(on_page());
    assert!(harness.query_by_label("Share my profile picture").is_none());
    let mut state = on_page();
    state.account = Some(spotified_client::models::Account {
        name: "Ada".into(),
        avatar_url: "https://yt3.ggpht.com/ada=s88".into(),
        ..Default::default()
    });
    let mut harness = super::harness(state);
    harness.get_by_label("Share my profile picture").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::SharePicture(true)));
}

#[test]
fn waiting_for_the_leader_can_be_given_up() {
    let mut state = on_page();
    state.together.phase = Phase::Waiting;
    state.held_scroll = Some(420.0);
    let mut harness = harness(state);
    harness.get_by_label("Waiting for the leader…");
    harness.get_by_label("Cancel request").click();
    harness.run();
    assert!(asked(&harness, |action| matches!(
        action,
        Action::TogetherLeave
    )));
}

#[test]
fn an_error_on_the_page_is_dismissed() {
    let mut state = on_page();
    state.together.error = Some("The leader ended the room.".into());
    let mut harness = harness(state);
    harness.get_by_label("The leader ended the room.");
    harness.get_by_label("Dismiss error").click();
    harness.run();
    assert!(asked_room(&harness, |ask| *ask == Ask::DismissError));
}
