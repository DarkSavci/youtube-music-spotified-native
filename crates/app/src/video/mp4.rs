//! Reading the picture out of the file YouTube sends: a fragmented MP4.
//!
//! The file opens with a description of the track (`moov`) and an index of
//! its pieces (`sidx`); the pieces follow, each a few seconds of pictures
//! that starts on one which stands alone. Windows' own reader of MP4 files
//! says such a file ends where it begins, so it is read here: only what
//! these files hold, and nothing more of the format than that.

/// Times here are in the units Media Foundation counts in: 100 ns.
pub const SECOND: i64 = 10_000_000;

/// What the opening of the file says of the track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub width: u32,
    pub height: u32,
    /// Units of the track's own clock in a second.
    pub timescale: u32,
    /// How many bytes say how long each unit of a picture is.
    pub length_size: usize,
    /// The parameter sets a decoder needs before any picture, each behind
    /// the mark that starts a unit.
    pub parameters: Vec<u8>,
    /// What a picture's time is counted from, in the track's units.
    pub media_start: i64,
    defaults: Defaults,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Defaults {
    duration: u32,
    size: u32,
    flags: u32,
}

/// One piece of the file: where its bytes are and when its pictures show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub offset: u64,
    pub size: u64,
    /// When its first picture shows, and how long the piece lasts.
    pub start: i64,
    pub duration: i64,
}

/// One picture inside a piece that has been fetched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Where its bytes are, counted from the start of the piece.
    pub offset: usize,
    pub size: usize,
    /// When it shows and for how long.
    pub time: i64,
    pub duration: i64,
    /// It stands alone: decoding can start here.
    pub key: bool,
}

/// What was wrong with a file, in a few words for the log.
pub type Fault = &'static str;

/// The mark that starts a unit in the form the decoder reads.
const START: [u8; 4] = [0, 0, 0, 1];
/// The sample flag that says a picture leans on another.
const NOT_KEY: u32 = 0x0001_0000;

struct Atom<'a> {
    kind: [u8; 4],
    body: &'a [u8],
    /// Where the atom starts and ends in what it was found in.
    start: usize,
    end: usize,
}

/// The atoms that lie whole within `data`, in order. One that runs past
/// the end stops the walk: the caller has not fetched it all.
fn atoms(data: &[u8]) -> impl Iterator<Item = Atom<'_>> {
    let mut at = 0;
    std::iter::from_fn(move || {
        let head = data.get(at..at + 8)?;
        let mut size = be32(head, 0)? as usize;
        let mut body = 8;
        if size == 1 {
            size = usize::try_from(be64(data.get(at..)?, 8)?).ok()?;
            body = 16;
        }
        if size < body {
            return None;
        }
        let end = at.checked_add(size)?;
        let atom = Atom {
            kind: [head[4], head[5], head[6], head[7]],
            body: data.get(at + body..end)?,
            start: at,
            end,
        };
        at = end;
        Some(atom)
    })
}

fn find<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<Atom<'a>> {
    atoms(data).find(|atom| &atom.kind == kind)
}

fn be16(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + 2)?;
    Some(u32::from(u16::from_be_bytes([bytes[0], bytes[1]])))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn be64(data: &[u8], at: usize) -> Option<u64> {
    Some((u64::from(be32(data, at)?) << 32) | u64::from(be32(data, at + 4)?))
}

/// How much of the file's opening is needed before `open` can read it, as
/// far as `data` can tell: the end of the index, or `None` while even that
/// cannot be seen.
pub fn opening_length(data: &[u8]) -> Option<usize> {
    find(data, b"moov")?;
    let mut at = 0;
    loop {
        let head = data.get(at..at + 8)?;
        let size = be32(head, 0)? as usize;
        if size < 8 {
            return None;
        }
        if &head[4..8] == b"sidx" {
            return Some(at + size);
        }
        at += size;
    }
}

/// Reads the opening of the file: the track and the index of its pieces.
pub fn open(data: &[u8]) -> Result<(Track, Vec<Segment>), Fault> {
    let moov = find(data, b"moov").ok_or("no track description")?;
    let track = track(moov.body)?;
    let sidx = find(data, b"sidx").ok_or("no index of pieces")?;
    let segments = index(sidx.body, sidx.end as u64).ok_or("the index is cut short")?;
    if segments.is_empty() {
        return Err("the index is empty");
    }
    Ok((track, segments))
}

