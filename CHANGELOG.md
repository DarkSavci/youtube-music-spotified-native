# Changelog

Every release of Youtube Music Spotified Native, newest first.

## 0.4.6 — 2026-10-05

### New

- Blocking an album blocks every song on it, so its songs are skipped on Home and in search results too, where a song does not say what album it is on
- In a Listen Together room the player bar names the album, and the song, its artists and its album lead to their pages and have their menus

### Fixed

- Pressing play on a paused song that was blocked played it; it now moves on to the next song
- A blocked song played by itself was cut off once its radio arrived
- A queue that ended in blocked songs stopped, where it now carries on with a radio
- A blocked artist's own mix failed to start
- Picking a blocked song at the end of a list played it
- Removing the song that plays could land on a blocked one
- A gap in the middle of the "Show music videos" description

## 0.4.5 — 2026-10-04

### New

- Block a song, an artist or an album from its right-click menu, or from the "…" menu on an album's or an artist's page: the queue steps over what is blocked, radios leave it out, and it is shown dimmed. Everything blocked is listed in Settings, each with a button to unblock it
- The player bar names the album after the artists, as a link to it
- A right click on the playing song's title, on one of its artists or on its album brings that one's menu

## 0.4.4 — 2026-10-04

### New

- A new equalizer, opened from the player bar, the queue or Settings: the real response drawn as a curve with the sliders on it, a preamp, "Prevent clipping", sixteen presets and your own saved ones. A band set to +6 now measures +6, and moving a slider no longer clicks

### Fixed

- The visualizer ran about a quarter of a second ahead of the music: it drew sound as it was queued, not as it was heard

## 0.4.3 — 2026-10-04

### New

- Music videos: a song that has one can be switched to its video, shown above the page, in the full-screen player and in the mini player, kept in step with the sound at any speed. "Show music videos" in Settings decides whether videos appear among songs, and a Listen Together room can follow each other's choice

### Fixed

- The cover in the mini player turned into the placeholder while the window was being resized

## 0.4.2 — 2026-10-04

### New

- Menus worth right-clicking: an icon on every entry, a highlight under the pointer, submenus, and the arrow keys
- Every artist of a song is a link of its own
- Text can be selected and copied in What's new, About, lyrics and error messages
- The mini player has the speed button; Listen Together offers "Retry playback" and tells the room when you close the app
- Your listening shows time listened; the sidebar's grid and rail play from the cover; the account menu shows profile pictures
- A downloaded update installs when you quit, and Windows tells you about it
- Each channel of an account keeps its own history, and recent searches are kept per account
- A dark tray menu, and log files named by date

### Fixed

- Titles with Japanese, Chinese, Korean, Arabic and other scripts, symbols or emoji showed as boxes
- An artist joined to another by "&" was sometimes dropped from a song

## 0.4.1 — 2026-10-04

### New

- Move from the old app: every account's sign-in, your listening history, folders and pins, cached songs and preferences come across from the Electron app, from Settings or the offer on first start. It only reads the old app, never double-counts a play, and can be run again to pick up what is new

## 0.4.0 — 2026-10-04

### New

- The look of the Electron app: its neutral greys are the dark theme (the blue-tinted one is kept as the "midnight" theme), the sidebar is Your Library alone and collapses to a rail of covers or widens over the page, with a grid view and the Recently added and Creator sorts
- The top bar has a Home button beside the search field, with What's new, Listen Together, Settings and the account at the right
- Home has mood chips, cards that fill the row, and more shelves as you scroll
- Albums, playlists and artists have the big header, a Play button with a "…" menu (queue, play next, add all to a playlist, share), and for artists Shuffle, Radio and Follow; hearts and "…" always show on rows
- Search finds Videos, Podcasts and Episodes too, shows a Songs table, and lets a recent search be removed; a song card starts a radio from that song
- An album's About, an artist's whole song list (Popular, Newest, By album), and long playlists that load as you scroll
- Playback speed from 0.5× to 3× with the pitch kept, volume boost to 200%, the mouse wheel on the volume, a Quiet / Normal / Loud level, and time remaining on a click
- A full-screen player on F
- Settings for autoplay, gapless playback, resuming on launch, sending listening to YouTube, the cache size, reduced motion, continuing from YouTube Music, and resetting preferences
- The Electron app's keyboard shortcuts
- Your listening: summary tiles, four periods, a lookup with plays per month, On repeat and Top albums
- Listen Together: saved servers with a connection test, searching inside the room, requests, vote to skip, ready check, roles, room settings, join approval, history and activity, saving the history as a playlist, and choosing the next leader
- Several Google accounts, switched from the account menu
- A flyout from the tray icon, buttons on the taskbar thumbnail, a daily yt-dlp update, and a problem report that can be saved from Settings
- What's new opens over the page, and an update says so in a toast

### Changed

- Crossfade starts at 6 seconds on a new profile
- Left and Right seek 5 seconds

### Fixed

- The top result of a search sometimes showed the cover of one of its songs

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
