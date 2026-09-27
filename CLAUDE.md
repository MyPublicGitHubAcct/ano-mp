# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

ano-mp is a music player for desktop (macOS, later Linux/Windows) and iPhone/iPad. It
plays MP3, FLAC and other common formats, shows file metadata enriched from services
such as MusicBrainz, and has an admin screen for configuring displayed fields, enabled
services, library sort/grouping rules and visualization preferences.

`PLAN.md` is the authoritative roadmap: phased plan, §4 decisions (JUCE commercial
license, FFmpeg, TagLib, Svelte 5, minimum OS targets), risks and release gates. Read it before starting
anything non-trivial, and update it when a phase completes or a decision is made.
Current state: Phases 0–3 are complete: the player UI (queue, browser,
search, cover art, now-playing bar) and OS media integration (macOS Now
Playing and media keys: `MediaControls` in the core, hosted by
`app/src-tauri/src/media.rs`, which keeps it in step with the queue). The C++ core
plays any supported file with gapless hand-off to a pre-opened next track;
the Rust queue (`app/src-tauri/src/queue/`) keeps it armed across the whole
queue. The Svelte UI is in `app/src/lib/` (`api.ts` has the payload types) and
`app/src/routes/`; the old dev panels are at `/dev`.
The core reads tags and art (`anomp_read_tags`, TagLib), the Rust library
(`app/src-tauri/src/library/`: SQLite DB and incremental folder scanner) fills
from it, and `library_browse` pages through it under configurable sort/grouping
rules (stored in `settings`). Each folder keeps a security-scoped bookmark
(`library/access.rs`, over the core's `FolderAccess`), and the bundled app is
sandboxed (`Entitlements.plist`). Phase 4 (online metadata) is in progress:
`app/src-tauri/src/metadata/` has the source settings (which sources supply
each kind of data, in what order), the rate-limited HTTP client, folder-image
art, MusicBrainz matching, Cover Art Archive covers downloaded into an
on-disk image cache (`metadata/images.rs`), and the metadata worker: a
thread (`metadata/worker.rs`) running `metadata/jobs.rs`, which matches
albums and artists and fetches covers and biographies by priority, pauses
while a service is unreachable, and emits `metadata-changed` and
`metadata-progress`. Artist pages (`library/artists.rs`,
`ArtistPage.svelte`) show MusicBrainz facts (`metadata/artists.rs`) and a
Wikipedia biography (`metadata/wikipedia.rs`, CC BY-SA: always credit the
article and licence where it's shown); the album header shows a Wikipedia
description of the album the same way, found through its MusicBrainz
release group. An artist page links to the artist's releases the library
lacks (`metadata/discography.rs`, `DiscographyPage.svelte`, from
MusicBrainz's release-group browse). The album details
(`library/albums.rs`), the "Find details", "Choose cover" and "Find artist"
dialogs (commands in `metadata/commands.rs`) and the Online sources panel
(`ServicesPanel.svelte`) are done; anything a command needs from a service
runs on the worker through `worker::call`. 4.8 settled which sources ship
(the sources table in `PLAN.md` Phase 4 has each decision): Discogs is an
opt-in second album-details source (`metadata/discogs.rs`) behind
`albums::ReleaseSource`, which MusicBrainz implements too. What remains of
Phase 4 is checking its exit in the app.
Phase 5 (visualization) is built: `PlayerEngine` writes what it plays to a
`SignalTap`, `AnalysisThread` runs `SpectrumAnalyser` over it while a
callback is set (`anomp_engine_set_analysis_callback`, called on that
thread, never the main one), `app/src-tauri/src/visualizer.rs` streams
binary frames over a Tauri `Channel` while the UI subscribes, and the
renderers in `app/src/lib/visualizer/` draw them (the cover wall's albums
come from `library/covers.rs`). What remains is checking them by eye with
music playing.
Phase 6 (settings) is built: `settings.rs` holds the typed `AppSettings`
(display, playback/ReplayGain, output device, visualizer) under `app` in
`settings`, applied by `settings_save`; the UI is `SettingsPage.svelte` with
its sections in `components/settings/`, and reads the settings through
`state/settings.svelte.ts` (loaded in `routes/+layout.ts` before any page
renders). What remains is checking device switching and ReplayGain by ear.
Phase 6b (the optional features O1–O19, `PLAN.md` §4.6) is built: each
has a switch in Settings › Features (`FeatureSettings` in `settings.rs`,
`FeaturesOptions.svelte`), and code that honours it. The core analyses
files (`FileAnalyser`, `anomp_analyse_file`: loudness, peaks, silences,
spectrum cutoff, waveform), plays parts of files with an optional skip
region (`anomp_track_options`), loops A–B, stretches tempo and pitch
(Signalsmith Stretch, `cmake/Signalsmith.cmake`), crossfeeds
(`Crossfeed`) and reports the signal path. Rust has the analysis worker
(`library/analysis.rs`), how each track is played (`library/playback.rs`:
parts, trims, gain offsets, computed ReplayGain, silence skips), cue
sheets (`library/cue.rs`), discovery, health, lyrics and preferences
(`library/{discover,health,lyrics,prefs}.rs`), the listening history and
ListenBrainz (`history/`), library radio (`queue/radio.rs`) and the LAN
remote (`remote/`). The UI adds the Home, History and Library health
views. What remains is checking them in the app (Phase 6b's exit).
The expected features F1–F21 (`PLAN.md` §4.7, Phase 6c) are built:
playlists and smart playlists (`library/playlists.rs`, `library/smart.rs`),
favourites and ratings (`library/marks.rs`), moved files keeping their ids
(the scanner), credited artists and compilations, substring search
(migrations 007–009), folder watching and rescans at launch
(`library/watch.rs`), Get Info (`library/info.rs`), files opened from outside
the library (`library/external.rs`), data export and import
(`library/transfer.rs`), crossfade and the `Equaliser` in the core, and the
app outside its page in `app/src-tauri/src/shell/` (menus, the Dock menu,
the menu-bar item, the mini player, opened files, notifications). What
remains is checking them in the app (Phase 6c's exit).
`docs/` is empty.

## Build & test

```sh
scripts/build-ffmpeg.sh   # once, and after changing its pin/flags (~1.5 min)
cmake --preset debug && cmake --build --preset debug && ctest --preset debug
scripts/format-cpp.py     # clang-format core/ after editing it (--check: report only)
scripts/format-python.py  # ruff format scripts/*.py after editing them (--check: diff only)
```

The format scripts are Python so they run on every platform (on Windows, run them
with `py`). The C++ and Python ones run a pinned formatter through `uvx` (`brew
install uv`); `.clang-format` is JUCE style, `core/include/.clang-format` keeps
`anomp.h` in C style.

FFmpeg is built by `scripts/build-ffmpeg.sh` (pinned version, LGPL, audio-only,
shared) into `third_party/ffmpeg/<platform>/` (git-ignored). CMake refuses to
configure without it. `cmake/FFmpeg.cmake` exposes it as `FFmpeg::avformat`,
`FFmpeg::avcodec`, `FFmpeg::swresample`, `FFmpeg::avutil`. To add a format, add its
demuxer/decoder/parser to the script's lists (the configure flags stay minimal on
purpose) and extend `core/tests/FFmpegBuildTests.cpp`.

Decoder tests use committed fixtures in `core/tests/fixtures/`, all encoding one
deterministic chirp that the tests regenerate (`core/tests/TestSignal.h`, which must
match `scripts/make-test-fixtures.py`). Regenerating needs `brew install ffmpeg
vorbis-tools` — Homebrew's FFmpeg is only a fixture encoder, never linked. Lengths and
lags in the tests' fixture table are properties of the encoded files, so update them if
you regenerate. The script rewrites every fixture, but only the Vorbis ones change
(oggenc picks random stream serials): `git checkout` them unless you meant to change
them. The two `tagged-*` fixtures carry the tags `TagReaderTests.cpp` expects.

JUCE 9.0.2 and Catch2 v3.16.0 are pinned in the top-level `CMakeLists.txt`, TagLib
2.3.2 (tarball + SHA-256) in `cmake/TagLib.cmake`, Signalsmith Stretch 1.4.0 and its
FFT library (MIT, header-only, tarballs + SHA-256) in `cmake/Signalsmith.cmake`, all
fetched by FetchContent into
`build/<preset>/_deps` — the first configure takes several minutes. TagLib is a
separate static lib (`libtag.a`), so `build.rs` links it next to `anomp_core`.
The `release` preset sets `ANOMP_BUILD_TESTS=OFF`, so tests only run in `debug`.
Tauri's crates and plugins are pinned exactly in `app/src-tauri/Cargo.toml`, and
their npm packages exactly in `app/package.json`; `tauri build` refuses mismatched
major.minor versions, so update each crate and its npm package together.

Each Catch2 `TEST_CASE` is registered with CTest individually via `catch_discover_tests`:

```sh
ctest --preset debug -N                              # list test names
ctest --preset debug -R "FormatRegistry round-trips" # run one by name (regex)
./build/debug/core/tests/anomp_core_tests "[formats]" # or run the binary by tag
```

The app (run from `app/`; `npm install` once):

```sh
npm run tauri dev            # run the desktop app
npm run check                # svelte-check / TypeScript
npm test                     # frontend unit tests (node --test tests/, plain .mjs)
cd src-tauri && cargo test   # Rust tests, including the C API wrappers
ANOMP_WRITE_BINDINGS=1 cargo test bindings  # regenerate src/lib/generated/settings.ts
../scripts/format-rust.py    # rustfmt the Rust code after editing it (--check: diff only)
```

`app/src-tauri/build.rs` builds `anomp_core` with the `cmake` crate (Ninja, tests off)
into Cargo's `target/` dir, separate from `build/<preset>`, so the first Cargo build
fetches JUCE again. `build.rs` reruns when `core/`, `cmake/` or the top-level `CMakeLists.txt`
changes. FFI declarations and their safe wrappers live only in
`app/src-tauri/src/anomp.rs`; add a wrapper there for each new C API function.

The library DB schema changes only by appending a numbered SQL file to
`app/src-tauri/src/library/migrations/` and listing it in `MIGRATIONS`
(`library/db.rs`); never edit a migration that has shipped. Tracks store paths
relative to their folder, '/'-separated (`library::track_path` joins them).
`db::configure` registers SQL functions (`anomp_sort_key`, `anomp_genres`,
`anomp_has_genre`) on every connection. Open connections only through `db`, and
never use these functions in the schema, an index or a migration, since other
SQLite clients don't have them. Browse queries (`library/browse.rs`) are built
from fixed SQL fragments; bind every value, never format it in.

Under the macOS sandbox a library file can be opened only while its folder's
bookmark is resolved, so anything that opens library files goes through
`library::access::open_folder`/`open_folder_of` first and holds the result
until the file is open (as `scan_folder`, `player_load` and `player_set_next`
do). `tauri dev` runs unsandboxed, so a missing call only fails in a
sandboxed bundle. A security-scoped bookmark resolves only in the build
that made it while the app is ad-hoc signed (the identity changes each
build; the error is "isn't in the correct format"): `open_folder` replaces
it when the folder is still readable, as in dev, but a rebuilt sandboxed
bundle needs its folders picked again (`add_folder` on a library folder
replaces its bookmark) (`npm run tauri build -- --bundles app`: ad-hoc signed,
hardened runtime off, FFmpeg embedded in `Contents/Frameworks`). The
`bundle.macOS.frameworks` list in `tauri.conf.json` names FFmpeg's major
versions, so update it when the FFmpeg pin changes. Only debug builds have an
rpath into `third_party/`; `cargo test --release` gets it from
`app/src-tauri/.cargo/config.toml`.

`PlayerEngine` (`core/src/PlayerEngine.*`) is a plain `juce::AudioSource` with no
device; `AudioEngine` owns the device and feeds it. Tests render it offline by calling
`getNextAudioBlock` directly (`core/tests/PlayerEngineTests.cpp`); `PLAN.md` Phase 1
records its design (why not `AudioTransportSource`, the host-owned queue, threading).

**The engine is main-thread only.** JUCE's message loop rides on the main run loop
that Tauri runs, so `anomp_engine_*` calls must happen on the main thread and event
callbacks arrive there. `app/src-tauri/src/audio.rs` enforces this: the engine lives
in a main-thread `thread_local`, commands go through `with_engine` (which hops to
the main thread if needed), and the engine is dropped on `RunEvent::Exit`. The queue
lives on the main thread too (`queue::run`); do database work before hopping there,
and never hold the library connection while waiting for the main thread. Queue logic
goes in `queue/model.rs` behind the `Player` trait, tested against a fake engine.
The media controls (`anomp_media_controls_*`) are main-thread only too, and
`media.rs` decides what to publish in `NowPlaying`, behind a `Publisher` trait.
`run_on_main_thread` called on the main thread runs the closure at once; it
doesn't defer it. Engine event callbacks may call the engine (the queue arms
the next track from `TrackEnded`); `anomp.h` states the rule.

The settings' TypeScript types (`app/src/lib/generated/settings.ts`) are
generated from the Rust types by ts-rs (a dev-dependency: derive it with
`#[cfg_attr(test, derive(ts_rs::TS))]` and list the type in
`settings::bindings`); `cargo test` fails while the file is stale, so
regenerate and commit it after changing them. Never edit it by hand. A new
setting goes in `AppSettings` with a default and a `validate` rule; stored
values are read leniently, so no migration is needed. Track gains
(ReplayGain) are computed in Rust (`PlaybackSettings::gain`) and passed to
the engine with each track.

A track is a part of a file: `tracks` is unique on (folder, path,
`range_start`), with `range_end` NULL for the end of the file (migration
005), so a cue sheet's tracks or a file's chapters are rows of one file.
Anything that opens a track for playback goes through
`library::playback::track_play`, which turns the row, the user's
preferences, the analysis and the feature settings into the engine's
`TrackOptions`; refresh a gain with `set_track_gain_at` (by file and
start), never by file alone. Each optional feature checks its switch in
`FeatureSettings` where it acts (commands refuse, workers idle, the UI
hides); a new one gets a switch there, off by default if it costs a lot,
changes what is heard, goes online or listens on the network. The LAN
remote answers local addresses only and keeps only hashes of tokens; any
change to `remote/` needs the security review in `PLAN.md` §8.1.

Search uses FTS5 tables kept in step by triggers: word indexes (recreated by
migration 008 over `IFNULL(artist_credit, …)`) and trigram indexes (009). A
schema change to `tracks`, `artists` or `albums` columns they index must update
both sets of triggers in a new migration. Run the search benchmark
(`library/bench.rs`) after changing the search SQL.

Every string the UI shows comes from `app/src/lib/i18n/en.json` through `t`
(typed keys; a plural message is an object of `Intl.PluralRules` forms), never
a literal in a component. Rust errors the UI shows are made with
`crate::coded` (`{code, params, message}`), each code with an `error.<code>`
message in `en.json` (`tests/i18n.test.mjs` checks), and the UI shows a
command's error through `errorText`, never `String(error)`.

Each window gets only the commands it needs: `build.rs` reads the handlers in
`lib.rs`'s `generate_handler!` and writes the main window's permission set, so
a new command needs no step for the main window; add it to
`permissions/mini-window.toml` if the mini player calls it.

Metadata services (`app/src-tauri/src/metadata/`) go through `http::Client`,
which rate-limits per host and backs off when offline; never call a service
another way. Tests never touch the network: they use the fake `Transport` and
`Clock` (`http::testing`) with recorded responses in `metadata/fixtures/`, and
live checks are `#[ignore]`d (`cargo test live_ -- --ignored`). A link row
with `chosen_by = 'user'` or an `album_art` row is the user's pick, and
automatic matching must never replace it. The `anomp-art` handler serves
only local and already-downloaded pictures; it never goes online.

Discogs' terms shape its code: store only the match (`album_links` with
`details` NULL; `SourceInfo::stores_details` is false), fetch its data
through `Client::get_json_fresh` (memory only, at most `discogs::MAX_AGE`,
no offline copy), never fetch or show its images, and show
`discogs::CREDIT` linked to the release page next to its data. Keys
(the Discogs token) live in the OS keychain through `metadata::keys`,
never in the settings JSON, which records only `hasKey`; tests use its
in-memory store.

## Architecture

Three layers, described in full in `PLAN.md` §1:

1. `core/` — `anomp_core`, a C++20 JUCE static library: decoding, playback, queue and
   gapless, tag reading, FFT/levels for the visualizer, OS media integration.
2. `app/src-tauri` — Rust: SQLite library DB, settings,
   MusicBrainz/Cover Art Archive clients, file scanning; bridges UI to core.
3. `app/src` — Svelte 5 + TypeScript frontend (SvelteKit with `adapter-static`, from
   the Tauri template).

Constraints that shape the code and must not be broken casually:

- **The core is statically linked into the Tauri process, never a sidecar.** iOS forbids
  spawning helper processes, so an in-process static library is the only design that
  works on all targets. Rust's `build.rs` builds it with the `cmake` crate.
- **`core/include/anomp/anomp.h` is the only public surface.** Plain C only: no C++
  types and no exceptions across the boundary. Everything in `core/src` is private
  (the C API translates, e.g. `bool` → `int`, null pointers → a safe default). Async
  events (position, track-ended, spectrum frames) will travel through registered C
  callbacks that Rust forwards as Tauri events/channels.
- **Rust owns all non-audio services** (DB, HTTP, settings), keeping the core small
  and testable.
- **FFmpeg will decode every format on every platform** (decided; see `PLAN.md` §4.3),
  wrapped as a single JUCE `AudioFormat` (`core/src/FFmpegAudioFormat.*`) so nothing
  else in the core knows FFmpeg exists; only that `.cpp` includes FFmpeg headers.
  `PLAN.md` Phase 1 records its design and the container quirks behind it (why
  rewinding reopens the demuxer, the seek margins, the length rule) — read that
  before changing the reader.
- **Keep the core platform-neutral.** Platform code (media controls, file access, audio
  session) lives behind small interfaces with one implementation per OS; no AppKit or
  CoreAudio calls elsewhere. UTF-8 across the C API, no assumed `/` separators or
  case-sensitive paths, and folder access stored as security-scoped bookmarks rather
  than raw paths.

## Conventions

- C++ code follows JUCE style: `anomp` namespace, `juce::` qualified everywhere, a
  space before the argument list in declarations and calls (`canDecodeExtension (ext)`),
  Allman braces. `juce_recommended_warning_flags` is on — keep it warning-clean.
- New core source files must be added to the `add_library` list in `core/CMakeLists.txt`
  (and tests to `core/tests/CMakeLists.txt`); there is no globbing.
- Version `0.1.0` is currently duplicated in `CMakeLists.txt` and hard-coded in
  `anomp_version()`. Per `PLAN.md` §8.2 the CMake `project(VERSION)` becomes the single
  source of truth — don't add a third copy.
