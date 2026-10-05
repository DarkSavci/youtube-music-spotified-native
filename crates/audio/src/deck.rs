//! One track being read and decoded, on a thread of its own.
//!
//! Reading waits on the network and decoding takes time; neither may hold
//! up the engine, which has to answer a pause or a skip at once. So each
//! track gets a thread that turns bytes into chunks of stereo audio at the
//! output's sample rate, a second or so ahead, and the engine takes them as
//! it needs them.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::{Receiver, Sender, TryRecvError, bounded, select, unbounded};

use crate::decode::{DecodeError, Decoder};
use crate::loudness::{self, Gain};
use crate::resample::Resampler;
use crate::source::{HttpSource, StreamError};

/// Chunks decoded ahead of the engine. A chunk is 20 to 120 ms, so this is
/// around a second: enough to ride out a slow read, little enough to throw
/// away on a seek.
const CHUNKS_AHEAD: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeckError {
    Stream(StreamError),
    /// The bytes arrived but are not audio this build can decode.
    Decode(String),
}

enum Message {
    Ready {
        duration_ms: Option<u64>,
        /// The whole stream's length in bytes, when the core stated it.
        bytes: Option<u64>,
    },
    Chunk {
        seek: u64,
        samples: Vec<f32>,
        position_ms: u64,
    },
    Ended {
        seek: u64,
    },
    Failed(DeckError),
}

/// What a deck has for the engine right now.
pub enum Poll {
    /// Nothing yet: still opening, or the network is behind.
    Pending,
    /// The track is open and its first audio is on the way.
    Ready {
        duration_ms: Option<u64>,
    },
    /// Interleaved stereo at the output rate, starting at `position_ms`.
    Chunk {
        samples: Vec<f32>,
        position_ms: u64,
    },
    Ended,
    Failed(DeckError),
}

pub struct Deck {
    video_id: String,
    messages: Receiver<Message>,
    seeks: Sender<(u64, u64)>,
    /// Counts seeks. Chunks decoded before the latest seek carry an older
    /// number and are dropped.
    seek: u64,
    /// Set by the thread when it gives up, so a deck waiting its turn can be
    /// checked without taking its audio.
    failed: Arc<AtomicBool>,
    /// The stream's bitrate in kilobits a second, once it is open and
    /// both its size and its length are known; until then, nought.
    kbps: u32,
}

/// A stream's average bitrate in kilobits a second: its size over its
/// length. Nought when either is unknown.
fn kbps_of(bytes: Option<u64>, duration_ms: Option<u64>) -> u32 {
    match (bytes, duration_ms) {
        (Some(bytes), Some(duration_ms)) if duration_ms > 0 => {
            let bits_a_second = bytes.saturating_mul(8000) / duration_ms;
            ((bits_a_second + 500) / 1000).min(u64::from(u32::MAX)) as u32
        }
        _ => 0,
    }
}

impl Deck {
    /// Starts reading `url` from `start_ms`, producing audio at
    /// `output_rate`. Returns at once; the outcome arrives through `poll`.
    pub fn spawn(
        agent: ureq::Agent,
        url: String,
        video_id: String,
        start_ms: u64,
        output_rate: u32,
        gain: Gain,
    ) -> Self {
        let (out, messages) = bounded(CHUNKS_AHEAD);
        let (seeks, seek_requests) = unbounded();
        let failed = Arc::new(AtomicBool::new(false));
        let name = format!("deck-{video_id}");
        let spawned = std::thread::Builder::new().name(name).spawn({
            let out = out.clone();
            let failed = failed.clone();
            move || {
                if let Err(error) = run(
                    agent,
                    url,
                    start_ms,
                    output_rate,
                    &out,
                    &seek_requests,
                    &gain,
                ) {
                    failed.store(true, Ordering::Relaxed);
                    let _ = out.send(Message::Failed(error));
                }
            }
        });
        if let Err(error) = spawned {
            let _ = out.send(Message::Failed(DeckError::Decode(error.to_string())));
        }
        Self {
            video_id,
            messages,
            seeks,
            seek: 0,
            failed,
            kbps: 0,
        }
    }

    /// The stream's bitrate in kilobits a second; nought until it is known.
    pub fn kbps(&self) -> u32 {
        self.kbps
    }

    pub fn video_id(&self) -> &str {
        &self.video_id
    }

    /// Whether the deck has given up. Unlike `poll`, takes nothing from it.
    pub fn has_failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    pub fn poll(&mut self) -> Poll {
        loop {
            return match self.messages.try_recv() {
                Ok(Message::Ready { duration_ms, bytes }) => {
                    self.kbps = kbps_of(bytes, duration_ms);
                    Poll::Ready { duration_ms }
                }
                Ok(Message::Chunk {
                    seek,
                    samples,
                    position_ms,
                }) if seek == self.seek => Poll::Chunk {
                    samples,
                    position_ms,
                },
                Ok(Message::Ended { seek }) if seek == self.seek => Poll::Ended,
                // From before the latest seek.
                Ok(Message::Chunk { .. } | Message::Ended { .. }) => continue,
                Ok(Message::Failed(error)) => Poll::Failed(error),
                Err(TryRecvError::Empty) => Poll::Pending,
                Err(TryRecvError::Disconnected) => {
                    Poll::Failed(DeckError::Decode("the decoder stopped".into()))
                }
            };
        }
    }

