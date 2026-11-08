# Sound quality: the plan

From the trace of 2026-11-08 through the resolver, the decoder, the engine and
the device callback, and the first run of the quality suite
(`crates/audio/src/quality`) on synthetic signals and four cached tracks.

The aim: the best stream YouTube Music has, reaching the system exactly as
decoded unless the listener asks for a change, and every change that is asked
for measured clean. YouTube Music has no lossless tier, so "no loss" means none
added after the decoder: Premium's Opus at about 256 kbps is the ceiling.

How each item is known:

- **[measured]** the suite or the report shows it.
- **[code]** certain from reading the code.
- **[try]** the code says so; check it on the device.

Run the suite with `cargo test -p spotified-audio quality`, and the report,
with real tracks, with
`cargo run -p spotified-audio --example quality --release -- <tracks...>`.

## Status, 2026-11-08

Phases 1 and 2 are done, with 3.1 and 4.2: all fourteen checks pass, and the
Rust and Go tests pass. Not yet committed, and not yet heard in the app.
Phase 5.1 turned out not to be a fix (see there).

| Check | Before | Now |
|---|---|---|
| Untouched path is bit-exact | every sample scaled, by up to -108.6 dBFS | identical |
| Full volume is a gain of exactly 1 | settles at 0.9999926 | exactly 1 |
| Peaks near full scale pass untouched | changed by up to -34 dBFS | identical |
| Equalizer off leaves peaks alone | same | identical |
| Resampler, both ways: flat | -3 dB at 20 kHz | -0.002 dB at 20 kHz |
| Resampler, both ways: clean | THD+N -82 dB at 1 kHz, -57 at 19 kHz | -133 at 1 kHz, -116 at 19 kHz |
| Channels, loudness cut, lower volume, equalizer | -142 to -152 dB | the same |
| Peaks over full scale held, not clipped | (new) | peak 0 dBFS, untouched again 470 ms after |
| Quiet detail survives a 16-bit device | (new) | 99% kept with dither, 0% without |

The resampler costs 0.56% of a core in a release build, and about 10% in a
debug build, where this crate is not optimised.

Real tracks, eight cached Opus tracks on a 48 kHz device: the two that never
pass full scale now reach the device identical. The six that do, by up to
+1.58 dBFS, are changed around those peaks by the limiter. On the loudest,
with 0.12% of samples over, overs come often enough that the limiter is
nearly always letting go of one: 82% of samples are turned down a little, by
at most 1.6 dB. The alternative is hard clipping. With normalisation on,
these tracks are turned down several decibels first and nothing is over.
**[measured]**

## Round two, 2026-11-08

| What | Before | Now |
|---|---|---|
| Padding at the end of each Opus track | played: 0.2 to 19.6 ms of encoder padding, heard as a gap between gapless tracks | trimmed, from the file's own `DiscardPadding` |
| End of a resampled (AAC) track | last 2 ms dropped | flushed through, whole to the frame |
| Limiting a tone 1 dB over full scale | THD+N -49.3 dB at 50 Hz, -74.3 at 1 kHz | -148.9 at 50 Hz, -151.6 at 1 kHz |
| Back to untouched after a peak | 470 ms | 275 ms; untouched until 2 ms before it |
| Loudest real track, every effect off | 83% of samples changed | 75% |

- **Gaps.** YouTube's WebM files state how much of the last Opus frame is
  padding (`DiscardPadding`; measured 0.2 to 19.6 ms on ten tracks), and
  symphonia read it and dropped it. `third_party/symphonia-format-mkv` is
  symphonia's demuxer with that one thing fixed, used through
  `[patch.crates-io]`. Decoded tracks now end exactly that much sooner, and
  the near-silent tails (up to 330 frames) are gone. The resampler now
  flushes its last milliseconds when a track ends.
- **Limiter.** The device callback peeks 2 ms ahead in the queue it already
  holds and brings the level down in a straight line to meet a peak, holds
  it 50 ms after the last one, then lets go over 50 ms. It costs, with the
  volume, 0.74% of a core.
- **Then:** AAC tracks kept their encoder delay (symphonia does not apply MP4
  edit lists). Fixed in round three.

## Round three, 2026-11-08

