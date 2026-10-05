# Notices

- **Spotifast** (MIT, Copyright (c) 2026 Carmine Paolino). The palette,
  layout measurements and several view idioms follow its `src/theme.rs` and
  `src/ui/`. The reader of Winamp skins in `crates/app/src/skin/` is its
  `src/skin/`, and the skinned mini player in
  `crates/app/src/views/mini/skinned/` follows its `src/ui/winamp/`. The
  built-in skin, `crates/app/assets/skins/classic.wsz`, is the one drawn for
  it, with the name on its title bars redrawn. MilkDrop's window in
  `crates/app/src/milkdrop/` is arranged as its `src/milkdrop/` is.
  https://github.com/crmne/spotifast
- **libprojectM** (LGPL 2.1, Copyright (c) the projectM team), which draws
  MilkDrop. It is not part of the program: it is `libprojectM-4.dll` beside
  it, built unchanged from its source by `scripts/build-projectm.ps1` and
  loaded when MilkDrop's window is opened, so it can be replaced with any
  other build of itself. Its licence is `libprojectM-LICENSE.txt`.
  https://github.com/projectM-visualizer/projectm
- **Lucide** icons (ISC, Copyright (c) Lucide Icons and Contributors), in
  `crates/app/assets/icons/`. Stroke and fill colours are rewritten to white
  so the interface can tint them.
- **Inter** (SIL Open Font License 1.1, Copyright (c) The Inter Project
  Authors), in `crates/app/assets/fonts/`.
