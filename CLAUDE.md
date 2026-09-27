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
2.3.2 (tarball + SHA-256) in `cmake/TagLib.cmake`, all fetched by FetchContent into
`build/<preset>/_deps` — the first configure takes several minutes. TagLib is a
separate static lib (`libtag.a`), so `build.rs` links it next to `anomp_core`.
The `release` preset sets `ANOMP_BUILD_TESTS=OFF`, so tests only run in `debug`.

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
cd src-tauri && cargo test   # Rust tests, including the C API wrappers
../scripts/format-rust.py    # rustfmt the Rust code after editing it (--check: diff only)
```

`app/src-tauri/build.rs` builds `anomp_core` with the `cmake` crate (Ninja, tests off)
into Cargo's `target/` dir, separate from `build/<preset>`, so the first Cargo build
fetches JUCE again. `build.rs` reruns when `core/` or the top-level `CMakeLists.txt`
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
sandboxed bundle (`npm run tauri build -- --bundles app`: ad-hoc signed,
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

Search uses FTS5 tables kept in step by triggers (migration 002); a schema change
to `tracks`, `artists` or `albums` columns they index must update those triggers
in a new migration.

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
