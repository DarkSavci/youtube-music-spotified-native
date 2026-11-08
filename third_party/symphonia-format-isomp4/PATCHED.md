# symphonia-format-isomp4 0.6.1, patched

Symphonia's MP4 demuxer, from crates.io, with one change: a track's edit
list (`elst`) is applied, as trims on its packets in decoded frames. Upstream
parses the edit list and ignores it, so an AAC track plays its encoder's
delay as silence at the start (1600 frames, 36 ms, on YouTube's itag 140
tracks) and the padding of its last frame at the end (up to 1023 frames),
both heard as a gap between gapless tracks.

- The packets before the edit's media time are trimmed from the start, and
  their timestamps shifted so the first presented frame is at 0 (packets in
  the delay get a negative pts). A seek takes and returns presentation
  timestamps; a seek to 0 starts at the media's first sample, so that it
  decodes the same samples as opening the track does.
- What runs past the edit's duration, or past a sample's own duration (an
  AAC frame decodes to 1024 frames whatever the container gives the last
  one), is trimmed from the end. The track's duration is the edit's.
- Without an edit list that skips a delay, an iTunes `iTunSMPB` tag supplies
  the delay and length instead. Edit lists other than one edit at normal
  rate (after any empty edits) are ignored, as before.

symphonia's AAC decoder ignores packet trims, so `crates/audio/src/decode.rs`
applies them for every codec but Opus (whose decoder applies its own).

The change is in `src/edit.rs` (added, with tests) and is marked "Patched"
in `src/demuxer.rs`, `src/atoms/elst.rs`, `src/atoms/mod.rs` and
`src/lib.rs`. Everything else is as published, under the Mozilla Public
License 2.0 (`LICENSE`). The workspace uses it through `[patch.crates-io]`;
drop this copy once upstream applies edit lists.
