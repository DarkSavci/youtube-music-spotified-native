//! Reads a LevelDB folder without opening it as a database.
//!
//! Chromium keeps a page's Local Storage in one, and the Electron app's
//! preferences are in there. A LevelDB library would want the folder's
//! lock, would replay its log and might compact it: all writes, in a
//! profile that must only be read, and one the other app may have open.
//! So the files are read as plain bytes instead. The format is small: a
//! write-ahead log of batches, and sorted tables of blocks that may be
//! Snappy-compressed. Every entry carries a sequence number, and the
//! newest entry for a key is the one that holds, so the manifest that says
//! which tables are live is not needed: a superseded table only has older
//! entries.
//!
//! Checksums are not verified. A torn tail is simply where reading stops,
//! and a preference read wrongly from a damaged file costs a default.

use std::collections::HashMap;
use std::io;
use std::path::Path;

/// A log is written in blocks of this size; a record never straddles the
/// last six bytes of one.
const LOG_BLOCK: usize = 32 * 1024;
const LOG_HEADER: usize = 7;
/// The footer of a table: two block handles, padded, and a magic number.
const FOOTER: usize = 48;
const TABLE_MAGIC: u64 = 0xdb47_7524_8b80_fb57;
/// What follows each block in a table: how it is compressed, and a checksum.
const BLOCK_TRAILER: usize = 5;

/// The newest thing said of a key: its value, or that it was deleted.
struct Entry {
    sequence: u64,
    value: Option<Vec<u8>>,
}

/// Every key in the folder with its current value.
pub fn read(folder: &Path) -> io::Result<HashMap<Vec<u8>, Vec<u8>>> {
    let mut entries: HashMap<Vec<u8>, Entry> = HashMap::new();
    let mut put = |key: &[u8], sequence: u64, value: Option<&[u8]>| {
        let newer = entries
            .get(key)
            .is_none_or(|entry| sequence >= entry.sequence);
        if newer {
            let value = value.map(<[u8]>::to_vec);
            entries.insert(key.to_vec(), Entry { sequence, value });
        }
    };
    for file in std::fs::read_dir(folder)?.flatten() {
        let path = file.path();
        let kind = path.extension().and_then(|kind| kind.to_str());
        match kind {
            Some("log") => read_log(&std::fs::read(&path)?, &mut put),
            // A file that is not a table after all holds nothing to read.
            Some("ldb" | "sst") => {
                let _ = read_table(&std::fs::read(&path)?, &mut put);
            }
            _ => {}
        }
    }
    Ok(entries
        .into_iter()
        .filter_map(|(key, entry)| Some((key, entry.value?)))
        .collect())
}

/// A cursor over bytes that answers `None` past the end, so a torn file
/// ends the reading and never panics.
struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        if count > self.0.len() {
            return None;
        }
        let (taken, rest) = self.0.split_at(count);
        self.0 = rest;
        Some(taken)
    }

    fn byte(&mut self) -> Option<u8> {
        self.take(1).map(|taken| taken[0])
    }

    fn u32(&mut self) -> Option<u32> {
        let taken = self.take(4)?;
        Some(u32::from_le_bytes(taken.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        let taken = self.take(8)?;
        Some(u64::from_le_bytes(taken.try_into().ok()?))
    }

    /// A number in seven-bit groups, least significant first.
    fn varint(&mut self) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }

    fn length(&mut self) -> Option<usize> {
        usize::try_from(self.varint()?).ok()
    }

    /// Bytes with their length before them.
    fn prefixed(&mut self) -> Option<&'a [u8]> {
        let length = self.length()?;
        self.take(length)
    }
}

/// Reads a write-ahead log: records, possibly in fragments, each a batch.
fn read_log(log: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) {
    const FULL: u8 = 1;
    const FIRST: u8 = 2;
    const MIDDLE: u8 = 3;
    const LAST: u8 = 4;
    let mut record: Vec<u8> = Vec::new();
    let mut at = 0;
    while at + LOG_HEADER <= log.len() {
        let room = LOG_BLOCK - at % LOG_BLOCK;
        if room < LOG_HEADER {
            at += room;
            continue;
        }
        let length = usize::from(u16::from_le_bytes([log[at + 4], log[at + 5]]));
        let kind = log[at + 6];
        let Some(data) = log.get(at + LOG_HEADER..at + LOG_HEADER + length) else {
            return;
        };
        at += LOG_HEADER + length;
        match kind {
            // A batch cut short has given what it had by the time it ends.
            FULL => {
                let _ = read_batch(data, put);
            }
            FIRST => record = data.to_vec(),
            MIDDLE => record.extend_from_slice(data),
            LAST => {
                record.extend_from_slice(data);
                let _ = read_batch(&record, put);
                record.clear();
            }
            // Zeroes: the rest of this block was set aside and never used.
            _ => at += LOG_BLOCK - at % LOG_BLOCK,
        }
    }
}

