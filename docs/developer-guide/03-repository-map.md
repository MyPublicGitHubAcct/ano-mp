# 3. Repository map

Every top-level folder, and every module of the four source trees, in a
line or two: what it owns, and where it matters, what it must not do.
Use it to find the file behind a behaviour; the chapters that follow
explain how the parts work together.

`scripts/check-developer-guide.py` (in `check-all.py`'s quick checks)
compares this page with the tree: a module added without a line here, or
a line for a module that has gone, fails it. When you add, rename or
remove a source file, change this page in the same commit.

## Top-level folders

- `app/`: the Tauri application: the Svelte frontend (`app/src/`), the
  Rust backend (`app/src-tauri/`), the frontend's tests (`app/tests/`),
  and npm's and Tauri's configuration. Chapters 6 and 7.
- `core/`: `anomp_core`, the C++ audio core: its one public header
  (`core/include/anomp/anomp.h`), its sources (`core/src/`), Catch2 tests
  and their fixtures (`core/tests/`) and fuzz targets (`core/fuzz/`).
  Chapter 4.
- `effects/`: `anomp_effects`, the real-time effects, a dependency-free
  C++ library the core links privately. Chapter 5.
- `cmake/`: CMake helpers: pinned, checked downloads (`Fetch.cmake`), and
  how FFmpeg, TagLib and Signalsmith Stretch are found or built.
- `scripts/`: every check, formatter, build and release script, in
  standard-library Python (plus `build-ffmpeg.sh`), with their tests in
  `scripts/tests/`, the pre-commit hook in `scripts/hooks/` and standard
  licence texts in `scripts/licenses/`. Chapters 2, 12 and 13.
- `docs/`: the user guide, this guide, finished work's design notes
  (`docs/design/`), release checklists and decisions.
- `.github/`: CI (`ci.yml`, which runs `check-all.py` and nothing else),
  the release, weekly audit and weekly fuzz workflows, and Dependabot.

Not in git: `build/` (CMake's presets, and the downloads they share),
`third_party/ffmpeg/` (FFmpeg as `build-ffmpeg.sh` builds it),
`app/node_modules/` and `app/src-tauri/target/` (Cargo, including its own
build of the core).

## The core: `core/src/`

Everything here is private to the core; Rust sees only `anomp.h`. A
platform file (`FolderAccess_apple.mm`, `FolderAccess_unsandboxed.cpp`,
`MediaControls_none.cpp`…) implements its unit's small interface for
one OS; no AppKit or CoreAudio call lives
anywhere else.

- `anomp_c_api`: the C API (`anomp.h`) over the C++ classes. Translates
  types (`bool` to `int`, null pointers to safe defaults), copies strings
  out into handles the caller frees, and catches every exception: nothing
  C++ crosses the boundary.
- `AudioEngine`: owns JUCE's runtime, the output device, the read-ahead
  thread and the `PlayerEngine`, and dispatches the player's events from a
  main-thread timer. Main thread only; it must let go of JUCE before the
  process's statics are destroyed.
- `PlayerEngine`: plays the current track and a pre-opened next one,
  handing off gaplessly or crossfading; seeks, loops, tempo, volume, and
  the order of the signal path (effects, analysis tap, equaliser,
  crossfeed, recording, volume). A plain `juce::AudioSource` with no
  device, so tests render it offline. Never opens or frees a track on the
  audio thread.
- `FormatRegistry`: the `juce::AudioFormatManager` with the one format the
  player registers, FFmpeg's, and the extensions it decodes.
- `FFmpegAudioFormat`: every supported file type as one read-only JUCE
  `AudioFormat`, reading through a `juce::InputStream`, with sample-exact
  seeking and lengths. One of only two files that include FFmpeg's
  headers; read the Phase 1 design notes before changing it.
- `FFmpegEncoder`: writes a recording's file (WAV, AIFF, FLAC, ALAC, AAC,
  MP3) with FFmpeg's encoders and muxers. The other file that includes
  FFmpeg.
- `TagReader`: a file's tags, pictures, chapters, lyrics and audio
  properties through TagLib; safe on any thread, concurrently.
- `FileAnalyser`: one pass over a file with its own decoder: loudness (EBU
  R128), peaks, silences, the spectrum's cutoff and the seek bar's
  waveform. Never touches the engine.
- `Recorder`: records what the player renders. The audio thread only
  copies into its FIFO; the writer thread encodes, marks tracks and
  reports failures.
- `SignalTap`: the latest stereo samples the player rendered, written by
  the audio thread and read by the analysis thread without locks.
- `SpectrumAnalyser`: one analysis frame for the visualizer from the tap:
  bands, chroma, notes, stereo balance, levels, a triggered waveform and
  beats.
- `AnalysisThread`: runs the `SpectrumAnalyser` at a steady frame rate on
  its own thread and calls the host's callback there, never on the main
  thread.
- `Equaliser`: the 10-band graphic equaliser and preamp, gliding between
  settings so a change never clicks.
- `Crossfeed`: headphone crossfeed (Bauer's), in three strengths.
- `FolderAccess`: durable access to a folder the user picked:
  security-scoped bookmarks on Apple platforms, the path elsewhere.
- `FileStatus`: whether a file is a dataless cloud placeholder, from
  `stat()`, which never downloads it.
- `MediaControls`: the OS's Now Playing and remote commands
  (`MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`). Main thread only.
- `DockMenu`: the Dock icon's menu, added to the host's app delegate at
  run time on macOS; nothing elsewhere.
- `OutputRoute`: whether the output device is headphones, for crossfeed
  and the equaliser's headphone profile.
- `VolumeWatcher`: reports drives, disk images and shares mounting and
  unmounting, so unavailable folders come back without a relaunch.
- `Log`: the core's log, forwarded to the host's callback, with JUCE's
  `Logger` and failed assertions routed into it.

## The effects: `effects/src/`

Plain C++20, no dependencies (JUCE included); only
`effects/include/anomp/effects/EffectChain.h` is public.

- `EffectChain`: the catalogue of effects and their parameters, and the
  chain that runs them in `chainOrder`, mixing each wet signal with the
  dry. Settings are atomics, read once per 64-sample chunk; with every
  effect off it leaves the signal bit-identical.
- `Effect`: the interface each effect implements: prepare, reset, set a
  parameter, turn a chunk into its wet signal, report its tail.
- `Dsp`: the shared building blocks: linear smoothing, delay lines, a
  one-pole filter, LFOs, noise, and flushing denormals.
- `Reverb`: Freeverb's room.
- `Echo`: the echo (repeats that darken as they fade) and the lo-fi
  effect (fewer bits, a lower rate).
- `Modulation`: chorus, flanger, phaser and tremolo (or auto-pan).
- `SpectralFreeze`: holds the spectrum of a moment while the freeze is
  held.
- `Fft`: an in-place radix-2 FFT for the freeze.

## The Rust backend: `app/src-tauri/src/`

Rust owns everything that isn't audio: the database, settings, HTTP,
scanning, and the bridge between the UI and the core. Commands are the
`#[tauri::command]` functions `lib.rs` registers.

### Start-up and the bridge to the core

- `main.rs`: the binary's entry point; calls `lib.rs`'s `run`.
- `lib.rs`: start-up order (`setup`), the command list
  (`generate_handler!`), the `anomp-art` URI scheme, window events and the
  shutdown order on `RunEvent::Exit`.
- `anomp.rs`: every FFI declaration of `anomp.h` and its safe wrapper
  (`Engine`, `Tags`, `FolderAccess`, `MediaControls`…). All `unsafe` code
  lives here, each block with its `// SAFETY:` comment; nowhere else
  declares a C function.
- `audio.rs`: hosts the `Engine` in a main-thread `thread_local`, routes
  its events (to the queue, media controls, history, the UI), opens the
  output device the settings name, and applies the equaliser and
  crossfeed. `with_engine` hops to the main thread; nothing else may hold
  the engine.
- `visualizer.rs`: starts the core's analysis while a visualization is
  subscribed, and streams its frames to the page in a compact binary form
  from the analysis thread.
- `media.rs`: what the OS's Now Playing shows (`NowPlaying`, which sends
  only differences, behind a `Publisher` trait) and the media keys'
  commands back into the queue.
- `effects.rs`: the effects' settings, applying them to the engine, and
  the commands that list them, preview a change and hold the freeze.
- `recording.rs`: recording what plays (X6): its settings, the
  recordings' folder (a writable bookmark of its own), start and stop, and
  the cue sheet that names the recording's tracks.

### Settings, errors and the shape of the UI's data

- `settings.rs`: `AppSettings` (display, playback, output, visualizer,
  features, equaliser, effects, recording, library, window, appearance),
  read leniently from the `app`
  setting, validated and applied on save. A new setting goes here with a
  default and a `validate` rule.
- `theme.rs`: themes as token values (never CSS), their validation, and
  export and import.
- `coded.rs`: errors the UI shows as `{code, params, message}`, each code
  with an `error.<code>` message in `en.json`.
- `bindings.rs`: test-only: generates `app/src/lib/generated/ipc.ts` and
  `commands.ts` from the Rust types and command signatures, and fails
  while they are stale.
- `guide.rs`: test-only: generates the user guide's settings reference
  from the defaults and `en.json`, and fails while it is stale.

### Commands over the library's features

- `collection.rs`: playlists, smart playlists, hearts, ratings, Get Info,
  and exporting and importing the user's data; emits
  `collection-changed`.
- `features.rs`: the optional features' commands (analysis, history,
  discovery, recommendations, health, lyrics, preferences), each refusing
  while its switch is off.
