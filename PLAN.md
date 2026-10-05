# Youtube Music Spotified, native

A native rewrite of Youtube Music Spotified in the manner of
[Spotifast](https://github.com/crmne/spotifast): Rust and egui, no browser
engine, a native audio pipeline, Spotifast's look with a green accent.

This project is separate from `../youtube-music-spotified` (the Electron app).
Nothing here writes to that repository, its data directory, or its port.

## 1. Goals and non-goals

Goals, in priority order:

1. **Native and fast.** A window on screen before the core is ready. No
   repaint while nothing changes. The UI thread never waits on network, disk
   or the core.
2. **Spotifast's look and behaviour.** Flat panels, its palette, sizes and
   type scale; single click selects, double click plays; multi-select; drag
   and drop; instant hover; optimistic controls that never flicker back.
3. **Feature parity with the Electron app** for everything a music client
   needs, reached milestone by milestone (section 8).
4. **Clean code.** Views draw and emit actions; one pure function applies
   them. `cargo fmt`, `cargo clippy -D warnings` and tests pass at every
   milestone. No file grows past about 600 lines without being split.

Non-goals for the first pass:

- Porting the Go core to Rust. It stays a sidecar process (section 3).
- Spotifast's Winamp skins and MilkDrop.
- Music video playback, and with it the song/video "versions" switch. The
  audio of a video track still plays.
- Playback speed. The Electron app leans on the browser's pitch-preserving
  stretch; a native one is its own piece of DSP. Revisit after parity.
- Transfer between devices. One window owns playback.
- macOS and Linux. Platform code lives under `platform/` behind `cfg`, so
  they can follow; Windows is what gets built and tested.
- i18n and a light theme. Colours come from one palette struct, so a light
  theme is a second palette later.

## 2. Budgets

| Measure | Budget | How it is held |
| --- | --- | --- |
| First frame, warm start | under 300 ms | the window opens before the core is spawned; settings are read synchronously; nothing else runs on the UI thread |
| Repaints while idle and paused | none without input or events | reactive repaint; output stream paused |
| CPU while playing | under 2% of a core | repaint every 250 ms; 60 fps only with the visualizer on |
| Time to sound, cached track | under 300 ms | the core serves the cache file; the decoder starts on the first bytes |
| Seek, cached track | under 150 ms | range read plus decoder reset |
| Memory the app adds to a blank window | under 60 MB | 64 MB art budget with LRU eviction; page caches capped; virtualised rows. A blank egui window on this machine's driver is already 167 MB private, so the total is not ours to budget |
| Binary | under 25 MB (plus the Go core) | glow, not wgpu; `default-features = false`; thin LTO, one codegen unit, strip |

`scripts/measure.ps1` records these (first frame from the process start time
to the first `ui` call, logged by the app; CPU from
`Get-Counter '\Process(*)\% Processor Time'` over 60 s paused and playing;
private working set), cold and warm, into `docs/perf.md` at each milestone.

## 3. Architecture

Two processes:

```
spotified.exe  (Rust: UI, audio engine, OS integration)
  └─ spotified-core.exe  (Go: YouTube Music, session, cache, stream proxy)
       └─ yt-dlp + deno  (per stream resolve)
```

### 3.1 Toolchain

This machine has no MSVC build tools. The Rust toolchain is
`stable-x86_64-pc-windows-gnu` with WinLibs MinGW (gcc, dlltool, windres) on
`PATH`. `scripts/env.ps1` sets that path. TLS uses Windows' own stack
(`native-tls`), which avoids a C crypto library.

### 3.2 The Go core, copied

`core/` is a copy of the Electron repo's `cmd/`, `internal/`, `go.mod`,
`go.sum` and `testdata/`. It owns the queue, session state machine, shuffle,
repeat, autoplay, resume, response cache, audio cache, stream resolving and
every YouTube call, and it is well tested. The Electron renderer was only
ever an engine reconciling toward the core's `Target`
(`core/internal/session/types.go`). The Rust app takes that role.

Launch: `spotified-core.exe -addr 127.0.0.1:0 -credentials <file> -db <file>
-account-scope <scope> -ytdlp <exe> -deno <exe> -exit-with-stdin`.

Changes to the copy, each small and listed here as it lands:

1. **Bind any free port and say which.** With `-addr 127.0.0.1:0` the
   "spotifier listening" log line already carries the bound address; the app
   reads it from the core's stderr. This removes port races and can never
   collide with the Electron app's 8674.
2. **`-exit-with-stdin`.** The core shuts down gracefully (final resume
   save) when its stdin closes. The app holds the pipe, so closing it is the
   clean stop, and a crashed app cannot leave an orphan. On Windows a kill
   never runs the graceful path, which is why this is not left to signals.
3. **Remove `removeLegacyCookieDirs`.** It deletes `%TEMP%\spotifier-cookies-*`,
   which belongs to the Electron app.
4. **Requests end with the run.** The server's base context is the run's,
   so the session's open event stream no longer holds a shutdown for its
   whole five-second timeout. Closing the app went from five seconds to a
   tenth of one.
5. (M1) A route for recent local plays if `/v1/me/history` and the stats
   routes do not already serve Home's "Recently played".
6. (later) playlist rename and reorder, save album or playlist to library.

The HTTP wire format is otherwise unchanged, including its quirk that
`Command`, `EngineEvent` and `Target` travel with Go field names (`Kind`,
`Epoch`, `VideoID`) while `Projection` is camelCase. `client` mirrors that
with serde renames and is pinned by a golden-JSON test: the Go tests write
sample projections, the Rust tests read them.

yt-dlp and deno: `scripts/fetch-tools.ps1` downloads the pinned releases
into `vendor/` and checks their published checksums, as the Electron build
does. Without deno, playback fails, so the app reports a missing tool at
startup instead of at the first play.

### 3.3 Isolation from the installed Electron app

- the core binds a free port; nothing listens on 8674;
- data in `%APPDATA%\SpotifiedNative` (settings, credentials, database) and
  `%LOCALAPPDATA%\SpotifiedNative` (caches, logs);
- its own single-instance lock, AppUserModelID
  (`dev.darksavci.spotified.native`) and core binary name;
- the yt-dlp cookie file and audio cache live beside this app's credentials.

Two cores signed in to one account each run their own rate governor; using
both apps at once doubles the request rate YouTube sees. That is a risk the
user controls by not running both.

### 3.4 The Rust workspace

```
crates/
  client/   HTTP + SSE client for the core, serde models. No UI, no audio.
  audio/    Decode, DSP, mixing, output. No UI.
  app/      The binary: state, actions, views, theme, images, platform.
```

Inside `app`, by domain rather than by layer-in-one-file:

```
app/src/
  main.rs            entry: args, logging, single instance, window
  state/             one module per domain (nav, library, pages, playback,
                     selection, search, toasts, settings)
  actions/           the Action enum and apply(), one module per domain
  backend/           Command/Event, worker pools, the session thread
  views/             one module per page or region; shared widgets/
  theme.rs  images.rs  paths.rs  sidecar.rs
  platform/windows/  tray, media keys, thumbnail buttons
```

Two pure functions carry the logic and are unit-tested without a window or
a sound device:

- `actions::apply(&mut State, Action) -> Vec<Effect>`: all state changes.
  Effects are backend commands, engine commands and repaint requests.
- `audio::reconcile(&EngineState, &Target) -> Vec<DeckOp>`: what the decks
  must do to reach the core's target.

Views receive `&State` and a `&mut Vec<Action>`; they cannot mutate state.

### 3.5 Threads and message flow

- **UI thread**: `logic()` drains events and applies queued actions; `ui()`
  draws.
- **API pool** (4 std threads) and **image pool** (4), separate so artwork
  cannot starve browsing. Blocking `ureq`; no async runtime.
- **Session thread**: holds the SSE stream (no overall timeout, read timeout
  above the core's 25 s keep-alive), re-registers on every reconnect because
  a dropped stream unregisters the device.
- **Engine thread**: owns decoders and the mixer; fills a lock-free ring;
  sends engine events to the core in order from one place.
- **Stream readers**: one thread per deck, never the API pool.
- **Audio callback** (cpal): drains the ring. Never locks, never allocates.

UI to backend: `crossbeam_channel::Sender<Command>`. Backend to UI:
`Receiver<Event>` drained with `try_iter`, then `request_repaint`. Every
response carries the generation or serial it was asked with. Requests in
flight are not aborted; stale answers are dropped on arrival.

### 3.6 Optimistic state

The core is local and authoritative and answers a command with the new
projection. So an optimistic overlay (play state, intended track, position,
volume, like) is set when the action is applied and cleared by the projection
that answers that command, not by a timer. A timer (2.5 s) exists only as a
backstop for a command that never answers. Likes go to YouTube, so they keep
Spotifast's write guard: stale reads are ignored until the write confirms,
and a failure flips back with a toast.

## 4. The engine contract

What the Rust side must implement, from `core/internal/session/types.go`,
`core/internal/api/session.go` and the Electron `engine.ts`,
`sessionclient.ts` and `playback.ts`.

Session client:

- `POST /v1/session/register {deviceId, name, capabilities}` returns the
  first projection. The device id is generated once and persisted.
- `GET /v1/session/events?deviceId=` (SSE, `projection` events, each a full
  snapshot). A monotonic `version` gate drops stale ones; the first snapshot
  after a reconnect is accepted regardless, since a restarted core restarts
  its versions.
- `POST /v1/session/command {deviceId, command}`. A rejection is a 200 with
  `rejected`.
- `POST /v1/session/engine-event`, `POST /v1/session/capabilities`.
- `POST /v1/session/settings`. `crossfadeMs` and `gapless` are not optional
  on the wire: every post sends both.
- `POST /v1/session/radio`, `GET /v1/network`, `POST /v1/me/plays` is not
  needed (the core writes the play log from position reports).

Engine:

- Reconcile toward `Target{Epoch, VideoID, StartAtMs, Playing,
  PreloadVideoID, Volume, Transition, UserChange}`. `UserChange` means cut
  now; otherwise apply `Transition` (gapless or crossfade) at the natural
  end. Stale epochs are ignored.
- Report `loaded`, `position`, `ended`, `failed`, `stalled`, `blocked` with
  `PositionMs`, `DurationMs`, `Reason`. Position reports are load-bearing:
  the core builds the play log and its "ended too early" guard from them.
  At most one `failed` per epoch. Reasons are the strings the core
  recognises (`stalled`, `network`, empty).
- Before failing a track, ask `/v1/tracks/{id}/health`; a rate limit becomes
  `blocked`, not `failed`.
- Loudness from `/v1/tracks/{id}/loudness` drives the per-deck gain; when it
  is absent (signed out), gain stays at unity. A measured leveller is a
  later refinement.
- Idle-deck reads carry `?preload=1`. A failed preload retries with backoff
  and never fails the current track.
- A stall watchdog: no decoded audio for 8 s while playing reports
  `stalled` and retries the read.
- There is no embedded-player fallback. A stream the core cannot serve as a
  URL (409) is reported `failed`; the core marks it unplayable and skips.

Stream reader (`/v1/stream/{id}`), as the core actually behaves
(`core/internal/api/stream.go`, `cache.go`):

- A cache hit, or a download in progress, is served from the growing cache
  file as 206. A seek far past what is downloaded falls through to an
  upstream relay capped at 1 MiB per response. So the reader loops on short
  206 responses, takes the total size from `Content-Range`, and treats an
  unknown size as "read until the server closes".
- A request can wait up to 60 s for a cold resolve. Status 429 (with
  `Retry-After`) maps to `blocked`, 503 to `stalled` (offline), 502 and 409
  to `failed`.
- A read-ahead buffer of a few seconds keeps the decoder off the network.

Pipeline:

```
reader → demux + decode (symphonia) → resample to device rate (rubato,
skipped when rates match) → per-deck gain → crossfade mixer (two decks,
equal power) → 10-band EQ → visualizer tap → limiter → volume → cpal
```

- **Formats**: the core picks the highest bitrate, which is Opus in WebM
  (itag 251, or 774 with Premium) for about two tracks in three and AAC-LC
  in MP4 for the rest. symphonia demuxes both containers and decodes AAC;
  Opus is decoded by libopus through `symphonia-adapter-libopus`, built into
  the binary. The M0 spike settled this: the pure-Rust `opus-decoder` is
  correct but decodes at only four times real time, where libopus manages
  about 380 times (`docs/perf.md`).
- **Gapless and crossfade**: the idle deck preloads `PreloadVideoID`. At a
  natural end the mixer butts the decks or ramps them. A skip uses a 10 ms
  fade, play and pause 250 ms. Silence-edge trimming (the Electron app's
  `silence.ts`) arrives with crossfade in M4.
- **Device**: cpal (WASAPI) following the default device, reopened on
  failure, paused when playback is paused.

## 5. Rendering and look

- eframe/egui **0.36** from crates.io with `glow`. Spotifast uses the same
  version through its own forks and about fourteen `fastframe` crates, so
  its view code is a reference for layout and numbers, not code to compile.
  Tray, media keys and single instance use `tray-icon` and `souvlaki`
  directly.
- Fonts: Inter Regular, Medium, SemiBold, Bold, bundled (OFL). Icons: Lucide
  SVGs (ISC), rasterised once per size.
- Repaint: 250 ms while playing, on events otherwise, 60 fps only with the
  visualizer on or an animation running.
- Images: disk cache keyed by URL hash; a 64 MB memory budget counting
  encoded, decoded and texture bytes; LRU eviction every 20 s; decode on the
  image pool.
- `--screenshot <file>` draws a few frames, saves the window with
  `ViewportCommand::Screenshot`, and exits. With `--demo` the core runs on
  its fixture catalogue in a temporary profile, so screenshots are
  deterministic and contain no account data.

Spotifast's dark palette, from its `src/theme.rs`:

| Token | Value |
| --- | --- |
| window | `#0f1114` |
| panel | `#15181c` |
| surface / hover / active | `#1d2127` / `#262b33` / `#2f353f` |
| outline | `#2a3038` |
| text / secondary / dim | `#f2f4f6` / `#a9b1bc` / `#6e7784` |
| accent / hover / on accent | `#1ed760` / `#3ce87a` / `#0a140e` |
| danger / warning | `#f5717f` / `#f2b85c` |
| overlay | `#22272e` |

Geometry: flat panels with no gaps; top bar 56; sidebar 250 (210 to 600,
resizable, Ctrl+B hides); right panel 360 (280 to 560); player bar 88 with a
1 px top line; rows 56 / 48 / 36; cards 172 wide with a 148 image; radii 8,
6 and 4. Loading is a spinner and "Loading…", never a skeleton. Hover is
instant. Only links show the hand cursor. OS title bar and caption buttons.

## 6. Sign-in

The core authenticates with the cookie header in `credentials.json`.

1. **Import** (M1): if the Electron app is signed in, offer to copy its
   active account's `credentials.json` (it lives in the account directory,
   not the data root) into this app's data directory. Read only, validated
   as JSON with the required cookies, retried once if caught mid-write.
   Google rotates cookies, so the copy can go stale; Settings keeps a
   "Re-import" action. Until M6 this is the only way to sign in, so the app
   is for existing users of the Electron app until then; signed out, it
   still browses and plays at standard quality.
2. **Browser sign-in** (M6): the Electron flow, ported. Chrome or Edge on a
   throwaway profile at the Google sign-in page; when YouTube Music is
   reached, the profile is reopened headless and its cookies read over the
   DevTools protocol; `credentials.json` written; profile deleted.

## 7. Conventions

- Views take `&State` and push `Action`s. Blocking work lives in the backend
  or the engine.
- No `unwrap` or `expect` outside tests and startup invariants. Errors are
  typed in the libraries and become a sentence for the user in the app.
- Comments say why, in the Electron repo's style. British English in UI
  text, matching it.
- Each dependency is justified by a comment in `Cargo.toml`.
- Settings and session files are versioned JSON, written by temp file and
  rename, read tolerantly (unknown fields ignored, a bad file falls back to
  defaults and is kept aside, never overwritten blindly).
- Code adapted from Spotifast (MIT) is credited in `NOTICE.md`.

## 8. Milestones

Each ends with: fmt, clippy with warnings denied, Rust and Go tests green,
the app run in `--demo`, a screenshot compared with Spotifast, and the
budgets measured.

**M0. Shell and spikes.**
- Workspace, toolchain script, copied core building with changes 1 to 3.
- Log file per launch and a panic hook (release builds abort on panic);
  the core's stderr captured into the same log.
- Single instance (two instances would share a database and cache).
- Sidecar: spawn after the first frame, read the bound address, readiness
  and failure states shown in the window, clean stop.
- Window with theme, fonts and icons; the four regions drawn (sidebar, top
  bar, page, player bar) at Spotifast's geometry with placeholder content.
- `--demo` and `--screenshot`.
- **Spike**: decode and seek a real Opus/WebM and an AAC/MP4 stream through
  symphonia to a WAV file. This decides the Opus decoder before M2.

**M1. Browse.** `client` crate (models, typed errors, golden-JSON test).
Image loader. Home, library sidebar (chips, sort, search, list and grid),
album, playlist, artist, liked songs, search with suggestions, history, top
result and filters. Navigation history. Virtualised track table with
windowed loading. Credentials import. Offline and rate-limit notices.

**M2. Play.** Session client. Audio engine per section 4. A fixture
resolver in the core so `--demo` has audio. Player bar, queue panel, play
from row and context, shuffle, repeat, seek, volume, gapless, resume.
Media keys and the Windows now-playing surface.

**M3. Interact.** Context menus, selection and keyboard navigation, drag
and drop (to playlists, liked, queue, reorder), like, follow, playlist
create, delete, add and remove, folders and pins, toasts, the shortcut set.

**M4. Listen.** Lyrics panel and fullscreen lyrics, settings page (with
cache size and clear), equalizer, crossfade with silence trimming,
normalisation, player-bar visualizer, album-art tint.

**M5. Desktop.** Tray and close to tray, CLI verbs through the single
instance channel, taskbar thumbnail buttons, mini player, launch at login,
diagnostics bundle, yt-dlp refresh, updater, Inno Setup installer, CI.

**M6. Parity.** Browser sign-in, accounts and channels, stats, mixes,
artist all-songs, moods and browse pages, podcasts, Listen Together,
what's new.

## 9. Status

Updated 2026-10-04.

- **M0: done.** Shell, sidecar, logging, single instance, `--demo`,
  `--profile`, `--screenshot`, `--open`, decoder spike.
- **M1: done.** `client` crate, backend workers, page caches, artwork
  loader with budget, Home, album, playlist and artist pages, search
  (debounced, stale answers dropped, filter chips, suggestions for the
  query, recent searches and a wall of mood tiles before anything is
  typed), library list with kind chips, its own search and sort,
  virtualised track rows, sign-in import, an offline badge. Left out: a
  grid view of the library, and windowed loading (a playlist is fetched
  whole; 282 songs were fine).
- **M2: done.** Session client, audio engine (ranged HTTP source, a
  decode thread per deck, resampler, lock-free output, gapless hand-over,
  seek, pause with fade, stall and failure reports), loudness
  normalisation (turns loud tracks down; never up), live player bar, play
  from a row or a card, queue panel, shortcuts, media keys and the
  system's now-playing card, and following a change of sound device (the
  engine reopens the output and its decks where the music had got to).
  Left out: a fixture resolver, so `--demo` has audio only from a seeded
  cache.
