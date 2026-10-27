//! A track's bytes, read from the core over HTTP as if from a file.
//!
//! The core serves `/v1/stream/{id}` with range requests. It may answer a
//! request with only part of what was asked for (it relays in windows while
//! its own download catches up), so a read that runs out before the end
//! simply asks again from where it stopped.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::time::Duration;

use symphonia::core::io::MediaSource;
use ureq::BodyReader;

/// A first request waits while the core resolves the stream, which can take
/// a minute for a track it has never seen.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(75);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A forward seek shorter than this reads and discards rather than opening
/// a new request.
const SKIP_LIMIT: u64 = 128 * 1024;
/// How often a connection that broke off mid-track is asked for again, and
/// how long the first wait before it is; each wait after is longer.
const RECONNECTS: u32 = 3;
const RECONNECT_WAIT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamError {
    /// YouTube is refusing requests for now (429).
    RateLimited,
    /// The core has no connection (503).
    Offline,
    /// The core cannot serve this track as a stream.
    Unplayable(u16),
    Network(String),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::RateLimited => write!(f, "rate limited"),
            StreamError::Offline => write!(f, "offline"),
            StreamError::Unplayable(status) => write!(f, "unplayable (status {status})"),
            StreamError::Network(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for StreamError {}

/// An agent for stream reads: patient with the first answer, and without an
/// overall limit, since a body is read for as long as the track plays.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .new_agent()
}

pub struct HttpSource {
    agent: ureq::Agent,
    url: String,
    /// The whole stream's length, once an answer has stated it.
    length: Option<u64>,
    /// Where the next read is to come from.
    position: u64,
    /// The open response, and where in the stream it has reached.
    body: Option<(BodyReader<'static>, u64)>,
}

impl HttpSource {
    /// Opens the stream and learns its length. Fails here, rather than on
    /// the first read, if the core cannot serve it.
    pub fn open(agent: ureq::Agent, url: String) -> Result<Self, StreamError> {
        let mut source = Self {
            agent,
            url,
            length: None,
            position: 0,
            body: None,
        };
        source.request(0)?;
        Ok(source)
    }

    /// Asks for the stream from `from` onwards.
    fn request(&mut self, from: u64) -> Result<(), StreamError> {
        self.body = None;
        let response = self
            .agent
            .get(&self.url)
            .header("Range", format!("bytes={from}-"))
            .call()
            .map_err(|error| StreamError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let total = response
            .headers()
            .get("Content-Range")
            .and_then(|value| value.to_str().ok())
            .and_then(total_length);
        match status {
            206 => {
                self.length = total.or(self.length);
                self.body = Some((response.into_body().into_reader(), from));
            }
            // The whole stream from the start, whatever was asked for.
            200 => {
                self.length = response
                    .headers()
                    .get("Content-Length")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse().ok())
                    .or(self.length);
                self.body = Some((response.into_body().into_reader(), 0));
            }
            // Asked for bytes past the end: there is nothing more.
            416 => {}
            429 => return Err(StreamError::RateLimited),
            503 => return Err(StreamError::Offline),
            status => return Err(StreamError::Unplayable(status)),
        }
        Ok(())
    }

    /// Brings the open response to `self.position`, by reading ahead a
    /// little or by asking again.
    fn align(&mut self) -> io::Result<()> {
        let reached = self.body.as_ref().map(|(_, reached)| *reached);
        match reached {
            Some(reached) if reached == self.position => Ok(()),
            Some(reached) if reached < self.position && self.position - reached <= SKIP_LIMIT => {
                let Some((body, reached)) = &mut self.body else {
                    return Ok(());
                };
                let skipped = io::copy(&mut body.take(self.position - *reached), &mut io::sink())?;
                *reached += skipped;
                if *reached == self.position {
                    Ok(())
                } else {
                    self.request(self.position).map_err(io::Error::other)
                }
            }
            _ => self.request(self.position).map_err(io::Error::other),
        }
    }

    /// The whole stream's length, once an answer has stated it.
    pub fn length(&self) -> Option<u64> {
        self.length
    }

    fn at_end(&self) -> bool {
        self.length.is_some_and(|length| self.position >= length)
    }

    /// Counts a connection that broke off, and waits a little longer each
    /// time before it is asked for again. Past the last try, the error is
    /// the network's, so the engine does not take the track for broken.
    fn broke_off(&self, broken: &mut u32, error: &str) -> io::Result<()> {
        *broken += 1;
        if *broken > RECONNECTS {
            log::warn!(
                "the stream broke off at byte {} and could not be had again: {error}",
                self.position
            );
            return Err(io::Error::other(StreamError::Network(error.to_owned())));
        }
        log::info!(
            "the stream broke off at byte {} ({error}); asking again from there",
            self.position
        );
        std::thread::sleep(RECONNECT_WAIT * *broken);
        Ok(())
    }
}

/// The total in a `Content-Range: bytes 0-99/1234` header.
fn total_length(content_range: &str) -> Option<u64> {
    content_range.rsplit_once('/')?.1.trim().parse().ok()
}

impl Read for HttpSource {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.at_end() {
            return Ok(0);
        }
        self.align()?;
        // A window that has just run out is asked for again once; nothing
        // twice is the end. A connection that breaks off is asked for again
        // from where it got to, a few times, before the track is given up:
        // the core relays YouTube, and either side may drop a long read.
        let (mut emptied, mut broken) = (false, 0);
        loop {
            let Some((body, reached)) = &mut self.body else {
                return Ok(0);
            };
            let outcome = body.read(buffer);
            if let Ok(read) = outcome {
                *reached += read as u64;
            }
            match outcome {
                Ok(0) if self.at_end() || emptied => return Ok(0),
                Ok(0) => emptied = true,
                Ok(read) => {
                    self.position += read as u64;
                    return Ok(read);
                }
                Err(error) => self.broke_off(&mut broken, &error.to_string())?,
            }
            loop {
                match self.request(self.position) {
                    Ok(()) => break,
                    Err(StreamError::Network(error)) => self.broke_off(&mut broken, &error)?,
                    Err(other) => return Err(io::Error::other(other)),
                }
            }
        }
    }
}

impl Seek for HttpSource {
    /// Only moves the position; the request waits for the next read, so a
    /// run of seeks costs one request, not one each.
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self
                .length
                .and_then(|length| length.checked_add_signed(delta)),
        };
        self.position = target.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "seek outside the stream")
        })?;
        Ok(self.position)
    }
}

