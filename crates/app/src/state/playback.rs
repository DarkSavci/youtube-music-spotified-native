//! What is playing, as last heard from the core.

use std::time::Instant;

use spotified_client::models::Track;
use spotified_client::session::{PlayState, Repeat, SessionState};

/// The core's session and when it arrived. The core reports a position a
/// few times a second; between reports the position is worked out from the
/// time passed, so the progress bar moves evenly.
pub struct Playback {
    pub session: SessionState,
    pub received: Instant,
    /// The core has lost its connection to YouTube.
    pub offline: bool,
    /// The core is mirroring a Listen Together room.
    pub following_room: bool,
    /// The room entry that has played to its end here, if the one that is
    /// current has.
    pub room_ended: Option<String>,
}

impl Playback {
    pub fn current(&self) -> Option<&Track> {
        self.session.current()
    }

    pub fn is_playing(&self) -> bool {
        self.session.state == PlayState::Playing
    }

    /// Whether the play button should show "pause": playing, or about to.
    pub fn wants_to_play(&self) -> bool {
        matches!(
            self.session.state,
            PlayState::Playing | PlayState::Loading | PlayState::Stalled
        )
    }

    /// Where the track has reached at `now`, held to its length.
    pub fn position_ms(&self, now: Instant) -> u64 {
        let moved = if self.is_playing() {
            now.duration_since(self.received).as_millis() as u64
        } else {
            0
        };
        let position = self.session.position_ms + moved;
        match self.current().map(|track| track.duration_ms) {
            Some(duration) if duration > 0 => position.min(duration),
            _ => position,
        }
    }

    /// The repeat mode after the one in force: off, all, one, off.
    pub fn next_repeat(&self) -> Repeat {
        match self.session.repeat {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use spotified_client::session::Queue;

    use super::*;

    fn playback(state: PlayState, position_ms: u64) -> Playback {
        Playback {
            session: SessionState {
                state,
                position_ms,
                queue: Queue {
                    items: vec![Track {
                        duration_ms: 10_000,
                        ..Track::default()
                    }],
                    ..Queue::default()
                },
                ..SessionState::default()
            },
            received: Instant::now(),
            offline: false,
            following_room: false,
            room_ended: None,
        }
    }

    #[test]
    fn a_playing_track_moves_on_between_reports() {
        let playback = playback(PlayState::Playing, 4000);
        let later = playback.received + Duration::from_millis(1500);
        assert_eq!(playback.position_ms(later), 5500);
    }

    #[test]
    fn a_paused_track_stays_where_it_is() {
        let playback = playback(PlayState::Paused, 4000);
        let later = playback.received + Duration::from_secs(30);
        assert_eq!(playback.position_ms(later), 4000);
    }

    #[test]
    fn the_position_never_passes_the_end() {
        let playback = playback(PlayState::Playing, 9500);
        let later = playback.received + Duration::from_secs(5);
        assert_eq!(playback.position_ms(later), 10_000);
    }

    #[test]
    fn repeat_cycles_off_all_one() {
        let mut playback = playback(PlayState::Paused, 0);
        assert_eq!(playback.next_repeat(), Repeat::All);
        playback.session.repeat = Repeat::All;
        assert_eq!(playback.next_repeat(), Repeat::One);
        playback.session.repeat = Repeat::One;
        assert_eq!(playback.next_repeat(), Repeat::Off);
    }
}
