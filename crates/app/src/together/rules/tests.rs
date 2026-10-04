use spotified_client::models::Track;

use super::super::protocol::{Member, Person, RoomTrack, Undo};
use super::*;

fn member(id: &str, role: &str) -> Member {
    Member {
        id: id.into(),
        name: id.into(),
        role: role.into(),
        connected: true,
        ..Member::default()
    }
}

fn entry(id: &str, track: &str, by: &str) -> Entry {
    Entry {
        id: id.into(),
        track: RoomTrack {
            id: track.into(),
            title: track.to_uppercase(),
            ..RoomTrack::default()
        },
        added_by: Person {
            id: by.into(),
            name: by.into(),
            ..Person::default()
        },
        ..Entry::default()
    }
}

fn request(id: &str, track: &str, by: &str) -> SongRequest {
    SongRequest {
        id: id.into(),
        track: RoomTrack {
            id: track.into(),
            title: track.to_uppercase(),
            ..RoomTrack::default()
        },
        by: Person {
            id: by.into(),
            name: by.into(),
            ..Person::default()
        },
    }
}

/// A room that takes requests: a leader, a DJ and a guest, three songs in
/// the queue with the second playing.
fn room() -> Room {
    Room {
        owner: "leader".into(),
        mode: Mode::Contributions,
        members: vec![
            member("leader", "listener"),
            member("dj", "dj"),
            member("guest", "listener"),
        ],
        queue: vec![
            entry("e1", "one", "leader"),
            entry("e2", "two", "leader"),
            entry("e3", "three", "guest"),
        ],
        current: Some("e2".into()),
        revision: 4,
        ..Room::default()
    }
}

fn track(id: &str) -> Track {
    Track {
        id: id.into(),
        playable: true,
        ..Track::default()
    }
}

#[test]
fn a_guests_additions_wait_for_approval_only_where_the_leader_approves() {
    let mut room = room();
    assert!(room.requesting("guest"));
    assert!(!room.requesting("dj"));
    assert!(!room.requesting("leader"));
    room.auto_accept = true;
    assert!(!room.requesting("guest"));
    room.auto_accept = false;
    room.mode = Mode::Listen;
    assert!(!room.requesting("guest"));
    assert!(!room.may_add("guest"));
    assert!(room.may_add("dj"));
    room.mode = Mode::Collaborative;
    assert!(!room.requesting("guest"));
    assert!(room.may_add("guest"));
}

#[test]
fn the_leader_and_djs_answer_requests_and_a_guest_sees_only_their_own() {
    let mut room = room();
    room.requests = vec![request("r1", "aaa", "guest"), request("r2", "bbb", "other")];
    assert!(room.answers("leader"));
    assert!(room.answers("dj"));
    assert!(!room.answers("guest"));
    assert_eq!(room.requests_for("dj").count(), 2);
    let own: Vec<&str> = room.requests_for("guest").map(|r| r.id.as_str()).collect();
    assert_eq!(own, ["r1"]);
}

#[test]
fn a_guest_may_remove_only_what_they_added_and_nobody_the_current_song() {
    let room = room();
    let [first, current, theirs] = &room.queue[..] else {
        panic!("three entries");
    };
    assert!(room.may_remove("guest", theirs));
    assert!(!room.may_remove("guest", first));
    assert!(room.may_remove("leader", first));
    assert!(!room.may_remove("leader", current));
}

#[test]
fn an_edit_is_taken_back_by_who_made_it_or_the_leader_and_only_in_time() {
    let mut room = room();
    room.undo = Some(Undo {
        revision: 4,
        expires: 10_000.0,
        by: "dj".into(),
    });
    assert!(room.may_undo("dj", 9000.0));
    assert!(room.may_undo("leader", 9000.0));
    assert!(!room.may_undo("guest", 9000.0));
    assert!(!room.may_undo("dj", 10_001.0));
    // The room has moved on since the edit: the relay would refuse it.
    room.revision = 5;
    assert!(!room.may_undo("dj", 9000.0));
}

#[test]
fn the_queue_is_shown_from_the_song_that_is_playing() {
    let mut room = room();
    let shown: Vec<usize> = room.upcoming().map(|(index, _)| index).collect();
    assert_eq!(shown, [1, 2]);
    assert_eq!(room.left(), 1);
    assert_eq!(room.next_entry().map(|entry| entry.id.as_str()), Some("e3"));
    room.current = None;
    assert_eq!(room.upcoming().count(), 3);
}

#[test]
fn only_the_connected_count_towards_a_ready_check() {
    let mut room = room();
    room.members[0].ready = true;
    room.members[1].ready = true;
    room.members[1].connected = false;
    assert_eq!(room.ready(), (1, 2));
}

