//! Listen Together before a room: the saved servers, and who and what a
//! room is entered as.

use super::room::{ask, leading};
use super::together::with_server;
use super::*;
use crate::state::Dialog;
use crate::together::protocol::Mode;
use crate::together::{Ask, Phase, SavedServer};

#[test]
fn a_room_cannot_be_entered_before_a_server_is_saved_and_the_form_opens() {
    let mut state = ready();
    assert!(apply(&mut state, Action::TogetherCreate).is_empty());
    assert_eq!(state.together.phase, Phase::Idle);
    assert!(state.together.manage.is_some());
}

#[test]
fn a_server_is_saved_chosen_and_kept() {
    let mut state = ready();
    ask(&mut state, Ask::ToggleManage);
    ask(&mut state, Ask::ServerName("Ours".into()));
    ask(&mut state, Ask::ServerAddress("ws://localhost:8791".into()));
    assert_eq!(ask(&mut state, Ask::SaveServer), [Effect::SaveSettings]);
    assert!(state.together.manage.is_none());
    let saved = SavedServer {
        id: "server-1".into(),
        name: "Ours".into(),
        url: "ws://localhost:8791".into(),
    };
    assert_eq!(
        state.settings.together_servers,
        std::slice::from_ref(&saved)
    );
    assert_eq!(state.settings.together_server(), Some(&saved));
}

#[test]
fn an_address_that_cannot_be_a_server_is_said_and_the_form_stays() {
    let mut state = ready();
    ask(&mut state, Ask::ToggleManage);
    ask(&mut state, Ask::ServerAddress("http://example.com".into()));
    assert!(ask(&mut state, Ask::SaveServer).is_empty());
    assert!(state.together.manage.is_some());
    assert_eq!(
        state.together.error.as_deref(),
        Some("Use wss:// for a remote server, or ws://localhost for local testing.")
    );
}

#[test]
fn managing_opens_on_the_chosen_server_to_rename_it_or_add_another() {
    let mut state = with_server();
    ask(&mut state, Ask::ToggleManage);
    let form = state.together.manage.clone().unwrap_or_default();
    assert_eq!(form.editing.as_deref(), Some("server-1"));
    assert_eq!(form.url, "wss://listen.example.com");
    ask(&mut state, Ask::ServerName("Renamed".into()));
    ask(&mut state, Ask::SaveServer);
    assert_eq!(state.settings.together_servers.len(), 1);
    assert_eq!(state.settings.together_servers[0].name, "Renamed");
    // Another one: the form empties, and saving adds rather than replaces.
    ask(&mut state, Ask::ToggleManage);
    ask(&mut state, Ask::AddAnother);
    ask(
        &mut state,
        Ask::ServerAddress("wss://other.example.com".into()),
    );
    ask(&mut state, Ask::SaveServer);
    assert_eq!(state.settings.together_servers.len(), 2);
    assert_eq!(state.settings.together_selected, "server-2");
    assert_eq!(
        ask(&mut state, Ask::SelectServer("server-1".into())),
        [Effect::SaveSettings]
    );
    assert_eq!(state.settings.together_selected, "server-1");
}

#[test]
fn removing_a_server_is_asked_about_first_and_leaves_none_chosen() {
    let mut state = with_server();
    ask(&mut state, Ask::ToggleManage);
    ask(&mut state, Ask::RemoveServer);
    assert!(matches!(
        &state.dialog,
        Some(Dialog::RemoveServer { id, name }) if id == "server-1" && name == "Ours"
    ));
    assert_eq!(
        apply(&mut state, Action::ConfirmDialog),
        [Effect::SaveSettings]
    );
    assert!(state.settings.together_servers.is_empty());
    assert!(state.settings.together_selected.is_empty());
    assert!(state.together.manage.is_none());
}

#[test]
fn testing_a_server_probes_its_address_and_says_what_came_of_it() {
    let mut state = with_server();
    ask(&mut state, Ask::ToggleManage);
    assert_eq!(
        ask(&mut state, Ask::TestServer),
        [Effect::TogetherProbe("wss://listen.example.com".into())]
    );
    let check = |state: &State| {
        state
            .together
            .manage
            .as_ref()
            .map(|form| form.check.clone())
    };
    assert_eq!(check(&state).as_deref(), Some("Checking…"));
    // A second press while it is being tested does not test it twice.
    assert!(ask(&mut state, Ask::TestServer).is_empty());
    ask(&mut state, Ask::Tested(Ok(())));
    assert_eq!(check(&state).as_deref(), Some("Ready for v2 rooms"));
    ask(&mut state, Ask::Tested(Err("No v2 handshake.".into())));
    assert_eq!(check(&state).as_deref(), Some("No v2 handshake."));
    // An address that is not one is said without asking anything of it.
    ask(&mut state, Ask::ServerAddress("nowhere".into()));
    assert!(ask(&mut state, Ask::TestServer).is_empty());
    assert_eq!(
        check(&state).as_deref(),
        Some("Enter the server's address, starting with wss://")
    );
}

#[test]
fn the_server_is_not_changed_under_a_room() {
    let mut state = leading();
    assert!(ask(&mut state, Ask::SelectServer("other".into())).is_empty());
    assert_eq!(state.settings.together_selected, "server-1");
    ask(&mut state, Ask::ToggleManage);
    assert!(state.together.manage.is_none());
}

#[test]
fn the_picture_is_shared_only_when_asked_and_only_if_there_is_one() {
    let mut state = with_server();
    state.account = Some(spotified_client::models::Account {
        name: "Ada".into(),
        avatar_url: "https://yt3.ggpht.com/ada=s88".into(),
        ..Default::default()
    });
    let avatar = |state: &mut State| match &apply(state, Action::TogetherCreate)[..] {
        [_, Effect::TogetherConnect(options)] => options.avatar.clone(),
        other => panic!("a connection, not {other:?}"),
    };
    assert_eq!(avatar(&mut state), "");
    state.together.reset(None);
    assert_eq!(
        ask(&mut state, Ask::SharePicture(true)),
        [Effect::SaveSettings]
    );
    assert_eq!(avatar(&mut state), "https://yt3.ggpht.com/ada=s88");
}

#[test]
fn the_room_made_is_named_and_run_as_last_chosen() {
    let mut state = with_server();
    apply(
        &mut state,
        Action::TogetherField(crate::together::Field::RoomName, " Friday ".into()),
    );
    apply(&mut state, Action::TogetherMode(Mode::Listen));
    assert_eq!(state.settings.together_room_name, " Friday ");
    let effects = apply(&mut state, Action::TogetherCreate);
    assert!(matches!(
        &effects[1],
        Effect::TogetherConnect(options) if options.enter == crate::together::Enter::Create {
            mode: Mode::Listen,
            room_name: "Friday".into(),
        }
    ));
}
