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
cargo run                  # the app
```

`cargo run -- --demo` runs on recorded responses in a throwaway profile, with
no account. `--screenshot <file.png>` saves the window and exits.
`--profile <dir>` keeps every file under a directory of your choosing.
`--open <what>` starts somewhere other than Home and can be given more than
once: `album:<id>`, `artist:<id>`, `playlist:<id>`, `podcast:<id>`,
`search:<query>`, `explore`, `moods`, `history`, `stats`, `settings`, `mini`,
or `track:<id>` to play one.

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