- `updates.rs`: checks GitHub Releases for a newer version (off by
  default); never installs anything.
- `diagnostics.rs`: "Copy diagnostics" (no paths or titles), "Show logs",
  the third-party notices, and the launch-to-first-paint time.
- `logging.rs`: the log file, its redaction and scrubbing, and the panic
  hook. Log through the `log` macros, never `eprintln!`.
- `dev.rs`: the /dev page's commands, debug builds only: they take raw
  paths and bypass the queue.
- `self_test.rs`: `ano-mp --self-test`, compiled only with the
  `self-test` feature: checks a sandboxed bundle and exits.

### `library/`

The SQLite library: folders, tracks, browsing, search and everything
kept about them. Open connections only through `db`.

- `mod.rs`: folders (add, relocate, remove), track rows,
  `track_path`, and removing orphaned albums and artists.
- `commands.rs`: the library's commands and `LibraryState` (the
  shared connection, the art cache, the image cache); scans on a blocking
  thread with their own connection; database start-up and upkeep.
- `db.rs`: opens and configures connections (registering the SQL
  functions), runs `MIGRATIONS`, and writes the copy before a migration.
- `recovery.rs`: the launch check (`PRAGMA quick_check`), and
  restoring or rebuilding a damaged database at the next launch.
- `access.rs`: resolving folders' bookmarks (`open_folder`,
  `open_folder_of`) and why a folder can't be read (`FolderState`); its
  `testing::FakeBookmarks` stands in for real bookmarks.
