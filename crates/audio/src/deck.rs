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
        /// What the stream decodes as, and whether it is Opus.
        format: crate::decode::Format,
        opus: bool,
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
    served: Option<crate::tap::Served>,
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
            served: None,
        }
    }

    /// What the stream is, once it has opened.
    pub fn served(&self) -> Option<crate::tap::Served> {
        self.served
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
                Ok(Message::Ready {
                    duration_ms,
                    bytes,
                    format,
                    opus,
                }) => {
                    self.kbps = kbps_of(bytes, duration_ms);
                    self.served = Some(crate::tap::Served {
                        codec: if opus { "Opus" } else { "AAC" },
                        kbps: self.kbps,
                        sample_rate: format.sample_rate,
                        channels: format.channels,
                    });
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
    let source = HttpSource::open(agent.clone(), url.clone()).map_err(DeckError::Stream)?;
    let bytes = source.length();
    let decoder = Decoder::open(Box::new(source)).map_err(decode_error)?;
    let mut reader = Reader {
        agent,
        url,
        decoder,
        exhausted: false,
    };
    if start_ms > 0 {
        reader.decoder.seek(start_ms).map_err(decode_error)?;
    }
    let decoder = &reader.decoder;
    let format = decoder.format();
    let mut resampler = (format.sample_rate != output_rate)
        .then(|| Resampler::new(format.sample_rate, output_rate));
    if out
        .send(Message::Ready {
            duration_ms: decoder.duration_ms(),
            bytes,
            format,
            opus: decoder.is_opus(),
        })
        .is_err()
    {
        return Ok(());
    }

    let mut seek = 0;
    let mut stereo = Vec::new();
    // The first sound at the level it will keep, rather than turned down
    // under the listener a moment in.
    gain.wait();
    let mut applied_gain = gain.get();
    // Where the last chunk ended, for the resampler's tail.
    let mut end_ms = 0;
    loop {
        // The latest seek wins; earlier ones are already out of date.
        if let Some((number, position_ms)) = seeks.try_iter().last() {
            seek = number;
            reader.seek(position_ms)?;
            if let Some(resampler) = &mut resampler {
                resampler.reset();
            }
        }
        let chunk = reader.decoder.next_chunk().map_err(decode_error)?;
        reader.exhausted = chunk.is_none();
        let message = match chunk {
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
                let frames = (chunk.samples.len() / format.channels.max(1)) as u64;
                end_ms = chunk.position_ms + frames * 1000 / u64::from(format.sample_rate);
                Message::Chunk {
                    seek,
                    samples,
                    position_ms: chunk.position_ms,
                }
            }
            // The resampler's last milliseconds first, then the end.
            None => match resampler.as_mut().map(Resampler::finish) {
                Some(mut samples) if !samples.is_empty() => {
                    applied_gain = loudness::apply(&mut samples, applied_gain, gain.get());
                    Message::Chunk {
                        seek,
                        samples,
                        position_ms: end_ms,
                    }
                }
                _ => Message::Ended { seek },
            },
        };
        let mut ended = matches!(message, Message::Ended { .. });
        // Wait for room, but not past a seek: what is waiting to be sent
        // would be stale.
        select! {
            send(out, message) -> sent => {
                if sent.is_err() {
                    return Ok(());
                }
            }
            recv(seeks) -> request => match request {
                Ok(request) => {
                    requeue(seeks, request, &mut seek, &mut reader, &mut resampler)?;
                    // The end went unsent with the place it was the end of.
                    // Waiting for a seek here, after one, left the deck
                    // silent until the next: a room's correction near the
                    // end of a song, then nothing until pause and play.
                    ended = false;
                }
                Err(_) => return Ok(()),
            },
        }
        if ended {
            // A seek back from the end (repeat one) starts it again; the
            // deck being dropped ends the wait.
            match seeks.recv() {
                Ok(request) => requeue(seeks, request, &mut seek, &mut reader, &mut resampler)?,
                Err(_) => return Ok(()),
            }
        }
    }
}

/// The decoder, and what it takes to open the stream again.
struct Reader {
    agent: ureq::Agent,
    url: String,
    decoder: Decoder,
    /// The demuxer has read to the end of the stream.
    exhausted: bool,
}

impl Reader {
    /// Moves to `position_ms`. A demuxer that has read to the end will not
    /// seek (WebM and MP4 alike refuse, as a malformed stream), so the
    /// stream is opened again: a seek back from the end, for repeat one or
    /// a room that went back into the song, failed the track instead.
    fn seek(&mut self, position_ms: u64) -> Result<(), DeckError> {
        if self.exhausted {
            let source = HttpSource::open(self.agent.clone(), self.url.clone())
                .map_err(DeckError::Stream)?;
            self.decoder = Decoder::open(Box::new(source)).map_err(decode_error)?;
            self.exhausted = false;
            if position_ms == 0 {
                return Ok(());
            }
        }
        self.decoder.seek(position_ms).map_err(decode_error)
    }
}

