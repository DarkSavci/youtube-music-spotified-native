# Youtube Music Spotified, macOS and Linux

A plan for the desktop app on macOS and Linux: the same Rust and egui window,
the same Go core, the same release. `PLAN.md` section 1 left these out so
"they can follow"; this is how they follow.

Status, 2026-10-30: built on the `mac-linux` branch. **Linux has been run; macOS
has only been type-checked**, because no Mac can be reached from where this was
written. Section 2 says what has been checked, and section 8 what a Mac must
still confirm. Sections 3 and 4 are the plan as it was written; section 9 says
where the work departed from it.

## 1. Goals and non-goals

Goals, in priority order:

1. **Plays properly.** Browse, sign-in, playback, crossfade, EQ, stats, Listen
   Together and the Winamp skins work as they do on Windows.
2. **Lives on the desktop properly.** Media keys and the system's now-playing
   card, a tray or menu bar icon, music that plays on with the window closed,
   starting at sign-in.
3. **Installs and updates itself,** from the same GitHub release as Windows
   and Android, from one `v` tag.
4. **MilkDrop and music videos,** after the first release.

Not attempted:

- Porting the core, or replacing egui or glow. Both are portable already.
- A second interface: no native macOS toolbar and no GTK or Qt look. The
  window is the app's own on every system.
- The taskbar thumbnail buttons, which have no counterpart on either system.
- The Mac App Store, Apple's Developer Program and notarisation, Flathub, or
  packages in a distribution's own archive.
- Intel Macs and ARM Linux in the first release. Both are cheap to add later:
  yt-dlp and deno publish builds for them, and the core already builds.

## 2. What has been checked

On Ubuntu 22.04 under WSLg, in a build of this tree, with the sound muted:

| What | Result |
| --- | --- |
| `cargo fmt`, clippy with `-D warnings`, `cargo test`, `go test` | Pass on Linux. Rust passes on Windows too (`go test` was not rerun, as the core did not change) |
| A real song, signed out | Plays, in step, through PulseAudio |
| MPRIS, by `playerctl` | Title, pause, play, next and seek all work; position is kept current |
| Window hidden, in a tray | With a stand-in tray host: media controls still work, a song ending starts the next, and a second launch brings the window back |
| No tray host | The tray is reported absent, and the app quits on close |
| No session bus | No media controls and no crash. souvlaki's thread panics without one, which a release build would die of, so the bus is asked after first |
| Fallback fonts | Japanese, Korean, Chinese, Arabic, Hebrew, Thai and Hindi all drawn, from Noto |
| Music video | 720p decoded by OpenH264, in step with the song, with no decode errors |
| MilkDrop | Draws presets from `libprojectM-4.so`; closes without error |
| Skins | A classic skin is worn; one with a `region.txt` has its window cut to shape |
| Notification, file manager | Right calls made, and a click heard, against stand-in services |
| AppImage and `.tar.gz` | Built by `scripts/package-linux.sh`, and run. Needs glibc 2.35. AppImage 105 MB |
| Self-update, whole | Two real AppImages, 0.6.0 and 0.6.1, with the feed pointed at a local server playing GitHub: quitting installs it and stays closed; a click on the notification restarts into 0.6.1. Not yet from GitHub itself |
| KDE Plasma 5.24, real, on X11 | The tray icon is drawn and registered. Closing the window hides it, music plays on, the next song starts, and a second launch brings it back. MPRIS seen |
| GNOME Shell 42, real, on X11 | No tray host is found, so closing the window quits the app. Plays, MPRIS seen. Both desktops were run on Xvfb, so a person's clicks on the tray menu have not been tried |
| Pure Wayland | Opens and draws. No tray, by design |
| No display at all | Exits with a message |
| Chrome's sign-in behaviour | Against a local page: the cookie's name reaches the file within 40 s, SIGTERM saves and ends it, DevTools reads the value back |

For macOS: `cargo clippy --all-targets --target aarch64-apple-darwin` passes with
`-D warnings`, with Apple's C compiler stubbed out so that the libraries written
in C were not built. So the Rust is known to be well typed against AppKit and
nothing more. **It has not been linked or run.**

Not checked anywhere:

- **A real Google sign-in**, on either system. It needs an account. The flow it
  rests on was tested against a local page only.
- **By hand, on a desktop.** Clicks on the tray icon and its menu, physical media
  keys, and **Add a skin**, whose file dialog wants a portal WSL does not have.