- `availability.rs`: which folders can be read now, checked at
  launch, after volume changes and by scans; `unreadable` for anything
  that picks tracks to play.
- `scanner.rs`: the incremental scanner: walks folders, reads new
  and changed files' tags in parallel, keeps track ids across moves,
  splits cue sheets and chapters, files compilations, and never empties a
  folder on its own (`holds`).
- `watch.rs`: the launch rescan and FSEvents watching, debounced
  into scans of the folders that changed.
- `cue.rs`: parses cue sheets.
- `genres.rs`: splits genre tags; the `anomp_genres` and
  `anomp_has_genre` SQL functions.
- `sort_key.rs`: sort keys that fold case and accents, order
  numbers by value and skip articles; the `anomp_sort_key` SQL function.
- `rules.rs`: sort and grouping rules (the sidebar's library
  views), stored under `library.sort`.
- `browse.rs`: one node of the library under a rule, a page at a
  time, from fixed SQL fragments with every value bound.
- `search.rs`: search over the FTS5 word and trigram indexes,
  with field filters, grouped into artists, albums and tracks.
- `artists.rs`: the artist page's albums and appearances.
- `albums.rs`: an album's details for its header: the tags, the
  sources' match, its cover's source.
- `playback.rs`: `track_play`: how the engine plays a track (its
  part of the file, gain, trims, silences to skip). Everything that opens
  a track for playback goes through it.
