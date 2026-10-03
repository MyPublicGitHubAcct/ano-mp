# Phase 6c — Expected features (F1–F21)

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 6c
keeps the phase's status, decisions and open steps.


Built 2026-09-27: all twenty-one features of §4.7, whatever their
priority; each item's **Decision** line there says how. They follow #6's
rules: nothing is written to the user's files, and what the user makes
lives in the library DB, keyed by ids that rescans and moves keep.
- **Core**: `TagReader` reads ratings, the compilation flag and every
  artist value, and `anomp_read_file_info` returns a file's every tag
  field, picture and format fact (F3, F11, F16); `PlayerEngine`
  crossfades into the pre-opened next track (F14) and runs the
  `Equaliser` (F15); `DockMenu` gives the Dock icon a menu (F6).
- **Library**: migration 007 (playlists, favourites, ratings, resume
  positions, kept picks), 008 (credited artists, compilations, the
  search triggers over the credit) and 009 (trigram indexes). The
  scanner scans folders together, moves rows for moved files, groups
  compilations, keeps the user's picks for deleted albums and artists,
  and seeds ratings; `playlists`, `smart`, `marks`, `info`, `external`,
  `transfer` and `watch` are the features' library code.
- **Queue**: stop after, the sleep timer and its fade, crossfade arming,
  resume positions, external items, moving several items at once.
- **Shell** (`app/src-tauri/src/shell/`): the menu bar, the Dock menu,
  the menu-bar controls, the mini player (with its own capability),
  files opened from the Finder, notifications. On macOS closing the main
  window hides it and playback goes on.
- **UI**: multi-select and dragging, playlists and their views, the smart
  playlist editor, Favourites, hearts and stars, Get Info, the welcome
  page and empty states, the sleep timer panel, the mini player, the
  equaliser and General settings, the visualizer's safety, the message
  catalogue.
- **Tests**: 99 Catch2 tests (ratings, credits, file info, crossfade,
  the equaliser, the Dock menu's C API), 343 `cargo test` tests and 18
  frontend tests (selection, equaliser presets, flash guard, catalogue,
  contrast). Benchmarks (release, 50,000 tracks): search as in F12; the
  ids under a top node 199 ms, a folder's tree 115 ms (as before); track
  infos for a 50,000-track queue 181 ms, up from 60 ms with Phase 6b's
  shuffle units (a query per album), not these features.
- **Closes known limits** of earlier phases: no rescan at launch or
  watching (Phase 2), compilations and multiple artists (Phase 2), words
  matched only from their start (Phase 3), no multi-select and the
  context menu's keyboard use (Phase 3), nothing in Now Playing after a
  relaunch (Phase 3), and user data lost when a file moves (Phase 6b).
- **Known limits**: a move between folders is caught only when both are
  scanned together; the watcher can download changed cloud placeholders
  (H12); smart playlists offer a fixed set of conditions; ratings are
  whole stars; crossfade doesn't cross a sample-rate change; the
  catalogue is English only, and some text Rust produces is English
  (F19's decision lists it); H9's logs aren't in the Help menu yet.
