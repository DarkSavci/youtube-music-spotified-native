use super::*;

struct At(i64);

impl Timed for At {
    fn time(&self) -> i64 {
        self.0
    }
}

const FRAME: i64 = SECOND / 25;

fn waiting(times: &[i64]) -> VecDeque<At> {
    times.iter().map(|time| At(*time)).collect()
}

fn clock(position: i64, playing: bool, speed: f32) -> Clock {
    Clock {
        position,
        playing,
        speed,
        read: Instant::now(),
    }
}

#[test]
fn a_playing_song_moves_on_from_its_reading_at_its_speed() {
    let reading = clock(10 * SECOND, true, 1.5);
    let later = reading.read + Duration::from_secs(2);
    assert_eq!(reading.at(later), 13 * SECOND);
    assert_eq!(reading.at(reading.read), 10 * SECOND);
}

#[test]
fn a_paused_song_stays_where_it_was_read() {
    let reading = clock(10 * SECOND, false, 1.0);
    assert_eq!(
        reading.at(reading.read + Duration::from_secs(60)),
        10 * SECOND
    );
}

#[test]
fn a_reading_that_follows_from_the_last_is_no_news() {
    let first = clock(10 * SECOND, true, 1.0);
    let mut next = first;
    next.read = first.read + Duration::from_millis(40);
    next.position = first.position + FRAME;
    assert!(!first.surprised_by(&next));
}

#[test]
fn a_seek_a_pause_and_a_change_of_speed_are_news() {
    let first = clock(10 * SECOND, true, 1.0);
    let mut sought = first;
    sought.position = 90 * SECOND;
    assert!(first.surprised_by(&sought));
    let mut paused = first;
    paused.playing = false;
    assert!(first.surprised_by(&paused));
    let mut faster = first;
    faster.speed = 1.5;
    assert!(first.surprised_by(&faster));
}

#[test]
fn the_picture_shown_is_the_last_whose_time_has_come() {
    let mut queue = waiting(&[0, FRAME, 2 * FRAME, 3 * FRAME]);
    let shown = due(&mut queue, 2 * FRAME + 10).map(|picture| picture.0);
    assert_eq!(shown, Some(2 * FRAME));
    // Those before it went with it; the one still to come waits.
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.front().map(|picture| picture.0), Some(3 * FRAME));
}

#[test]
fn a_picture_whose_time_has_not_come_waits() {
    let mut queue = waiting(&[5 * FRAME, 6 * FRAME]);
    assert!(due(&mut queue, 5 * FRAME - 1).is_none());
    assert_eq!(queue.len(), 2);
    // On the instant it is due, it shows.
    assert_eq!(
        due(&mut queue, 5 * FRAME).map(|picture| picture.0),
        Some(5 * FRAME)
    );
}

#[test]
fn pictures_from_before_a_jump_are_not_shown() {
    // The song jumped a minute ahead; what waits is from where it was.
    let mut queue = waiting(&[40 * SECOND, 40 * SECOND + FRAME]);
    assert!(due(&mut queue, 100 * SECOND).is_none());
    assert!(queue.is_empty());
}

#[test]
fn the_window_waits_for_the_next_picture_by_the_songs_speed() {
    let queue = waiting(&[SECOND]);
    let half = Duration::from_millis(500);
    assert_eq!(until_next(&queue, SECOND / 2, 1.0), Some(half));
    assert_eq!(until_next(&queue, SECOND / 2, 2.0), Some(half / 2));
    assert_eq!(until_next(&queue, SECOND / 2, 0.5), Some(half * 2));
    // One that is overdue is due now.
    assert_eq!(until_next(&queue, 2 * SECOND, 1.0), Some(Duration::ZERO));
    assert_eq!(until_next(&waiting(&[]), 0, 1.0), None);
}

#[test]
fn nothing_decoded_yet_means_a_jump_to_where_the_song_is() {
    assert!(must_jump(30 * SECOND, None));
}

#[test]
fn a_song_within_reach_is_decoded_towards_not_jumped_to() {
    let span = Some(Span {
        from: 30 * SECOND,
        to: 31 * SECOND,
    });
    assert!(!must_jump(30 * SECOND + FRAME, span));
    // A little ahead of what is decoded: quicker to decode on.
    assert!(!must_jump(32 * SECOND, span));
    // A report a moment old does not send the decoder back.
    assert!(!must_jump(30 * SECOND - SECOND / 10, span));
}

#[test]
fn a_seek_either_way_is_a_jump() {
    let span = Some(Span {
        from: 30 * SECOND,
        to: 31 * SECOND,
    });
    assert!(must_jump(100 * SECOND, span));
    assert!(must_jump(5 * SECOND, span));
}

#[test]
fn a_picture_is_late_once_the_song_has_passed_its_end() {
    assert!(is_late(0, FRAME, FRAME));
    assert!(is_late(0, FRAME, 10 * FRAME));
    // The song is within it: this is the picture to show now, which is
    // what a paused song gets after a seek.
    assert!(!is_late(0, FRAME, FRAME - 1));
    assert!(!is_late(FRAME, FRAME, 0));
}

#[test]
fn pictures_are_made_no_bigger_than_the_surface_needs() {
    assert_eq!(height_for(320, 1080), 360);
    assert_eq!(height_for(360, 1080), 360);
    assert_eq!(height_for(361, 1080), 480);
    assert_eq!(height_for(800, 1080), 1080);
    // A screen taller than the film gets the film as it is.
    assert_eq!(height_for(2160, 1080), 1080);
    assert_eq!(height_for(2160, 720), 720);
    assert_eq!(height_for(100, 144), 144);
}

#[test]
fn a_smaller_picture_keeps_the_films_shape() {
    assert_eq!(size_for(360, (1920, 1080)), (640, 360));
    assert_eq!(size_for(1080, (1920, 1080)), (1920, 1080));
    assert_eq!(size_for(2000, (1920, 1080)), (1920, 1080));
    // An odd width is rounded down to a pair of pixels.
    assert_eq!(size_for(240, (1280, 718)), (426, 240));
}
