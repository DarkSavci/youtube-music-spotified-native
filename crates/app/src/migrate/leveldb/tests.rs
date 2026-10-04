//! The reader against files built here in the same format, since a real
//! profile's are somebody's preferences.

use std::path::PathBuf;

use super::*;

pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("spotified-migrate-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value & 0x7f) as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn prefixed(bytes: &[u8], out: &mut Vec<u8>) {
    varint(bytes.len() as u64, out);
    out.extend_from_slice(bytes);
}

/// One write: a key and its value, or `None` for a deletion.
pub type Write<'a> = (&'a [u8], Option<&'a [u8]>);

/// A log holding these batches, the first write of each numbered as given.
pub fn log_file(batches: &[(u64, Vec<Write<'_>>)]) -> Vec<u8> {
    let mut log = Vec::new();
    for (sequence, writes) in batches {
        let mut batch = Vec::new();
        batch.extend_from_slice(&sequence.to_le_bytes());
        batch.extend_from_slice(&(writes.len() as u32).to_le_bytes());
        for (key, value) in writes {
            batch.push(u8::from(value.is_some()));
            prefixed(key, &mut batch);
            if let Some(value) = value {
                prefixed(value, &mut batch);
            }
        }
        // In fragments where it does not fit what is left of the block.
        let mut rest = batch.as_slice();
        let mut first = true;
        loop {
            let room = LOG_BLOCK - log.len() % LOG_BLOCK;
            if room < LOG_HEADER {
                log.resize(log.len() + room, 0);
                continue;
            }
            let fits = rest.len().min(room - LOG_HEADER);
            let last = fits == rest.len();
            let kind = match (first, last) {
                (true, true) => 1,
                (true, false) => 2,
                (false, false) => 3,
                (false, true) => 4,
            };
            log.extend_from_slice(&[0; 4]);
            log.extend_from_slice(&(fits as u16).to_le_bytes());
            log.push(kind);
            log.extend_from_slice(&rest[..fits]);
            rest = &rest[fits..];
            first = false;
            if last {
                break;
            }
        }
    }
    log
}

/// The bytes as Snappy would hold them if it found nothing to compress.
fn snapped(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    varint(bytes.len() as u64, &mut out);
    for run in bytes.chunks(60) {
        out.push(((run.len() - 1) as u8) << 2);
        out.extend_from_slice(run);
    }
    out
}

fn add_block(table: &mut Vec<u8>, contents: &[u8], compress: bool) -> (u64, u64) {
    let offset = table.len() as u64;
    let stored = if compress {
        snapped(contents)
    } else {
        contents.to_vec()
    };
    table.extend_from_slice(&stored);
    table.push(u8::from(compress));
    table.extend_from_slice(&[0; 4]);
    (offset, stored.len() as u64)
}

/// A block of these entries, each key sharing what it can with the last.
fn entries_block(entries: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let mut block = Vec::new();
    let mut last: &[u8] = &[];
    for (key, value) in entries {
        let shared = key.iter().zip(last).take_while(|(a, b)| a == b).count();
        varint(shared as u64, &mut block);
        varint((key.len() - shared) as u64, &mut block);
        varint(value.len() as u64, &mut block);
        block.extend_from_slice(&key[shared..]);
        block.extend_from_slice(value);
        last = key;
    }
    block.extend_from_slice(&0u32.to_le_bytes());
    block.extend_from_slice(&1u32.to_le_bytes());
    block
}

/// A write with the sequence number it was made under.
pub type Numbered<'a> = (&'a [u8], u64, Option<&'a [u8]>);

/// A table of one data block holding these writes.
pub fn table_file(writes: &[Numbered<'_>], compress: bool) -> Vec<u8> {
    let entries: Vec<(Vec<u8>, Vec<u8>)> = writes
        .iter()
        .map(|(key, sequence, value)| {
            let mut internal = key.to_vec();
            let trailer = (sequence << 8) | u64::from(value.is_some());
            internal.extend_from_slice(&trailer.to_le_bytes());
            (internal, value.unwrap_or_default().to_vec())
        })
        .collect();
    let mut table = Vec::new();
    let (offset, size) = add_block(&mut table, &entries_block(&entries), compress);
    let mut handle = Vec::new();
    varint(offset, &mut handle);
    varint(size, &mut handle);
    let index = entries_block(&[(b"\xff".to_vec(), handle)]);
    let (index_offset, index_size) = add_block(&mut table, &index, false);
    let mut footer = Vec::new();
    varint(0, &mut footer);
    varint(0, &mut footer);
    varint(index_offset, &mut footer);
    varint(index_size, &mut footer);
    footer.resize(FOOTER - 8, 0);
    footer.extend_from_slice(&TABLE_MAGIC.to_le_bytes());
    table.extend_from_slice(&footer);
    table
}

#[test]
fn the_newest_write_for_a_key_is_the_one_read_whichever_file_it_is_in() {
    let dir = scratch("leveldb-newest");
    let table = table_file(
        &[
            (b"alpha", 5, Some(b"from the table")),
            (b"alphabet", 6, Some(b"kept")),
            (b"gone", 7, Some(b"deleted later")),
            (b"stays", 30, Some(b"newer than the log")),
        ],
        false,
    );
    std::fs::write(dir.join("000005.ldb"), table).expect("write");
    let log = log_file(&[
        (10, vec![(b"alpha", Some(b"from the log")), (b"gone", None)]),
        (
            12,
            vec![(b"stays", Some(b"older")), (b"fresh", Some(b"new"))],
        ),
    ]);
    std::fs::write(dir.join("000007.log"), log).expect("write");
    std::fs::write(dir.join("LOCK"), b"").expect("write");

    let read = read(&dir).expect("read");
    let value = |key: &[u8]| read.get(key).map(Vec::as_slice);
    assert_eq!(value(b"alpha"), Some(&b"from the log"[..]));
    assert_eq!(value(b"alphabet"), Some(&b"kept"[..]));
    assert_eq!(value(b"gone"), None);
    assert_eq!(value(b"stays"), Some(&b"newer than the log"[..]));
    assert_eq!(value(b"fresh"), Some(&b"new"[..]));
}

#[test]
fn a_write_longer_than_a_block_of_the_log_is_put_back_together() {
    let dir = scratch("leveldb-long");
    let long = vec![b'x'; LOG_BLOCK * 2 + 100];
    let log = log_file(&[
        (1, vec![(b"short", Some(b"one"))]),
        (2, vec![(b"long", Some(&long))]),
        (3, vec![(b"after", Some(b"two"))]),
    ]);
    std::fs::write(dir.join("000003.log"), log).expect("write");
    let read = read(&dir).expect("read");
    assert_eq!(read.get(&b"long"[..]), Some(&long));
    assert_eq!(
        read.get(&b"after"[..]).map(Vec::as_slice),
        Some(&b"two"[..])
    );
}

#[test]
fn a_compressed_table_is_read_like_a_plain_one() {
    let dir = scratch("leveldb-snappy");
    let value = vec![b'v'; 200];
    let table = table_file(&[(b"key", 1, Some(&value))], true);
    std::fs::write(dir.join("000004.ldb"), table).expect("write");
    assert_eq!(read(&dir).expect("read").get(&b"key"[..]), Some(&value));
}

#[test]
fn snappy_copies_are_undone_including_one_that_overlaps_itself() {
    // "abcd", then four bytes from four back, then ten from one back.
    let compressed = [
        18, // eighteen bytes when undone
        3 << 2,
        b'a',
        b'b',
        b'c',
        b'd',
        1, // a copy of four with a one-byte offset
        4,
        (9 << 2) | 2, // a copy of ten with a two-byte offset
        1,
        0,
    ];
    assert_eq!(
        unsnap(&compressed).as_deref(),
        Some(&b"abcdabcddddddddddd"[..])
    );
    // A copy from before the start is a damaged block.
    assert_eq!(unsnap(&[4, 1, 9]), None);
}

#[test]
fn a_torn_file_ends_the_reading_and_keeps_what_came_before() {
    let dir = scratch("leveldb-torn");
    let mut log = log_file(&[
        (1, vec![(b"whole", Some(b"yes"))]),
        (2, vec![(b"torn", Some(b"this one is cut short"))]),
    ]);
    log.truncate(log.len() - 6);
    std::fs::write(dir.join("000003.log"), log).expect("write");
    std::fs::write(dir.join("000009.ldb"), b"not a table").expect("write");
    let read = read(&dir).expect("read");
    assert_eq!(read.len(), 1);
    assert!(read.contains_key(&b"whole"[..]));
}

#[test]
fn a_folder_that_is_not_there_is_an_error_and_not_an_empty_store() {
    assert!(read(&scratch("leveldb-absent").join("nope")).is_err());
}
