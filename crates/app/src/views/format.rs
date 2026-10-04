//! Numbers as people read them.

/// A track's length: `3:07`, or `1:02:45` past the hour.
pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// What the end of a seek bar says: the track's length, or, when asked for
/// the time left, that with a minus sign before it.
pub fn end_time(remaining: bool, position_ms: u64, duration_ms: u64) -> String {
    if remaining && duration_ms > 0 {
        format!(
            "\u{2212}{}",
            duration(duration_ms.saturating_sub(position_ms))
        )
    } else {
        duration(duration_ms)
    }
}

/// Room on disk: `340 MB`, or `1.2 GB` from a gigabyte up.
pub fn bytes(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    let megabytes = bytes as f64 / MB;
    if megabytes >= 1024.0 {
        format!("{:.1} GB", megabytes / 1024.0)
    } else {
        format!("{megabytes:.0} MB")
    }
}

/// The time of day of a moment given in milliseconds since 1970, on a
/// clock `zone_minutes` ahead of Greenwich: `21:05`.
pub fn clock(at_ms: f64, zone_minutes: i32) -> String {
    let minutes = (at_ms / 60_000.0).floor() as i64 + i64::from(zone_minutes);
    let of_day = minutes.rem_euclid(24 * 60);
    format!("{:02}:{:02}", of_day / 60, of_day % 60)
}

pub fn songs(count: usize) -> String {
    match count {
        1 => "1 song".to_owned(),
        count => format!("{count} songs"),
    }
}

/// The parts that are present, with a dot between neighbours, as the
/// headers of pages and the library write them.
pub fn middle_dotted<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A release's running time, as its page writes it: `1 hr 51 min`, or
/// `38 min 12 sec` under the hour.
pub fn release_length(ms: u64) -> String {
    let seconds = (ms + 500) / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match (hours, minutes, seconds) {
        (0, minutes, 0) => format!("{minutes} min"),
        (0, minutes, seconds) => format!("{minutes} min {seconds} sec"),
        (hours, 0, _) => format!("{hours} hr"),
        (hours, minutes, _) => format!("{hours} hr {minutes} min"),
    }
}

/// What the player bar shows for a time it does not have.
pub const NO_TIME: &str = "--:--";

/// Time listened, in minutes, and in hours and minutes past the hour.
pub fn listened(ms: u64) -> String {
    let minutes = (ms + 30_000) / 60_000;
    match (ms, minutes) {
        (0, _) => "0 min".to_owned(),
        (_, 0) => "<1 min".to_owned(),
        (_, minutes) if minutes < 60 => format!("{minutes} min"),
        (_, minutes) if minutes % 60 == 0 => format!("{} h", minutes / 60),
        (_, minutes) => format!("{} h {} min", minutes / 60, minutes % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_end_of_the_bar_says_the_length_or_what_is_left() {
        assert_eq!(end_time(false, 60_000, 187_000), "3:07");
        assert_eq!(end_time(true, 60_000, 187_000), "\u{2212}2:07");
        // Past the end, as a seek can briefly be, nothing is left.
        assert_eq!(end_time(true, 190_000, 187_000), "\u{2212}0:00");
        // A length that is not known has no time left to count.
        assert_eq!(end_time(true, 5000, 0), "0:00");
    }

    #[test]
    fn a_moment_reads_as_the_time_of_day_where_the_listener_is() {
        // 2026-01-01T21:05:30Z.
        let at = 1_767_301_530_000.0;
        assert_eq!(clock(at, 0), "21:05");
        assert_eq!(clock(at, 180), "00:05");
        assert_eq!(clock(at, -330), "15:35");
    }

    #[test]
    fn a_track_length_reads_as_a_clock() {
        assert_eq!(duration(0), "0:00");
        assert_eq!(duration(187_900), "3:07");
        assert_eq!(duration(3_765_000), "1:02:45");
    }

    #[test]
    fn a_release_length_reads_in_words() {
        assert_eq!(release_length(61 * 60_000 + 20_000), "1 hr 1 min");
        assert_eq!(release_length(2 * 3_600_000), "2 hr");
        assert_eq!(release_length(38 * 60_000 + 12_000), "38 min 12 sec");
        assert_eq!(release_length(4 * 60_000), "4 min");
    }

    #[test]
    fn present_parts_are_joined_with_a_dot() {
        assert_eq!(
            middle_dotted(["Album", "", "Daft Punk"]),
            "Album · Daft Punk"
        );
        assert_eq!(middle_dotted([""]), "");
    }

    #[test]
    fn room_on_disk_reads_in_megabytes_then_gigabytes() {
        assert_eq!(bytes(0), "0 MB");
        assert_eq!(bytes(340 * 1024 * 1024), "340 MB");
        assert_eq!(bytes(1288 * 1024 * 1024), "1.3 GB");
    }

    #[test]
    fn one_song_is_singular() {
        assert_eq!(songs(1), "1 song");
        assert_eq!(songs(13), "13 songs");
    }

    #[test]
    fn time_listened_is_said_in_minutes_and_then_in_hours_and_minutes() {
        assert_eq!(listened(0), "0 min");
        assert_eq!(listened(20_000), "<1 min");
        assert_eq!(listened(47 * 60_000), "47 min");
        assert_eq!(listened(120 * 60_000), "2 h");
        assert_eq!(listened(135 * 60_000), "2 h 15 min");
    }
}