fn track(moov: &[u8]) -> Result<Track, Fault> {
    for trak in atoms(moov).filter(|atom| &atom.kind == b"trak") {
        let Some(mdia) = find(trak.body, b"mdia") else {
            continue;
        };
        let handler = find(mdia.body, b"hdlr").and_then(|hdlr| hdlr.body.get(8..12));
        if handler != Some(b"vide".as_slice()) {
            continue;
        }
        let mdhd = find(mdia.body, b"mdhd").ok_or("no clock for the track")?;
        let long = mdhd.body.first() == Some(&1);
        let timescale = be32(mdhd.body, if long { 20 } else { 12 }).ok_or("no clock")?;
        if timescale == 0 {
            return Err("the track's clock does not run");
        }
        let entry = find(mdia.body, b"minf")
            .and_then(|minf| find(minf.body, b"stbl"))
            .and_then(|stbl| find(stbl.body, b"stsd"))
            .and_then(|stsd| stsd.body.get(8..))
            .and_then(|entries| find(entries, b"avc1"))
            .ok_or("the picture is not H.264")?;
        let width = be16(entry.body, 24).ok_or("no picture size")?;
        let height = be16(entry.body, 26).ok_or("no picture size")?;
        let config = entry
            .body
            .get(78..)
            .and_then(|inner| find(inner, b"avcC"))
            .ok_or("no decoder configuration")?;
        let (length_size, parameters) =
            parameters(config.body).ok_or("the decoder configuration is cut short")?;
        let defaults = find(moov, b"mvex")
            .and_then(|mvex| find(mvex.body, b"trex"))
            .and_then(|trex| {
                Some(Defaults {
                    duration: be32(trex.body, 12)?,
                    size: be32(trex.body, 16)?,
                    flags: be32(trex.body, 20)?,
                })
            })
            .unwrap_or_default();
        return Ok(Track {
            width,
            height,
            timescale,
            length_size,
            parameters,
            media_start: media_start(trak.body),
            defaults,
        });
    }
    Err("no picture track")
}

/// The decoder's parameter sets, each behind a start mark.
fn parameters(config: &[u8]) -> Option<(usize, Vec<u8>)> {
    let length_size = usize::from(config.get(4)? & 3) + 1;
    let mut out = Vec::new();
    let mut at = 5;
    for mask in [0x1f, 0xff] {
        let count = config.get(at)? & mask;
        at += 1;
        for _ in 0..count {
            let length = be16(config, at)? as usize;
            out.extend_from_slice(&START);
            out.extend_from_slice(config.get(at + 2..at + 2 + length)?);
            at += 2 + length;
        }
    }
    Some((length_size, out))
}

/// Where the track's edit list says showing starts: pictures that are
/// stored out of order are all given a delay, and this takes it off again.
fn media_start(trak: &[u8]) -> i64 {
    let list = find(trak, b"edts").and_then(|edts| find(edts.body, b"elst"));
    let Some(list) = list else { return 0 };
    let long = list.body.first() == Some(&1);
    let start = if long {
        be64(list.body, 16).map(|time| time as i64)
    } else {
        be32(list.body, 12).map(|time| i64::from(time as i32))
    };
    start.filter(|time| *time > 0).unwrap_or(0)
}

fn index(sidx: &[u8], end: u64) -> Option<Vec<Segment>> {
    let long = *sidx.first()? == 1;
    let timescale = i64::from(be32(sidx, 8)?);
    if timescale == 0 {
        return None;
    }
    let (earliest, first, mut at) = if long {
        (be64(sidx, 12)?, be64(sidx, 20)?, 28)
    } else {
        (u64::from(be32(sidx, 12)?), u64::from(be32(sidx, 16)?), 20)
    };
    let count = be16(sidx, at + 2)? as usize;
    at += 4;
    let mut offset = end + first;
    let mut ticks = earliest as i64;
    let mut segments = Vec::with_capacity(count);
    for _ in 0..count {
        let size = u64::from(be32(sidx, at)? & 0x7fff_ffff);
        let length = i64::from(be32(sidx, at + 4)?);
        segments.push(Segment {
            offset,
            size,
            start: scale(ticks, timescale),
            duration: scale(length, timescale),
        });
        offset += size;
        ticks += length;
        at += 12;
    }
    Some(segments)
}

fn scale(ticks: i64, timescale: i64) -> i64 {
    (i128::from(ticks) * i128::from(SECOND) / i128::from(timescale.max(1))) as i64
}

/// The piece that holds the picture showing at `time`: the last one that
/// starts at or before it.
pub fn segment_at(segments: &[Segment], time: i64) -> usize {
    segments
        .partition_point(|segment| segment.start <= time)
        .saturating_sub(1)
}