- **M3: done.** Likes with a pending-write guard, add to queue and play
  next, add to playlist, playlist create, delete and remove, follow,
  right-click menus on songs, selections and cards, a play button on
  cards, copy link, toasts, row selection (click, Ctrl, Shift, arrows,
  Ctrl+A, Escape), drag and drop of a song or a selection onto a sidebar
  playlist, Liked Music or the queue, artist and album links in rows and
  the player bar, the big play button under a header, dialogs, accessible
  names on every hand-drawn control, pins (kept first in either order)
  and folders in the library.
- **M4: done.** Lyrics panel and fullscreen lyrics (timed lines follow the
  song, click to seek), settings page (account, playback, equalizer,
  appearance, storage, window, shortcuts, troubleshooting, about),
  normalisation, crossfade (equal-power, started the fade's length before
  the end), a ten-band equalizer with presets and a limiter after it,
  album-art tint behind page headers and in the player bar, a spectrum
  visualizer behind the player bar (off by default), the size of the song
  cache and a button to empty it, and silence trimming for crossfade (the
  end of a track's music is read ahead through a second connection, and
  the quiet at the start of the next is passed over).
- **M5: done.** Tray icon with a menu, close to tray,
  a second launch shows the running window, app icon on the window and
  the executable, start with Windows (a `Run` entry with `--hidden`; the
  switch has not been flipped on this machine, only its read side has
  run), a mini player (Ctrl+M: the window made small and kept on top),
  a button that opens the logs folder, a button that runs yt-dlp's own
  updater, `scripts/package.ps1` (a portable folder and zip), an Inno
  Setup installer (`packaging/windows/spotified.iss`; compiles, not run),
  a CI workflow (not run). An updater (`crates/app/src/update.rs`): it asks GitHub for the
  latest release of `DarkSavci/youtube-music-spotified-native`, downloads
  the installer, checks it against the release's `SHA256SUMS.txt`, and
  runs it on "Restart to update". That repository does not exist yet, so
  until a release is published there the check finds nothing; a release
  is the three files `scripts/package.ps1` leaves in `dist/` under a tag
  `vX.Y.Z`. A What's new page read from the bundled `CHANGELOG.md`. The
  app's own title bar on Windows (the top bar is the title bar, with the
  window's buttons at its right and its edges resizing it), which a
  switch in Settings gives back to the system. Left out: taskbar
  thumbnail buttons, which the system's now-playing card already covers.