- **`.github/workflows`.** Neither the new CI matrix nor the release jobs have run.
- **Output devices.** cpal's ALSA host may list raw `hw:` devices on a real
  machine. The picker may want a filter.
- **Performance** against `PLAN.md` section 2. WSLg draws in software, so its
  numbers say nothing. A video at 720p is decoded on the processor here.

## 3. Decisions

**Linux before macOS.** It can be run on this machine through WSLg, with a
window and sound, while macOS has only CI runners here. Nearly all the
non-Windows work is shared between them.

**On Linux the app asks for X11, and takes Wayland only without it.** winit
documents `set_visible` and `set_outer_position` as unsupported on Wayland,
and `outer_position` as always failing. The app depends on all three:
close-to-tray is `Visible(false)` (`app.rs:457`), and the flyout
(`flyout.rs:47`) and the mini player (`mini.rs:80`) place themselves. Under
XWayland these should all work. The cost is some blur at fractional scaling,
and a desktop without XWayland falls back. On pure Wayland the app still runs,
with close-to-tray off and no flyout. This is set with `event_loop_builder` in
`NativeOptions`.

**No GTK on Linux.** `ksni` for the tray, `use_zbus` for media controls, the
desktop portal for the file dialog. GTK would be a large dependency to bundle
for one menu.

**TLS stays native-tls.** Security.framework on macOS. On Linux it is OpenSSL,
built in with the `vendored` feature, because distributions disagree on
libssl's version. `openssl-probe` is already in the lock file to find the
certificates; this must be tried on Fedora and Arch as well as Ubuntu.

**Linux ships as an AppImage and a `.tar.gz`.** The AppImage is one file, runs
on most distributions and can replace itself. It is built on Ubuntu 22.04,
which sets the floor at glibc 2.35. libasound stays out of it, so the system's
PipeWire and PulseAudio plugins keep working. `.deb`, `.rpm` and Flatpak can
come later if asked for.

**macOS ships as a `.dmg`, plus a `.tar.gz` of the `.app` for the updater.**
Apple Silicon only, at first.

**Unix scripts are new `.sh` files beside the `.ps1` ones.** `pwsh` is on
GitHub's runners but rarely on a Mac or Linux developer's machine, and the
packaging has nothing in common with Inno Setup anyway.

**The first release on each is marked beta.** Its files ship in the normal
release, and the README says beta until each system has had a real pass.

## 4. What has to change

### Build

- `crates/app/Cargo.toml`: eframe has `default-features = false`, which drops
  its `x11` and `wayland` features. Linux needs them back, or winit has no
  windowing backend. Add them under `cfg(target_os = "linux")`.
- The same file: `winit`, `glutin` and `glutin-winit` are Windows-only and
  name only `wgl` and `egl`. When MilkDrop opens up, they need `glx`, `x11` and
  `wayland` on Linux, and `cgl` on macOS.
- `souvlaki`: `default-features = false, features = ["use_zbus"]`.
- `tray-icon`: `features = ["ksni"]` on Linux.
- `native-tls` with `vendored`, on Linux only.
- CI: `check.yml` runs `clippy -D warnings`, and the stubs will fail it. For
  example `markup` and `escaped` in `notify.rs` are only called on Windows, so
  they are dead code elsewhere. Windows-only helpers move inside their `win`
  modules.
- Tests: some 15 lines of Rust hold `C:` paths and some tests assume Windows;
  they get gated or generalised. `report/tests.rs` uses `system_tar`.

### Shared by both systems