/// A batch: the sequence number of its first write, how many there are,
/// then each as a put or a deletion.
fn read_batch(batch: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> Option<()> {
    const DELETION: u8 = 0;
    const VALUE: u8 = 1;
    let mut bytes = Bytes(batch);
    let first = bytes.u64()?;
    let count = bytes.u32()?;
    for index in 0..u64::from(count) {
        let kind = bytes.byte()?;
        let key = bytes.prefixed()?;
        match kind {
            VALUE => put(key, first + index, Some(bytes.prefixed()?)),
            DELETION => put(key, first + index, None),
            _ => return None,
        }
    }
    Some(())
}

/// Reads a sorted table: its index names the data blocks, and each data
/// block holds keys that end in a sequence number and a kind.
fn read_table(table: &[u8], put: &mut impl FnMut(&[u8], u64, Option<&[u8]>)) -> Option<()> {
    let footer = table.get(table.len().checked_sub(FOOTER)?..)?;
    let magic = u64::from_le_bytes(footer.get(FOOTER - 8..)?.try_into().ok()?);
    if magic != TABLE_MAGIC {
        return None;
    }
    let mut handles = Bytes(footer);
    let _filters = (handles.varint()?, handles.varint()?);
    let index = block(table, handles.length()?, handles.length()?)?;
    for_each_entry(&index, |_, handle| {
        let mut handle = Bytes(handle);
        let (Some(offset), Some(size)) = (handle.length(), handle.length()) else {
            return;
        };
        let Some(data) = block(table, offset, size) else {
            return;
        };
        for_each_entry(&data, |key, value| {
            // The last eight bytes: the sequence number over the kind.
            let Some(split) = key.len().checked_sub(8) else {
                return;
            };
            let (user_key, trailer) = key.split_at(split);
            let Ok(trailer) = <[u8; 8]>::try_from(trailer) else {
                return;
            };
            let trailer = u64::from_le_bytes(trailer);
            let value = (trailer & 0xff == 1).then_some(value);
            put(user_key, trailer >> 8, value);
        });
    });
    Some(())
}

/// The block at `offset`, decompressed if it was compressed.
fn block(table: &[u8], offset: usize, size: usize) -> Option<Vec<u8>> {
    const SNAPPY: u8 = 1;
    let end = offset.checked_add(size)?;
    let contents = table.get(offset..end)?;
    let compression = *table.get(end..end.checked_add(BLOCK_TRAILER)?)?.first()?;
    if compression == SNAPPY {
        unsnap(contents)
    } else {
        Some(contents.to_vec())
    }
}

/// Calls `each` with every key and value of a block. A key is stored as
/// what it shares with the one before it and what it does not.
fn for_each_entry(block: &[u8], mut each: impl FnMut(&[u8], &[u8])) -> Option<()> {
    let restarts = Bytes(block.get(block.len().checked_sub(4)?..)?).u32()?;
    let restarts = usize::try_from(restarts).ok()?.checked_mul(4)?;
    let end = block.len().checked_sub(4)?.checked_sub(restarts)?;
    let mut bytes = Bytes(block.get(..end)?);
    let mut key: Vec<u8> = Vec::new();
    while !bytes.0.is_empty() {
        let shared = bytes.length()?;
        let unshared = bytes.length()?;
        let value_length = bytes.length()?;
        key.truncate(shared.min(key.len()));
        key.extend_from_slice(bytes.take(unshared)?);
        each(&key, bytes.take(value_length)?);
    }
    Some(())
}

/// Undoes Snappy: a length, then runs of literal bytes and of copies from
/// what has been written so far.
fn unsnap(compressed: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Bytes(compressed);
    let length = bytes.length()?;
    // A length no real block has is a damaged file, not a reason to
    // reserve memory.
    if length > 64 << 20 {
        return None;
    }
    let mut out: Vec<u8> = Vec::with_capacity(length);
    while !bytes.0.is_empty() {
        let tag = bytes.byte()?;
        let upper = usize::from(tag >> 2);
        let (count, offset) = match tag & 3 {
            0 => {
                let count = if upper < 60 {
                    upper + 1
                } else {
                    let mut wide = [0u8; 4];
                    let width = upper - 59;
                    wide[..width].copy_from_slice(bytes.take(width)?);
                    usize::try_from(u32::from_le_bytes(wide)).ok()? + 1
                };
                out.extend_from_slice(bytes.take(count)?);
                continue;
            }
            1 => (
                (upper & 7) + 4,
                (usize::from(tag >> 5) << 8) | usize::from(bytes.byte()?),
            ),
            2 => {
                let low = bytes.take(2)?;
                (upper + 1, usize::from(u16::from_le_bytes([low[0], low[1]])))
            }
            _ => (upper + 1, usize::try_from(bytes.u32()?).ok()?),
        };
        let from = out.len().checked_sub(offset).filter(|_| offset > 0)?;
        // Byte by byte: a copy may run into what it is itself writing.
        for index in from..from + count {
            out.push(out[index]);
        }
    }
    (out.len() == length).then_some(out)
}

#[cfg(test)]
pub(super) mod tests;