- **M6: done.** Browser sign-in (Chrome or Edge on
  a throwaway profile, cookies read back over DevTools) and sign-out, the
  account's name and a choice between its channels, the listening stats
  page, mixes on Home with a page each, Explore, moods and genres and any
  other surface of shelves with "Show all" behind a shelf, recently
  played, song radio, an artist's shuffle, radio, full discography and
  all-songs list, podcasts (cards, a show's page, episodes that play as
  songs do). Listen Together (`crates/app/src/together/`): the relay's
  version 2 over a WebSocket, with rooms made or joined by PIN, the
  player's own buttons sent to the room as its commands, and the core
  following the room. Left out of it: the room's own search, requests to
  approve, votes, ready checks and the leader's settings; a guest can add
  songs and a leader steers, but the finer controls are the Electron
  app's only.

The look, changed on 2026-10-04 after comparing with the Electron app:
panels are rounded cards with an 8-point gutter and the window's darker
colour between them; the queue has Played, Now playing and Next sections,
a click to jump, a remove button under the pointer, a right-click menu and
drag to reorder; lyrics sit on the cover's colour with every line bold and
the sung line white; hovers fade in over an eighth of a second with a
round wash on icon buttons.

Added on 2026-10-04, later the same day:

- The mini player became a window of its own (a second egui viewport),
  frameless and kept on top, laid out by its size as the Electron app's
  is: bar, square, wide, tall. It remembers its size, place and whether
  it stays on top.