| Place | Today | Change |
| --- | --- | --- |
| `sidecar.rs:19` | `vendor/yt-dlp/yt-dlp.exe`, `deno.exe` | Names per system. `CORE_BINARY` is already right |
| `sidecar.rs:92` | Log line names `fetch-tools.ps1` | Name the right script |
| `sidecar.rs` `locate` | Looks beside the executable | Also `../Resources` in a macOS bundle, where signing requires data to be |
| `resolver.rs:20` | `yt-dlp_win.zip`, `yt-dlp.exe`, and `tar.exe` from `SystemRoot` | The right archive per system. GNU tar cannot read a zip, so Linux unpacks with `skin/zip.rs`, which must learn to restore the executable bit. macOS uses `/usr/bin/tar`, which handles the symlinks a PyInstaller build may carry |
| `report.rs:143` | Writes the diagnostics zip with `tar.exe` | `skin::zip::write`, on every system |
| `report.rs:302` | `cmd /c ver` | `sw_vers`; `/etc/os-release` |
| `signin.rs:100` | Chrome and Edge at Windows paths | macOS `.app` paths, and on Linux `google-chrome`, `chromium`, `microsoft-edge`, `brave-browser` on `PATH` |
| `signin.rs:172` | PowerShell watches the window title | See below |
| `shell.rs:155` `reveal` | `explorer /select,` | `open -R`; on Linux the file manager's D-Bus interface, or `xdg-open` on the folder |
| `shell.rs:131` `memory` | Stub | `/proc/meminfo` on Linux, `sysctl` on macOS, with no new dependency |
| `shell.rs:92` `display_at` | Stub | Needed for the flyout on macOS. Linux reads the work area from X11 |
| `pick.rs` | Stub: Add a skin silently offers nothing | `rfd`, with its portal backend on Linux; or `NSOpenPanel` and a portal call by hand |
| `notify.rs` | Stub | macOS keeps to the in-window toast at first, as `UNUserNotificationCenter` may not work for an ad-hoc signed app (to be tried). Linux uses `org.freedesktop.Notifications` over zbus |
| `region.rs` | Stub | `LANG` on Linux, `AppleLocale` on macOS |
| `autostart.rs` | `reg.exe`, called on every system | Linux: `~/.config/autostart/*.desktop`. macOS: a LaunchAgent plist. Same `--hidden`, same `refresh` |
| `fonts.rs:86` | Windows' files only; elsewhere an empty box for anything Inter lacks | See below |
| `tray.rs:17`, `media_keys.rs` | Written for Windows' limits | Mostly work. See the sections below |
| `update.rs` | `-setup.exe`, `unins000.exe`, Inno's switches | See below |
| Wording | "Starting with Windows", `%APPDATA%` in the README, and so on | Say the right system. Check for "Ctrl" in shortcut labels on macOS |

**Fonts.** This is what stops a Japanese, Korean, Arabic or Thai title from
being empty boxes, so it is not optional for the first release. Each system
gets its own table in the same shape as `FACES`. On macOS, `Hiragino`,
`AppleSDGothicNeo`, `Thonburi` and `Kohinoor` from `/System/Library/Fonts`,
each to be confirmed on a real Mac; Apple's emoji are bitmaps that egui cannot
draw, so emoji stay with egui's own. On Linux the files move about, so
`fc-match` finds the faces, on a background thread so that the first frame's
budget holds. A font that is not there is passed over, as now.

**Sign-in.** This is the part most likely to surprise. The flow waits for the
browser process to exit (`signin.rs:148`), and on macOS closing Chrome's last
window does not end the process. It would wait forever. And the watcher that
closes the window on landing is PowerShell. So on both systems:

- Watch the throwaway profile's `Default/Network/Cookies` for the plain bytes
  `LOGIN_INFO`. As far as is known, cookie names are not encrypted, Chrome
  writes the file every 30 seconds or so, and SIGTERM makes it save and exit
  cleanly; none of the three has been tried here. When the bytes appear, send
  SIGTERM.
- Show "I've signed in" in the app as well, doing the same, for when the
  watch misses.
- Pass `--use-mock-keychain` on macOS and `--password-store=basic` on Linux,
  to both steps, so that a throwaway profile raises no keychain prompt.
- Try a snap Chromium, whose confinement may refuse the profile folder. If so,
  say so in the error rather than failing obscurely.

### Linux only

- **Tray:** `Tray::new` will succeed with `ksni` even when nothing is there to
  show the icon, as on plain GNOME. Close-to-tray would then leave a running
  app with no way back but launching it again. Before building the tray, ask
  the session bus whether `org.kde.StatusNotifierWatcher` has an owner; if not,
  `Tray::new` returns `None`, and the existing behaviour (quit when the window
  closes) takes over.
- **Tray clicks:** `ksni` reports `Rect::default()`, so `Flyout(Area)` would
  put the flyout at the screen's corner. A left click shows the window on
  Linux; the menu has everything else.
- **AppImage paths:** `current_exe()` inside an AppImage is a temporary mount.
  Autostart, the updater and the desktop entry use `$APPIMAGE` instead.
- **Desktop entry:** `StartupWMClass` must equal `APP_ID`, or the dock shows a
  second, generic icon. Icons in `hicolor` from `scripts/make-icon.py`.
