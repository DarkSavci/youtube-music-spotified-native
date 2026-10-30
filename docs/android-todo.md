# Android app: what to fix

From the audit of 2026-10-30, on 0.5.0, using a Galaxy S24 FE's logs, audio
mixer history and battery stats, the Android code, the Go core, and the desktop
app for comparison.

How each item is known:

- **[device]** seen in the phone's own data.
- **[code]** certain from reading the code.
- **[try]** the code says so, but check it on the phone first.

The screens were read in code, not driven by hand, so run the gesture items
after fixing them.

## Status, 2026-10-30

Everything ticked is written, and the Go, Rust and Android unit tests pass, but
nothing has been committed or installed on a real phone. An item with a note
explains what is partial. The emulator confirmed the crossfade fix (2 of 2, full
gain), gestures, tabs and the queue, signed out and with audio off.

**Still to check by hand on the phone:**

- [ ] Sound: level and bass in the car against YouTube Music; boost, bass and
  visualizer audible as expected; crossfade by ear.
- [ ] Signed in: library, playlists, rename, save and remove, folders, multi-select.
- [ ] Listen Together in a real room: Undo, invite links, how others see "paused"
  during a call or headset unplug.
- [ ] Bluetooth, a real headset, and Android Auto in a car, with the first-connection steps.
- [ ] Reboot and resume from the system media controls.
- [ ] Download Live Update and Pause on Android 16; TalkBack.
- [ ] Word-timed and unsynced lyrics: no such song played on the emulator.

**Known gaps found while fixing:**

- [ ] Downloads have no foreground service, so Android can stop them with the app
  in the background and nothing playing.
- [ ] Clear queue's autoplay hold doesn't survive a process restart.
- [ ] Episodes' big Play button starts from the top; rows resume.
- [ ] Offline Home has no Recents, because the core keeps no copy of history.
- [ ] `/v1/session/*`, `/v1/auth/*` and `/v1/migrate/*` have no cross-site guard on desktop.
- [ ] First forward page can appear late, likely first-composition cost; check a release build.
- [ ] One landscape pager swipe back didn't take effect; it works in portrait.

## 1. Bugs you can hear

- [x] **Songs restart about 6 s in, after every crossfade.** [device] All 4
  crossfades in the mixer history did it: the new deck's track played 5.8-6.1 s,
  was destroyed, and a new one began (13:34:23, 13:38:59, 13:43:37, 13:49:50).
  The core sets `StartAtMs` to 0 when it names the track
  (`core.go` `startTrack`). `EvLoaded` then publishes that 0 before the first
  position report (`core.go:531-550`). `reconcile` sees a deck 6 s away from 0
  and seeks (`Reconcile.kt:55-56`). Its test only covers the first projection
  (`ReconcileTest.kt:54`).
  - Fix: a seek should be something the core asks for, not something inferred
    from a position echo. Add a seek counter to `Target` and seek only when it
    changes, or send the track's real position when the engine ran into it.
  - Add a test: a deck 6 s in, then a second projection at 0, and no seek.
  - `crates/audio/src/engine.rs:130` has the same logic, so check the desktop.
  - Until it is fixed, Crossfade at 0 in Settings > Playback avoids this path.
- [x] **Any late position echo can also pull a song back.** [code] The tolerance
  is 1.5 s and reports come once a second, so about 0.5 s of delay on a
  non-position update is enough. The seek counter above fixes this too.
- [x] **Log every seek with its reason.** [code] Nothing records them today, which
  is why this was invisible in logcat.
- [x] **Everything plays 10 dB too quiet.** [device] Every track in the mixer
  history is at gain 0.314972, -10 dB. The core's `DefaultVolume` is 0.5
  (`core.go:111`), `gainFor` turns that into 0.315 (`Reconcile.kt:84`), and the
  phone has no volume slider, since nothing sends `Command.SetVolume`.
  - Fix: on a phone the system volume is the volume, so the engine should play at
    1.0. Keep a room's volume working in Listen Together.
  - This is the most likely reason YouTube Music sounds louder and fuller, because
    bass is heard less at lower levels. Compare again after this fix before adding
    anything below.
