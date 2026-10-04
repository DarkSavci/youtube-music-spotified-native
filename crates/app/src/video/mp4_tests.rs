//! A film put together by hand, in the shape YouTube's are: 25 pictures a
//! second on a clock of 12800, two pieces, the first picture of each
//! standing alone.

use super::*;

fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn be(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

const SPS: [u8; 4] = [0x67, 0x64, 0x00, 0x28];
const PPS: [u8; 2] = [0x68, 0xee];

fn moov() -> Vec<u8> {
    let mut avcc = vec![1, 0x64, 0, 0x28, 0xff, 0xe1, 0, SPS.len() as u8];
    avcc.extend_from_slice(&SPS);
    avcc.extend_from_slice(&[1, 0, PPS.len() as u8]);
    avcc.extend_from_slice(&PPS);
    let mut entry = vec![0u8; 78];
    entry[24..26].copy_from_slice(&1920u16.to_be_bytes());
    entry[26..28].copy_from_slice(&1080u16.to_be_bytes());
    entry.extend(atom(b"avcC", &avcc));
    let stsd = atom(b"stsd", &[be(&[0, 1]), atom(b"avc1", &entry)].concat());
    let minf = atom(b"minf", &atom(b"stbl", &stsd));
    let mdhd = atom(b"mdhd", &be(&[0, 0, 0, 12800, 0, 0]));
    let hdlr = atom(
        b"hdlr",
        &[be(&[0, 0]), b"vide".to_vec(), vec![0; 12]].concat(),
    );
    let mdia = atom(b"mdia", &[mdhd, hdlr, minf].concat());
    // Showing starts 1024 units in: the delay pictures out of order need.
    let elst = atom(b"elst", &be(&[0, 1, 0, 1024, 0x0001_0000]));
    let trak = atom(b"trak", &[atom(b"edts", &elst), mdia].concat());
    // By default a picture lasts 512 units and leans on another.
    let trex = atom(b"trex", &be(&[0, 1, 1, 512, 0, 0x0101_0000]));
    atom(b"moov", &[atom(b"mvex", &trex), trak].concat())
}

/// An index of two pieces, the first 2000 bytes and five seconds.
fn sidx() -> Vec<u8> {
    let head = be(&[0, 1, 12800, 0, 0]);
    let count = [0u8, 0, 0, 2];
    let pieces = be(&[2000, 64000, 0x9000_0000, 3000, 51200, 0x9000_0000]);
    atom(b"sidx", &[head, count.to_vec(), pieces].concat())
}

fn opening() -> Vec<u8> {
    [atom(b"ftyp", b"dash\0\0\0\0"), moov(), sidx()].concat()
}

/// A piece of three pictures of 10, 20 and 30 bytes, decoded from
/// `decode` on; the second is shown after the third.
fn piece(decode: u32) -> Vec<u8> {
    let table = |data_offset: u32| {
        let tfhd = atom(b"tfhd", &be(&[0x0002_0000, 1]));
        let tfdt = atom(b"tfdt", &be(&[0, decode]));
        // Flags: where the data is, the first picture's flags, and for
        // each its size and how long after decoding it shows.
        let mut trun = be(&[0x0000_0a05, 3, data_offset, 0x0200_0000]);
        trun.extend(be(&[10, 1024, 20, 2048, 30, 512]));
        let traf = atom(b"traf", &[tfhd, tfdt, atom(b"trun", &trun)].concat());
        atom(b"moof", &[atom(b"mfhd", &be(&[0, 1])), traf].concat())
    };
    let length = table(0).len() as u32;
    [table(length + 8), atom(b"mdat", &[7u8; 60])].concat()
}

#[test]
fn the_opening_says_what_the_film_is() {
    let (track, _) = open(&opening()).expect("a film");
    assert_eq!((track.width, track.height), (1920, 1080));
    assert_eq!(track.timescale, 12800);
    assert_eq!(track.length_size, 4);
    assert_eq!(track.media_start, 1024);
    let mut parameters = vec![0, 0, 0, 1];
    parameters.extend_from_slice(&SPS);
    parameters.extend_from_slice(&[0, 0, 0, 1]);
    parameters.extend_from_slice(&PPS);
    assert_eq!(track.parameters, parameters);
}

#[test]
fn the_index_places_each_piece_in_the_file_and_in_time() {
    let file = opening();
    let (_, segments) = open(&file).expect("a film");
    let first = Segment {
        offset: file.len() as u64,
        size: 2000,
        start: 0,
        duration: 5 * SECOND,
    };
    let second = Segment {
        offset: file.len() as u64 + 2000,
        size: 3000,
        start: 5 * SECOND,
        duration: 4 * SECOND,
    };
    assert_eq!(segments, [first, second]);
}

#[test]
fn the_opening_is_known_to_be_whole_only_once_the_index_is() {
    let file = opening();
    assert_eq!(opening_length(&file), Some(file.len()));
    // The index's own header is enough to say how long it is.
    let short = &file[..file.len() - 20];
    assert_eq!(opening_length(short), Some(file.len()));
    assert!(open(short).is_err());
    assert_eq!(opening_length(&file[..40]), None);
}

#[test]
fn a_time_is_in_the_last_piece_that_starts_by_then() {
    let (_, segments) = open(&opening()).expect("a film");
    assert_eq!(segment_at(&segments, 0), 0);
    assert_eq!(segment_at(&segments, 5 * SECOND - 1), 0);
    assert_eq!(segment_at(&segments, 5 * SECOND), 1);
    assert_eq!(segment_at(&segments, 500 * SECOND), 1);
    assert_eq!(segment_at(&segments, -SECOND), 0);
}

#[test]
fn a_pieces_pictures_come_with_their_places_and_times() {
    let (track, _) = open(&opening()).expect("a film");
    let bytes = piece(64000);
    let samples = samples(&track, &bytes).expect("pictures");
    let data = bytes.len() - 60;
    let frame = SECOND / 25;
    let picture = |offset, size, time, key| Sample {
        offset,
        size,
        time,
        duration: frame,
        key,
    };
    // Decoded at 5 s, 5.04 and 5.08; shown two pictures after the edit
    // list's start, then four, then one: the middle one is shown last.
    let expected = [
        picture(data, 10, 5 * SECOND, true),
        picture(data + 10, 20, 5 * SECOND + 3 * frame, false),
        picture(data + 30, 30, 5 * SECOND + frame, false),
    ];
    assert_eq!(samples, expected);
}

#[test]
fn a_piece_cut_short_is_refused_not_read_past() {
    let (track, _) = open(&opening()).expect("a film");
    let bytes = piece(0);
    assert!(samples(&track, &bytes[..bytes.len() - 1]).is_err());
    assert!(samples(&track, &bytes[..30]).is_err());
    assert!(samples(&track, b"not a film at all").is_err());
}

#[test]
fn a_picture_is_rewritten_with_start_marks_for_the_decoder() {
    let (track, _) = open(&opening()).expect("a film");
    // Two units, of three bytes and of one, each behind its length.
    let sample = [0, 0, 0, 3, 0x65, 1, 2, 0, 0, 0, 1, 0x06];
    let mut out = vec![9, 9];
    annex_b(&track, &sample, false, &mut out).expect("a picture");
    assert_eq!(out, [0, 0, 0, 1, 0x65, 1, 2, 0, 0, 0, 1, 0x06]);
    // One that stands alone is given the parameter sets first.
    annex_b(&track, &sample, true, &mut out).expect("a picture");
    assert!(out.starts_with(&track.parameters));
    assert_eq!(out.len(), track.parameters.len() + 12);
    // A length that runs past the end is a fault, not a panic.
    assert!(annex_b(&track, &[0, 0, 0, 9, 1], false, &mut out).is_err());
}
