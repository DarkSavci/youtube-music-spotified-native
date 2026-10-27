use spotified_client::models::{Album, ArtistRef};

use super::*;
use crate::actions::Shared;
use crate::services::{Found, FoundKind, Service, Wanted};

fn song() -> Track {
    Track {
        id: "abc".into(),
        title: "One More Time".into(),
        artists: vec![ArtistRef {
            id: "UCx".into(),
            name: "Daft Punk".into(),
        }],
        ..Track::default()
    }
}

#[test]
fn a_song_is_looked_for_on_another_service_by_its_names() {
    let mut state = state();
    let effects = apply(
        &mut state,
        Action::ShareOn(Service::Spotify, Shared::Song(Box::new(song()))),
    );
    assert_eq!(
        effects,
        [Effect::FindOn(
            Service::Spotify,
            Wanted::Song {
                title: "One More Time".into(),
                artist: "Daft Punk".into(),
                album: String::new(),
            }
        )]
    );
}

#[test]
fn an_album_not_yet_read_is_read_first_and_then_looked_for() {
    let mut state = ready();
    let effects = apply(
        &mut state,
        Action::ShareOn(Service::Deezer, Shared::Album("MPREb_1".into())),
    );
    assert_eq!(effects, [Effect::Fetch(Request::Album("MPREb_1".into()))]);
    let album = Album {
        id: "MPREb_1".into(),
        title: "Discovery".into(),
        artists: vec![ArtistRef {
            id: "UCx".into(),
            name: "Daft Punk".into(),
        }],
        tracks: vec![song()],
        ..Album::default()
    };
    let read = Response::Album("MPREb_1".into(), Ok(album));
    let effects = apply(&mut state, Action::Loaded(Box::new(read)));
    assert!(effects.contains(&Effect::FindOn(
        Service::Deezer,
        Wanted::Album {
            title: "Discovery".into(),
            artist: "Daft Punk".into(),
            songs: vec!["One More Time".into()],
        }
    )));
    // Read again later, it is not looked for twice.
    let again = Response::Album("MPREb_1".into(), Ok(Album::default()));
    let effects = apply(&mut state, Action::Loaded(Box::new(again)));
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::FindOn(..)))
    );
}

#[test]
fn a_link_found_is_copied_and_one_not_found_is_said_to_be_missing() {
    let mut state = state();
    let effects = apply(
        &mut state,
        Action::FoundOn {
            service: Service::AppleMusic,
            noun: "song",
            found: Ok(Some("https://music.apple.com/x".into())),
        },
    );
    assert_eq!(
        effects,
        [Effect::CopyToClipboard("https://music.apple.com/x".into())]
    );
    let effects = apply(
        &mut state,
        Action::FoundOn {
            service: Service::Spotify,
            noun: "song",
            found: Ok(None),
        },
    );
    assert!(effects.is_empty());
    let said = state.toasts.last().map(|toast| toast.text.as_str());
    assert_eq!(said, Some("Couldn't find this song on Spotify"));
}

#[test]
fn a_link_pasted_into_search_is_read_and_what_it_is_to_searched_for() {
    let mut state = ready();
    let link = "https://open.spotify.com/track/5F0fiSkDxZL5V3SlnTNhKh";
    let effects = apply(&mut state, Action::SetSearchQuery(link.into()));
    assert_eq!(effects, [Effect::ReadLink(link.into())]);

    let read = Found {
        kind: FoundKind::Song,
        title: "One More Time".into(),
        artist: "Daft Punk".into(),
    };
    let effects = apply(
        &mut state,
        Action::LinkRead {
            link: link.into(),
            read: Ok(read),
        },
    );
    assert_eq!(state.search.query, "One More Time Daft Punk");
    assert_eq!(state.search.filter, SearchFilter::Songs);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        Effect::Fetch(Request::Search { query, .. }) if query == "One More Time Daft Punk"
    )));
}

#[test]
fn a_link_read_after_something_else_was_typed_is_let_go() {
    let mut state = ready();
    let link = "https://www.deezer.com/track/3135556";
    apply(&mut state, Action::SetSearchQuery(link.into()));
    apply(&mut state, Action::SetSearchQuery("daft punk".into()));
    let read = Found {
        kind: FoundKind::Song,
        title: "Harder, Better, Faster, Stronger".into(),
        artist: "Daft Punk".into(),
    };
    let effects = apply(
        &mut state,
        Action::LinkRead {
            link: link.into(),
            read: Ok(read),
        },
    );
    assert!(effects.is_empty());
    assert_eq!(state.search.query, "daft punk");
}