- [x] **Mobile data caps quality at 170 kbps by default.** [code] `mobileQuality`
  is `standard` (`Settings.kt:50`, `quality.go:17`), and a car usually means mobile
  data. Consider `high` as the default, or show the current quality in the player.
- [x] **Add a volume boost and level.** [code] The desktop has Volume boost and
  Volume level; the phone caps at 1.0 (`gainFor`). Add `LoudnessEnhancer` on the
  session, or the limiter `docs/android.md` 5.3 planned.
- [x] **Add a bass control.** [code] The equalizer is off by default, with only
  a "Bass" preset behind Settings > Playback > Equalizer. Consider a car or bass
  option in the player. Check the equalizer really reaches the shared session:
  the settings page opens a second `Equalizer` to read the bands.
- [x] **Instagram interrupts the music.** [device] At 13:45:36 two focus grabs
  within 188 ms gave pause, play, pause, play (`PlaybackService.kt:396`). Wait
  300-500 ms before pausing on a temporary loss.
- [x] **The seek bar may not seek.** [try] `ThinSeekBar` uses
  `pointerInput(Unit)` (`Player.kt:344`), which keeps the first `onRelease`, and
  that one saw `duration` 0, so `if (duration > 0)` would never seek. If it works,
  still fix it with `rememberUpdatedState`.
- [x] **Accidental seeks.** [code] `detectTapGestures.onPress` seeks on any touch,
  even one that starts a page scroll, and a drag can call `onRelease` twice
  (`Player.kt:345-359`). `ThinSlider` has the same pattern
  (`Components.kt:287-301`).
- [x] **Seek bar snaps back on release.** [code] It returns to the old position
  until the engine catches up.

## 2. Gestures you asked about

- [x] **Pull to refresh.** [code] It exists nowhere. `Loader.reload(quietly)` is
  ready but nothing calls it quietly (`Components.kt:506`). Add it to Home, Library,
  Search results, album, artist, playlist, Liked, History, Stats, Browse, Podcast.
- [x] **Tap the current tab to scroll to the top.** [code] `tabTo`'s comment
  promises it (`Root.kt:183`), but `launchSingleTop` makes it do nothing. Hoist each
  tab's list state and scroll it. A tap on a page under the tab should return to the
  tab's root first.
- [x] **Swipe down to close Now Playing.** [code] The gesture sits on the parent
  and the inner `verticalScroll` takes the drag first (`Player.kt:201,217`). It also
  checks one event (`drag > 30`) rather than the total. Use a sheet with
  `anchoredDraggable` and `nestedScroll`, where the page follows the finger and
  closes from the top of the scroll.
- [x] **Swipe up on the mini player.** [code] Same problem: per-event threshold,
  no following the finger, and `onOpen` can fire repeatedly (`Player.kt:129`).
- [x] **Queue dragging.** All in `Menus.kt:307-352`:
  - [code] Long press opens the song menu **and** starts the drag, because
    `TrackRow`'s `onLongClick` and the drag gesture both fire. Likely the worst of it.
  - [code] The row does not move with the finger. `dragBy` is only counted, and the
    row just gets a background.
  - [code] The drop point assumes 64 dp rows, there is no gap preview, no
    `animateItem`, no auto-scroll at the edges and no haptics.
  - [code] Keys contain the index (`"$i:${t.id}"`), so every row rebuilds on a move.
  - [code] A song can be dropped above the one playing (`coerceIn(0, size - 1)`).
  - [code] The sheet's own drag competes with the list.
  - [code] The comment promises a handle and swipe to remove; neither exists.
  - Fix: a drag handle, a real reorderable list, swipe to remove with undo.
- [x] **Queue flow.** [code] Opening a song's menu replaces the queue, and closing
  it does not come back. Timer does the same. The playing row has no marker beyond
  its colour. Add Clear queue and show what has played (`docs/android.md` section 7).

## 3. Whole-app patterns

- [x] **Stale pages.** [code] Nothing refreshes after a change: a new, deleted or
  pinned playlist stays wrong in Library, and "Remove from this playlist" leaves the
  row until you leave the page. Add invalidation after each write.
