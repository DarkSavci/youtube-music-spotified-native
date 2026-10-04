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

/// A collection's length: `43 min`, or `1 hr 14 min`.
pub fn long_duration(ms: u64) -> String {
    let minutes = ms / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, minutes) => format!("{hours} hr {minutes} min"),
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

pub fn songs(count: usize) -> String {
    match count {
        1 => "1 song".to_owned(),
        count => format!("{count} songs"),
    }
}

/// The parts that are present, joined with a bullet.
pub fn bulleted<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" • ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_length_reads_as_a_clock() {
        assert_eq!(duration(0), "0:00");
        assert_eq!(duration(187_900), "3:07");
        assert_eq!(duration(3_765_000), "1:02:45");
    }

    #[test]
    fn a_collection_length_reads_in_hours_and_minutes() {
        assert_eq!(long_duration(43 * 60_000), "43 min");
        assert_eq!(long_duration(74 * 60_000 + 43_000), "1 hr 14 min");
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
    fn missing_parts_leave_no_stray_bullets() {
        assert_eq!(bulleted(["Album", "", "2013"]), "Album • 2013");
        assert_eq!(bulleted(["", ""]), "");
    }
}