| What | Before | Now |
|---|---|---|
| Start of an AAC track | 1600 frames (36 ms) of encoder delay played | trimmed from the edit list; length equals the stated duration to the frame on four tracks |
| End of an AAC track | 375 to 669 frames of padding played | trimmed |
| Stream under the 170 kbps ceiling | Opus first would take 70 kbps Opus over 128 kbps AAC | Opus wins only at three quarters of the AAC's bitrate or more |
| Crossfade between tracks of one album in a row | faded | back to back |
| Headphone response | uncorrected | AutoEQ's ten filters: THD+N -141 dB, the curve met within 0.000 dB, top kept at 0 dB |
| Windows' mixer and effects | always in the path | bypassed with exclusive output (checked on this machine: 48 kHz, 16-bit dithered) |
| Album loudness | each track by itself | optional: one gain for the album's tracks in a row in the queue, from their loudness averaged as energy |

- **AAC trimming:** `third_party/symphonia-format-isomp4` is symphonia's MP4
  demuxer with the edit list (and, without one, `iTunSMPB`) applied as packet
  trims. Symphonia's AAC decoder ignores trims, so `decode.rs` applies them
  for every codec but Opus, whose decoder does it itself. On YouTube's
  itag 140 the output matches ffmpeg's sample for sample, less the end
  padding ffmpeg keeps.
- **The served format** is shown under Settings, Sound quality: codec,
  bitrate, rate, the device's rate and format, shared or exclusive, and
  what costs quality on the way (a standard tier stream, resampling, a
  device not at 48 kHz, an exclusive request refused).
- **Presets:** Untouched, Best quality, Balanced (what a new profile has),
  Data saver. "Custom" once a setting a preset sets is moved; held to that
  by a test.
- **Android:** see Phase 5; one fault fixed (the system equalizer was on
  with every band flat).
- **Found on the way:** the app runs the core from
  `target/core/spotified-core.exe`, which `cargo build` does not rebuild.
  It dated from 28 October, so no Go change of either round had been in an
  app run until it was rebuilt (`scripts/build-core.ps1`); with it, a
  signed-out track that had played as AAC 130 kbps plays as Opus 129 kbps
  with nothing resampled. The app's `build.rs` now builds the core whenever
  the Go code changes (a warning, not a failure, without Go; skipped when
  cross-compiling or with `SPOTIFIED_SKIP_CORE_BUILD`).
- **A gain answered late** was heard as the level dropping a few seconds
  into a song, most of all an album's, which asks for every track's. The
  first sound now waits for the answer, at most 1.5 s from when it was
  asked; the stream nearly always takes longer to open.
- **AutoEQ** is checked against GitHub by an ignored test (`cargo test -p
  spotified headphones -- --ignored`): the list, the search, and two
  profiles, one with brackets in its name.
- **Decided:** the served format is shown on the Settings page only, not in
  the player bar.
- **Not done:** a blind listening test; exclusive output on macOS and Linux;
  the rate following each track in exclusive mode (it stays at 48 kHz, so
  AAC is still resampled there: following it would reopen the device
  between tracks of different rates and break gapless there).

## Decided: what to do with peaks above full scale

Decoded Opus goes past 1.0 on loud masters. Something has to happen to those
samples:

- **Pass them on.** Bit-perfect. The system or the device clips them hard.
- **Limit above 1.0 only.** Samples under full scale stay bit for bit; the
  few above are brought down smoothly instead of being clipped.
  *Recommended*, for every preset. A preset that does this is "untouched
  below full scale", not "bit-perfect", and should say so.

With normalisation on, loud tracks are turned down by several decibels before
any of this, and the overs disappear on their own.

Done: limit above 1.0, in the device callback's limiter, for every setting.

## Phase 1: stop changing the sound (desktop engine) — done

1. **Snap the volume to its target.** `output.rs`, `Volume::step`: once within
   a step of the target, take the target, as the limiters already do. Full
   volume becomes a multiplication by exactly 1. **[measured]**
2. **Take the equalizer's limiter out of the path when there is nothing to
   limit.** `eq.rs:214` calls `limiter.hold` even when the equalizer is idle,
   with a ceiling of 0.98. Run it only while the equalizer is shaping or the
   gain is raised, or while a crossfade sums two tracks; otherwise, apply the
   decision above (pass on, or limit above 1.0 only). **[measured]**
3. **One limiter, not two, in the untouched path.** The device callback has
   its own for boost. Whatever handles overs should live in one place, so the
   suite can check it in one place. **[code]**