- [x] **No confirmation or undo on destructive actions.** [code] Delete playlist
  (`Collections.kt:203`), Log out (`Settings.kt:150`), Remove account
  (`Settings.kt:289`), Empty the cache, Remove from queue and Remove from playlist.
- [x] **No haptics, no list animation and no predictive back.** [code] `BackHandler`
  is used where the plan asked for `PredictiveBackHandler`, and there are no screen
  transitions beyond the player's slide.
- [x] **Spinners instead of skeletons, and a full-page spinner on every chip.** [code]
  Filter and chip changes wipe the page. Keep the old content while loading.
- [x] **Silent failures.** [code] `runCatching` hides failed lyrics, library, account
  and folder loads, so the page just looks empty. "Pinned" is confirmed in one place
  only; the menu's pin says nothing.
- [x] **Loading state on play.** [code] The play button only shrinks to 94% while
  loading, and the mini player shows nothing. Add a spinner.
- [x] **Current song not marked everywhere.** [code] Home, Search, History, Stats,
  Downloads and shelves do not pass `playing`.
- [x] **Rows disabled with their menu.** [code] An unplayable song's long press is
  disabled too (`TrackRow`), so you cannot remove or inspect it, and there is no
  explanation of why it is grey.
- [x] **Landscape and tablets.** [code] Portrait is not locked and no layout adapts.
  Now Playing's art takes the full width in landscape. The plan asked for a side rail.
- [x] **Accessibility.** [code] Many tappable rows, chips and images have no role or
  description; icons pass `null`. Check with TalkBack.
- [x] **Tab highlight disappears on sub-pages.** [code] It compares the exact route
  (`Root.kt:176-178`), and the tabs have no filled icon as planned.
- [x] **A tab can get stuck on a sub-page.** [try] From an album under Library,
  tapping Library may restore the same album through `restoreState`.

## 4. Page by page

**Home**
- [x] [code] The chosen mood is lost on tab changes (`remember` at `Home.kt:64`); use
  `rememberSaveable`.
- [x] [code] Podcasts is probed on every load and logs a failure (`Home.kt:67`).
  Remember the answer and don't log a 404 as failed.
- [x] [code] Chips are side-effects in composition (`Home.kt:79`), and the empty
  `Text("")` at the end does nothing (`Home.kt:123`).
- [x] [code] Recents only show on All, with no way to see more.
- [x] [code] No offline Home, though the plan has one.

**Search**
- [x] [code] No suggestions and no recent searches, though `api.suggest` exists and
  is never called.
- [x] [code] No Cancel, and the keyboard stays up while scrolling results.
- [x] [code] A tapped song in Top result plays alone, while the rest plays its shelf.
- [x] [code] A new loader is kept per query for the life of the page.

**Library**
- [x] [code] No search or filter within the library.
- [x] [code] No unpin: nothing passes `pinned = false`, so a pin is permanent on the
  phone. The menu offers Pin on items already pinned.
- [x] [code] Folder chips show, but folders can't be created, deleted or filled (see
  section 5).
- [x] [code] Sort is a cycling chip with no list of choices.
- [x] [code] Grid and list choices aren't kept after the app closes.
- [x] [code] Folder chips use an emoji where the app uses Lucide icons.

**Album, playlist, podcast, artist**
- [x] [code] The header's download button removes every download on one tap.
- [x] [code] Playlist: "Add" just opens Search with no way back or target. "Delete"
  has no confirmation. There's no Edit, Sort or Rename.
  **Add, Rename and Edit are done. Sort is a view order only; reordering on YouTube is not done.**
- [x] [code] `playingHere` matches by the origin's name, so two lists with the same
  title both show as playing.
- [x] [code] Podcast episodes don't mark what's playing or show progress.
- [x] [code] Artist: no menu, the "About" text is cut at 8 lines with no way to expand,
  and "From Wikipedia" isn't a link.
- [x] [code] No "Save to library" on albums and playlists.

**Now Playing**
- [x] [code] Art swipe doesn't follow the finger or show the next cover; it's an
  invisible 120 px threshold (`Player.kt:243`). Use a pager.
- [x] [code] Full art style has no swipe, though its comment says the space is kept
  for one (`Player.kt:236`). Lyrics-first has none either.
