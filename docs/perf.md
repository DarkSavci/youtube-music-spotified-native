# Measurements

Budgets are in `PLAN.md`, section 2. Each milestone adds a row.

Machine: Windows 11, AMD graphics, release build with the GNU toolchain.

## M0 (shell), 2026-10-03

| Measure | Result | Budget |
| --- | --- | --- |
| Binary (`spotified.exe`) | 9.1 MB | under 25 MB |
| First frame, warm, three runs | 216, 182, 178 ms | under 300 ms |
| Core ready after launch | about 1.4 s after the first frame | not on the UI thread |

## M1 in progress (browsing), 2026-10-03

Release build, `--demo`, Home page with about sixty covers loaded.

| Measure | Result | Budget |
| --- | --- | --- |
| Binary | 10.4 MB | under 25 MB |
| First frame, warm, two runs | 192, 181 ms | under 300 ms |
| CPU over 10 s, idle on Home | 0 ms | no repaints without input |
| Working set | 92 MB | |
| Private memory | 168 MB | under 150 MB: **over** |
| Core working set | 21 MB | |
| Core stops when the app closes | yes | |

Private memory looked over the first budget (150 MB), so it was traced. A
blank egui window with one label, built the same way, is 167 MB private
and 96 MB working set on this machine: the cost is the graphics driver and
the toolkit, not the app. The app adds about 2 MB to it on Home. The budget
is now stated as what the app adds.

## M2 (playback), 2026-10-03

Release build, a cached Opus track playing, window on Home.

| Measure | Result | Budget |
| --- | --- | --- |
| Binary | 11.7 MB | under 25 MB |
| Time to sound, cached track | about 50 ms to loaded, first audio within 250 ms | under 300 ms |
| CPU while playing, 30 s | 2.0% of one core | under 2%: at the line |
| CPU of the core while playing | none measurable | |
| Memory against a blank window | +2 MB private | under +60 MB |
| Closing the app | 0.13 s | |

CPU while playing was 3.0% at first. The engine polled its decks every
10 ms and the window redrew on a timer as well as on each position report;
polling every 20 ms and leaving redraws to the reports brought it to 2.0%.
What remains is spread evenly over the UI thread, the engine, and two
driver threads. The next saving is in the core: it resends the whole queue
with every position report, which the app parses four times a second.

Gapless hand-over, checked with two cached tracks: the first reports its
end at its last sample and the second is already playing; when the core
names it two seconds later it is two seconds in, not restarted.

## Everything in, 2026-10-03

Release build, a cached Opus track playing, window on Home, on a 180 Hz
monitor.

| Measure | Result |
| --- | --- |
| Binary | 12.5 MB |
| CPU while playing, 15 s | 2.0% of one core |
| CPU while playing, visualizer on | 9% of one core |
| CPU while playing, as a mini player | 1.6% of one core |
| Private memory | 168 MB, a blank window's 167 plus one |

The visualizer first asked for a redraw every frame, which on this monitor
is 180 a second: 23 to 25% of a core, on any page. A frame costs about
half a millisecond in the app's own code and about two in painting and
presenting it, so the saving had to come from drawing fewer. It now asks
for thirty a second, which is where the 9% comes from, and it is off
unless switched on in Settings. Working out the transform's window and
sines once instead of every frame made no measurable difference; it stays
because it is the plainer code.

## 0.2.0, 2026-10-04

Release build, a cached track playing, the queue panel open.

| Measure | Result |
| --- | --- |
| Binary | 12.5 MB |
| CPU while playing, queue open | about 3% of one core |
| Private memory | 168 MB |

The queue's playing row has an animated equalizer, which redraws the
window eight times a second while music plays and the panel is open; that
is the difference from the 2% measured with the panel shut. Hovers fade
over an eighth of a second and ask for frames only while they do.

## Decoder spike

One real track of each kind, decoded to WAV by
`cargo run -p spotified-audio --release --example decode`.

| Stream | Decoder | Audio | Decode time | Speed |
| --- | --- | --- | --- | --- |
| Opus in WebM, 48 kHz | `opus-decoder` (pure Rust) | 208 s | 53.5 s | 4× real time |
| Opus in WebM, 48 kHz | libopus (`symphonia-adapter-libopus`) | 208 s | 0.55 s | 380× |
| AAC-LC in MP4, 44.1 kHz | symphonia | 268 s | 0.28 s | 950× |

The two Opus decoders agree to within 2 parts in 32768 per sample. A seek to
60 s lands exactly on the samples a full decode gives at that point, for
both formats (compared sample by sample). libopus is the decoder used.