Done when: the first four checks pass, and the report shows real 48 kHz tracks
reaching a 48 kHz device unchanged (or, with the limiter above 1.0, changed
only on the samples that were over).

Done as: the volume glide snaps within 1e-4 of its target; the equalizer
skips its limiter while idle; the callback's limiter, at 1.0, runs on every
frame and is bit-exact while not holding anything.

## Phase 2: a resampler that is transparent — done

AAC tracks (44.1 kHz) always go through it; Opus does too on a device at
44.1 kHz.

1. **Interpolate between the tabulated positions.** Now each output sample
   uses the nearest of 512 phases. The distortion this causes rises 6 dB an
   octave (-82 dB at 1 kHz, -62 at 10, -57 at 19), the mark of position
   rounding. Interpolating linearly between two adjacent phase rows takes it
   below -120 dB. **[measured]**
2. **A longer filter, Kaiser-windowed.** 32 taps under Blackman roll off from
   about 17 kHz and are 3 dB down at 20. Neither codec carries anything above
   20 kHz, so the filter can be flat to 20 kHz and stop by 22.05: a band of
   about 2 kHz, about 160 taps at 110 dB of rejection. **[code]**
3. **Cost.** About 160 taps × 2 channels × 48,000 frames, doubled by the
   interpolation: about 30 million multiply-adds a second, a few percent of a
   core. Measure it against `docs/perf.md` before settling the length.

Done when: the four resampler checks pass (±0.1 dB to 20 kHz, THD+N under
-100 dB at 1, 10 and 19 kHz).

Done as: a Kaiser window sized from the two rates (170 taps up from 44.1 kHz,
184 down from 48, at 120 dB), 1024 positions blended in pairs, one table
shared per pair of rates. Left as it was: the filter holds back its last two
milliseconds of a track, as the old one held back a third of one.

## Phase 3: the best stream

1. **Prefer Opus at the same tier.** The resolvers pick the highest stated
   bitrate (`bestAdaptiveAudio`, `bestYtdlpAudio`). That happens to choose
   Opus 774 over AAC 141 for Premium, but by accident of the numbers. Rank by
   tier, then Opus over AAC, then bitrate: Opus is 48 kHz, so it skips the
   resampler on most devices. **[code]** *Done*, in all three resolvers,
   and DRC copies (YouTube's dynamic-range-compressed versions, served at the
   same bitrate) now lose to the full range.
2. **Show what was served.** The core already reports codec, bitrate and
   whether the format is Premium (`describeQuality`). Show it beside the
   track, and say when the engine is resampling, and from what to what.
3. **Explain Premium.** Signed out, or without Premium, the best is Opus 251
   at about 130 to 160 kbps. The quality settings should say that the best
   tier needs a signed-in Premium account.

Done when: Go tests cover the ranking, and the app shows the served format.

## Phase 4: the device

1. **Warn about the device's rate.** The report already reads it. In the
   playback settings, say when the device runs at anything but 48 kHz, and
   how to change it. **[measured]** (this machine: 48 kHz, float)
2. **Dither when the device takes integers.** The callback converts `f32` to
   `i16` or `i32` by rounding. Add TPDF dither for 16-bit, and a check for it
   in the suite. Shared mode on Windows is nearly always float, so this
   matters most for item 3. **[code]** *Done*: cpal cut towards zero,
   which silenced the last two steps; it now gets values already on a step.
3. **Exclusive output, as an option (later).** It bypasses the system's mixer,
   effects and volume, and lets the device run at each track's own rate, so
   nothing is resampled at all. cpal does not offer it: it needs its own
   backend (the `wasapi` crate on Windows; hog mode and the nominal rate on
   macOS; the allowed rates in PipeWire on Linux). Costs: other apps go
   silent; a change of rate between tracks reopens the device and breaks
   gapless; formats are integer, so it needs item 2. A setting, off by
   default.

## Phase 5: Android

1. **Float output: not a fix.** ExoPlayer's documentation says audio
   processing, speed included, is not available with float output, so it
   would take away playback speed and the visualizer's tap. And the
   platform's Opus decoder hands over 16-bit either way, which is -96 dB of
   noise under a lossy stream. Left off. **[code]**
2. **Speed by the platform.** If speed and float are both wanted, speed can
   be done by the platform (`setEnableAudioOutputPlaybackParameters`) and
   the tap moved; whether that works with float output needs a phone. **[try]**