- Themes (`crates/app/src/themes.rs`): follow the system, light, dark, or
  a JSON file in `%APPDATA%\SpotifiedNative	hemes`, which is given six
  presets on first run. The mini player wears the same palette. Spotifast's
  own mini-player theming is Winamp `.wsz` skins; that was not built then,
  and was on 2026-10-05 (below).
- Opus streams state their length in the container, not the track, and
  were read as having none: no length shown, no seeking, no crossfade.
  The decoder now falls back to the container's.
- A narrow window gives the page at least 420 points: the sidebar steps
  aside when a right panel is open and there is not room for all three.
- Browse all (the button in the search field), a fuller account menu, a
  reworked Listen Together page, a proper search field in the library.

How it has been checked:

- 261 Rust tests, among them headless UI tests (`crates/app/src/views/
  tests.rs` and the modules under it) that find controls by name, click
  and drag them; clippy with warnings denied; the Go suite.
- Every new page was drawn from the recorded fixtures and looked at:
  Explore, the mood tiles, search with suggestions, the artist page with
  its mixes, the mini player, the visualizer over a playing track.
- The folder, cache and account routes were called against a demo core and
  their answers compared with what the client reads.
- Listen Together was run against the Electron repository's own relay on
  this computer: one copy made a room with music playing (the room took
  its two songs and its place in the first), a second copy joined by PIN,
  followed, and loaded the room's song. Each player was told to follow
  once and then left alone. Steering from the guest, leaving, and a
  dropped line are covered by tests only.