- `external.rs`: files opened from outside the library, known by
  negative ids for this session only.
- `art.rs`: cover art for the UI through the `anomp-art` scheme:
  the user's pick, else the sources in order; cached in memory. Never
  goes online.
- `thumbs.rs`: cover thumbnails in the image cache, named by
  their picture's SHA-256, indexed in `art_thumbs`.
- `covers.rs`: the albums the visualizer's cover wall shows.
- `playlists.rs`: playlists, M3U8 import and export.
- `smart.rs`: smart playlists' rules and their queries.
- `marks.rs`: hearts and ratings.
- `prefs.rs`: per-track and per-album playback preferences (O7).
- `analysis.rs`: the background loudness analysis and what the
  same pass finds; never reads a cloud placeholder.
- `health.rs`: the library health report (read-only).
- `info.rs`: Get Info: every tag field, the format, the file.
- `lyrics.rs`: lyrics from a `.lrc` file or the tags, read when
  shown, never stored.
- `discover.rs`: Recently added, On this day, More in this genre.
- `similar.rs`: "More like this" and Home's suggestions, scored
  from the library alone.
- `transfer.rs`: exporting and importing everything the user
  made, as one JSON file.
- `test_library.rs`: an in-memory library of synthetic tracks
  for tests.
- `bench.rs`: the ignored 50,000-track benchmarks `bench.py`
  runs.

### `queue/`

- `mod.rs`: the queue's host on the main thread (`run`), its
  commands, `EnginePlayer` (the engine behind the `Player` trait),
  publishing `queue-changed`, saving, and the transport functions the
  menus, media keys and remote call.
- `model.rs`: the queue's logic, apart from the engine and Tauri:
  play order, shuffle, repeat, arming the next track, crossfade choices,
  stop-after and the sleep timer, logging every list change as an `Edit`.
  Tested against a fake player.
- `opening.rs`: opening tracks off the main thread (H11): resolve
  the row and folder on a blocking thread, hand the file to the engine,
  hold the folder open until the engine reports it.
- `store.rs`: the saved queue as `queue_items` rows, kept in step
  by applying the same edits the UI gets.
- `radio.rs`: library radio: tracks that fit a seed, each with why.

### `metadata/`

Online and folder metadata. Every request goes through `http::Client`;
anything a command needs from a service runs on the worker.

- `mod.rs`: the module's `Error` (offline, an HTTP status, a
  response that can't be used) and its rules.
- `settings.rs`: the sources (`SourceId`), which are on, and in
  what order for each kind of data (`metadata.services`).
- `commands.rs`: the metadata commands and the "Find details",
  "Choose cover" and "Find artist" dialogs' data.
- `worker.rs`: the worker's thread in the app: its HTTP client
  and connection; `request` and `call` queue work without waiting.
- `jobs.rs`: what the worker does, a step at a time, by
  priority; which jobs are still needed; batching its events.
- `http.rs`: `Client`: per-host rate limits, retries, offline
  back-off, the cached (`get_json`) and memory-only (`get_json_fresh`)
  JSON fetches; `testing::FakeTransport` and `FakeClock`.
- `cache.rs`: the response cache (`mb_cache`), pruned at launch.
- `images.rs`: downloaded pictures on disk, named by their URL's
  SHA-256, kept under a size budget.
- `keys.rs`: keys and tokens in the OS keychain (an in-memory
  store in tests); registers each with the log's redaction.
- `matcher.rs`: scoring how well a release matches an album, and
  when to accept it without asking.
- `albums.rs`: matching albums to releases (`album_links`),
  through each source's `ReleaseSource`; never replaces the user's pick.
- `artists.rs`: matching artists on MusicBrainz
  (`artist_links`).
- `musicbrainz.rs`: the MusicBrainz API, parsed.
- `coverartarchive.rs`: covers from the Cover Art Archive.
- `wikipedia.rs`: biographies and album descriptions (CC BY-SA:
  always credited).