- [x] [code] No devices or output button, though the plan and mini player layout have one.
- [x] [code] Speed shows no active state when it isn't 1×, and the queue sheet's Timer
  button reads its state once instead of following it.
- [x] [code] The elapsed and remaining times can't be toggled.
- [x] [code] A tap on the artist name opens only the first artist.

**Lyrics**
- [x] [code] Auto-scroll fights the user: any line change scrolls back
  (`Lyrics.kt:167`). Pause it while the user scrolls, with a "back to the song" button.
- [x] [code] No play or seek controls, though the plan has them, and no swipe down.
- [x] [code] "No lyrics" flashes while loading in the panel (`Lyrics.kt:103`).
- [x] [code] Failed lyrics have no retry (the desktop has `RetryLyrics`).
- [x] [code] The share button changes meaning with no label; choosing mode is only a
  black icon.

**Menus and sheets**
- [x] [code] Create > Playlist opens "Add to playlist" with no songs. Tapping an existing
  playlist says "Added" with nothing added (`Menus.kt:262`). It should ask for a name.
- [x] [code] A new playlist isn't opened after it's made.
- [x] [code] Sheet state is rebuilt when the target changes kind, so sheets jump
  (`Menus.kt:68`).
- [x] [code] Sleep timer: the remaining time doesn't tick, and the sheet needs reopening.

**Settings**
- [x] [code] No search, and related items are split: Equalizer sits under Playback, the
  cache under Downloads.
- [x] [code] Add another account restarts the core before sign-in, leaving a blank
  account if you back out.
- [x] [code] The account switch has no progress, and playback state during it is unclear.
- [x] [code] "Check for updates" gives no result until it finishes, and no automatic
  check or badge.
- [x] [code] No logs or problem report, no licences, no reset to defaults.
- [x] [code] Sign-in: no loading state or error, and "Signed in (21 cookies kept)" is
  developer text.

**Stats and recap**
- [x] [code] A period change wipes the page.
- [x] [code] Top artists and albums rows do nothing when they have no id, with no hint.
- [x] [code] Recap shares text, though the plan says images.
- [x] [code] Recap is built from the last 365 days, not the calendar year it names.

**Downloads and local files**
- [x] [code] No storage used, no pause, cancel or retry, and no grouping by album.
- [x] [code] Play uses only finished songs, but rows offer unfinished ones.
- [x] [code] Refused permission only ever shows the same retry, with no route to
  system settings.
- [x] [code] No progress notification, though the plan has one.

**Listen Together**
- [x] [code] The lobby shows Start, Join and Server settings all at once; the server
  should be hidden until needed.
- [x] [code] Room settings items have no explanations (Lock, Approve, Duplicates).
- [x] [code] "Accept all" and "Remove" give no undo; "Undo edit" appears with no timer.
- [x] [code] Leaving is a panel inside the scroll, which can open off-screen. Use a dialog.
- [x] [code] Controls without permission are only dimmed and still tappable.
- [x] [code] Invite shares a server address and PIN as plain text with no link, though
  the join text mentions pasting a link.

**Widgets, notification, car**
- [x] [try] Widget play may not start the service: only `MainActivity` starts it, so
  after the app is closed the button could change state with no sound.
- [x] [code] Recent widget refetches history on tap and uses an index, so it can play
  the wrong song (`RecentWidget.kt:78`). Pass the song id.
- [x] [code] Recent widget has no refresh and doesn't show art. Player widget has one
  layout for all sizes.
- [x] [code] Widgets are always dark and ignore the theme.
- [x] [code] Notification: only Like and Shuffle are added (`PlaybackService.kt:150`),
  so there's no close button, though the plan has one. Shuffle always uses the "on" icon
  whatever its state.
- [x] [code] Car: nothing explains Auto's "Unknown sources" on first connection, though
  the plan has it. Android Auto on a real head unit is still untested.

## 5. Missing from the Android app

Compared against the desktop's actions and the plan in `docs/android.md`.

**On the desktop, missing here**
- [x] **Copy links, including Spotify, Apple Music and Deezer.** [code] The phone has
  only a share sheet with the YouTube Music link (`Actions.kt:121`). The lookup is
  already ported (`Services.find`, `Services.kt:74`) and nothing calls it. Add a Share
  submenu to song, album and artist menus and the player: Copy link, Spotify, Apple
  Music, Deezer, Share via…
