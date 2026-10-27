# Youtube Music Spotified, Android

A plan for a native Android app beside the Windows one: the same Go core,
a Kotlin and Jetpack Compose face, and Media3 for everything the system
expects of a music player (the notification, the lock screen, Bluetooth,
Android Auto, Wear OS, the Assistant).

Status, 2026-10-28: built, in `android/`, and run on an Android 16
emulator signed in with a Premium account. Section 13 says what has been
checked and what has not.

Features at a glance:

- **System:** media notification (art, seek bar, like, close), lock screen,
  Bluetooth and headset buttons, steering wheel buttons, Android Auto
  (browse, search, voice, like, shuffle, repeat, radio), Wear OS controls,
  Google Assistant voice, resume from the system's media controls after a
  reboot, audio focus and pause on unplug, home screen widget, app
  shortcuts, opening YouTube Music, YouTube, Spotify, Apple Music, Deezer
  and SoundCloud links, the share sheet, an output picker, more widgets,
  download progress as a Live Update, Material 3 Expressive media controls,
  predictive back.
- **Playback:** Premium quality, gapless, crossfade, normalisation, a
  ten-band EQ, speed, sleep timer, quality per network, skip silence,
  SponsorBlock, local files beside the streams, downloads for offline with
  playlist sync.
- **The desktop's:** Home, search with moods, library with pins and
  folders, album, artist, playlist and podcast pages, likes, playlists,
  queue, timed lyrics, radios, history, stats, Listen Together.
- **Beyond the desktop:** lyrics translation, romanisation, word-by-word
  timing, a timing offset and lyric cards; playlist import from Spotify,
  Apple Music and Deezer; new release alerts; several accounts; Last.fm
  scrobbling; a yearly recap; an all-black theme; Now Playing styles.
- **Later:** Google Cast, Android Automotive OS, Auto's new media
  templates.

## 1. Goals and non-goals

Goals, in priority order:

1. **A proper Android music player.** Plays with the screen off and the app
   swiped away; a media notification with art, seek bar, like and close;
   lock screen and Bluetooth controls; Android Auto; resumes from the
   system's media controls after a reboot. These come from one
   `MediaLibraryService`, so they are one piece of work, not six.
2. **Parity with the desktop app** for what makes sense on a phone (section
   7), reached milestone by milestone.
3. **The core stays the brain.** Queue, shuffle, repeat, autoplay, resume,
   history, stats, folders, pins, caches and every YouTube call stay in the
   Go core, unchanged where possible. The Android app reconciles toward the
   core's `Target`, as the Rust app does (`PLAN.md` section 4).
4. **Light on the battery.** No wake locks beyond what playback needs; no
   polling while paused; the core idles when nothing plays.

Non-goals for the first pass:

- Reusing the Rust crates. egui on Android has no system media integration,
  no accessibility tree TalkBack can use and no Compose-grade gestures;
  Media3 gives the notification, Auto and Wear for free. The audio engine's
  DSP is small enough to port (section 5.3).
- Winamp skins, MilkDrop, the tray, the mini player window, start with the
  system.