- `discogs.rs`: the Discogs API, with its terms' limits: the
  match only is stored, data in memory for at most `MAX_AGE`, no images.
- `folder_art.rs`: cover images next to the tracks.
- `discography.rs`: an artist's releases the library lacks.
- `listenbrainz.rs`: ListenBrainz's similar artists (X5).
- `outside.rs`: recommendations from outside the library (X5).

### `history/`

- `mod.rs`: listening history: when a play counts, the history
  thread that records it, and ListenBrainz submission.
- `listenbrainz.rs`: queueing and sending listens (opt-in).
- `views.rs`: Recently played, the top 20, and the history
  page's highlights.

### `shell/`

The app outside its main page.

- `mod.rs`: start-up of the menus, Dock, tray and notifications;
  what they show of the current track; showing the main window again;
  files opened from the Finder or dropped on the window.
- `menu.rs`: the menu bar; transport items run here, the rest reach
  the page as `menu` events (`PAGE_ITEMS`).
- `dock.rs`: the Dock menu through the core's `DockMenu`.
- `tray.rs`: the optional menu-bar controls.
- `mini.rs`: the mini player window.
- `help.rs`: the Help window (the bundled user guide).
- `notifications.rs`: track-change notifications (opt-in).

### `remote/`

Any change here needs the security review in `PLAN.md` §8.1.

- `mod.rs`: the LAN remote: listens only while on, answers local
  addresses only, pairs with a code, keeps only hashes of tokens.
- `http.rs`: the bounded HTTP/1.1 the remote page needs, testable
  without sockets.

## The frontend: `app/src/`

Svelte 5 and TypeScript, built by SvelteKit's static adapter into a
single-page app per window.

- `app.html`: the HTML shell SvelteKit fills.
- `app.d.ts`: the build-time constants (`__DEV_TOOLS__`).
- `lib/generated/`: TypeScript generated from the Rust types and commands
  (`settings.ts`, `ipc.ts`, `commands.ts`); never edited by hand.

### `routes/`

- `+layout.ts`: reads the settings and the theme before any page
  renders, and starts logging the page's errors.
- `+layout.svelte`: global styles: the theme's custom properties and the
  base look of controls, and reduced motion.
- `+page.svelte`: the main window: sidebar, browser, queue and playing
  bar; menu events, dropped files, the welcome view.
- `mini/+page.svelte`: the mini player window: the playing bar alone.
- `help/+page.svelte`: the Help window: the user guide's pages, parsed
  by `guide.ts`, drawn with ordinary elements.
- `dev/+page.ts` and `dev/+page.svelte`: the /dev page, loaded only when
  `__DEV_TOOLS__` is true.

### `lib/`

- `api.ts`: the one way components reach the backend: typed wrappers over
  `generated/commands.ts`, the event map (`on`), `artUrl`. Never `invoke`
  with a string.
- `columns.ts`: the fields track lists and album summaries can show.
- `effects.ts`: the effects' presets, slider scales and value display.
- `equaliser.ts`: the equaliser's presets and band labels.
- `featureMenu.ts`: the menu items the optional features add.
- `trackMenu.ts`: the menu for one track or a selection.
- `folders.ts`: which folders' tracks to show dimmed.
- `format.ts`: times and dates as the UI writes them.
- `guide.ts`: the parser for the user guide's Markdown subset.
- `links.ts`: `webLink`: only https links (http upgraded) may be opened.
- `openLink.ts`: opens web links in the browser, never the webview.
- `logErrors.ts`: sends the page's uncaught errors to the app's log.
- `queueEdits.ts`: applies the queue's numbered edits to the UI's copy.
- `releases.ts`: groups releases by MusicBrainz release-group type.
- `selection.ts`: multi-selection over list rows.
- `similar.ts`: why a recommendation was made, as a message.
- `theme.ts`: themes as custom properties, the contrast checks
  (`COLOR_TOKENS`, `PAIRS`), the accent from a cover.
- `themes.json`: the built-in themes (read by Rust too).

### `lib/i18n/`

- `index.ts`: `t` (typed message keys, plurals) and `errorText` for
  coded errors.