    pub fn seek(&mut self, position_ms: u64) {
        self.seek += 1;
        let _ = self.seeks.send((self.seek, position_ms));
    }
}

fn run(
    agent: ureq::Agent,
    url: String,
    start_ms: u64,
    output_rate: u32,
    out: &Sender<Message>,
    seeks: &Receiver<(u64, u64)>,
    gain: &Gain,
) -> Result<(), DeckError> {
    let source = HttpSource::open(agent, url).map_err(DeckError::Stream)?;
    let bytes = source.length();
    let mut decoder = Decoder::open(Box::new(source)).map_err(decode_error)?;
    if start_ms > 0 {
        decoder.seek(start_ms).map_err(decode_error)?;
    }
    let format = decoder.format();
    let mut resampler = (format.sample_rate != output_rate)
        .then(|| Resampler::new(format.sample_rate, output_rate));
    if out
        .send(Message::Ready {
            duration_ms: decoder.duration_ms(),
            bytes,
        })
        .is_err()
    {
        return Ok(());
    }

    let mut seek = 0;
    let mut stereo = Vec::new();
    let mut applied_gain = gain.get();
    loop {
        // The latest seek wins; earlier ones are already out of date.
        if let Some((number, position_ms)) = seeks.try_iter().last() {
            seek = number;
            decoder.seek(position_ms).map_err(decode_error)?;
            if let Some(resampler) = &mut resampler {
                resampler.reset();
            }
        }
        let message = match decoder.next_chunk().map_err(decode_error)? {
            Some(chunk) => {
                to_stereo(chunk.samples, format.channels, &mut stereo);
                let mut samples = match &mut resampler {
                    Some(resampler) => {
                        let mut resampled = Vec::with_capacity(stereo.len() * 2);
                        resampler.process(&stereo, &mut resampled);
                        resampled
                    }
                    None => stereo.clone(),
                };
                applied_gain = loudness::apply(&mut samples, applied_gain, gain.get());
                Message::Chunk {
                    seek,
                    samples,
                    position_ms: chunk.position_ms,
                }
            }
            None => Message::Ended { seek },
        };
        let ended = matches!(message, Message::Ended { .. });
        // Wait for room, but not past a seek: what is waiting to be sent
        // would be stale.
        select! {
            send(out, message) -> sent => {
                if sent.is_err() {
                    return Ok(());
                }
            }
            recv(seeks) -> request => match request {
                Ok(request) => requeue(seeks, request, &mut seek, &mut decoder, &mut resampler)?,
                Err(_) => return Ok(()),
            },
        }
        if ended {
            // A seek back from the end (repeat one) starts it again; the
            // deck being dropped ends the wait.
            match seeks.recv() {
                Ok(request) => requeue(seeks, request, &mut seek, &mut decoder, &mut resampler)?,
                Err(_) => return Ok(()),
            }
        }
    }
}

/// Applies a seek that arrived while the thread was waiting, or a later one
/// if several are queued.
fn requeue(
    seeks: &Receiver<(u64, u64)>,
    request: (u64, u64),
    seek: &mut u64,
    decoder: &mut Decoder,
    resampler: &mut Option<Resampler>,
) -> Result<(), DeckError> {
    let (number, position_ms) = seeks.try_iter().last().unwrap_or(request);
    *seek = number;
    decoder.seek(position_ms).map_err(decode_error)?;
    if let Some(resampler) = resampler {
        resampler.reset();
    }
    Ok(())
}

/// A read that failed for a reason the stream gave is reported as that
/// reason, so the engine can tell a rate limit from a broken file.
fn decode_error(error: DecodeError) -> DeckError {
    if let DecodeError::Stream(symphonia::core::errors::Error::IoError(io)) = &error
        && let Some(stream) = io
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<StreamError>())
    {
        return DeckError::Stream(stream.clone());
    }
    DeckError::Decode(error.to_string())
}

/// Mono is doubled; anything wider keeps its first two channels.
fn to_stereo(samples: &[f32], channels: usize, stereo: &mut Vec<f32>) {
    stereo.clear();
    match channels {
        2 => stereo.extend_from_slice(samples),
        1 => stereo.extend(samples.iter().flat_map(|&sample| [sample, sample])),
        _ => stereo.extend(
            samples
                .chunks_exact(channels)
                .flat_map(|frame| [frame[0], frame[1]]),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_doubled_and_surround_keeps_the_front_pair() {
        let mut stereo = Vec::new();
        to_stereo(&[0.1, 0.2], 1, &mut stereo);
        assert_eq!(stereo, [0.1, 0.1, 0.2, 0.2]);
        to_stereo(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, &mut stereo);
        assert_eq!(stereo, [1.0, 2.0, 4.0, 5.0]);
    }
}

#[cfg(test)]
mod kbps_tests {
    use super::kbps_of;

    #[test]
    fn a_streams_bitrate_is_its_size_over_its_length() {
        // 3.3 MB over three and a half minutes is an Opus stream at 126.
        assert_eq!(kbps_of(Some(3_300_000), Some(210_000)), 126);
        assert_eq!(kbps_of(Some(8_000_000), Some(250_000)), 256);
        assert_eq!(kbps_of(None, Some(210_000)), 0);
        assert_eq!(kbps_of(Some(3_300_000), None), 0);
        assert_eq!(kbps_of(Some(3_300_000), Some(0)), 0);
    }
}
