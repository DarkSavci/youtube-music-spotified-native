use super::*;

fn song(id: &str, title: &str, album: (&str, &str), plays: &str) -> Track {
    Track {
        id: id.into(),
        title: title.into(),
        artists: vec![who()],
        album: (!album.0.is_empty() || !album.1.is_empty()).then(|| AlbumRef {
            id: album.0.into(),
            name: album.1.into(),
        }),
        play_count: plays.into(),
        playable: true,
        ..Track::default()
    }
}

fn who() -> ArtistRef {
    ArtistRef {
        id: "UC-artist".into(),
        name: "The Artist".into(),
    }
}

fn release(id: &str, title: &str, year: &str) -> Album {
    Album {
        id: id.into(),
        title: title.into(),
        year: year.into(),
        artists: vec![who()],
        ..Album::default()
    }
}

fn ids(tracks: &[Track]) -> Vec<&str> {
    tracks.iter().map(|track| track.id.as_str()).collect()
}

#[test]
fn a_play_count_is_read_with_its_scale() {
    assert_eq!(play_count("1.9B plays"), 1.9e9);
    assert_eq!(play_count("23M plays"), 23e6);
    assert_eq!(play_count("1,204 plays"), 1204.0);
    assert_eq!(play_count("no idea"), 0.0);
}

#[test]
fn songs_are_put_most_played_first_only_when_every_one_has_a_count() {
    let counted = vec![
        song("a", "A", ("", ""), "2M plays"),
        song("b", "B", ("", ""), "1.1B plays"),
        song("c", "C", ("", ""), "2M plays"),
    ];
    assert_eq!(ids(&by_plays(counted)), ["b", "a", "c"]);
    let one_without = vec![
        song("a", "A", ("", ""), "2M plays"),
        song("b", "B", ("", ""), ""),
    ];
    assert_eq!(ids(&by_plays(one_without)), ["a", "b"]);
}

#[test]
fn a_shuffle_keeps_every_song() {
    let mut numbers = [0.9, 0.1, 0.5, 0.3].into_iter().cycle();
    let mut mixed = shuffled(vec![1, 2, 3, 4, 5], || numbers.next().unwrap_or(0.0));
    assert_ne!(mixed, [1, 2, 3, 4, 5]);
    mixed.sort_unstable();
    assert_eq!(mixed, [1, 2, 3, 4, 5]);
    let mut random = random();
    assert!((0..100).all(|_| (0.0..1.0).contains(&random())));
}

#[test]
fn an_edition_is_taken_off_a_releases_name() {
    assert_eq!(edition_title("Discovery (Deluxe Edition)"), "Discovery");
    assert_eq!(edition_title("Homework [Remastered 2011]"), "Homework");
    assert_eq!(edition_title("Alive 2007 - Expanded Edition"), "Alive 2007");
    assert_eq!(
        edition_title("Thriller (Deluxe) (25th Anniversary Edition)"),
        "Thriller"
    );
    // Brackets that say something else are part of the name.
    assert_eq!(
        edition_title("Human After All (Live)"),
        "Human After All (Live)"
    );
    assert_eq!(edition_title("(Deluxe)"), "(Deluxe)");
}

#[test]
fn a_compilation_is_known_by_its_title() {
    assert!(is_compilation("Greatest Hits, Vol. 2"));
    assert!(is_compilation("En İyileri"));
    assert!(!is_compilation("Recollections"));
}

#[test]
fn the_editions_of_a_release_stand_under_the_earliest() {
    let albums = [
        release("deluxe", "Discovery (Deluxe Edition)", "2011"),
        release("plain", "Discovery", "2001"),
        release("other", "Homework", "1997"),
    ];
    let canon = editions(&albums);
    assert_eq!(canon["deluxe"].id, "plain");
    assert_eq!(canon["deluxe"].year, Some(2001));
    assert_eq!(canon["other"].title, "Homework");
}

#[test]
fn a_song_on_a_release_is_added_once_and_only_when_it_credits_the_artist() {
    let listed = [
        song("a", "One More Time", ("hits", "Greatest Hits"), "9M plays"),
        song("a2", "One more  time", ("disc", "Discovery"), "1M plays"),
    ];
    let mut album = release("disc", "Discovery", "2001");
    let mut guest = song("g", "Someone Else's", ("", ""), "");
    guest.artists = vec![ArtistRef {
        id: "UC-other".into(),
        name: "Other".into(),
    }];
    album.tracks = vec![
        song("a3", "One More Time", ("", ""), ""),
        song("b", "Aerodynamic", ("", ""), ""),
        guest,
    ];
    let all = with_releases(&listed, &[&album], &who());
    // The copy on its own album takes the greatest-hits copy's place.
    assert_eq!(ids(&all), ["a2", "b"]);
    assert_eq!(
        all[1].album.as_ref().map(|album| album.id.as_str()),
        Some("disc")
    );
}

#[test]
fn newest_first_puts_songs_of_unknown_year_last() {
    let tracks = [
        song("old", "Old", ("o", "Old"), ""),
        song("none", "None", ("", ""), ""),
        song("new", "New", ("n", "New"), ""),
        song("old2", "Old two", ("o", "Old"), ""),
    ];
    let years = HashMap::from([("o".to_owned(), 1997), ("n".to_owned(), 2013)]);
    let canon = HashMap::new();
    let dates = Dates {
        years: &years,
        canon: &canon,
    };
    assert_eq!(
        ids(&newest_first(&tracks, &dates)),
        ["new", "old", "old2", "none"]
    );
}

#[test]
fn songs_are_grouped_by_album_newest_first_with_the_albumless_last() {
    let tracks = [
        song("none", "None", ("", ""), ""),
        song("old", "Old", ("o", "Old"), ""),
        song("undated", "Undated", ("u", "Undated"), ""),
        song("new", "New", ("n-deluxe", "New (Deluxe)"), ""),
    ];
    let albums = [
        release("n", "New", "2013"),
        release("n-deluxe", "New (Deluxe)", "2014"),
        release("o", "Old", "1997"),
    ];
    let years = years_by_album(&albums);
    let canon = editions(&albums);
    let dates = Dates {
        years: &years,
        canon: &canon,
    };
    let groups = by_album(&tracks, &dates);
    let titles: Vec<&str> = groups.iter().map(|group| group.title.as_str()).collect();
    assert_eq!(titles, ["New", "Old", "Undated", "Other songs"]);
    assert_eq!(groups[0].id, "n");
    assert_eq!(groups[0].year, Some(2013));
}

#[test]
fn releases_are_opened_undated_first_and_covered_singles_last() {
    let albums = [
        release("al", "Album", "2001"),
        release("hits", "Best Of", "2010"),
    ];
    let singles = [
        release("s1", "Known Song", "2000"),
        release("s2", "New Song", "2002"),
    ];
    let known = [song("k", "Known song", ("", ""), "")];
    let plan = release_plan(
        &["mystery".to_owned()],
        &albums.iter().collect::<Vec<_>>(),
        &singles.iter().collect::<Vec<_>>(),
        &known,
    );
    assert_eq!(plan.order, ["mystery", "al", "s2", "s1"]);
    assert!(plan.covered.contains("s1"));
    assert_eq!(
        missing_albums(&known, &HashMap::new()),
        Vec::<String>::new()
    );
}