/// Applies a seek that arrived while the thread was waiting, or a later one
/// if several are queued.
fn requeue(
    seeks: &Receiver<(u64, u64)>,
    request: (u64, u64),
    seek: &mut u64,
    reader: &mut Reader,
    resampler: &mut Option<Resampler>,
) -> Result<(), DeckError> {
    let (number, position_ms) = seeks.try_iter().last().unwrap_or(request);
    *seek = number;
    reader.seek(position_ms)?;
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
pub(crate) fn to_stereo(samples: &[f32], channels: usize, stereo: &mut Vec<f32>) {
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
mod seek_tests {
    use std::time::{Duration, Instant};

    use super::*;

    /// A second of 440 Hz in each of the two ways YouTube serves audio.
    const OPUS: (&[u8], u32) = (include_bytes!("../tests/fixtures/sine-1s.webm"), 48_000);
    const AAC: (&[u8], u32) = (include_bytes!("../tests/fixtures/sine-1s.m4a"), 44_100);

    /// Serves `whole` with byte ranges, as the core does, to any number of
    /// requests.
    fn server(whole: &'static [u8]) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let address = listener.local_addr().expect("an address");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut from = 0;
                let mut reader = BufReader::new(stream.try_clone().expect("a clone"));
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 2) {
                    if let Some(range) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        from = range.trim().trim_end_matches('-').parse().unwrap_or(0);
                    }
                    line.clear();
                }
                let rest = &whole[from.min(whole.len())..];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n",
                    rest.len(),
                    whole.len() - 1,
                    whole.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(rest);
            }
        });
        format!("http://{address}/v1/stream/sine")
    }

    fn deck(url: &str, rate: u32) -> Deck {
        let agent = crate::source::agent();
        Deck::spawn(agent, url.into(), "sine".into(), 0, rate, Gain::unity())
    }

    /// Polls until `wanted` says stop, or gives up after ten seconds.
    fn poll_until(deck: &mut Deck, mut wanted: impl FnMut(&Poll) -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            let poll = deck.poll();
            if let Poll::Failed(error) = &poll {
                panic!("the deck failed: {error:?}");
            }
            if wanted(&poll) {
                return true;
            }
            if matches!(poll, Poll::Pending) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        false
    }

    fn chunk(poll: &Poll) -> bool {
        matches!(poll, Poll::Chunk { .. })
    }

    /// How many chunks the track is, read through once.
    fn chunks_in(url: &str, rate: u32) -> usize {
        let mut counting = deck(url, rate);
        let mut chunks = 0;
        assert!(poll_until(&mut counting, |poll| {
            chunks += usize::from(chunk(poll));
            matches!(poll, Poll::Ended)
        }));
        assert!(
            chunks > CHUNKS_AHEAD,
            "{chunks} chunks is too short a track"
        );
        chunks
    }

    /// A room corrects the player near the end of a song: the seek reaches
    /// the deck while the end waits behind the last chunks. It used to be
    /// taken as the seek back from the end, and the deck then waited for
    /// another, silent, until a pause and a play brought one.
    fn seek_while_the_end_waits((bytes, rate): (&'static [u8], u32)) {
        let url = server(bytes);
        let chunks = chunks_in(&url, rate);
        let mut deck = deck(&url, rate);
        // Taken from until the channel can hold only the last chunks, so
        // the end waits for room behind them.
        let mut taken = 0;
        assert!(poll_until(&mut deck, |poll| {
            taken += usize::from(chunk(poll));
            taken == chunks - CHUNKS_AHEAD
        }));
        std::thread::sleep(Duration::from_millis(300));
        deck.seek(200);
        assert!(
            poll_until(&mut deck, chunk),
            "nothing came after the seek: the deck is stuck"
        );
    }

    /// Repeat one: the end is reported, and the seek back plays it again.
    fn seek_back_from_the_end((bytes, rate): (&'static [u8], u32)) {
        let url = server(bytes);
        let mut deck = deck(&url, rate);
        assert!(poll_until(&mut deck, |poll| matches!(poll, Poll::Ended)));
        deck.seek(0);
        assert!(poll_until(&mut deck, chunk), "nothing came after the seek");
    }

    #[test]
    fn a_seek_while_an_opus_end_waits_is_played() {
        seek_while_the_end_waits(OPUS);
    }

    #[test]
    fn a_seek_while_an_aac_end_waits_is_played() {
        seek_while_the_end_waits(AAC);
    }

    #[test]
    fn an_opus_track_is_played_again_from_its_end() {
        seek_back_from_the_end(OPUS);
    }

    #[test]
    fn an_aac_track_is_played_again_from_its_end() {
        seek_back_from_the_end(AAC);
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