- **Output devices:** cpal's ALSA host may list many raw `hw:` and `plughw:`
  entries. The output picker gets a filter once seen on a real system.

### macOS only

- **The bundle:** `Info.plist`, an `.icns`, the app in `Contents/MacOS`, the
  core and tools where signing allows. `update::installed` and
  `sidecar::packaged` look for this shape.
- **Signing:** Apple Silicon refuses to run unsigned code, so
  `package-macos.sh` ad-hoc signs everything inside the bundle, then the bundle
  itself, with `codesign --sign -`. This needs no account. There is no
  notarisation, no hardened runtime, and no entitlements for deno.
- **First launch:** the person approves the app once in System Settings. The
  updater's download carries no quarantine flag, so an update should not ask
  again, but that has to be seen on the Mac. If it does ask, the updater tells
  the person what to do before restarting.
- **Notifications:** Apple's notification interface may not work for an ad-hoc
  signed app, and this is the permanent arrangement, not a first step. If it
  fails on the Mac, the in-window toast is the answer for good.
- **Closing the window:** on a Mac the Dock icon stays. A click on it while
  the window is hidden must bring it back, and winit 0.30 has no reopen event.
  Listen for `NSApplicationDidBecomeActiveNotification`, or minimise instead of
  hiding if that proves unreliable.
- **Quitting:** Cmd+Q must go through the same path as the tray's Quit, so the
  core stops and the waiting update installs.
- **The menu bar icon:** `tray-icon` handles it, and it needs a template image
  to suit light and dark bars.
- **Translocation:** an unsigned app opened from Downloads runs from a random
  read-only path. The updater detects this and asks the person to move the app
  to Applications.

### Updater

`update.rs` keeps its feed, checksums and comparison. Installation becomes per
system, and neither Unix system needs a helper, because a running file can be
replaced:

- `offer_in` looks for the suffix of this system: `-setup.exe`,
  `-macos-arm64.tar.gz` or `-linux-x86_64.AppImage`. Older Windows copies look
  only for `-setup.exe`, so the new assets do not disturb them.
- Linux: write beside `$APPIMAGE`, set it executable, rename over it, and start
  it again.
- macOS: unpack beside the bundle, swap the two by rename, and start it again.
- A `.tar.gz` on Linux reports `Unavailable`, as the Windows portable folder
  does.

`resolver.rs` needs one thing checked on a Mac: that a yt-dlp it stages runs
without being stopped by Gatekeeper. It should, as `ureq` sets no quarantine
flag, but that has to be seen.

### After the first release

- **MilkDrop.** `child.rs` is winit and glutin, so it mostly moves as it is.
  `library.rs` swaps `LoadLibraryW` for `dlopen`, `LIBRARY` gets per-system
  names (`libprojectM-4.so`, `.dylib`), and the `-static` flags leave the build
  script. macOS has OpenGL up to 4.1, enough for projectM's 3.3, though Apple
  has deprecated it. The child process must set an accessory activation policy,
  or it gets a Dock icon of its own.
- **Music videos.** `mp4.rs` and `sync.rs` are portable. `worker.rs` and
  `fetch.rs` are gated but mostly are not Windows code, so a decoder seam goes
  between them and `decode.rs`. macOS uses VideoToolbox, on the graphics chip.
  Linux uses the `openh264` crate, in software. Building Cisco's source does not
  carry the patent licence its prebuilt binary does, so the alternative is
  loading the system's `libavcodec`. Until then the cover stays where the
  picture would be, as now.
- **Shaped modern skins.** `shape_window` is a stub that reports success, so a
  `.wal` skin draws as a rectangle. XShape on X11, and a transparent window on
  macOS. It is the last in line, and a rectangle is an acceptable first answer.

## 5. What each system has

| Feature | macOS (unrun) | Linux (X11 or XWayland) | Linux (pure Wayland) |
| --- | --- | --- | --- |
| Everything inside the window | Yes | Yes | Yes |
| Sign-in | Yes | Yes | Yes |
| Media keys and now-playing | Control Centre | MPRIS | MPRIS |
| Tray or menu bar icon | Yes | Where the desktop has a host | Where the desktop has a host |
| Flyout | Yes | No, a click shows the window | No |
| Plays on with the window closed | Yes, minimised | Yes, given a tray | No, the app quits on close |
| Mini player and classic skins | Yes | Yes | Yes, without remembering its place |
| Skins with a shape | Rectangular | Cut to shape | Rectangular |
| Custom title bar | System's | System's | System's |
| Taskbar thumbnail buttons | None | None | None |
| Update notification | In the window | Yes | Yes |
| Self-update | Yes, once in Applications | AppImage only | AppImage only |
| Start at sign-in | Yes | Yes | Yes |
| MilkDrop | Yes | Yes | Yes |
| Music videos | Yes, 720p in software | Yes, 720p in software | Yes, 720p in software |