- Google Cast and Android Automotive OS (a car's own Android, not Auto).
  Both later, after parity (section 9).
- The Play Store. An app that streams YouTube outside YouTube's player is
  refused there; distribution is GitHub releases (section 8).
- Music video playback, as on the desktop.

## 2. Architecture

One process, three layers:

```
app (Kotlin, Compose)            screens, view models, navigation
  └─ playback (Kotlin, Media3)    MediaLibraryService, CorePlayer, ExoPlayer decks
       └─ core (Go, gomobile AAR)  the same core, in-process, on 127.0.0.1:<port>
            └─ resolver bridge     yt-dlp under Chaquopy + QuickJS (section 4)
```

### 2.1 The core, in-process

`gomobile bind -target=android/arm64,android/amd64` builds `core/` into an
AAR. A small `core/mobile` package exports what the desktop passes as flags:

```go
// Start runs the core and returns the address it bound.
func Start(cfg *Config) (string, error)
func Stop()                          // the graceful path -exit-with-stdin takes
type Resolver interface { Resolve(videoID string, cookies string) (string, error) }
func SetResolver(r Resolver)         // implemented in Kotlin, section 4
```

It still serves HTTP on `127.0.0.1:0`. That keeps one wire format for both
apps, lets the golden-JSON tests pin the Kotlin models as they pin the Rust
ones, and keeps ExoPlayer reading `/v1/stream/{id}` with ordinary range
requests. modernc SQLite and goja are pure Go, so nothing in the core needs
the NDK.

The core is started by the playback service, not the activity, so it lives
as long as playback does. Data goes in `filesDir` (credentials, database),
the audio cache in `cacheDir` (Android may clear it; downloads, section 6,
go elsewhere). A Go panic takes the process down with it: every goroutine
the mobile entry starts recovers and logs.

Changes to the core, each small:

1. `core/mobile`, above.
2. The resolver chain takes an injected resolver in place of exec'ing yt-dlp.
3. A "keep" flag on cached files, so downloads are not evicted by the
   2 GB cap.
4. `Stop` on low memory or when the service is destroyed, with the final
   resume save.
5. Done in A0: the core's setup moved from `cmd/spotifier` to
   `internal/run`, which both entry points call; yt-dlp can be a function
   (`resolver.RunFunc`), given its arguments as a JSON array; ids outside
   YouTube's alphabet never reach yt-dlp.
6. Done in A0: every request must carry a per-start token
   (`mobile.Token()`, header `X-Spotifier-Client`), because any app on the
   phone can connect to the loopback address and the core acts for the
   account.
7. Done in A0: Android builds use `ncruces/go-sqlite3` (SQLite as
   WebAssembly) in place of `modernc.org/sqlite`, whose x86_64 build makes
   system calls Android's seccomp filter kills the process for. The desktop
   keeps modernc; `go test -tags sqlitewasm ./...` runs the suite on the
   Android driver.

### 2.2 Gradle modules

```
android/
  app/          Compose UI, navigation, view models, widget, shortcuts
  playback/     MediaLibraryService, CorePlayer, ExoPlayer, audio processors,
                browse tree for Auto, downloads
  client/       OkHttp + kotlinx.serialization models for the core, SSE
                session client. Port of crates/client
  together/     Listen Together protocol v2 over OkHttp WebSocket.
                Port of crates/app/src/together
  resolver/     Chaquopy, yt-dlp, the QuickJS runtime
  core/         the gomobile AAR, built by scripts/build-core-android.ps1
```

Kotlin, Compose with Material 3, Media3 (ExoPlayer, session, datasource
OkHttp), Coil 3 for artwork, Hilt for wiring, DataStore for settings, Glance
for the widget. minSdk 26 (Android 8), targetSdk the current one.

### 2.3 CorePlayer: the core's queue as a Media3 Player

The crux. A `MediaSession` wants a `Player` whose playlist and commands it
can show and send; the core owns the queue and sends a `Target`. The bridge
is a `SimpleBasePlayer` subclass, `CorePlayer`:

- **State out.** Each projection (`State` with the queue, `Target`,
  `Offline`) becomes `CorePlayer`'s `State`: the playlist is the core's
  queue, the current index and position come from it, so the notification,
  lock screen, Auto and Wear all show the core's queue.
- **Commands in.** `handleSetPlayWhenReady`, `handleSeek` (to a position or
  an item), `handleSetShuffleModeEnabled`, `handleSetRepeatMode`,
  `handleSetMediaItems`, `handleAddMediaItems`, `handleMoveMediaItems`,
  `handleRemoveMediaItems` each post a core `Command`. The optimistic
  overlay of `PLAN.md` section 3.6 is `SimpleBasePlayer`'s placeholder
  state, cleared by the answering projection.
- **Engine underneath.** Two ExoPlayer instances are the two decks.
  `reconcile(engineState, target)` is ported from `crates/audio/src/engine.rs`
  as a pure Kotlin function with the same tests. Decks report `loaded`,
  `position`, `ended`, `failed`, `stalled`, `blocked` exactly as the
  contract asks, including the health check before failing and the
  8-second stall watchdog.

Gapless: ExoPlayer is gapless within one playlist, but the core decides the
next track, so the idle deck preloads `PreloadVideoID` and the hand-over
butts the decks, as on the desktop. Crossfade ramps the two decks' volumes
(equal power) from the core's `Transition`.

## 3. System integration

All from `MediaLibraryService` + `CorePlayer` unless noted.

| Feature | How |
| --- | --- |
| Media notification | Media3's `DefaultMediaNotificationProvider`; custom layout buttons for like and close; art from the core's artwork URL through a Coil `BitmapLoader` |
| Lock screen, quick settings media player | the session; Android 13+ draws it from the session's state |
| Bluetooth (AVRCP), headset buttons, steering wheel buttons | the session; double and triple press for next and previous |
| Audio focus, ducking, pause on unplug | ExoPlayer's `handleAudioFocus` and `setHandleAudioBecomingNoisy`, wired through to core `Command`s so the core's state agrees |
| Playback resumption after reboot | `onPlaybackResumption`, from the core's saved resume point; `MediaButtonReceiver` in the manifest |
| Android Auto | section 3.1 |
| Wear OS, Pixel watch, Galaxy watch | the phone's session is mirrored by the watch's media controls; nothing to build |
| Google Assistant ("play X on Spotified") | `MEDIA_PLAY_FROM_SEARCH` and `onSetMediaItems` with a search query → core search → play the top result or the artist's radio |
| Foreground service | `foregroundServiceType="mediaPlayback"` (Android 14 rule), `POST_NOTIFICATIONS` asked on Android 13+ |
| Background limits | an in-app check for battery optimisation with a link to its setting, and a page naming the OEMs that kill players (dontkillmyapp.com) |
| Home screen widget | Glance: art, title, play, next, like, in two sizes |
| App shortcuts | static: Search, Liked Music, Shuffle library; dynamic: the last three playlists |
| Links | intent filters for `music.youtube.com`, `youtube.com/watch` and `youtu.be` open the song, album, artist or playlist; Spotify, Apple Music, Deezer and SoundCloud links shared to the app are matched, as search does on the desktop |
| Share | the share sheet with the YouTube Music link, and the Spotify, Apple Music and Deezer links of 0.4.8 |
| Sleep timer | in minutes or at the end of the song; fades the last ten seconds |
| Output picker | a devices button on the mini player and Now Playing opens Media3's output switcher (phone, wired, Bluetooth, later Cast) without the notification shade |
| More widgets | beside the player widget: one playlist's songs, and recently played, both Glance |
| Download progress | Android 16's progress-centric notification (`ProgressStyle`, a Live Update) for downloads and playlist sync. Media playback itself cannot be a Live Update; Android excludes media notifications |
| Material 3 Expressive media controls | Android 16 restyles the media notification and the Quick Settings player itself. It looks right only with square art of at least 512 px and the standard commands, so the like and close buttons go through Media3 1.9's media button preferences rather than custom layouts |
| Predictive back | Now Playing, the queue and every sheet shrink with the back gesture through `PredictiveBackHandler`; no plain enabled `BackHandler`, which would suppress the system animation |

### 3.1 Android Auto

- `automotive_app_desc.xml` declaring `media`, and the service's
  `MediaLibraryService` intent filter.
- Browse tree from `onGetLibraryRoot` and `onGetChildren`, four tabs (Auto
  shows at most four): **Home** (the core's home shelves), **Library**
  (pinned first, then playlists, albums, artists), **Liked**, **Recent**.
  Content style hints: grid for albums and playlists, list for songs.
- `onSearch` and `onGetSearchResult` for the car's search and voice.
- Artwork through a `ContentProvider` that serves cached art as
  `content://` URIs, since the car's head unit cannot reach the core's port.
- Custom actions: like, shuffle, repeat, start radio.
- Testing: the Desktop Head Unit on Windows, then a car.
- An app not installed from the Play Store only shows in Auto with Auto's
  developer setting "Unknown sources" on. The app explains this once, on
  first connection, with the steps.
- Google announced media browse and playback templates for Auto at I/O
  2026 (`MediaPlaybackTemplate` and an adaptive mini player, in Car App
  Library 1.8 and 1.9), through an early-access programme. The
  `MediaLibraryService` above works without them; adopt them when they
  are open to all, or apply for early access.

## 4. Stream resolving

Three resolvers, tried in order, the same on Android and the desktop
(`core/internal/run`, `buildResolver`):

1. **Native** (`core/internal/resolver/native.go`), for Premium accounts.
   The core makes YouTube Music's own player request with the account,
   through its existing InnerTube client (cookies, request signature, rate
   limit). It puts the stream address through the player's sig and n
   functions with `core/internal/jsc`, which runs yt-dlp-ejs, the solver
   yt-dlp itself uses. That runs in one JavaScript process that stays up
   and keeps the player parsed: QuickJS-NG on Android, deno on the desktop.
   The parsed player is saved to disk, and the core warms it at start.
   Measured on 2026-10-27: a song resolves in 150-240 ms on the emulator
   and on the desktop alike, against about 4 s through yt-dlp. The QuickJS
   process holds 40-60 MB and stops after ten idle minutes.
2. **yt-dlp**, for everyone else: under Chaquopy with QuickJS on Android,
   the program with deno on the desktop. YouTube Music's web client asks
   every listener but a Premium one for a proof-of-origin token, which the
   native resolver has not got; yt-dlp reaches them through other clients.
   About 4 s a song once warm, 21 s signed out on the emulator. On Android
   Python starts on yt-dlp's first call, so a Premium listener never pays
   for it.
3. **The pure-Go library** (`resolver/library.go`), standard quality, last.

The native resolver answers `ErrNotApplicable` for what it cannot serve:
signed out, or an account whose answers show no subscriber formats three
times running. The chain passes those to yt-dlp without counting them
against it.

## 5. Sound

### 5.1 Formats

Opus in WebM and AAC in MP4. ExoPlayer demuxes both, and Android's own
MediaCodec decodes both on every version the app supports. Media3's
libopus extension is kept in reserve for phones whose Opus decoder
misbehaves.

### 5.2 Data and quality

A quality setting for Wi-Fi and one for mobile data (highest, standard,
low), passed to the core's resolver. Downloads only on Wi-Fi unless told
otherwise.

### 5.3 Effects

An ExoPlayer `AudioProcessor` chain, ported from `crates/audio`: loudness
gain per deck (from `/v1/tracks/{id}/loudness`), the ten-band EQ with its
presets and preamp (`eq/design.rs`, biquads, a small port), the limiter
(`eq/limiter.rs`), silence trimming (`silence.rs`). Playback speed and
pitch are ExoPlayer's `PlaybackParameters`, so the desktop's
`stretch.rs` is not needed. A button opens the system equalizer for
those who prefer it (the audio session id is published).

A visualizer is a tap processor feeding the Now Playing screen's spectrum.
Android's `Visualizer` class is not used: it needs the microphone
permission.

Skip silence, a setting off by default, removes the quiet inside a song or
an episode (live recordings, podcasts) through ExoPlayer's
`setSkipSilenceEnabled`. It is separate from the silence trimming at a
song's edges that crossfade uses.

SponsorBlock, a setting off by default: for a song that comes from a
YouTube upload rather than an official release, the segments that
SponsorBlock's public database marks as music-free (intros, talk,
sponsor reads) are skipped. The core asks the database by video id and
sends the segments with the projection; the deck seeks past them.

### 5.4 Local files

Music on the phone plays beside the streams. `MediaStore` is read with
`READ_MEDIA_AUDIO`, the tracks are listed under Library as a "On this
phone" kind, can be added to the queue and to the app's own folders, and
play straight from their `content://` URIs in a deck. They cannot go in
a YouTube playlist, which can only hold YouTube songs; the menu says so.

## 6. Offline

- Download a song, album or playlist. Files go in `filesDir/downloads`,
  served by the core like cache hits but marked "keep" (section 2.1).
- A downloaded playlist follows its source: new songs are downloaded on the
  next sync, on Wi-Fi, by a `WorkManager` job with the battery and network
  constraints set.
- A Downloads page under Library, the space used in Settings, and a
  download badge on rows.
- With no network, Home shows the downloads and the library from the
  core's response cache. The core's `Offline` flag already holds playback
  rather than skipping.

### 6.1 Beyond the desktop

Features the desktop does not have yet, which phone players are expected
to.

**Lyrics.**
- Translation into the phone's language, and romanisation of Japanese,
  Korean, Chinese, Cyrillic and other scripts, each a toggle on the lyrics
  screen. Romanisation is done on the phone (ICU's transliterators);
  translation through an online service, to be chosen when A8 starts.
- Word-by-word timing where the source has it (LRCLIB's enhanced LRC,
  YouTube's own word timings), with the sung part of the line filled in.
- A timing offset, in tenths of a second, kept per song, for lyrics that
  run early or late.
- Lyric cards: select one to five lines and share them as an image over
  the art's colour, with the cover and title.

**Library and discovery.**
- Importing playlists from Spotify, Apple Music and Deezer, by link or
  from their export files: each song is matched by title, artist and
  length through the core's search, the doubtful matches are shown for a
  check, and a YouTube Music playlist is created.
- Alerts for new releases from followed artists: a daily `WorkManager`
  job asks the core for each followed artist's latest release and posts a
  notification for what is new.
- Several accounts: each has its own credentials, database and cache
  directory, as the core's `-account-scope` already allows; switching
  restarts the core.

**Social.**
- Last.fm scrobbling: sign-in through Last.fm's web auth, "now playing"
  at the start of a song and a scrobble after half of it or four minutes,
  from the core's position reports, queued while offline.
- A yearly recap, Wrapped's shape: top songs, artists and albums, minutes
  played and the month of each, from the core's stats routes, as a story of
  cards that can be shared as images. In December, and on demand.

**Personalisation.**
- An all-black theme beside dark and light: the dark palette with
  `window` and `panel` at `#000000`, for OLED screens.
- Now Playing styles: Spotify's (section 7.1) by default, a full-bleed
  art style with the controls over a blur, and a lyrics-first style.

## 7. Screens

Spotify's shape, which is what the desktop copies: a bottom bar with Home,
Search, Library; a mini player above it; Now Playing a swipe up away.

| Screen | Contents |
| --- | --- |
| Home | the core's shelves, recently played, mixes, mood chips |
| Search | debounced, filter chips, suggestions, recent searches, mood wall, pasted links matched |
| Library | kind chips, sort, search, pins first, folders, grid or list, Downloads |
| Album, playlist, artist, podcast | header with art tint, play and shuffle, rows with a long-press menu |
| Now Playing | art (swipe for next and previous), seek bar, like, queue, lyrics, share, sleep timer, speed, output device chip |
| Queue | drag to reorder, swipe to remove, played songs above, as 0.4.8 allows |
| Lyrics | timed lines follow the song, tap a line to seek, full screen |
| Listen Together | the room page of 0.4.8: song, name, PIN, queue; join by PIN or link |
| Stats, history | the core's stats routes |
| Settings | account, playback (crossfade, gapless, normalisation, quality, EQ), downloads, appearance, storage, battery, about, logs |

Song menus (long press, or a selection made with a long press and taps):
play next, add to queue, add to playlist, like, go to album or artist,
start radio, download, share.

Theme: the desktop's `Palette` (`crates/app/src/theme.rs`) everywhere,
copied value for value into one Kotlin object: `DARK` by default (window
`#0f0f0f`, panel `#181818`, surface `#212121`, text white, secondary
`#aaaaaa`, accent `#ff0033`), `LIGHT` as the second. Inter, the desktop's
font, in its four weights. Album-art tint behind headers and Now Playing,
as the desktop does; no Material You dynamic colour. Edge to edge,
predictive back, tablet and foldable layouts with a side rail and the queue
beside Now Playing. TalkBack labels on every control.

### 7.1 Spotify's phone layout, from Mobbin

Mobbin has Spotify's iOS app only; its Android app has the same layout,
except that the status bar and back gestures are Android's. What follows
is the layout to copy. The colours are this app's, not Spotify's: where
Spotify uses its green, this app uses `accent`.

**Chrome.**
- A bottom bar of four tabs: Home, Search, Your Library, Create. Each has an
  outline icon with a label under it, and the icon fills in when its tab is
  selected. The bar is transparent over a dark gradient, so the page fades
  out under it.
- The mini player floats above the bar as a rounded card (radius about
  8 dp, 8 dp in from each side) tinted from the album art. It shows the art,
  the title, the artist, a devices button and play/pause, with a 2 dp
  progress line along its bottom edge. Tapping it, or swiping it up, opens
  Now Playing.
- There is no top app bar on the tab roots. Home starts with chips. Library
  has a large title with an avatar, a search button and a +. Pages pushed
  above a root have a small centred title and a back chevron.
  ([home](https://mobbin.com/screens/c2609ecf-234e-4f04-af1d-18199a45f2d5),
  [library](https://mobbin.com/screens/ea3aa1f4-68fd-4890-b99c-29c84094f6b1))

**Home.**
- A row of filter chips, "All" first, with the selected chip filled.
- Under them, a two-column grid of eight recent items. Each is a compact
  tile, about 56 dp tall, with square art on its left edge and a bold
  two-line title.
- Then shelves, each a bold 22 sp heading over a row of square covers
  (about 150 dp) that scrolls sideways, with two lines of grey text under
  each cover. Some shelves are lists of songs instead.
  ([home](https://mobbin.com/screens/c2609ecf-234e-4f04-af1d-18199a45f2d5),
  [shelves](https://mobbin.com/screens/df111c93-2570-4992-8d6c-831396630d7c))

**Search.**
- Before anything is typed: a white search field with dark text ("What do
  you want to listen to?"), and under it a two-column wall of coloured mood
  tiles. Each tile has a bold title and a small cover tilted in its
  bottom-right corner. This maps onto the desktop's mood wall.
- While typing: the field turns grey and gets a clear button and Cancel,
  with a row of filter chips under it. Result rows are 48 dp art (round for
  artists) with a title over "Kind · Artist", and a ⊕ or Follow on the
  right.
  ([browse](https://mobbin.com/screens/7d1d7f6f-e8d8-4569-8ff4-401b84695a0a),
  [results](https://mobbin.com/screens/d6b7eccc-85ca-45dc-93a8-6f7a770fddf9))

**Library.**
- A large "Your Library" title, then dismissable kind chips (Playlists,
  Albums, Artists, Podcasts).
- A sort row ("Recents") with a list/grid switch on its right.
- Rows are 64 dp: square art for playlists and albums, round for artists,
  a title, and "Playlist · Owner" under it. Pinned items show a small green
  pin before that subtitle, which this app draws in `accent`.
- The grid view has three columns, and the end of the grid has "Add
  artists" and "Add podcasts" tiles.
  ([list](https://mobbin.com/screens/ea3aa1f4-68fd-4890-b99c-29c84094f6b1),
  [grid](https://mobbin.com/screens/6334a3f9-fcff-43ec-87df-401348800ac8))

**Album and playlist.**
- A gradient from the art's dominant colour down to the background, with
  centred art about 240 dp across.
- Below the art, left-aligned: the title (bold, about 24 sp), the owner
  with an avatar, and the duration.
- An action row: download, add a collaborator, ⋯, and on the right shuffle
  and a big round play button (56 dp) in the accent colour.
- Under that, on your own playlists, a sideways row of pill chips (Add,
  Edit, Sort).
- Track rows are 56 dp: art, title, artist with an "E" badge when the song
  is explicit, and ⋯ on the right. The row that is playing has its title in
  the accent colour.
  ([playlist](https://mobbin.com/screens/e2697a6f-9951-4d66-adcd-2b3ee0e69de3))

**Artist.**
- A full-bleed photo with the name over it in very large bold type (about
  48 sp), and "monthly listeners" under it.
- A row with a Follow pill, ⋯, shuffle and the accent play button.
- "Popular" as a numbered list of five songs with their play counts, then
  a "See more" pill, an artist pick, and shelves of releases. The bar at the
  top fades from clear to solid with a small centred title as you scroll.
  ([artist](https://mobbin.com/screens/cd26e49b-2714-436f-85bd-ccd0ee3b9b3d),
  [scrolled](https://mobbin.com/screens/9b59162a-9579-4d70-8714-97bfa7f5b885))

**Now Playing.** A full-screen sheet over a gradient from the art's
colour, top to bottom:
- A ⌄ to close, a centred "Playing from …" title, and ⋯.
- The art, square and full width less a 24 dp margin.
- The title (bold, about 22 sp) and artist on the left, with the like
  button (⊕, which becomes a filled ✓ in the accent colour once liked) on
  the right.
- A thin seek bar with times under both ends.
- The controls: shuffle, previous, a 64 dp white round play button with a
  black glyph, next and repeat. Shuffle and repeat take the accent colour
  when they are on.
- A bottom row: devices on the left, share and queue on the right.
- Scrolling down shows cards: Lyrics, tinted, with the first lines and an
  expand button; then about the artist, then credits.
  ([now playing](https://mobbin.com/screens/43a707cc-5c25-4bc8-8f7a-f24852d1f59b))

**Lyrics.**
- Full screen on a solid colour taken from the art.
- Large bold lines (about 24 sp). Lines already sung are white, lines to
  come are dimmed, and the current line is kept near the top.
- Share and ⋯ sit above a seek bar and a big play button at the bottom.
  ([lyrics](https://mobbin.com/screens/27859727-f251-4887-8b72-e4776a6d7597))

**Queue.**
- A sheet over Now Playing headed "Queue · Playing <source>", with Clear on
  the right.
- The current song comes first, with its title in the accent colour and a
  play button; under it, rows with drag handles.
- Three wide buttons along the bottom: Shuffle, Repeat and Timer (the
  sleep timer).
- Spotify's Jam shows the same queue with each listener's avatar on their
  songs, which is a fit for Listen Together.
  ([queue](https://mobbin.com/screens/2426f0a0-51cf-449e-8197-da0eaf41329a),
  [Jam](https://mobbin.com/screens/2363add4-3caa-4df6-9c23-d970451e4504))

**Menus.** Long press or ⋯ opens a bottom sheet:
- A grab handle, then a header with the art, title and "Artist · Album".
- A list of 48 dp rows, each an outline icon and a label: Share, Add to
  Liked Songs, Add to playlist, Add to Queue, Go to Queue, Start a Jam, Go
  to radio, Go to album, Go to artist.
- This app's menu is the same list, with Listen Together in place of Jam
  and Download added.
  ([song](https://mobbin.com/screens/facdc774-78cf-4c5f-a873-b70201c92bed),
  [playlist](https://mobbin.com/screens/e2b49760-5731-4e34-a639-3ed60e79664c))

**Shared measures.**
- Margins are 16 dp on each side.
- Type is bold for titles and regular for everything else, with secondary
  text in grey.
- Pill shapes for chips and secondary buttons.
- The round accent play button is the one strong colour on a page; the
  artwork carries the rest.

## 8. Sign-in and distribution

**Sign-in.** A `WebView` opens Google's sign-in with YouTube Music as its
continue page. When it lands there, `CookieManager` gives the cookies
(HttpOnly ones included) for `.youtube.com` and `.google.com`; the same 23
the desktop keeps become `credentials.json`; the WebView's storage is
cleared. Google sometimes refuses embedded sign-in; the fallback is
importing the desktop's `credentials.json` (a file, or a QR code shown by
the desktop app's Settings and scanned by the phone). Signed out, the app
browses and plays at standard quality, as the desktop does.

**Distribution.** GitHub releases: an APK per ABI (arm64-v8a first,
x86_64 for the emulator) signed with one release key, plus a
`SHA256SUMS.txt`. The app updates itself as the desktop does, through
`PackageInstaller` with `REQUEST_INSTALL_PACKAGES`. It is also listed for
Obtainium. Not the Play Store (section 1).

## 9. Milestones

Each ends with: the Go tests, Kotlin unit tests (`reconcile`, the
projection mapping, the client's golden JSON), a run on a phone and the
emulator, and battery and memory measured (section 10).

- **A0: spike, about a week.** gomobile AAR on a phone; a hard-coded song
  plays through ExoPlayer from `/v1/stream`; yt-dlp under Chaquopy with
  QuickJS resolves one Premium stream; WebView sign-in yields working
  cookies; APK size measured. Decides section 4: A1 does not start until a
  Premium stream (itag 774) has been resolved on the phone by option 1, or
  option 3 has been chosen in its place.

  Done on 2026-10-27, on an Android 16 x86_64 emulator, with the
  `android/app` spike (`scripts/build-core-android.ps1`,
  `scripts/build-qjs-android.ps1`). Option 1 holds:
  - the core starts in-process in 1.2-1.6 s;
  - signed in (the desktop's credentials.json copied in), yt-dlp under
    Chaquopy with QuickJS-NG 0.17 resolved itag 774 (Opus 261 kbps) and 141
    (AAC 257 kbps) in 10-13 s, one run each; signed out, 251 or 140 in
    about 21 s, because the YouTube Music client needs a PO token and
    yt-dlp tries again the default way;
  - ExoPlayer plays from `/v1/stream` in 0.5-2.3 s;
  - APK, arm64 release before R8: 38 MB (the core 22 MB, Python and its
    libraries about 23 MB uncompressed);
  - WebView sign-in works: Google accepted it with the WebView marker
    taken out of the user agent, `CookieManager` gave 21 of the 23 cookies
    the desktop keeps (HttpOnly ones included), and the session it made
    resolved Premium (itag 141) after the core restarted.
- **A1: browse.** Project, modules, theme, navigation, `client`, Home,
  Search, album, artist, playlist, Library, artwork.
- **A2: play.** `CorePlayer`, decks and `reconcile`, `MediaLibraryService`,
  notification, lock screen, Bluetooth, audio focus, unplug, mini player,
  Now Playing, Queue, resumption after reboot.
- **A3: in the car and on the wrist.** Auto browse tree, search, custom
  actions, art provider, the Desktop Head Unit; Assistant voice; checked on
  a watch.
- **A4: account.** Sign-in, likes, playlists (create, add, remove,
  delete), follow, history, lyrics, share and links in.
- **A5: sound.** Normalisation, EQ, limiter, gapless, crossfade, silence
  trimming, speed, sleep timer, quality per network.
- **A6: offline and home screen.** Downloads and sync, offline Home,
  widget, shortcuts.
- **A7: the rest.** Listen Together, stats, podcasts, moods, settings
  complete, updater, release build and signing, CI building the APK.
- **A8: beyond the desktop.** In order of value against effort: skip
  silence, Last.fm, lyrics translation and romanisation, the all-black
  theme, playlist import, the output picker, lyric cards and the
  offset; then local files, new release alerts, word-by-word lyrics, the
  yearly recap, Now Playing styles, more widgets, several accounts,
  SponsorBlock.
- **Later.** Google Cast (the receiver fetches the stream itself; whether
  googlevideo URLs resolved on the phone play on a Chromecast on the same
  network is the first thing to find out), Android Automotive OS, Auto's
  new media templates.

## 10. Budgets

| Measure | Budget |
| --- | --- |
| Cold start to Home drawn | under 1 s on a mid-range phone; the core starts in parallel and the first screen comes from the response cache |
| Battery while playing, screen off | under 4% an hour |
| Memory while playing, in the background | under 150 MB |
| Time to sound, cached song | under 400 ms |
| APK, arm64 | under 60 MB with Chaquopy, under 30 MB without |

## 11. Risks

| Risk | Mitigation |
| --- | --- |
| yt-dlp does not run well under Chaquopy, or QuickJS is too slow for the challenges | the A0 spike decides; the pure-Go resolver already works; NewPipeExtractor is the third way |
| The native resolver depends on how YouTube Music's web client asks for a stream today: the player request, the player's functions, and Premium needing no token | yt-dlp stays behind it in the chain and catches what it gets wrong; yt-dlp-ejs is vendored and updated with yt-dlp; live tests (`SPOTIFIER_LIVE=1` in `internal/jsc` and `internal/resolver`) check it against YouTube |
| QuickJS takes about 7 s to parse a new player | once per player version, which changes every few days; saved to disk and warmed when the core starts |
| A listener who is not Premium waits about 4 s a song (yt-dlp), 21 s signed out | the core's prefetch resolves the next songs ahead, so the wait falls on the first |
| Google refuses sign-in in a WebView | import the desktop's credentials by file or QR code |
| Android Auto hides sideloaded apps | first-connection instructions for Auto's "Unknown sources" |
| OEM battery managers kill the service | foreground service done by the book; an in-app battery page; dontkillmyapp.com linked |
| A Go panic kills the app | recover in every goroutine the mobile entry starts; the core's tests |
| Two cores (phone and desktop) on one account double the request rate | each core's own rate governor; documented, as with the Electron app |
| The core's queue and Media3's model of a playlist disagree (for example, Auto skipping to an item the core has since moved) | commands carry the queue item's id, not its index; the core rejects stale ones and the projection corrects the view |
| Play Store refusal | not attempted; GitHub and Obtainium |
## 12. Decided

Settled on 2026-10-27:

1. Premium-quality audio at launch. The A0 spike gates A1 on it (section 4).
2. The desktop's Spotify palette everywhere; no Material You (section 7).
3. One repository: the app lives in `android/` here and builds `core/`
   from this tree.

## 13. Status

Built on 2026-10-28, as one Gradle module (`android/app`) in packages
rather than the modules of section 2.2: `core` (the Go core and the Python
bridge), `api` (the core's client, a port of `crates/client`), `playback`
(the engine, `CorePlayer`, the service, the browse tree, artwork for Auto,
the sleep timer), `data` (settings, likes, downloads, Last.fm, other
services, import, local files, release alerts, the updater), `together`,
`widget` and `ui`.

Checked on the emulator, signed in:

- Home, search (with the mood wall), the library, album, artist, playlist
  and Now Playing pages, in the desktop's palette;
- playing from a row: the core's queue, the engine's decks, the mini
  player, and the system's media controls with like and shuffle;
- the native resolver: songs start in about 0.2 s;
- `connectedDebugAndroidTest`: what Android Auto asks of the app through a
  media browser (the tree, search, a song chosen in a list).

Not yet checked by hand: Android Auto in a car or the Desktop Head Unit
(it needs the Android Auto app, from the Play Store), a watch, Bluetooth
buttons, the Assistant's voice, a release build on a phone, Last.fm with a
real key, the importer against each service, and a release published for
the updater. Word-by-word lyrics wait on the core: its lyrics carry lines,
not words.

Also built since the plan was written: SponsorBlock (skipped by the
engine, from SponsorBlock's public database), Now Playing's three styles,
audio quality per network (a bitrate ceiling the core's resolvers honour,
sent as the phone moves between Wi-Fi and mobile data), a recently played
widget, and several accounts, each with its own folder, switched by
starting the core again on another.