/// The pictures in a piece, in the order they are to be decoded.
pub fn samples(track: &Track, segment: &[u8]) -> Result<Vec<Sample>, Fault> {
    let moof = find(segment, b"moof").ok_or("a piece has no table of pictures")?;
    let mut samples = Vec::new();
    for traf in atoms(moof.body).filter(|atom| &atom.kind == b"traf") {
        fragment(track, traf.body, moof.start, segment.len(), &mut samples)
            .ok_or("a table of pictures is cut short")?;
    }
    if samples.is_empty() {
        return Err("a piece holds no pictures");
    }
    Ok(samples)
}

fn fragment(
    track: &Track,
    traf: &[u8],
    moof_start: usize,
    length: usize,
    samples: &mut Vec<Sample>,
) -> Option<()> {
    let tfhd = find(traf, b"tfhd")?;
    let flags = be32(tfhd.body, 0)? & 0x00ff_ffff;
    let mut at = 8;
    let mut defaults = track.defaults;
    // An absolute base would count from the start of the file; these files
    // count from the piece's own table, which is where the piece starts.
    if flags & 0x1 != 0 {
        at += 8;
    }
    if flags & 0x2 != 0 {
        at += 4;
    }
    for (bit, field) in [
        (0x8, &mut defaults.duration),
        (0x10, &mut defaults.size),
        (0x20, &mut defaults.flags),
    ] {
        if flags & bit != 0 {
            *field = be32(tfhd.body, at)?;
            at += 4;
        }
    }
    let tfdt = find(traf, b"tfdt")?;
    let mut decode = if tfdt.body.first() == Some(&1) {
        be64(tfdt.body, 4)? as i64
    } else {
        i64::from(be32(tfdt.body, 4)?)
    };
    let timescale = i64::from(track.timescale);
    for trun in atoms(traf).filter(|atom| &atom.kind == b"trun") {
        let signed = trun.body.first() != Some(&0);
        let flags = be32(trun.body, 0)? & 0x00ff_ffff;
        let count = be32(trun.body, 4)? as usize;
        let mut at = 8;
        let mut offset = moof_start;
        if flags & 0x1 != 0 {
            offset = offset.checked_add_signed(be32(trun.body, at)? as i32 as isize)?;
            at += 4;
        }
        let first_flags = if flags & 0x4 != 0 {
            at += 4;
            Some(be32(trun.body, at - 4)?)
        } else {
            None
        };
        for index in 0..count {
            let mut field = |bit: u32, default: u32| -> Option<u32> {
                if flags & bit == 0 {
                    return Some(default);
                }
                at += 4;
                be32(trun.body, at - 4)
            };
            let duration = field(0x100, defaults.duration)?;
            let size = field(0x200, defaults.size)? as usize;
            let mut sample_flags = field(0x400, defaults.flags)?;
            let shift = field(0x800, 0)?;
            if let (0, Some(first)) = (index, first_flags) {
                sample_flags = first;
            }
            let shift = if signed {
                i64::from(shift as i32)
            } else {
                i64::from(shift)
            };
            if offset.checked_add(size)? > length {
                return None;
            }
            samples.push(Sample {
                offset,
                size,
                time: scale(decode + shift - track.media_start, timescale),
                duration: scale(i64::from(duration), timescale),
                key: sample_flags & NOT_KEY == 0,
            });
            offset += size;
            decode += i64::from(duration);
        }
    }
    Some(())
}

/// Rewrites a picture from the form the file keeps it in, each unit behind
/// its length, to the form the decoder reads, each behind a start mark. A
/// picture that stands alone is given the parameter sets first, so decoding
/// can begin at it.
pub fn annex_b(track: &Track, sample: &[u8], key: bool, out: &mut Vec<u8>) -> Result<(), Fault> {
    out.clear();
    if key {
        out.extend_from_slice(&track.parameters);
    }
    let mut at = 0;
    while at < sample.len() {
        let head = sample
            .get(at..at + track.length_size)
            .ok_or("a picture is cut short")?;
        let length = head
            .iter()
            .fold(0usize, |sum, byte| (sum << 8) | usize::from(*byte));
        at += track.length_size;
        let unit = sample
            .get(at..at + length)
            .ok_or("a picture is cut short")?;
        out.extend_from_slice(&START);
        out.extend_from_slice(unit);
        at += length;
    }
    Ok(())
}

#[cfg(test)]
#[path = "mp4_tests.rs"]
mod tests;
