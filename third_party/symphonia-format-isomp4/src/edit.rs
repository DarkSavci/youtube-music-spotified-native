// Symphonia
// Copyright (c) 2019-2026 The Project Symphonia Developers.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Patched: added. The part of a track's media that is presented.
//!
//! An AAC encoder starts with priming samples (its delay) and pads the last
//! frame; an MP4 says which part of the decoded media to play with an edit
//! list (`elst`), or, in files from iTunes, with an `iTunSMPB` tag. Upstream
//! parses the edit list and ignores it, so the delay is heard as silence at
//! the start of a track and the padding as silence at its end.

use crate::atoms::ElstAtom;

/// The presented part of a track, in the track's (media) timescale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edit {
    /// Media time of the first presented sample.
    pub start: u64,
    /// Length of the presented part, or `None` when it runs to the end.
    pub len: Option<u64>,
}

/// How one sample (packet) of the media is presented under an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trimmed {
    /// Presentation timestamp of the sample's first decoded frame. Negative
    /// for samples that begin in the encoder delay.
    pub pts: i64,
    /// Length of the presented part of the sample.
    pub dur: u64,
    /// Length cut from the start of the sample.
    pub trim_start: u64,
    /// Length cut from the end of the sample.
    pub trim_end: u64,
}

impl Edit {
    /// The edit a track's edit list describes, when it is one this demuxer
    /// can apply: any number of leading empty edits (ignored), then a single
    /// edit at normal rate.
    pub fn from_elst(elst: &ElstAtom, movie_timescale: u32, media_timescale: u32) -> Option<Edit> {
        let mut entries = elst.entries.iter().skip_while(|entry| entry.media_time == -1);
        let entry = entries.next()?;

        if entries.next().is_some() {
            return None;
        }
        if entry.media_time < 0 || entry.media_rate_int != 1 || entry.media_rate_frac != 0 {
            return None;
        }

        // A zero segment duration means the edit runs to the end of the media (common in
        // fragmented files, whose length is not known when the moov atom is written).
        let len = match entry.segment_duration {
            0 => None,
            dur => Some(rescale(dur, movie_timescale, media_timescale)),
        };

        Some(Edit { start: entry.media_time as u64, len })
    }

    /// The edit an iTunes `iTunSMPB` tag describes. Its fields, in hex, are a reserved word,
    /// the encoder delay, the padding, and the original length, all in frames at the sample
    /// rate.
    pub fn from_itunsmpb(value: &str, sample_rate: u32, media_timescale: u32) -> Option<Edit> {
        let mut fields = value.split_whitespace().map(|field| u64::from_str_radix(field, 16));

        let _reserved = fields.next()?.ok()?;
        let delay = fields.next()?.ok()?;
        let _padding = fields.next()?.ok()?;
        let len = fields.next()?.ok()?;

        if delay == 0 && len == 0 {
            return None;
        }

        Some(Edit {
            start: rescale(delay, sample_rate, media_timescale),
            len: if len == 0 { None } else { Some(rescale(len, sample_rate, media_timescale)) },
        })
    }

    /// Whether the edit changes anything.
    pub fn is_identity(&self) -> bool {
        self.start == 0 && self.len.is_none()
    }

    /// The edit that presents all of the media.
    pub const IDENTITY: Edit = Edit { start: 0, len: None };

    /// How a sample at media timestamp `ts`, lasting `dur` by the container and decoding to
    /// `decoded` (both in the media timescale), is presented. A sample that decodes to more
    /// than it lasts (the last AAC frame of a track, which an encoder pads to a whole frame) is
    /// cut to the length the container gives it.
    pub fn trim(&self, ts: u64, dur: u64, decoded: u64) -> Trimmed {
        let end = ts.saturating_add(dur);
        let decoded_end = ts.saturating_add(decoded.max(dur));
        let edit_end = self.len.map_or(u64::MAX, |len| self.start.saturating_add(len));

        // The presented part of the sample: [first, last).
        let first = ts.max(self.start).min(end);
        let last = end.min(edit_end).max(first);

        Trimmed {
            pts: (ts as i64).saturating_sub(self.start as i64),
            dur: last - first,
            trim_start: first - ts,
            trim_end: decoded_end - last,
        }
    }

    /// The media timestamp of presentation timestamp `ts`.
    pub fn media_ts(&self, ts: u64) -> u64 {
        // The start of the presentation is the start of the media, so that a seek to the
        // start decodes the same samples as opening the track does.
        if ts == 0 { 0 } else { ts.saturating_add(self.start) }
    }
}