- [x] **Block a song, artist or album, with a list to undo it.** [code] The core supports
  it, but `forCore` never sends any (`Settings.kt:64`) and there's no menu entry.
- [x] **Folders: create, delete, move items in and out.** [code] `createFolder` and
  `deleteFolder` exist in `Api.kt` with no screen using them.
- [x] **Unpin.** [code]
- [x] **Save to and remove from library.** [code]
  **Albums and playlists are done; podcasts are not, as the core has no such surface.**
- [x] **Multi-select:** long press, then bulk queue, like, add to playlist, download. [code]
- [x] **Like all** on a list. [code]
- [x] **Search suggestions and recent searches.** [code]
- [x] **Volume level and boost.** [code]
- [x] **Output device choice.** [code]
- [x] **Visualizer.** [code]
- [x] **Switch YouTube channel** (brand accounts), and refresh channels. [code]
- [x] **Continue from YouTube Music** setting. [code]
- [x] **Retry lyrics.** [code]
- [x] **Stats lookup** for any song or artist. [code]
- [x] **What's new** after an update. [code]
- [x] **Problem report, and logs.** [code]
- [x] **Update yt-dlp** from the app. [code]
  **Not applicable: yt-dlp is bundled and updates with the app.**
- [x] **Reduce motion, and reset settings.** [code]
- [x] **Remaining time toggle** on the seek bar. [code]
- [x] **Music videos:** the desktop plays them; the plan makes them a non-goal on the
  phone. Keep or reverse that, and at least add the "show music videos" filter. [code]

**In the plan, not built**
- [x] Clear queue and played songs in the queue. [code]
- [x] Predictive back. [code]
- [ ] Tablet and foldable layouts. [code]
  **Landscape is adapted; no tablet side rail yet.**
- [x] Download progress as a Live Update. [code]
- [x] Offline Home from the response cache. [code]
- [x] Import from an export file, not only a link or pasted lines. [code]
- [x] Credentials import from the desktop (file or QR), as the sign-in fallback. [code]
  **File import is done; QR is not, as the desktop shows none.**
- [x] Playlist widget. [code]
- [x] Dynamic shortcuts for the last three playlists. [code]
- [ ] Credits on Now Playing. [code]
  **Not possible yet: the core provides no credits.**
- [x] Listen Together: joining from a link, per-listener avatars on queue songs. [try]
- [ ] Google Cast and Android Automotive polish (already marked later). [code]
  **Cast is not started, by plan; Automotive had nothing plainly broken.**

## 6. Efficiency and diagnostics

- [ ] **Data use.** [device] 260 MB for 54 minutes of audio since the last charge, about
  640 kbps, against about 104 MB at 256 kbps. The mobile radio is 55.7 of the app's
  116 mAh. The suspect is prefetching and whole-track caching (`core/internal/api/cache.go`),
  not proven. Log bytes per stream, then limit prefetching on mobile data.
  **Partly: bytes are logged and prefetching is limited on metered networks, but the cause of 640 kbps is still unproven. Read the new logs after real use.**
- [x] **Queue rebuilt on the main thread.** [device + code] 125 session updates in 91 s,
  with a 72-song queue rebuilt and JSON-encoded each time (`CorePlayer.kt:72,181`). Refresh
  only on real changes and reuse the items.
- [x] **Notification buttons reset on every update.** [code] Add `distinctUntilChanged`
  (`PlaybackService.kt:105`). 28 reposts in 11 minutes, up to 7 a second.
- [x] **Buffered position is never reported.** [device] It equalled the position in all
  125 samples. Set it from the engine (`CorePlayer.getState`).
- [x] **Go logs all show as errors.** [device] `core/mobile/mobile.go:98` writes to
  stderr, so `level=INFO` lines are tagged `E GoLog`. Map the levels.
- [x] **Keep a log file.** [device] Logcat kept only about 7 minutes of the app's lines.
  A small rotating file, exported from Settings, would cover this.
- [x] **No watch-time report on pause.** [device] The last 17 s before the 13:54:36 pause
  went unreported. Send one on pause.
