//! The sound, shared with the MilkDrop process: a second of stereo in a
//! file both have mapped into memory.
//!
//! One side writes and one reads. The layout is a count of all the frames
//! ever written, how many of the newest have yet to reach the ear, and
//! then the frames themselves, round and round. A frame half written when
//! it is read is a wrong dot in a picture for one frame, which is cheaper
//! than a lock the audio thread would wait on.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use memmap2::MmapMut;

/// Stereo frames kept: a second at 48 kHz.
pub const FRAMES: usize = 48_000;
/// The count sits in the first eight bytes and the lag in the next four;
/// the frames start after a whole sixteen, so they stay aligned.
const HEADER: usize = 16;
/// The whole mapping's size: the header, and two floats to a frame.
const SIZE: usize = HEADER + FRAMES * 8;

/// A handle on the shared ring, held by the writer and by the reader.
pub struct Ring {
    map: MmapMut,
    // The mapping keeps working for as long as both sides hold it, even
    // once the file is gone from its folder.
    _file: File,
}

impl Ring {
    /// Makes the file, sizes it, and maps it: the writer's side.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(SIZE as u64)?;
        // SAFETY: the file is this process's own, freshly sized to `SIZE`.
        let map = unsafe { MmapMut::map_mut(&file)? };
        let ring = Self { map, _file: file };
        ring.count().store(0, Ordering::Release);
        ring.lag().store(0, Ordering::Release);
        Ok(ring)
    }

    /// Maps a file the app already made: the reader's side.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        if (file.metadata()?.len() as usize) < SIZE {
            let small = "the MilkDrop audio buffer is too small";
            return Err(io::Error::new(io::ErrorKind::InvalidData, small));
        }
        // SAFETY: the file is at least `SIZE` bytes, sized by the app.
        let map = unsafe { MmapMut::map_mut(&file)? };
        Ok(Self { map, _file: file })
    }

    /// How many frames have ever been written.
    fn count(&self) -> &AtomicU64 {
        // SAFETY: the mapping is at least `HEADER` bytes and its base is
        // page aligned, so its first eight bytes are an aligned `AtomicU64`.
        unsafe { &*self.map.as_ptr().cast::<AtomicU64>() }
    }

    /// How many of the newest frames have yet to reach the ear.
    fn lag(&self) -> &AtomicU32 {
        // SAFETY: as `count`, eight bytes further on, which is aligned for
        // four bytes and within the header.
        unsafe { &*self.map.as_ptr().add(8).cast::<AtomicU32>() }
    }

    /// The ring of frames, as floats: left, right, left, right.
    fn floats(&self) -> *mut f32 {
        // SAFETY: `HEADER` is a multiple of four, so this is aligned for a
        // float, and the mapping holds `FRAMES * 2` of them past it.
        unsafe { self.map.as_ptr().add(HEADER).cast::<f32>().cast_mut() }
    }

    /// Appends interleaved stereo, the oldest making way once it is full.
    /// `ahead` is how many frames lie between the last of it and the ear.
    pub fn push(&self, interleaved: &[f32], ahead: usize) {
        let base = self.floats();
        let mut total = self.count().load(Ordering::Relaxed);
        for frame in interleaved.as_chunks::<2>().0 {
            let slot = (total as usize % FRAMES) * 2;
            // SAFETY: `slot` is within the ring, which holds `FRAMES * 2`
            // floats; the reader puts up with a frame half written.
            unsafe {
                *base.add(slot) = frame[0];
                *base.add(slot + 1) = frame[1];
            }
            total += 1;
        }
        let ahead = ahead.min(FRAMES - 1) as u32;
        self.lag().store(ahead, Ordering::Release);
        self.count().store(total, Ordering::Release);
    }

    /// The frames that have reached the ear since `cursor`, which is moved
    /// past them.
    pub fn since(&self, cursor: &mut u64) -> Vec<[f32; 2]> {
        let total = self.count().load(Ordering::Acquire);
        let lag = u64::from(self.lag().load(Ordering::Acquire));
        let end = total.saturating_sub(lag);
        let oldest = total.saturating_sub(FRAMES as u64);
        let start = (*cursor).max(oldest).min(end);
        let base = self.floats();
        let frames = (start..end)
            .map(|frame| {
                let slot = (frame as usize % FRAMES) * 2;
                // SAFETY: `slot` is within the ring.
                unsafe { [*base.add(slot), *base.add(slot + 1)] }
            })
            .collect();
        *cursor = end.max(*cursor);
        frames
    }
}

// SAFETY: one side writes and one reads; the counts are atomic and a frame
// half written is put up with, so the handle can cross threads: the audio
// thread writes through it.
unsafe impl Send for Ring {}
unsafe impl Sync for Ring {}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("spotified-ring-{name}-{}", std::process::id()))
    }

    #[test]
    fn what_is_written_is_read_once_it_has_been_heard() {
        let path = scratch("heard");
        let writer = Ring::create(&path).expect("a ring");
        let reader = Ring::open(&path).expect("the same ring");
        let mut cursor = 0;
        assert!(reader.since(&mut cursor).is_empty());
        // Four frames, the last two of them still on their way to the ear.
        writer.push(&[0.1, -0.1, 0.2, -0.2, 0.3, -0.3, 0.4, -0.4], 2);
        assert_eq!(reader.since(&mut cursor), [[0.1, -0.1], [0.2, -0.2]]);
        assert!(reader.since(&mut cursor).is_empty());
        // They arrive as more is written behind them.
        writer.push(&[0.5, -0.5], 0);
        let heard = reader.since(&mut cursor);
        assert_eq!(heard, [[0.3, -0.3], [0.4, -0.4], [0.5, -0.5]]);
        drop((writer, reader));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_reader_that_falls_behind_takes_up_at_the_oldest_kept() {
        let path = scratch("behind");
        let ring = Ring::create(&path).expect("a ring");
        let mut cursor = 0;
        let block = vec![0.25f32; 2 * 1000];
        for _ in 0..(FRAMES / 1000 + 5) {
            ring.push(&block, 0);
        }
        assert_eq!(ring.since(&mut cursor).len(), FRAMES);
        assert_eq!(cursor, (FRAMES + 5000) as u64);
        drop(ring);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_file_that_is_not_a_ring_is_refused() {
        let path = scratch("small");
        std::fs::write(&path, b"too small").expect("a file");
        assert!(Ring::open(&path).is_err());
        let _ = std::fs::remove_file(path);
    }
}
