//! Silence at the ends of a track, so a crossfade joins music to music.
//!
//! Many tracks trail off into seconds of nothing, and some begin with it.
//! Faded across as they stand, the outgoing song has gone before the fade
//! is over and the incoming one arrives late. So the end of a track's music
//! is looked for ahead of time, and the fade starts from there; and the
//! quiet at the start of the incoming track is passed over.
//!
//! Only a crossfade trims. Played gapless, an album's own pauses are part
//! of it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::decode::Decoder;
use crate::source::HttpSource;

/// Below this a sample counts as silence: 50 dB under full scale.
const QUIET: f32 = 0.003;
/// How much of the end of a track is listened to for its last sound, which
/// is also the most that can be trimmed from it.
const TAIL_HEARD_MS: u64 = 15_000;
/// The most that is passed over at the start of a track.
pub const MOST_LEAD_MS: u64 = 5_000;

/// Where a track's music ends, which may be learnt after it has started.
#[derive(Clone)]
pub struct Tail(Arc<AtomicU64>);

/// Stands for "not known" in the slot; no track ends at its first instant.
const UNKNOWN: u64 = 0;

impl Tail {
    pub fn unknown() -> Self {
        Self(Arc::new(AtomicU64::new(UNKNOWN)))
    }

    /// Unknown now; known once the end of the stream at `url` has been
    /// read on a thread of its own. Stays unknown if that fails: the track
    /// is then faded from its stated end, as it would be without this.
    pub fn listen(agent: ureq::Agent, url: String) -> Self {
        let tail = Self::unknown();
        let slot = tail.0.clone();
        let spawned =
            std::thread::Builder::new()
                .name("tail".into())
                .spawn(move || match music_end(agent, url) {
                    Ok(Some(end_ms)) => {
                        log::debug!("the music ends at {end_ms} ms");
                        slot.store(end_ms, Ordering::Relaxed);
                    }
                    Ok(None) => {}
                    Err(error) => log::debug!("the end of the track was not read: {error}"),
                });
        if let Err(error) = spawned {
            log::debug!("the end of the track was not read: {error}");
        }
        tail
    }

    /// Where the music ends, if that is known.
    pub fn music_ends_ms(&self) -> Option<u64> {
        match self.0.load(Ordering::Relaxed) {
            UNKNOWN => None,
            end_ms => Some(end_ms),
        }
    }
}

/// Reads the last stretch of a stream and returns where its last sound is.
/// `None` for a stream that does not state its length.
fn music_end(agent: ureq::Agent, url: String) -> Result<Option<u64>, String> {
    let said = |error: &dyn std::fmt::Display| error.to_string();
    let source = HttpSource::open(agent, url).map_err(|error| said(&error))?;
    let mut decoder = Decoder::open(Box::new(source)).map_err(|error| said(&error))?;
    let Some(duration_ms) = decoder.duration_ms() else {
        return Ok(None);
    };
    let from_ms = duration_ms.saturating_sub(TAIL_HEARD_MS);
    decoder.seek(from_ms).map_err(|error| said(&error))?;
    let format = decoder.format();
    // A track silent throughout the stretch heard ends, for the fade's
    // purposes, where the stretch begins.
    let mut last_sound_ms = from_ms;
    while let Some(chunk) = decoder.next_chunk().map_err(|error| said(&error))? {
        if let Some(sample) = chunk
            .samples
            .iter()
            .rposition(|sample| sample.abs() > QUIET)
        {
            let frame = (sample / format.channels) as u64;
            last_sound_ms = chunk.position_ms + frame * 1000 / u64::from(format.sample_rate);
        }
    }
    Ok(Some(
        last_sound_ms.clamp(from_ms.max(1), duration_ms.max(1)),
    ))
}

/// How many samples of interleaved stereo at the start of `samples` are
/// silence, in whole frames.
pub fn leading_quiet(samples: &[f32]) -> usize {
    samples
        .iter()
        .position(|sample| sample.abs() > QUIET)
        .map_or(samples.len(), |first| first & !1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_is_counted_in_whole_frames_up_to_the_first_sound() {
        assert_eq!(leading_quiet(&[0.0, 0.0, 0.001, -0.002, 0.5, 0.0]), 4);
        // The right channel sounds first: its frame is kept whole.
        assert_eq!(leading_quiet(&[0.0, 0.0, 0.0, 0.2]), 2);
        assert_eq!(leading_quiet(&[0.4, 0.0]), 0);
    }

    #[test]
    fn a_chunk_of_nothing_is_all_quiet() {
        assert_eq!(leading_quiet(&[0.0; 8]), 8);
        assert_eq!(leading_quiet(&[]), 0);
    }

    #[test]
    fn where_the_music_ends_is_unknown_until_it_is_stored() {
        let tail = Tail::unknown();
        assert_eq!(tail.music_ends_ms(), None);
        tail.0.store(183_000, Ordering::Relaxed);
        assert_eq!(tail.clone().music_ends_ms(), Some(183_000));
    }
}
