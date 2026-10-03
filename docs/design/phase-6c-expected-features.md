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

## Proposals and decisions (§4.7)

Moved from `PLAN.md` on 2026-10-03, unchanged but for the headings.
Section numbers (§) refer to `PLAN.md`, whose §4.7 keeps the
table of items with their decisions or priorities.

- **F1 Playlists.** No playlists exist; the queue is the only list.
  - A migration adds `playlists` (name, created, updated) and
    `playlist_items` (playlist, position, track id).
  - Create from a selection (F4) or with "Save queue as playlist".
    Rename, reorder, remove items, delete. Playlists appear in a sidebar
    section, and play or add to the queue like albums.
  - Import M3U/M3U8 (UTF-8, or a legacy code page detected, as O5's cue
    sheets): resolve each entry against the library folders, and list
    the ones not found. Export M3U8 to a place the user picks, with paths
    relative to it where possible. Export needs the
    `files.user-selected.read-write` entitlement. It writes only the file
    the user names, never a library file.
  - F10 lands with it, or a moved file drops out of every playlist.
  - **Decision:** built 2026-09-27. Migration 007 adds `playlists` and
    `playlist_items` (with the tables F2, F3 and F17 need);
    `library/playlists.rs` holds them, `collection.rs` the `playlists_*`
    commands. Playlists sit in a sidebar section (rename in place, drop
    tracks on one), every track menu has "Add to Playlist ▸" (a new one
    from the selection, or an existing one), and the queue has "Save
    Queue as Playlist". Import (Open With, a drop, or the File menu)
    resolves an entry by its path, then by the longest path tail that
    matches one library track, and lists what it didn't find. Export
    writes UTF-8 M3U8 with paths relative to the file where it can. The
    entitlement is now `files.user-selected.read-write`. A track removed
    from the library leaves its playlists; a moved one stays (F10).
- **F2 Smart playlists.** Saved rules over the DB: genre, year range,
  format, date added (O15), favourite or rating (F3), play count (O8).
  Built from fixed SQL fragments with every value bound, as
  `library/browse.rs` is. Results refresh after each scan.
  - **Decision:** built 2026-09-27 as a playlist with `rules` (JSON) and
    no items (`library/smart.rs`): all or any of genre, year range, format,
    added within, favourite, rating at least, play count, not played for,
    and artist, ordered by random (a stored seed, so the order holds until
    reshuffled), date added, most or last played, or rating, with an
    optional limit. Each condition is a fixed SQL fragment with its value
    bound. It's evaluated when shown or played, so it's always current
    rather than refreshed after a scan. The editor previews the matches as
    the rules change (`playlists_preview`).