#[test]
fn the_leader_is_told_of_a_new_request_and_of_several_in_one_line() {
    let before = room();
    let mut after = room();
    after.requests = vec![request("r1", "aaa", "guest")];
    let mut withdrawn = Vec::new();
    assert_eq!(
        request_news(&before, &after, "leader", &mut withdrawn),
        ["guest requested “AAA”."]
    );
    after.requests.push(request("r2", "bbb", "guest"));
    assert_eq!(
        request_news(&before, &after, "dj", &mut withdrawn),
        ["2 new song requests."]
    );
}

#[test]
fn a_guest_is_told_their_request_was_sent_and_not_about_anyone_elses() {
    let before = room();
    let mut after = room();
    after.requests = vec![request("r1", "aaa", "guest"), request("r2", "bbb", "other")];
    let mut withdrawn = Vec::new();
    assert_eq!(
        request_news(&before, &after, "guest", &mut withdrawn),
        ["Request sent. The leader decides what plays."]
    );
    assert!(request_news(&after, &after, "guest", &mut withdrawn).is_empty());
}

#[test]
fn a_guest_is_told_what_became_of_their_requests() {
    let mut before = room();
    before.requests = vec![request("r1", "aaa", "guest"), request("r2", "bbb", "guest")];
    let mut withdrawn = Vec::new();
    // One accepted: it is in the queue, marked with the request it was.
    let mut after = room();
    after.requests = vec![request("r2", "bbb", "guest")];
    let mut accepted = entry("e4", "aaa", "guest");
    accepted.request = Some("r1".into());
    after.queue.push(accepted);
    assert_eq!(
        request_news(&before, &after, "guest", &mut withdrawn),
        ["“AAA” was added to the queue."]
    );
    // The other declined.
    let mut declined = after.clone();
    declined.requests.clear();
    assert_eq!(
        request_news(&after, &declined, "guest", &mut withdrawn),
        ["Your request for “BBB” wasn’t added."]
    );
    // Both answered at once, one each way.
    assert_eq!(
        request_news(&before, &declined, "guest", &mut withdrawn),
        ["1 of your requests was added to the queue, 1 wasn’t."]
    );
}

#[test]
fn a_request_the_guest_withdrew_is_not_said_to_have_been_refused() {
    let mut before = room();
    before.requests = vec![request("r1", "aaa", "guest")];
    let after = room();
    let mut withdrawn = vec!["r1".to_owned()];
    assert!(request_news(&before, &after, "guest", &mut withdrawn).is_empty());
    assert!(withdrawn.is_empty());
}

#[test]
fn radio_leaves_out_what_the_room_has_and_has_had_and_what_cannot_play() {
    let mut room = room();
    room.history = vec![entry("h1", "old", "leader")];
    let mut unplayable = track("gone");
    unplayable.playable = false;
    let found = vec![
        track("two"),
        track("old"),
        track("heard"),
        track("seed"),
        unplayable,
        track("new"),
        track("new"),
    ];
    let fresh = fresh_radio(found, "seed", &room, &["heard".to_owned()]);
    let ids: Vec<&str> = fresh.iter().map(|track| track.id.as_str()).collect();
    assert_eq!(ids, ["new"]);
}

#[test]
fn only_so_much_radio_waits_in_a_room_at_once() {
    let mut room = room();
    assert_eq!(room.radio_allowance(), 50);
    for index in 0..48 {
        let mut waiting = entry(&format!("r{index}"), &format!("radio{index}"), "leader");
        waiting.radio = true;
        room.queue.push(waiting);
    }
    assert_eq!(room.radio_allowance(), 2);
    let found = (0..10).map(|index| track(&format!("x{index}"))).collect();
    assert_eq!(fresh_radio(found, "seed", &room, &[]).len(), 2);
}

#[test]
fn what_the_room_has_played_is_remembered_most_recent_last() {
    let mut room = room();
    room.history = vec![entry("h1", "old", "leader")];
    let mut heard = vec!["one".to_owned(), "before".to_owned()];
    remember_heard(&mut heard, &room);
    assert_eq!(heard, ["before", "old", "one", "two"]);
}

#[test]
fn a_seek_in_the_room_is_told_from_the_song_running_on() {
    let mut before = room();
    before.playing = true;
    before.position_ms = 10_000.0;
    before.at = 1000.0;
    let mut after = before.clone();
    after.position_ms = 12_000.0;
    after.at = 3000.0;
    assert!(!jumped(&before, &after));
    after.position_ms = 60_000.0;
    assert!(jumped(&before, &after));
    // Another song is not a seek.
    after.current = Some("e3".into());
    assert!(!jumped(&before, &after));
}
