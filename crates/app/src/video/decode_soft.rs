//! Turning H.264 into pictures on a Mac or Linux: OpenH264, on the processor,
//! and the colours worked out here. The worker asks the core for no more
//! than 720p in that case, which a processor manages.
//!
//! The same shape as `decode.rs`, which does this on Windows with what
//! Windows brings. What leaves it is a time and a row of pixels.

use std::collections::VecDeque;

use openh264::decoder::{Decoder as OpenH264, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use openh264::{OpenH264API, nal_units};

use super::Color32;

/// How many units in a row may fail before the stream is given up on. One
/// that will not decode is a glitch; the next that stands alone mends it.
const MOST_REFUSED: u32 = 30;

/// Nothing has to be started for this decoder. Kept so the worker reads the
/// same on every system.
pub struct Runtime;

impl Runtime {
    pub fn start() -> Result<Self, String> {
        Ok(Self)
    }
}

/// A decoded picture, not yet in the screen's colours. Making those costs
/// more than decoding, so a picture that is already late is dropped as this.
pub struct Decoded {
    size: (usize, usize),
    /// Luma, then the two chromas at half the width and height, with what
    /// each of their rows takes up.
    planes: [Vec<u8>; 3],
    strides: [usize; 3],
    /// When it shows, in 100 ns.
    pub time: i64,
    pub duration: i64,
}

pub struct Decoder {
    decoder: OpenH264,
    /// The times of what has gone in and not yet come out. Pictures come out
    /// in the order they show, which is not the order they go in.
    times: Vec<(i64, i64)>,
    ready: VecDeque<Decoded>,
    source: (u32, u32),
    size: (u32, u32),
    refused: u32,
}

/// A decoder that holds pictures back until it is their turn to show. Left to
/// itself this wrapper has it hand each out at once, which a film with
/// B-frames cannot be decoded by.
fn fresh() -> Result<OpenH264, String> {
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    OpenH264::with_api_config(OpenH264API::from_source(), config)
        .map_err(|error| format!("no H.264 decoder: {error}"))
}

impl Decoder {
    /// A decoder for pictures of `source` size that hands them on at `size`.
    /// With `hardware` there is none: the processor is all this has.
    pub fn new(source: (u32, u32), size: (u32, u32), hardware: bool) -> Result<Self, String> {
        if hardware {
            return Err("there is no decoding on the graphics card here".to_owned());
        }
        Ok(Self {
            decoder: fresh()?,
            times: Vec::new(),
            ready: VecDeque::new(),
            source,
            size,
            refused: 0,
        })
    }

    pub fn hardware(&self) -> bool {
        false
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Hands the decoder one picture's bytes, a unit at a time, as it reads
    /// them. It always takes them.
    pub fn push(&mut self, data: &[u8], time: i64, duration: i64) -> Result<bool, String> {
        self.times.push((time, duration));
        for unit in nal_units(data) {
            match self.decoder.decode(unit) {
                Ok(picture) => {
                    self.refused = 0;
                    if let Some(picture) = picture {
                        self.ready.push_back(kept(&mut self.times, &picture));
                    }
                }
                Err(error) => {
                    self.refused += 1;
                    if self.refused > MOST_REFUSED {
                        return Err(format!("decoding failed: {error}"));
                    }
                    log::debug!("video: a picture would not decode: {error}");
                }
            }
        }
        Ok(true)
    }

    /// The next picture the decoder has ready, or `None` when it wants more
    /// of the stream first.
    pub fn pull(&mut self) -> Result<Option<Decoded>, String> {
        Ok(self.ready.pop_front())
    }

    /// Makes of a decoded picture the pixels egui draws, at this decoder's
    /// size. `pixels` is reused from picture to picture.
    pub fn convert(&mut self, decoded: Decoded, pixels: &mut Vec<Color32>) -> Result<(), String> {
        let film = (
            decoded.size.0.min(self.source.0 as usize),
            decoded.size.1.min(self.source.1 as usize),
        );
        convert(
            &decoded,
            film,
            (self.size.0 as usize, self.size.1 as usize),
            pixels,
        );
        Ok(())
    }

    /// Has pictures leave at another size from here on.
    pub fn resize(&mut self, size: (u32, u32)) {
        self.size = size;
    }

    /// Forgets everything in hand, for a jump to another place.
    pub fn flush(&mut self) {
        self.times.clear();
        self.ready.clear();
        self.refused = 0;
        match fresh() {
            Ok(decoder) => self.decoder = decoder,
            Err(error) => log::warn!("video: {error}"),
        }
    }
}

/// A picture that has come out, copied, with its time: the earliest still
/// waiting, as they come out in the order they show.
fn kept(times: &mut Vec<(i64, i64)>, picture: &impl YUVSource) -> Decoded {
    let earliest = times
        .iter()
        .enumerate()
        .min_by_key(|(_, (time, _))| *time)
        .map(|(place, _)| place);
    let (time, duration) = earliest.map_or((0, 0), |place| times.swap_remove(place));
    let (ystride, ustride, vstride) = picture.strides();
    Decoded {
        size: picture.dimensions(),
        planes: [
            picture.y().to_vec(),
            picture.u().to_vec(),
            picture.v().to_vec(),
        ],
        strides: [ystride, ustride, vstride],
        time,
        duration,
    }
}

/// Of the `film` part of a picture, the pixels at `size`: each pixel of the
/// result the nearest one of the picture, in the colours of HD video.
fn convert(
    decoded: &Decoded,
    film: (usize, usize),
    size: (usize, usize),
    pixels: &mut Vec<Color32>,
) {
    let (width, height) = size;
    let [luma, blue, red] = &decoded.planes;
    let [luma_stride, blue_stride, red_stride] = decoded.strides;
    pixels.clear();
    pixels.reserve(width * height);
    let columns: Vec<usize> = (0..width)
        .map(|x| (x * film.0 / width.max(1)).min(film.0.saturating_sub(1)))
        .collect();
    for y in 0..height {
        let row = (y * film.1 / height.max(1)).min(film.1.saturating_sub(1));
        let luma_at = row * luma_stride;
        let blue_at = (row / 2) * blue_stride;
        let red_at = (row / 2) * red_stride;
        pixels.extend(columns.iter().map(|&column| {
            let get = |plane: &[u8], at: usize| plane.get(at).copied().map_or(0, i32::from);
            colour(
                get(luma, luma_at + column),
                get(blue, blue_at + column / 2),
                get(red, red_at + column / 2),
            )
        }));
    }
}

/// One pixel from studio-range BT.709, in fixed point.
fn colour(luma: i32, blue: i32, red: i32) -> Color32 {
    let (y, u, v) = ((luma - 16) * 298, blue - 128, red - 128);
    let clamp = |value: i32| ((value + 128) >> 8).clamp(0, 255) as u8;
    Color32::from_rgb(
        clamp(y + 459 * v),
        clamp(y - 55 * u - 136 * v),
        clamp(y + 541 * u),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ten pictures of 64 by 64, the first half red and the rest blue, made
    /// with B-frames by `ffmpeg -c:v libx264 -bf 2`: the order they go in is
    /// not the order they show.
    const CLIP: &[u8] = include_bytes!("../../assets/testdata/red-blue.h264");

    /// The access units of a stream of one slice to a picture: each ends where
    /// the next one's picture has been passed.
    fn units(stream: &[u8]) -> Vec<&[u8]> {
        let starts: Vec<usize> = (0..stream.len().saturating_sub(3))
            .filter(|&at| stream[at..].starts_with(&[0, 0, 1]))
            .collect();
        let mut found = Vec::new();
        let mut begun = 0;
        for pair in starts.windows(2) {
            if matches!(stream[pair[0] + 3] & 0x1f, 1 | 5) {
                found.push(&stream[begun..pair[1]]);
                begun = pair[1];
            }
        }
        found.push(&stream[begun..]);
        found
    }

    #[test]
    fn studio_black_and_white_are_black_and_white() {
        assert_eq!(colour(16, 128, 128), Color32::from_rgb(0, 0, 0));
        assert_eq!(colour(235, 128, 128), Color32::from_rgb(255, 255, 255));
    }

    #[test]
    fn a_picture_is_made_the_size_asked_for() {
        let decoded = Decoded {
            size: (2, 2),
            planes: [vec![235; 4], vec![128], vec![128]],
            strides: [2, 1, 1],
            time: 0,
            duration: 0,
        };
        let mut pixels = Vec::new();
        convert(&decoded, (2, 2), (4, 3), &mut pixels);
        assert_eq!(pixels.len(), 12);
        assert!(pixels.iter().all(|pixel| *pixel == Color32::WHITE));
    }

    /// Whether a pixel is mostly red, as against mostly blue.
    fn is_red(pixel: Color32) -> bool {
        pixel.r() > pixel.b()
    }

    #[test]
    fn a_stream_with_b_frames_comes_out_in_the_order_it_shows() {
        let mut decoder = Decoder::new((64, 64), (8, 8), false).expect("a decoder");
        // Stamped as a demuxer would, by the order they show; the test only
        // needs them to come out rising.
        let units = units(CLIP);
        assert!(units.len() >= 10, "{} units", units.len());
        let mut out = Vec::new();
        for (place, unit) in units.iter().enumerate() {
            decoder.push(unit, place as i64, 1).expect("taken");
            while let Some(decoded) = decoder.pull().expect("pulled") {
                out.push(decoded);
            }
        }
        assert!(out.len() >= 5, "only {} came out", out.len());
        let mut pixels = Vec::new();
        let first = out.remove(0);
        decoder.convert(first, &mut pixels).expect("converted");
        assert_eq!(pixels.len(), 64);
        assert!(is_red(pixels[0]), "the film starts red: {:?}", pixels[0]);
        let times: Vec<i64> = out.iter().map(|decoded| decoded.time).collect();
        assert!(
            times.windows(2).all(|pair| pair[0] < pair[1]),
            "{times:?} is not rising"
        );
    }

    #[test]
    fn after_a_flush_the_stream_starts_again_from_a_picture_that_stands_alone() {
        let mut decoder = Decoder::new((64, 64), (8, 8), false).expect("a decoder");
        let units = units(CLIP);
        decoder.push(units[0], 0, 1).expect("taken");
        decoder.flush();
        assert!(decoder.pull().expect("pulled").is_none());
        for (place, unit) in units.iter().enumerate() {
            decoder.push(unit, place as i64, 1).expect("taken");
        }
        assert!(decoder.pull().expect("pulled").is_some());
    }
}
