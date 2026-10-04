# Changelog

Every release of Youtube Music Spotified Native, newest first.

## 0.3.2 — 2026-10-04

### New

- Browse all is the search page with nothing typed, as in the Electron app: tiles for Discover, Charts, New releases and every mood and genre, each with a cover from its page tipped into the corner

## 0.3.1 — 2026-10-04

### Fixed

- The mini player could not be resized: a press on its edge moved it instead

## 0.3.0 — 2026-10-04

### New

- The mini player is a window of its own, kept on top, as in the Electron app: a strip, the cover with controls under the pointer, cover beside controls, or tall with the queue or lyrics, by its size
- Themes: follow the system, light, dark, or any JSON theme in the themes folder; six well-known palettes come with it, and the mini player wears the same one
- Browse all, from the button in the search field: new albums, top songs and trending with their covers, then moods and genres
- A better account menu, with who is signed in and a way to sign in or out
- A better Listen Together page

### Fixed

- Songs that arrive as Opus showed no length and could not be sought; crossfade never started on them either
- A narrow window with the lyrics or queue open squeezed the page; the sidebar now steps aside first
- The search field in the library is a proper field
- The core's warnings are kept in every log

## 0.2.1 — 2026-10-04

### New

- Listen Together: a shared queue with friends through a relay you choose, joined with a PIN
- The app draws its own title bar, with the search field in it; the system's can be brought back in Settings
- Panels are rounded cards with a gap between them, as in the Electron app
- The queue shows what has played, takes a click to jump, and can be reordered by dragging; rows can be removed or moved from a right-click menu
- Lyrics sit on the cover's colour with every line bold, and the full view has its own transport
- Updates download in the background and install on restart, and this page says what changed
- The accent is YouTube's red, as in the Electron app
- Crossfade skips the silence at the end of one song and the start of the next

### Fixed

- A crash half a minute after starting an installed copy, when it first looked for an update
- The pointer becomes a hand over anything that can be clicked
- Controls answer the pointer with a fade and a round highlight instead of a jump in colour
- The installer's shortcuts no longer share a name with the Electron app's

## 0.1.0 — 2026-10-03

### New

- The first native build: browsing, search, the library with pins and folders, playback with gapless and crossfade, an equalizer, lyrics, radios, podcasts, listening stats, a visualizer and a mini player
