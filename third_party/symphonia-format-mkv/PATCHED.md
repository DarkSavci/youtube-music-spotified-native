# symphonia-format-mkv 0.6.1, patched

Symphonia's Matroska and WebM demuxer, from crates.io, with one change: a
block group's `DiscardPadding` becomes the trim at the end of its last
packet, in decoded frames. Upstream reads the element and drops it, so the
padding of an Opus stream's last frame (0.2 to 19.6 ms on YouTube's tracks)
is played as part of the track and heard as a gap between gapless tracks.

The change is marked "Patched" in `src/demuxer.rs`; `src/lacing.rs` carries
the trim on each frame. Everything else is as published, under the Mozilla
Public License 2.0 (`LICENSE`). The workspace uses it through
`[patch.crates-io]`; drop this copy once upstream applies the padding.
