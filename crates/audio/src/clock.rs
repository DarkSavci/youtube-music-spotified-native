//! Where in the track the listener is, for whatever must keep time with
//! the sound: a music video's picture.
//!
//! The core's position reports come four times a second and by way of two
//! processes, which is fine for a progress bar and too coarse for a picture.
//! This is the engine's own figure, written on every tick of its thread.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The furthest a reading is run on past the moment it was taken. The
/// engine writes one every tick while it plays; one older than this means
/// it has stopped feeding the device, and the sound has stopped with it.
const RUN_ON: Duration = Duration::from_millis(60);

/// The engine's position at one moment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Reading {
    /// The track the position is in; empty when nothing is loaded.
    pub video_id: String,
    pub position_ms: u64,
    /// When the position was taken; `None` before the first.
    pub at: Option<Instant>,
    /// Sound is reaching the device, so the position is moving on.
    pub moving: bool,
    /// How many times as fast as the clock the position moves.
    pub speed: f32,
}

impl Reading {
    /// Where the track has reached at `now`.
    pub fn position_at(&self, now: Instant) -> u64 {
        let Some(at) = self.at.filter(|_| self.moving) else {
            return self.position_ms;
        };
        let passed = now.saturating_duration_since(at).min(RUN_ON);
        self.position_ms + (passed.as_secs_f64() * 1000.0 * f64::from(self.speed)) as u64
    }
}

#[derive(Default)]
pub struct Clock {
    reading: Mutex<Reading>,
}

impl Clock {
    pub fn read(&self) -> Reading {
        self.reading
            .lock()
            .map(|reading| reading.clone())
            .unwrap_or_default()
    }

    pub(crate) fn set(&self, video_id: &str, position_ms: u64, moving: bool, speed: f32) {
        let Ok(mut reading) = self.reading.lock() else {
            return;
        };
        // The name is only copied when the track changes: this runs fifty
        // times a second.
        if reading.video_id != video_id {
            reading.video_id = video_id.to_owned();
        }
        reading.position_ms = position_ms;
        reading.at = Some(Instant::now());
        reading.moving = moving;
        reading.speed = speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(moving: bool, speed: f32) -> Reading {
        Reading {
            video_id: "abc".into(),
            position_ms: 10_000,
            at: Some(Instant::now()),
            moving,
            speed,
        }
    }

    #[test]
    fn a_moving_position_runs_on_between_readings_at_the_speed_played() {
        let reading = reading(true, 1.5);
        let later = reading.at.expect("a time") + Duration::from_millis(20);
        assert_eq!(reading.position_at(later), 10_030);
    }

    #[test]
    fn a_position_that_is_not_moving_stays_where_it_was_read() {
        let reading = reading(false, 1.0);
        let later = reading.at.expect("a time") + Duration::from_secs(5);
        assert_eq!(reading.position_at(later), 10_000);
    }

    #[test]
    fn a_reading_gone_stale_is_not_run_on_for_ever() {
        let reading = reading(true, 1.0);
        let later = reading.at.expect("a time") + Duration::from_secs(5);
        assert_eq!(reading.position_at(later), 10_060);
    }

    #[test]
    fn the_clock_gives_back_what_it_was_last_told() {
        let clock = Clock::default();
        assert_eq!(clock.read(), Reading::default());
        clock.set("abc", 1234, true, 2.0);
        let read = clock.read();
        assert_eq!(
            (
                read.video_id.as_str(),
                read.position_ms,
                read.moving,
                read.speed
            ),
            ("abc", 1234, true, 2.0)
        );
    }
}
