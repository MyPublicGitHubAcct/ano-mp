# ano-mp

A music player for macOS, with iPhone/iPad, Linux and Windows to follow. It plays
MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis, Opus, WAV, AIFF and WMA, shows file metadata
enriched from services such as MusicBrainz, and has an admin screen for choosing
displayed fields, library sorting and visualization.

**Status:** early development. The app currently opens a window that shows the audio
core's version, plays a test tone and reports audio device changes. Library,
playback and metadata features are in progress; see [PLAN.md](PLAN.md) for the
roadmap and current phase.

## How it's built

| Layer | Location | Technology |
|---|---|---|
| Audio core: decoding, playback, device output | `core/` | C++20, [JUCE](https://juce.com), [FFmpeg](https://ffmpeg.org) |
| Backend: library database, settings, online services | `app/src-tauri/` | Rust, [Tauri 2](https://tauri.app) |
| Frontend | `app/src/` | Svelte 5, TypeScript |

The core is a static library with a plain C API (`core/include/anomp/anomp.h`) that
the Rust backend links directly; it does not run as a separate process.
[PLAN.md](PLAN.md) §1 explains why.

## Requirements (macOS)

- macOS 14 (Sonoma) or later, Apple silicon or Intel
- Xcode Command Line Tools (`xcode-select --install`); the full Xcode app is not
  needed yet
- [Homebrew](https://brew.sh) packages:
  ```sh
  brew install cmake ninja nasm pkg-config node
  ```
  CMake 3.25 or later is required.
- Rust (stable), via [rustup](https://rustup.rs):
  ```sh
  curl https://sh.rustup.rs -sSf | sh
  ```

## First-time setup

Run these from the repository root. You need an internet connection for the first
build.

1. **Build FFmpeg** (about 1.5 minutes). This downloads a pinned FFmpeg release,
   checks it and builds audio-only libraries into `third_party/ffmpeg/`:
   ```sh
   scripts/build-ffmpeg.sh
   ```
   The CMake build refuses to configure until this has run. Rerunning it does
   nothing unless the pinned version or its settings change.

2. **Build and test the audio core.** The first configure downloads JUCE and Catch2
   and takes several minutes:
   ```sh
   cmake --preset debug && cmake --build --preset debug && ctest --preset debug
   ```

3. **Install the frontend dependencies:**
   ```sh
   cd app && npm install
   ```

4. **Run the app** (from `app/`):
   ```sh
   npm run tauri dev
   ```
   The first run takes a few minutes: Cargo builds its own copy of the audio core,
   which downloads JUCE again. Later runs are fast.

## Everyday commands

| Task | Command | Run from |
|---|---|---|
| Run the app with live reload | `npm run tauri dev` | `app/` |
| Core tests (C++) | `cmake --build --preset debug && ctest --preset debug` | repo root |
| One core test by name | `ctest --preset debug -R "<name regex>"` | repo root |
| Backend tests (Rust) | `cargo test` | `app/src-tauri/` |
| Frontend type check | `npm run check` | `app/` |
| Rebuild FFmpeg from scratch | `scripts/build-ffmpeg.sh --force` | repo root |
| Regenerate decoder test fixtures (rarely; needs `brew install ffmpeg vorbis-tools`) | `scripts/make-test-fixtures.py` | repo root |

## Troubleshooting

- **`FFmpeg not found in …third_party/ffmpeg/macos-universal`**: run
  `scripts/build-ffmpeg.sh` from the repository root, then configure again.
- **The first build seems stuck:** JUCE is being downloaded (once for the CMake
  build, once for the Cargo build), which can take several minutes.
- **No sound from the test tone:** check that the "Output device" shown in the app
  is the one you are listening on, and that it isn't muted.

## Repository layout

```
core/          C++ audio core (static library), its C API and Catch2 tests
app/           Tauri app: Svelte frontend (src/) and Rust backend (src-tauri/)
scripts/       build-ffmpeg.sh and other tooling
cmake/         CMake helpers (FFmpeg imported targets)
third_party/   locally built FFmpeg (git-ignored)
PLAN.md        roadmap, decisions, risks and release plan
CLAUDE.md      conventions and notes for AI-assisted development
```

## License

Proprietary; all rights reserved. Third-party components keep their own licenses:
JUCE is used under a commercial license, and FFmpeg under the LGPL 2.1 or later as
shared libraries. See [PLAN.md](PLAN.md) §4.1.