/// Converts `value` from one timescale to another.
pub fn rescale(value: u64, from: u32, to: u32) -> u64 {
    if from == to || from == 0 {
        return value;
    }
    (u128::from(value) * u128::from(to) / u128::from(from)).min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atoms::{ElstAtom, ElstEntry};

    fn elst(entries: &[(u64, i64)]) -> ElstAtom {
        ElstAtom {
            entries: entries
                .iter()
                .map(|&(segment_duration, media_time)| ElstEntry {
                    segment_duration,
                    media_time,
                    media_rate_int: 1,
                    media_rate_frac: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn edit_list_of_a_youtube_track() {
        // From a YouTube itag 140 file: 3380 AAC frames of 1024, delay 1600.
        let edit = Edit::from_elst(&elst(&[(3_458_851, 1600)]), 44_100, 44_100).unwrap();
        assert_eq!(edit, Edit { start: 1600, len: Some(3_458_851) });

        let first = edit.trim(0, 1024, 1024);
        assert_eq!(first, Trimmed { pts: -1600, dur: 0, trim_start: 1024, trim_end: 0 });

        let second = edit.trim(1024, 1024, 1024);
        assert_eq!(second, Trimmed { pts: -576, dur: 448, trim_start: 576, trim_end: 0 });

        // The file gives its last sample a duration of 355, the rest of its 1024 frames being
        // the encoder's padding. Cut either way, by the edit or by the sample's duration.
        for dur in [1024, 355] {
            let last = edit.trim(3379 * 1024, dur, 1024);
            assert_eq!(last, Trimmed { pts: 3_458_496, dur: 355, trim_start: 0, trim_end: 669 });
        }

        // Every frame presented exactly once, and as many as the edit says.
        let total: u64 = (0..3380).map(|i| edit.trim(i * 1024, 1024, 1024).dur).sum();
        assert_eq!(total, 3_458_851);
    }

    #[test]
    fn segment_duration_is_in_the_movie_timescale() {
        let edit = Edit::from_elst(&elst(&[(1000, 2112)]), 1000, 48_000).unwrap();
        assert_eq!(edit, Edit { start: 2112, len: Some(48_000) });
    }

    #[test]
    fn leading_empty_edits_are_skipped_and_zero_duration_runs_to_the_end() {
        let edit = Edit::from_elst(&elst(&[(500, -1), (0, 1024)]), 1000, 44_100).unwrap();
        assert_eq!(edit, Edit { start: 1024, len: None });
        assert_eq!(edit.trim(5 * 1024, 1024, 1024).trim_end, 0);
    }

    #[test]
    fn unsupported_edit_lists_are_ignored() {
        assert_eq!(Edit::from_elst(&elst(&[]), 1000, 1000), None);
        assert_eq!(Edit::from_elst(&elst(&[(10, 0), (10, 20)]), 1000, 1000), None);

        let mut slow = elst(&[(10, 0)]);
        slow.entries[0].media_rate_int = 2;
        assert_eq!(Edit::from_elst(&slow, 1000, 1000), None);
    }

    #[test]
    fn a_short_last_sample_is_cut_without_an_edit() {
        let last = Edit::IDENTITY.trim(10 * 1024, 300, 1024);
        assert_eq!(last, Trimmed { pts: 10 * 1024, dur: 300, trim_start: 0, trim_end: 724 });
        let whole = Edit::IDENTITY.trim(10 * 1024, 1024, 1024);
        assert_eq!(whole, Trimmed { pts: 10 * 1024, dur: 1024, trim_start: 0, trim_end: 0 });
    }

    #[test]
    fn itunsmpb() {
        let tag = " 00000000 00000840 000001CA 00000000003F31F6 00000000 00000000 00000000 \
                   00000000 00000000 00000000 00000000 00000000";
        let edit = Edit::from_itunsmpb(tag, 44_100, 44_100).unwrap();
        assert_eq!(edit, Edit { start: 0x840, len: Some(0x3F31F6) });
        assert_eq!(Edit::from_itunsmpb("garbage", 44_100, 44_100), None);
        assert_eq!(Edit::from_itunsmpb(" 0 0 0 0", 44_100, 44_100), None);
    }

    #[test]
    fn seeking_to_the_start_starts_at_the_first_sample() {
        let edit = Edit { start: 1600, len: None };
        assert_eq!(edit.media_ts(0), 0);
        assert_eq!(edit.media_ts(44_100), 45_700);
    }
}