- Silence trimming: with a track that ends in three seconds of silence,
  the fade began three seconds earlier than without it.
- Crossfade and gapless: run through the engine with two cached tracks
  and their events read. Nobody has listened to the fade itself.
- Resume across a restart was seen working: the last track came back
  paused at its position with the queue behind it.
- A signed-in run on 2026-10-03 in an isolated test profile, with the
  sign-in copied from the Electron app and deleted afterwards: personal
  Home, the library, Liked Music with its hearts, and an uncached track
  resolved in 3.4 s and played. Nothing was written to the account: likes,
  playlist edits and follow are covered by tests only and have never been
  sent to YouTube from this app.
- Close to tray and the second-launch wake-up were checked against the
  real window. The tray icon's own click and menu were not: nothing here
  can click the notification area.
- Browser sign-in: the second step (reopening a profile without a window
  and reading its cookies over DevTools) runs against the installed
  browser in an ignored test. The first step needs a person to sign in at
  Google and has not been done.
- The equalizer's filters and limiter are checked by measuring tones in
  tests; nobody has listened to it.

Not checked, because each needs something this machine or session lacks:

- With a signed-in account: recently played, recent searches, the
  account's name, channel switching, a real podcast, an artist's shuffle
  and radio, song radio. The fixtures have no recording of them, so they
  are covered by tests of the code on either side of the request only.