- `en.json`: every string the UI shows.

### `lib/state/`

Svelte 5 rune stores, each a singleton following its backend events.

- `settings.svelte.ts`: the settings, loaded before the first render,
  saved whole after each change.
- `player.svelte.ts`: the queue and transport from `queue-changed` and
  `player-state`.
- `position.svelte.ts`: the position alone, so only the seek bar
  re-renders every 50 ms.
- `library.svelte.ts`: folders, sort rules, where the browser is, the
  search query, scanning.
- `collection.svelte.ts`: playlists and the actions on playlists, hearts
  and ratings.
- `features.svelte.ts`: which features are on, the analysis's progress,
  waveforms.
- `metadata.svelte.ts`: what the metadata worker is doing.
- `appearance.svelte.ts`: the theme on screen, with the system's
  appearance and the cover's accent.
- `effects.svelte.ts`: the spectral freeze's Hold.
- `recording.svelte.ts`: whether a recording runs, and its length.
- `visualizer.svelte.ts`: which visualization, full screen, its caption.
- `drag.svelte.ts`: dragging tracks onto playlists and the queue
  (pointer events, not HTML drag and drop).
- `ui.svelte.ts`: the main view, the settings section, menus and
  dialogs.
- `toasts.svelte.ts`: short messages; `attempt` turns a failed command
  into one.
- `updates.svelte.ts`: newer releases.

### `lib/components/`

- `Sidebar.svelte`: views, library views, playlists, folders, scan
  progress, the metadata worker's status.
- `Header.svelte`: breadcrumbs, the search box, the sidebar button.
- `BrowsePane.svelte`: one node of the library, groups and tracks, a page
  at a time.
- `VirtualList.svelte`: renders only the rows in view and asks for the
  pages it needs; selection and keyboard.
- `TrackText.svelte`: a track row's text and columns.
- `SearchResults.svelte`: search results, 150 ms after typing stops.
- `HomeView.svelte`: Home's sections, each while its feature is on.
- `ArtistsView.svelte`: every artist, filterable.
- `ArtistPage.svelte`: an artist's page: facts, biography, albums.
- `ArtistSimilar.svelte`: an artist page's similar artists.
- `DiscographyPage.svelte`: an artist's releases the library lacks.
- `AlbumInfo.svelte`: the album header: cover, details, description.
- `AlbumWorks.svelte`: an album's classical works and movements.
- `AlbumCards.svelte`: a row or grid of albums with covers.
- `SimilarAlbums.svelte`: an album page's "More like this".
- `MoreInGenre.svelte`: five random albums sharing a genre.
- `OutsideArtists.svelte`: artists outside the library, with links out.
- `FavouritesView.svelte`: everything hearted.
- `HistoryView.svelte`: the top 20 and Recently played.
- `HealthView.svelte`: the library health report.
- `PlaylistView.svelte`: a playlist, reorderable; a smart playlist's
  matches.
- `QueuePanel.svelte`: the queue, with selection and drag to reorder.
- `NowPlaying.svelte`: the current track with its cover, large.
- `NowPlayingBar.svelte`: the playing bar: transport, seek, volume,
  shuffle, repeat, sleep, Hold, Record, the queue toggle.
- `SeekBar.svelte`: the position slider (and waveform); the only reader
  of the position store.
- `Popover.svelte`: a panel opened from the playing bar.
- `SleepTimerPanel.svelte`: the sleep timer.
- `PracticePanel.svelte`: the A–B loop, tempo and pitch.
- `SignalPathPanel.svelte`: every step from the file to the speakers.
- `LyricsPanel.svelte`: the current track's lyrics.
- `Visualizer.svelte`: the canvas and draw loop, fed by the analysis
  stream outside Svelte's reactivity.
- `VisualizerView.svelte`: the visualizer's view, picker and controls.
- `WelcomeView.svelte`: the first run (no folders yet).
- `MissingFolders.svelte`: folders that can't be read, and what to do.
- `SettingsPage.svelte`: Settings' sections (`ALL_SECTIONS`) and their
  shared styles.
