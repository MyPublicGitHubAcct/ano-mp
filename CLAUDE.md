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
Current state: the C++ core plus a scaffolded Tauri app that links it and shows
`anomp_version()` (Phase 0 in progress); `docs/` is empty.

## Build & test

```sh
cmake --preset debug && cmake --build --preset debug && ctest --preset debug
```

JUCE 9.0.2 and Catch2 v3.16.0 are pinned in the top-level `CMakeLists.txt` and fetched
by FetchContent into `build/<preset>/_deps` — the first configure takes several minutes.
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
```

`app/src-tauri/build.rs` builds `anomp_core` with the `cmake` crate (Ninja, tests off)
into Cargo's `target/` dir, separate from `build/<preset>`, so the first Cargo build
fetches JUCE again. `build.rs` reruns when `core/` or the top-level `CMakeLists.txt`
changes. FFI declarations and their safe wrappers live only in
`app/src-tauri/src/anomp.rs`; add a wrapper there for each new C API function.

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
  wrapped as a single JUCE `AudioFormat` so nothing else in the core knows FFmpeg
  exists. `FormatRegistry` currently calls JUCE's `registerBasicFormats()` as a
  placeholder and is replaced in Phase 1.
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