- Following a change of sound device: playing was seen not to trip the
  check, but no device was plugged in or pulled out.
- The yt-dlp update button, which would replace the vendored binary.
- The app's own title bar by hand: it was drawn and looked at, but moving,
  resizing, maximising and snapping the window need a pointer.
- Hover effects and the queue's drag to reorder, for the same reason.
- The updater end to end: there is no release to find. Its parts (version
  comparison, the release's JSON, checksums, hashing) are tested.
- Lyrics over a real cover's colour: the fixture's track has no lyrics.

## 10. Risks

| Risk | Mitigation |
| --- | --- |
| libopus is C, so the build needs gcc and cmake | WinLibs provides both; `scripts/env.ps1` puts them on PATH |
| Seeking past the cached prefix is slow (1 MiB relay windows) | the reader loops on short responses; the core's whole-file download usually wins the race; measured in M2 |
| egui 0.36 from crates.io lacks fixes Spotifast's fork carries (bidi shaping, a busy-loop fix) | read the registry source; measure idle repaints in M0; adopt a fork only on evidence |
| Scope: parity is large | each milestone is usable on its own; non-goals are explicit |
| Imported credentials go stale | re-import action; browser sign-in in M6 |
| Two cores on one account double the request rate | documented; the user runs one app at a time |

Added on 2026-10-05:

- Winamp skins for the mini player, which section 1 had left out. The
  reader (`crates/app/src/skin/`) is Spotifast's: a `.wsz` is unzipped in
  memory, its sheets become nearest-neighbour textures, and what a skin
  lacks comes from the built-in one. `crates/app/src/skins.rs` lists the
  skins folder and installs into it; the view is
  `views/mini/skinned/`, the main window with its shade mode, the
  equalizer (the app's own ten bands) and the playlist (the queue), all
  in the mini player's one window. A skin with a `region.txt` has its
  window cut to that shape, as Winamp cut it. Skins are installed by
  dropping a file on either window or from Settings.
- The rest of the skin, the same day: the balance slider turns one side
  down in the engine (`Engine::set_balance`, after the tap, so the picture
  is still the whole sound); the bitrate box shows the stream's size over
  its length, which the deck knows once it is open; the display has
  Winamp's oscilloscope as well as its analyser (`Tap::wave`); the playlist
  rolls up, takes a selection of several rows, and has menus behind its
  five buttons.
- Modern skins (`.wal`), as far as they go without their scripts
  (`crates/app/src/skin/modern.rs`): the XML is read by a parser of our
  own that forgives what skins get wrong, includes are followed, and the
  main window's normal layout is flattened into a list of pictures,
  buttons, sliders, text and an analyser, each where the XML puts it.
  Buttons and sliders with a standard action are wired to the player; the
  window is cut to where its pictures are solid. MAKI, the bytecode that
  moves a skin's parts about, is not run, so a skin is its resting state.
  Running it would mean a virtual machine and Winamp's object model, which
  is a project the size of this app's views.
- MilkDrop (`crates/app/src/milkdrop/`), through libprojectM. Spotifast
  links it into the program with MSVC and vcpkg; this app is built with
  MinGW, so here it is a library of its own, built by
  `scripts/build-projectm.ps1` with CMake and Ninja and loaded by name
  when the window is opened, as the core and yt-dlp are files beside the
  app. That also keeps an LGPL library out of an MIT program. The window
  is the program started again with `--milkdrop-child` (winit allows one
  event loop to a process), and the engine's tap hands it the sound
  through a ring in a mapped file, with how far ahead of the ear it is, so
  the picture keeps time. Presets are fetched on request, not shipped.