- **F3 Favourites, then ratings.** A heart on tracks, albums and artists,
  shown in lists, with a Favourites view and a filter. Stored in the DB by
  id, never in the files. Ratings (0–5) follow in P2. They are seeded
  once from rating tags already in the files (ID3 `POPM`, Vorbis
  `RATING`/`FMPS_RATING`, MP4 `rate`), which `TagReader` reads, and
  never written back.
  - **Decision:** built 2026-09-27, ratings included. Hearts on tracks,
    albums and artists (`library/marks.rs`), shown in lists, on album and
    artist pages, in the playing bar and in Get Info; a Favourites view,
    and a favourites filter in the browser. Ratings are whole stars (a
    Rating column, a "Rate ▸" menu, the stars in Get Info). `TagReader`
    reads POPM (Windows Media Player's scale), `FMPS_RATING`, `RATING` and
    MP4 `rate` into 1–100; the scanner rounds to stars and follows the
    tags until the user rates the track, and a rating the user clears
    stays cleared. Nothing is written back.
- **F4 Multi-select, drag and drop.** Closes Phase 3's "no
  multi-select".
  - Shift- and ⌘-click and keyboard selection in `VirtualList.svelte`.
  - Bulk actions: play, play next, add to the queue, add to a playlist,
    remove from the queue.
  - Drag to reorder the queue, and to drop tracks on a playlist.
  - Dropping from Finder: a folder offers to become a library folder,
    and files play. Check that a bookmark can be made from a dropped URL
    in the sandbox.
  - **Decision:** built 2026-09-27. `lib/selection.ts` is a pure
    selection model (click, ⌘-click, shift-click, arrows with shift,
    ⌘A), tested in `tests/selection.test.mjs`; `VirtualList` uses it in
    the browser, search, queue, playlists and favourites. Menus act on the
    selection. Dragging is pointer-based (`state/drag.svelte.ts`), since
    Tauri's window takes HTML drag-and-drop for Finder drops: rows drag to
    reorder the queue or a playlist, and onto a playlist or the queue in
    the sidebar. From the Finder, files play and a folder is offered as a
    library folder (`add_folder` makes its bookmark).
- **F5 Open files from Finder.** `bundle.fileAssociations` for the
  supported types, handled through `RunEvent::Opened`.
  - A file outside every library folder plays without being added. The
    queue needs an "external file" item kind for it, since queue items
    are library tracks today.
  - The sandbox grants the opened file for the session only, so an
    external item is dropped from the saved queue.
  - **Decision:** built 2026-09-27. `bundle.fileAssociations` lists the
    audio types and M3U/M3U8 (role Viewer, rank Alternate, so the app
    never claims the default). `RunEvent::Opened` goes to
    `shell::opened`: playlists import (F1), audio files queue and play. A
    file in a library folder plays as its library track; any other is an
    external item (`library/external.rs`: a negative id, tags read at
    once, never in the DB), left out of the saved queue and of the
    history.
- **F6 Menu bar, shortcuts and Dock menu.** The app has only Tauri's
  default menu. Shortcuts exist only as page `keydown` handlers, which a
  focused text field swallows and nothing lists.
  - Add a Controls menu: play/pause, next (⌘→), previous (⌘←), volume
    (⌘↑/⌘↓), shuffle, repeat, and "Go to current track" (⌘L). Add a View
    menu (visualizer, queue, full screen) and a Help menu (shortcuts
    sheet, logs from H9).
  - A Dock menu with transport and the current track.
  - Menu items call the same queue functions as the media keys.
  - **Decision:** built 2026-09-27 (`shell/menu.rs`): File, Edit,
    Controls, View, Window and Help menus. Controls has play/pause,
    next (⌘→), previous (⌘←), volume (⌘↑/⌘↓), shuffle, repeat,
    stop after this track, the sleep timer and "Go to Current Track"
    (⌘L); View has the views, the queue, the visualizer, the mini player
    and full screen; Help has the shortcuts sheet. Items are enabled and
    checked from the queue's state. The Dock menu is the core's
    `DockMenu` (`anomp_dock_menu_*`, AppKit behind the core's platform
    rule), since Tauri has none: the current track, then play/pause,
    next and previous. H9's logs aren't in Help yet.
- **F7 Mini player and menu-bar controls.** A compact, optionally
  always-on-top second window reusing `NowPlayingBar.svelte`. An
  optional menu-bar (tray) item with transport and the current track.
  Needs H3's per-window permissions first.
  - **Decision:** built 2026-09-27. The mini player is a second window
    (`shell/mini.rs`, route `/mini`) with the playing bar in a compact
    layout, optionally always on top. Its capability
    (`capabilities/mini.json`) allows only the commands it uses: `build.rs`
    now declares the app's commands (Tauri's app manifest) and writes the
    main window's permission set from `generate_handler!`, so every
    command needs a permission (H3's second half). The menu-bar item
    (`shell/tray.rs`, a setting, off by default) shows the current track
    and the transport.
- **F8 First run and empty states.** A first launch shows how to add a
  folder (suggesting `~/Music`), says which online sources are on and
  what they send (the privacy policy's content, §8.1), and shows scan
  progress. Every empty view says what to do next. A folder whose
  bookmark doesn't resolve (drive unplugged, folder moved) shows as such
  in the sidebar with "Locate…", not only as a failed scan.
  - **Decision:** built 2026-09-27. With no folders the main view is a
    welcome page: add `~/Music` (suggested) or another folder, what each
    online source sends and how to turn it off, and scan progress. Empty
    views say what to do next. `library_folders` reports each folder's
    availability; an unavailable one shows a warning in the sidebar and
    settings with "Locate…" (`library_locate_folder`: the picked folder
    replaces the path and bookmark, and the rescan keeps the tracks'
    ids, as F10 does).
- **F9 Keep the library in step with the disk.** Phase 2 records "no
  rescan at launch and no file watching yet".
  - P1: an incremental rescan of each folder at launch, in the
    background at low priority. The scanner already skips unchanged files
    by mtime and size.
  - P2: FSEvents watching through the `notify` crate while the app runs.
    It is debounced and per folder, only while the folder's bookmark is
    open, and behind a setting.
  - Mind H12: a watcher or scan must not download cloud placeholders.
  - **Decision:** built 2026-09-27 (`library/watch.rs`), both behind
    Settings › Library, both on by default. At launch every folder is
    rescanned in the background, on half the scan threads at utility QoS.
    While the app runs, `notify` (FSEvents) watches each folder, and a
    change rescans that folder after 3 s of quiet (at most 30 s after the
    first change). A folder is watched only while it's available. H12's
    cloud placeholders are not handled yet: a rescan reads only changed
    files, but a changed placeholder would be downloaded.
- **F10 Keep the user's data when files move or are renamed.** A track's
  identity is its folder and path. A move or rename deletes the row, and
  with it anything keyed by the track (O15 notes this). This matters
  once playlists, favourites and plays (F1, F3, O8) exist.
  - Before a scan deletes a missing track, match it against the new files
    by recording MBID, or by size, length and tags. Move the existing id
    to the new path instead of deleting and inserting.
  - Keep an album's user picks (`album_links` with `chosen_by = 'user'`,
    `album_art`) when the album is re-created with the same MBID, or the
    same album artist and title.
  - Check which foreign keys cascade today, and test moves across folders.
  - **Decision:** built 2026-09-27 in `scanner::scan_folders`. All
    folders being scanned are walked first; a missing track is matched to
    a new file of the same size by mtime, recording MBID, or length
    (±1 s) with title, artist and album, and the row moves to the new path
    with its id, so everything keyed by the track stays. The scan report
    counts moves. When an album or artist is deleted, the user's picks
    (chosen links, cover, preferences, heart) go to `kept_albums` and
    `kept_artists` and return when it's re-created with the same MBID, or
    the same title and artist, for a year. A move between folders is
    caught only when both are scanned together, as a launch or "Rescan
    all" does.
- **F11 Compilations and multiple artists.** Two known limits from Phase
  2.
  - Compilations tagged without an album artist split into one album per
    track artist. Group them by the compilation flag (ID3 `TCMP`, MP4
    `cpil`, Vorbis `COMPILATION`), or by one album title in one folder
    with three or more track artists, under "Various Artists".
  - Several artists in one tag count as one. Read multi-valued artist
    tags (TagLib's `ARTISTS` and property-map lists), split "A; B", and
    store credited artists in a new table for browse, artist pages and
    MusicBrainz matching. Keep the tag's text for display.
  - The migration updates the FTS triggers (migration 002's rule).
  - **Decision:** built 2026-09-27. `TagReader` reads the compilation
    flag and every artist value; `split_artists` also splits "A; B". Migration 008 adds `track_artists` (credited artists, in
    order), `tracks.artist_credit` (the tag's text, shown), `compilation`
    and `album_artist_tagged`, and updates the search triggers. Artist
    browsing and artist pages go through the credits, so a duet is under
    both. Compilations without an album artist, or one album title in a
    folder with three or more track artists, group under "Various
    Artists" (MusicBrainz's special artist). MusicBrainz matching still
    uses the first credited artist.
- **F12 Substring and field search.** Phase 3's known limit: words match
  only from their start. Add an FTS5 `trigram` index for substrings of
  three or more characters, sized against the 50,000-track benchmarks.
  Add field filters: `artist:`, `album:`, `genre:`, `year:1994`,
  `year:1990-1999`.
  - **Decision:** built 2026-09-27. Migration 009 adds contentless
    FTS5 `trigram` indexes (diacritics removed) beside the word indexes.
    Each term matches a word start or, from three characters, a
    substring; word-start matches rank first. Filters: `artist:`,
    `album:`, `title:`, `composer:`, `genre:`, `year:1994`,
    `year:1990-1999`, quoted for spaces. At 50,000 tracks the trigram
    indexes add 10 MB to the DB (14 → 24 MB) and 2.1 s to a full scan's
    inserts; searches take 2–12 ms, and 50 ms for a one- or two-letter
    prefix matching 28% of tracks (words alone were 4 and 31 ms). Ranking
    materialises the word matches (a CTE), which took "love" from
    520 ms to 12 ms.
- **F13 Sleep timer and stop after this track.** "Stop after this track"
  in the queue, and a timer (15, 30, 60 minutes, end of album) that
  fades out over the last 10 s through the engine volume. Tested against
  the fake engine in `queue/model.rs`.
  - **Decision:** built 2026-09-27 in `queue/model.rs`, tested against
    the fake engine: stop after the current track or after any queue
    item; a sleep timer of 15, 30, 45, 60 or 90 minutes (any 1–1,440),
    the end of the track or the end of the album. A timed sleep fades the
    engine volume over the last 10 s, stops, and restores the volume.
- **F14 Crossfade.** Off by default, and never between consecutive tracks
  of one album, which stay gapless. The engine mixes two readers during
  the fade, which touches the gapless hand-off, so design it with Phase
  1's hand-off design first. Tested offline through
  `getNextAudioBlock`.
  - **Decision:** built 2026-09-27, off by default (Settings ›
    Playback, 1–12 s). `PlayerEngine` mixes the current reader's tail
    with the pre-opened next track's head at equal power; the hand-off
    stays sample-exact, and the queue arms each next track with its fade
    length. The queue never fades between consecutive tracks of an album
    or a shuffle unit, nor does the engine across a sample-rate change or
    while looping, and each side gives at most half its length. Tested
    offline through `getNextAudioBlock`.
- **F15 Equaliser.** A 10-band graphic EQ, or a few parametric bands,
  with presets and a preamp. It sits in `PlayerEngine` next to O11's
  crossfeed, after the per-track gain and before the volume. Written with
  JUCE's IIR filters, or in-house, with no GUI module. Presets can
  follow the output device (headphones or speakers).
  - **Decision:** built 2026-09-27 in-house (`core/src/Equaliser.*`,
    RBJ peaking biquads, no JUCE DSP module): ten bands from 31 Hz to
    16 kHz, ±12 dB, a preamp, ten presets and custom, and a second
    profile for headphones when it follows the output (O11's
    `OutputRoute`). It runs after the visualizer's tap and before
    crossfeed, and gains glide, so moving a slider doesn't click. Off and
    flat, it's bypassed.
- **F16 Track info panel.** A read-only "Get Info": every tag TagLib
  reports, the format, bitrate, sample rate and channels, the embedded
  pictures, the path with "Reveal in Finder" (the queue has it), and the
  MusicBrainz links. It shares the format facts with O10's signal path
  panel.
  - **Decision:** built 2026-09-27. `anomp_read_file_info` returns
    every field TagLib's property map has (and frames it can't map,
    marked), the pictures, the tag types, and the decoder's format facts;
    `library_track_details` adds the path, MBIDs and the part of the file.
    Get Info (⌘I, the track menu) shows them with the heart and stars,
    and "Show in Finder".
- **F17 Resume where the user left off.** Phase 3's known limit: nothing
  is published to Now Playing after a relaunch until playback starts, so
  the media keys can't resume. Publish the restored queue, paused at its
  saved position. Also remember the position of long tracks (over 20
  minutes: audiobooks, DJ mixes, lectures) and resume there.
  - **Decision:** built 2026-09-27. `media.rs` publishes the restored
    queue's current item paused at its saved position, so the media keys
    resume. Tracks of 20 minutes or more remember where they were left
    (`track_positions`, unless within 30 s of either end) and start
    there.
- **F18 Accessibility, and a safe visualizer.**
  - A VoiceOver pass over the main views: list roles and labels, and a
    live region announcing track changes.
  - Full keyboard use, including the context menu's arrow keys (Phase
    3's known limit). A visible focus ring, contrast checked in both
    themes, and layouts that survive larger text.
  - `prefers-reduced-motion` is not read anywhere. Beat-driven
    visualizations can flash, and WCAG 2.3.1 allows no more than three
    flashes a second. Limit flash rate and contrast in the renderers
    (`app/src/lib/visualizer/renderers/`). Under reduced motion, calm
    them further or default to a still one. Show a photosensitivity note
    the first time the visualizer opens.
  - **Decision:** built 2026-09-27. Lists are ARIA listboxes with
    `aria-activedescendant` and a visible keyboard row; menus and submenus
    work from the keyboard; a live region announces track changes and
    results; a skip link jumps past the sidebar. Theme colours meet WCAG
    AA in both themes (`tests/contrast.test.mjs`; the light theme's faint
    colour was darkened, and text no longer uses it). Reduced motion stops
    transitions. The visualizer has a flash guard (`visualizer/safety.ts`:
    past three flashes a second the picture dims), a calm mode (no beat
    pulses, slower movement; always on under reduced motion), and a
    photosensitivity note the first time it opens. Larger text is left
    to check in the app.
- **F19 Localisation groundwork.** Every UI string is an English literal
  in a component, and Rust returns English error text. Move strings into
  a typed message catalogue (one JSON file per locale), and format
  numbers, dates and durations with `Intl` everywhere (`format.ts` does
  some already). Rust errors the UI shows become codes with parameters.
  This is cheap now and costly later. Translations can wait until after
  the first release.
  - **Decision:** built 2026-09-27. `lib/i18n` has a typed catalogue
    (`en.json`, about 990 messages), `t`, plural messages through
    `Intl.PluralRules`, and `Intl` number and date formatting; `npm run
    check` rejects an unknown key. Rust errors the UI shows are coded
    (`coded.rs`: `{code, params, message}`), and `errorText` shows the
    catalogue's `error.<code>`; `tests/i18n.test.mjs` checks every code
    has a message. English text Rust still produces: the native menus,
    the Dock and menu-bar menus, notifications, source notes and
    candidate labels in the metadata dialogs, health-report details, and
    database or I/O errors.
- **F20 Export and import the user's data.** One JSON file with what the
  user made: settings, sort rules, online source settings, the user's
  picks (`album_links` chosen by the user, `album_art`), the queue, and
  later playlists, favourites and history. Keyed by folder-relative
  paths and MBIDs, so it imports into a fresh library after a scan. It
  covers a lost or corrupt DB (H10) and moving to another Mac, and later
  carries data to iOS (Phase 8).
  - **Decision:** built 2026-09-27 (`library/transfer.rs`, Settings ›
    Library): one JSON file (`ano-mp.user-data`, version 1) with the
    settings (optional on import), and, keyed by folder-relative path and
    MBIDs, favourites, ratings, plays, resume positions, preferences, the
    user's album and artist picks, playlists and the queue. Import
    matches what the library has and reports what it couldn't place.
- **F21 Track-change notifications.** Opt-in, only while the window isn't
  focused, through `tauri-plugin-notification`, with the cover.
  - **Decision:** built 2026-09-27, off by default (Settings ›
    General): while no window of the app is in front, a track change
    shows a notification with the cover. It goes through `notify-rust`
    directly, since `tauri-plugin-notification` leaves out the image on
    desktop.
