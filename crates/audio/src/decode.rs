//! One track's bytes turned into samples.
//!
//! YouTube Music serves Opus in WebM or AAC in MP4. symphonia reads both
//! containers and decodes AAC; Opus is decoded by libopus through an adapter
//! registered beside symphonia's own codecs. Either way the caller sees
//! interleaved `f32` frames and a position in milliseconds.

use std::fmt;
use std::sync::OnceLock;

use symphonia::core::codecs::audio::well_known::CODEC_ID_OPUS;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::{Time, TimeBase, Timestamp};
use symphonia_adapter_libopus::OpusDecoder;

/// What an Opus decoder needs to hear after a seek before its output is
/// right (RFC 7845 recommends 80 ms).
const OPUS_PRE_ROLL_MS: u64 = 80;

#[derive(Debug)]
pub enum DecodeError {
    /// The container or codec is not one this build reads.
    Unsupported(String),
    /// The stream could not be read or is damaged beyond a single packet.
    Stream(SymphoniaError),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Unsupported(what) => write!(f, "unsupported audio: {what}"),
            DecodeError::Stream(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<SymphoniaError> for DecodeError {
    fn from(error: SymphoniaError) -> Self {
        DecodeError::Stream(error)
    }
}

/// symphonia's codecs plus Opus.
fn codecs() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(|| {
        let mut registry = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        registry.register_audio_decoder::<OpusDecoder>();
        registry
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    pub sample_rate: u32,
    pub channels: usize,
}

/// A run of decoded audio.
pub struct Chunk<'a> {
    /// Interleaved frames, `channels` samples each.
    pub samples: &'a [f32],
    /// Where in the track the first frame sits.
    pub position_ms: u64,
}

pub struct Decoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    /// Opus needs a pre-roll after a seek; AAC does not.
    is_opus: bool,
    track_id: u32,
    time_base: Option<TimeBase>,
    format: Format,
    duration_ms: Option<u64>,
    samples: Vec<f32>,
    /// Frames to drop before the next ones are heard: the pre-roll decoded
    /// after a seek only to settle the decoder.
    skip_frames: usize,
}

impl Decoder {
    pub fn open(source: Box<dyn MediaSource>) -> Result<Self, DecodeError> {
        let stream = MediaSourceStream::new(source, Default::default());
        let reader = symphonia::default::get_probe().probe(
            &Hint::new(),
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )?;
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| DecodeError::Unsupported("no audio track".into()))?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .ok_or_else(|| DecodeError::Unsupported("no audio parameters".into()))?;
        let decoder = codecs().make_audio_decoder(params, &AudioDecoderOptions::default())?;
        let format = Format {
            sample_rate: params
                .sample_rate
                .ok_or_else(|| DecodeError::Unsupported("no sample rate".into()))?,
            channels: params
                .channels
                .as_ref()
                .map_or(2, |channels| channels.count()),
        };
        let time_base = track.time_base;
        let length = |base: Option<TimeBase>, duration| {
            base?
                .calc_duration(duration?)
                .and_then(|time| u64::try_from(time.as_millis()).ok())
        };
        // MP4 states each track's length. Matroska, which every Opus stream
        // arrives in, states only the whole file's.
        let media = reader.media_info();
        let duration_ms =
            length(time_base, track.duration).or_else(|| length(media.time_base, media.duration));
        Ok(Self {
            is_opus: params.codec == CODEC_ID_OPUS,
            track_id: track.id,
            time_base,
            format,
            duration_ms,
            samples: Vec::new(),
            skip_frames: 0,
            decoder,
            reader,
        })
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// The track's length, when the container states it.
    pub fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }

    /// The next run of audio, or `None` at the end of the track. A packet
    /// that fails to decode is skipped: one bad packet is a click, not the
    /// end of the song.
    pub fn next_chunk(&mut self) -> Result<Option<Chunk<'_>>, DecodeError> {
        loop {
            let Some(packet) = self.reader.next_packet()? else {
                return Ok(None);
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    self.samples.resize(buffer.samples_interleaved(), 0.0);
                    buffer.copy_to_slice_interleaved(&mut self.samples);
                }
                Err(SymphoniaError::DecodeError(reason)) => {
                    log::debug!("skipped a packet: {reason}");
                    continue;
                }
                Err(error) => return Err(error.into()),
            }

            let channels = self.format.channels;
            let frames = self.samples.len() / channels;
            let skipped = self.skip_frames.min(frames);
            self.skip_frames -= skipped;
            if skipped == frames {
                continue;
            }
            let position_ms = self.position_ms(packet.pts) + frames_to_ms(skipped, self.format);
            return Ok(Some(Chunk {
                samples: &self.samples[skipped * channels..],
                position_ms,
            }));
        }
    }

    /// Moves to `position_ms`. The next chunk starts at or just before it.
    pub fn seek(&mut self, position_ms: u64) -> Result<(), DecodeError> {
        // An Opus decoder converges over its first packets, so land early
        // and throw the pre-roll away.
        let target_ms = if self.is_opus {
            position_ms.saturating_sub(OPUS_PRE_ROLL_MS)
        } else {
            position_ms
        };
        let time = Time::try_from_secs_f64(target_ms as f64 / 1000.0)
            .ok_or_else(|| DecodeError::Unsupported("seek position out of range".into()))?;
        let landed = self.reader.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        )?;
        self.decoder.reset();
        self.skip_frames = if self.is_opus {
            let landed_ms = self.position_ms(landed.actual_ts);
            ms_to_frames(position_ms.saturating_sub(landed_ms), self.format)
        } else {
            0
        };
        Ok(())
    }

    fn position_ms(&self, timestamp: Timestamp) -> u64 {
        self.time_base
            .and_then(|base| base.calc_time(timestamp))
            .and_then(|time| u64::try_from(time.as_millis()).ok())
            .unwrap_or(0)
    }
}

fn frames_to_ms(frames: usize, format: Format) -> u64 {
    frames as u64 * 1000 / u64::from(format.sample_rate)
}

fn ms_to_frames(ms: u64, format: Format) -> usize {
    (ms * u64::from(format.sample_rate) / 1000) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    const CD: Format = Format {
        sample_rate: 44_100,
        channels: 2,
    };

    #[test]
    fn frames_and_milliseconds_convert_both_ways() {
        assert_eq!(frames_to_ms(44_100, CD), 1000);
        assert_eq!(ms_to_frames(1000, CD), 44_100);
        assert_eq!(ms_to_frames(80, CD), 3528);
    }
}
