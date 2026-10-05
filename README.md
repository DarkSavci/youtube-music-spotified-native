# Youtube Music Spotified, native

A native YouTube Music client: Rust and egui, no browser engine. A rewrite of
[Youtube Music Spotified](https://github.com/DarkSavci/youtube-music-spotified) in the manner of
[Spotifast](https://github.com/crmne/spotifast).

It browses, searches and plays; keeps a library with pins and folders, a
queue, likes and playlists; shows lyrics, radios, podcasts and listening
stats; has an equalizer, crossfade, a visualizer and a mini player; and
shares a queue with friends through Listen Together.
`PLAN.md` has the design and, in section 9, what is done, what was left out
and what has not been checked by hand; `docs/perf.md` has what has been
measured.

Unofficial personal project. Not affiliated with Spotify or YouTube.

## Build

Requires Rust (GNU toolchain on Windows, with WinLibs MinGW for gcc and
cmake) and Go 1.26+.

```powershell
. scripts/env.ps1          # toolchain on PATH for this shell
scripts/build-core.ps1     # the Go core, to target/core
scripts/fetch-tools.ps1    # yt-dlp and deno, to vendor/
scripts/build-projectm.ps1 # libprojectM, for MilkDrop (optional)
cargo run                  # the app
```

`cargo run -- --demo` runs on recorded responses in a throwaway profile, with
no account. `--screenshot <file.png>` saves the window and exits.
`--profile <dir>` keeps every file under a directory of your choosing.
`--open <what>` starts somewhere other than Home and can be given more than
once: `album:<id>`, `artist:<id>`, `playlist:<id>`, `podcast:<id>`,
`search:<query>`, `explore`, `moods`, `history`, `stats`, `settings`, `mini`,
or `track:<id>` to play one. `skin:classic` opens the mini player in the
built-in Winamp skin, and `skin-add:<file.wsz>` installs one and wears it.

## Winamp skins

The mini player can wear a classic Winamp 2 skin (`.wsz`). Drop the file on
either window, or use **Add a skin** in Settings under Appearance; it is
copied to `%APPDATA%\SpotifiedNative\skins` and worn at once. Settings lists
the skins there, and **No skin** goes back to the app's own mini player.
Skins are to be had at the [Winamp Skin Museum](https://skins.webamp.org).

In a skin, right-click the title bar (or click the logo or **O**) for the
size, from 1x to 4x, the skin and keeping the window on top. **EQ** and
**PL** open the equalizer and the queue under the player, and the shade
button or a double click on a title bar rolls that window up. A click on
the display goes from the spectrum to the oscilloscope to nothing. In the
playlist, Ctrl and Shift select several songs, and the buttons along the
bottom open menus for them. Stop pauses and
rewinds; Eject and the logo at the bottom right bring the main window
forward.

Modern skins (`.wal`, for Winamp 3 and 5) are installed the same way and
drawn as they rest: their main window's layout, with the buttons, sliders
and text that have a standard meaning wired to the player, and the window
cut to the skin's shape. Their scripts are not run, so nothing slides out
or animates, and a part a script would have hidden may show. A simple
skin is whole; an elaborate one is a still of itself. A right click
anywhere in one brings the menu.

## MilkDrop

**Open MilkDrop** in Settings under Appearance (or MilkDrop in a skin's
menu) draws the music with MilkDrop's presets, in a window of its own. No
presets come with the app: **Get presets** fetches the 550 that shipped
with MilkDrop 2 into `%APPDATA%\SpotifiedNative\milkdrop`, and any `.milk`
file put there is used. In the window, Space or N is the next preset and
Backspace or P the one before, L stays on one, F or a double click fills
the screen, and Escape leaves.

The drawing is libprojectM's, a library beside the app
(`libprojectM-4.dll`) and not part of it; without the file there is no
MilkDrop, and nothing else changes.

## Package

```powershell
scripts/package.ps1        # dist/Youtube Music Spotified/ and a zip of it
```

The folder runs from anywhere: the app, the core beside it, and yt-dlp and
deno under `vendor/`.

## Check

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd core; go test ./...
```

## Layout

- `crates/app`: the window. State, actions, views, theme.
- `crates/client`: the HTTP client and models for the core.
- `crates/audio`: the engine: stream source, decoding, resampling, output.
- `core/`: the Go service that talks to YouTube Music, copied from the
  Electron app and changed as `PLAN.md` section 3.2 lists.

## License

[MIT](LICENSE). Third-party notices are in [NOTICE.md](NOTICE.md).