3. **System effects off unless they change something.** The system
   `Equalizer`, `LoudnessEnhancer` and `BassBoost` (`SoundEffects.kt`) are
   made only when first switched on, and disabled, not released, when
   switched off. The one fault: the equalizer was enabled whenever its switch
   was on, flat bands included, which on many phones still runs every sample
   through the maker's filters. It is now enabled only when a band is off 0
   (`wantedEffects`). **[code, tested]**
4. **Bit-perfect USB (later).** Android 14 can hand a USB DAC the stream
   untouched (`AudioMixerAttributes`, bit-perfect mixer behaviour).

### The path, 2026-11-08

The phone runs the same core, so it gets the same stream as the desktop: the
same quality ceiling (`resolver/quality.go`; "standard" is 170 kbps and
"low" 80 kbps, by default only when the phone's mobile-data setting says so,
`Settings.forCore`), the same ranking, and the core's stream endpoint relays
the bytes untouched. The app hands the core yt-dlp through Chaquopy
(`Core.kt`), so Premium formats are offered as on the desktop.

From there, with every setting off, normalisation included:

| Stage | Where | What happens to the samples |
|---|---|---|
| Decoder | MediaCodec, platform | Opus to 16-bit at 48 kHz; AAC at 44.1 kHz |
| Tap | `Spectrum.kt` `SpectrumTap` | always active, a byte copy: identical |
| Silence skipping | `DefaultAudioProcessorChain` | inactive while off |
| Speed (Sonic) | `DefaultAudioProcessorChain` | inactive at speed 1 |
| Sink | `Engine.kt` `TappedRenderers` | 16-bit PCM, no float, no offload |
| Volume | `Engine.applyVolumes` | `AudioTrack` volume of exactly 1 |
| Mixer | AudioFlinger | AAC resampled to the device's rate; Opus not, on a 48 kHz phone |

Only the decoder's 16 bits and the system's resampling of AAC remain, and
neither is ours to change. With settings on: normalisation is a cut only
(`loudnessGain`, never above 1), applied as the `AudioTrack` volume, which the
mixer does in float; a duck for a notification is 0.2; a crossfade is equal
power across the two decks' volumes. The core's room volume is not applied on
a phone; the system volume is the volume.

`AudioChainTest` builds the processor chain as `Engine` does, configures it
for a 48 kHz stereo track at speed 1, finds the tap the only active
processor, and passes a second of full-range noise through it bit-exactly;
at speed 1.5, or with skipping on, it sees Sonic or the skipper come in.
`SoundEffectsTest` holds that default settings, and a flat equalizer, switch
no effect on. **[tested, JVM]**

Still for a phone: what the maker's own processing does (Dolby, Samsung's
Adapt Sound, the Android 13 spatializer, which the app leaves at its default),
the HAL's rate and depth (`adb shell dumpsys media.audio_flinger` while
playing), and whether merely attaching a disabled effect moves the track off
the fast path. An instrumented test of the effects needs a device or emulator
too.

## Phase 6: the presets

A preset is a set of setting values; the settings show "Custom" once one is
moved. Each preset gets a check in the suite that builds its `Setup` and holds
it to what it claims.

| Setting | Untouched | Best quality | Balanced | Data saver (phone, mobile data) |
|---|---|---|---|---|
| Stream | high | high | high | standard |
| Normalise volume | off | on (cut only) | on | on |
| Equalizer | off | off | as set | as set |
| Crossfade | 0 | 0 | as set | as set |
| Gapless | on | on | on | on |
| Speed, balance | 1, centre | 1, centre | as set | as set |
| Volume boost | off | off | as set | as set |
| Overs above 1.0 | per the decision | limited | limited | limited |
| Claim | untouched below full scale | every change measured clean | | |

"Best quality" may well be the better choice for most people: normalisation
turns loud tracks down in float, adds nothing above -100 dB (measured -150),
and leaves no overs to clip.

## Phase 7: proving it

1. All twelve checks pass, plus the new ones (dither, presets).
2. The report shows every cached Opus track reaching a 48 kHz device
   unchanged (below full scale).
3. **What leaves the system (later).** A capture of WASAPI loopback compared
   with what the engine pushed shows whether Windows changes anything:
   effects, spatial sound, a mix rate other than the device's.

## Order

Phases 1 and 3.1 first: small, and they fix what is heard today on every
track. Then 2, then 5.1 and 5.2, then the presets on top. Phase 4.3 and 5.4
when the rest is done.