- `ServicesPanel.svelte`: Settings › Online sources.
- `Dialog.svelte`: a modal dialog (the native `<dialog>`).
- `Dialogs.svelte`: whichever dialog `ui.dialog` asks for.
- `FindDetailsDialog.svelte`: pick an album's release from each source.
- `FindArtistDialog.svelte`: pick an artist's MusicBrainz match.
- `ChooseCoverDialog.svelte`: pick an album's cover.
- `TrackInfoDialog.svelte`: Get Info.
- `PrefsDialog.svelte`: playback preferences for a track or album.
- `SimilarDialog.svelte`: "More like this".
- `SmartPlaylistDialog.svelte`: a smart playlist's rules.
- `ShortcutsDialog.svelte`: the keyboard shortcuts.
- `NoticesDialog.svelte`: the third-party notices.
- `DbRepairDialog.svelte`: the offer to restore or rebuild a damaged
  database.
- `ContextMenu.svelte`: the right-click menu, with submenus and keyboard.
- `DragGhost.svelte`: what a drag carries, beside the pointer.
- `Art.svelte`: a cover from the `anomp-art` scheme, with a placeholder.
- `Heart.svelte` and `Stars.svelte`: a favourite toggle and a rating.
- `Fold.svelte`: a section that folds away, remembered per viewer.
- `Icon.svelte`: the icons, as 24×24 paths.
- `Toasts.svelte`: the toasts at the bottom of the window.

### `lib/components/settings/`

One component per Settings section, plus a shared control.

- `LibraryFolders.svelte`: folders, rescans, file watching, export and
  import.
- `SortRules.svelte`: the sort and grouping rules and ignored articles.
- `DisplayOptions.svelte`: what lists and album pages show.
- `AppearanceOptions.svelte`: the theme picker and editor.
- `PlaybackOptions.svelte`: the output device, buffer, ReplayGain.
- `EqualiserOptions.svelte`: the equaliser.
- `EffectsOptions.svelte`: the effects.
- `RecordingOptions.svelte`: recording's folder and format.
- `VisualizerOptions.svelte`: the visualizer's options.
- `FeaturesOptions.svelte`: every optional feature's switch and options.
- `GeneralOptions.svelte`: menu-bar controls, mini player,
  notifications, shortcuts.
- `AboutOptions.svelte`: versions, logs, diagnostics, notices, updates.
- `OrderedChoices.svelte`: a reorderable subset of options.

### `lib/components/dev/`

- `DevPage.svelte`: the /dev page: core version, test tone, loading files
  by path (debug builds only).

### `lib/visualizer/`

Canvas renderers outside Svelte, and the pure analysis they share.

- `index.ts`: the visualizations in picker order.
- `types.ts`: what a renderer draws from and must provide.
- `frame.ts`: decodes the backend's binary analysis frames.
- `combine.ts`: visualizations made of two others.
- `palette.ts`: colours from the current cover.
- `safety.ts`: the flash guard (at most three flashes a second).
- `key.ts`: key estimation from chroma.
- `music.ts`: triads, intervals, the Tonnetz, plate modes, tempo.
- `recurrence.ts`: the recurrence plot's self-similarity matrix.
- `util.ts`: smoothing, colour and stage helpers.

### `lib/visualizer/renderers/`

One visualization each (`harmony.ts` and `resonance.ts` combine two).

- `bars.ts`, `covers.ts`, `cymatics.ts`, `fifths.ts`, `harmonograph.ts`,
  `harmony.ts`, `kaleidoscope.ts`, `portrait.ts`, `recurrence.ts`,
  `resonance.ts`, `rhythm.ts`, `ridges.ts`, `scope.ts`, `spiral.ts`,
  `stage.ts`, `tonnetz.ts`, `vectorscope.ts` and `vu.ts`: spectrum bars,
  the cover wall, cymatics, the circle of fifths, a harmonograph, harmony
  (spiral and Tonnetz), a kaleidoscope, a phase portrait, a recurrence
  plot, resonance (cymatics and portrait), rhythm rings, ridgelines, an
  oscilloscope, a pitch spiral, the stereo stage, the Tonnetz, a
  vectorscope and VU meters.
