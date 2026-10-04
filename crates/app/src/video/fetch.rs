//! Fetching parts of the picture's file from the core, which fetches them
//! from YouTube. The core hands over at most a mebibyte a request, so a
//! longer part is asked for a window at a time.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The first request waits while the core finds the stream, which is
/// yt-dlp starting up and asking YouTube: seconds, now and then many.
const OPENING_TIMEOUT: Duration = Duration::from_secs(150);
/// After that the stream is known and a window is a second's work.
const TIMEOUT: Duration = Duration::from_secs(30);
/// The most the core serves at once.
const WINDOW: u64 = 1 << 20;
/// Read in pieces this small, so being told to stop is noticed quickly.
const PIECE: usize = 64 * 1024;

/// Why a fetch gave nothing.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// Told to stop: the track changed, or the picture was closed.
    Stopped,
    Failed(String),
}

pub struct Source {
    agent: ureq::Agent,
    url: String,
}

impl Source {
    pub fn new(url: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .build()
            .new_agent();
        Self { agent, url }
    }

    /// The first `length` bytes of the file.
    pub fn opening(&self, length: u64, stop: &AtomicBool) -> Result<Vec<u8>, Failure> {
        self.read(0, length, OPENING_TIMEOUT, stop)
    }

    /// `length` bytes from `from`.
    pub fn part(&self, from: u64, length: u64, stop: &AtomicBool) -> Result<Vec<u8>, Failure> {
        self.read(from, length, TIMEOUT, stop)
    }

    fn read(
        &self,
        from: u64,
        length: u64,
        timeout: Duration,
        stop: &AtomicBool,
    ) -> Result<Vec<u8>, Failure> {
        let mut bytes = Vec::with_capacity(length as usize);
        let end = from + length;
        while (bytes.len() as u64) < length {
            if stop.load(Ordering::Relaxed) {
                return Err(Failure::Stopped);
            }
            let at = from + bytes.len() as u64;
            let last = end.min(at + WINDOW) - 1;
            let response = self
                .agent
                .get(&self.url)
                .header("Range", format!("bytes={at}-{last}"))
                .config()
                .timeout_global(Some(timeout))
                .build()
                .call()
                .map_err(|error| Failure::Failed(format!("the video was not served: {error}")))?;
            let before = bytes.len();
            let mut body = response.into_body().into_reader().take(last + 1 - at);
            let mut piece = vec![0u8; PIECE];
            loop {
                if stop.load(Ordering::Relaxed) {
                    return Err(Failure::Stopped);
                }
                match body.read(&mut piece) {
                    Ok(0) => break,
                    Ok(read) => bytes.extend_from_slice(&piece[..read]),
                    Err(error) => {
                        return Err(Failure::Failed(format!(
                            "the video stopped arriving: {error}"
                        )));
                    }
                }
            }
            if bytes.len() == before {
                return Err(Failure::Failed("the video ended early".to_owned()));
            }
        }
        Ok(bytes)
    }
}