## 6. The largest unknowns, answered

1. **Does the app run with its window hidden?** On X11, yes: eframe paints an
   invisible window directly, for Windows' sake, and that serves here too. A song
   ending starts the next. On a Mac the window is minimised rather than hidden,
   and this is the first thing to try there.
2. **Does Google accept the sign-in?** Not yet known. See section 8.
3. **Does glow run acceptably?** It runs, in software, under WSLg. Not yet known on
   real hardware.
4. **Does `--demo --screenshot` run in CI?** Under `xvfb`, yes, as tried here;
   `check.yml` does it on every push. A screenshot on macOS is left out for now.

## 7. Order of work, as it went

Steps 0 to 5 of the plan are written, with Linux run and macOS type-checked. What
remains:

1. Push the branch, and let `check.yml` run on all three systems. This is the first
   time the macOS code is linked and its tests run.
2. Section 8, on the Mac.
3. Trying the tray menu, media keys and **Add a skin** by hand on a Linux desktop.
4. Bump to `0.7.0-beta.1` with a changelog section, tag it, and try each updater
   against it by hand. Neither updater offers a beta by itself.

## 8. What a Mac must confirm

Settled, 2026-10-30: a Mac is available, and there will be no Apple Developer
Program. The app is ad-hoc signed and not notarised, so the person approves it in
System Settings under Privacy & Security on first launch.

`sh scripts/package-macos.sh` needs Xcode's command line tools, Rust, Go, CMake and
Ninja. Then, in the order that would change the most:

1. **It opens.** The DMG mounts and the app launches once approved, with its core
   running. If the core will not start, look at quarantine first
   (`sidecar::clear_quarantine`).
2. **Plays on with the window closed,** and a click on the Dock's icon brings it
   back. If a minimised window stops drawing or advancing, this is the largest
   piece of work left.
3. **Sign-in.** Google accepts it, the window closes by itself within about 40
   seconds of landing, and no keychain prompt appears. If the window is closed by
   hand, Chrome stays running, and the app waits up to 15 minutes before giving up;
   Cmd+Q ends the wait.
4. **Media keys and Control Centre:** title, cover, play, pause, next, seek.
5. **The menu bar icon and its flyout:** in the right place on a Retina screen and
   on a second display. `shell::mac::display_at` is the part most likely to be
   wrong.
6. **Cmd+Q** stops the core and installs a waiting update.
7. **Fonts.** A Japanese, Korean, Arabic and Thai title each draw, not boxes. The
   table in `fonts.rs` is from memory, and a file or a face that is not there is
   passed over without a word.
8. **MilkDrop** opens without a second Dock icon, and takes the keys.
9. **Self-update,** from Applications: no second approval is asked for, and yt-dlp's
   own update runs. From Downloads it should say it does not update itself.
10. **Start at login** starts it hidden.

Still open: whether to add `.deb` or Flatpak. Not until someone asks.

## 9. Where the work departed from the plan

- **MilkDrop and video were done in the first pass,** not left for after a release,
  and shaped skins on X11 with them.
- **OpenH264 on the Mac as well,** not VideoToolbox: one decoder, which could be
  tested here, rather than one that could not. It costs a Mac its hardware decoding
  and caps video at 720p, so VideoToolbox is worth revisiting. OpenH264 is built from
  source, which Cisco's patent licence does not cover. `NOTICE.md` says so; it is
  for the owner to judge for an unofficial, unpaid project.
- **A Mac minimises instead of hiding.** winit 0.30 does not report a click on the
  Dock's icon, and a minimised window is restored by the Dock itself.
- **No "I've signed in" button.** Watching the cookie file was enough on Linux, where
  closing the window also works.
- **The problem report and yt-dlp's update use the zip code in `skin/zip.rs`** on
  every system, Windows included, rather than `tar.exe`.
- **A macOS shaped skin stays a rectangle,** as the plan allowed.
- **The cookie file is `Default/Cookies` on current Chrome,** not
  `Default/Network/Cookies` as first assumed; both are looked at.