impl MediaSource for HttpSource {
    fn is_seekable(&self) -> bool {
        self.length.is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server of `whole` that breaks off its first answer after `cut`
    /// bytes, as a dropped connection does, and answers properly after.
    fn breaking_server(whole: Vec<u8>, cut: usize) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let address = listener.local_addr().expect("an address");
        std::thread::spawn(move || {
            for (served, stream) in listener.incoming().enumerate() {
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
                let rest = &whole[from..];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\n\
                     Content-Range: bytes {from}-{}/{}\r\nConnection: close\r\n\r\n",
                    rest.len(),
                    whole.len() - 1,
                    whole.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let sent = if served == 0 { &rest[..cut] } else { rest };
                let _ = stream.write_all(sent);
                // Dropped here: the first answer ends short of its length.
            }
        });
        format!("http://{address}/v1/stream/x")
    }

    #[test]
    fn a_connection_that_breaks_off_is_asked_for_again_from_where_it_got_to() {
        let whole: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let url = breaking_server(whole.clone(), 70_000);
        let mut source = HttpSource::open(agent(), url).expect("opened");
        let mut read = Vec::new();
        source.read_to_end(&mut read).expect("read whole");
        assert_eq!(read.len(), whole.len());
        assert!(read == whole, "the bytes after the break follow on");
    }

    #[test]
    fn the_total_is_read_from_a_content_range() {
        assert_eq!(total_length("bytes 0-1048575/7610310"), Some(7_610_310));
        assert_eq!(total_length("bytes 0-99/*"), None);
        assert_eq!(total_length("nonsense"), None);
    }
}
