# ano-mp

A music player for macOS, with iPhone/iPad, Linux and Windows to follow. It plays
MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis, Opus, WAV, AIFF and WMA, shows file metadata
enriched from services such as MusicBrainz, and has an admin screen for choosing
displayed fields, library sorting and visualization.

**Status:** in development, macOS only so far. Working: gapless playback of a
play queue, a library of your music folders with browsing, search and cover art,
macOS Now Playing and media keys, online details (MusicBrainz, Cover Art Archive,
Wikipedia, optionally Discogs) and visualizations. The admin screen is next. See
[PLAN.md](PLAN.md) for the roadmap and current phase.

## How it's built

| Layer | Location | Technology |
|---|---|---|
| Audio core: decoding, playback, tags, device output | `core/` | C++20, [JUCE](https://juce.com), [FFmpeg](https://ffmpeg.org), [TagLib](https://taglib.org) |
| Backend: library database, settings, online services | `app/src-tauri/` | Rust, [Tauri 2](https://tauri.app), SQLite |
| Frontend | `app/src/` | Svelte 5, TypeScript |

The core is a static library with a plain C API (`core/include/anomp/anomp.h`) that
the Rust backend links directly; it does not run as a separate process.
[PLAN.md](PLAN.md) §1 explains why.

## Requirements (macOS)

- macOS 14 (Sonoma) or later, Apple silicon or Intel
- Xcode Command Line Tools (`xcode-select --install`). The full Xcode app is only
  needed to create a signing certificate (see [The app bundle](#the-app-bundle))
  and, later, for iPhone/iPad builds.
- [Homebrew](https://brew.sh) packages:
  ```sh
  brew install cmake ninja nasm pkg-config node uv
  ```
  CMake 3.25 or later is required. `uv` runs the pinned code formatters.
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

2. **Build and test the audio core.** The first configure downloads JUCE, TagLib
   and Catch2 and takes several minutes:
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
   which downloads JUCE again. Later runs are fast. Add a music folder from the
   sidebar to start.

## The app bundle

To build a standalone `ano-mp.app` (from `app/`):

```sh
npm run tauri build -- --bundles app
```

It lands in `app/src-tauri/target/release/bundle/macos/`. Unlike `tauri dev`, the
bundle runs in the macOS App Sandbox, keeps its data in
`~/Library/Containers/dev.anomp.player/` (separate from the dev app's library) and
has FFmpeg embedded. Rebuild it after pulling changes; it doesn't update itself.

**Signing and your music folders.** The sandbox lets the app read a folder you
picked only through a saved bookmark, and macOS honours a bookmark only for the
app identity that saved it. The bundle is ad-hoc signed by default
(`signingIdentity: "-"` in `tauri.conf.json`), which gives every build a new
identity, so after each rebuild its tracks are skipped until you add each music
folder again ("Add a folder…" and pick the same folder: this renews its access and
keeps everything else). The dev app repairs this by itself.

To stop that, sign with a free Apple Development certificate, which stays the same
across builds:

1. Install Xcode, add your Apple ID under **Settings → Accounts**, then
   **Manage Certificates… → + → Apple Development**.
2. Find its name with `security find-identity -v -p codesigning`, for example
   `Apple Development: you@example.com (TEAMID1234)`.
3. Build with it:
   ```sh
   APPLE_SIGNING_IDENTITY="Apple Development: you@example.com (TEAMID1234)" \
     npm run tauri build -- --bundles app
   ```
4. Add your music folders again once. Later builds keep them, until the
   certificate is renewed (yearly).

A bundle signed this way runs only on your own Macs; distribution needs Developer
ID signing and notarization ([PLAN.md](PLAN.md) §8).

## Everyday commands

| Task | Command | Run from |
|---|---|---|
| Run the app with live reload | `npm run tauri dev` | `app/` |
| Build the app bundle | `npm run tauri build -- --bundles app` | `app/` |
| Core tests (C++) | `cmake --build --preset debug && ctest --preset debug` | repo root |
| One core test by name | `ctest --preset debug -R "<name regex>"` | repo root |
| Backend tests (Rust) | `cargo test` | `app/src-tauri/` |
| Live checks against the online services | `cargo test live_ -- --ignored` | `app/src-tauri/` |
| Frontend tests | `npm test` | `app/` |
| Frontend type check | `npm run check` | `app/` |
| Format after editing (add `--check` to only report) | `scripts/format-cpp.py`, `scripts/format-rust.py`, `scripts/format-python.py` | repo root |
| Rebuild FFmpeg from scratch | `scripts/build-ffmpeg.sh --force` | repo root |
| Regenerate decoder test fixtures (rarely; needs `brew install ffmpeg vorbis-tools`) | `scripts/make-test-fixtures.py` | repo root |

## Troubleshooting

- **`FFmpeg not found in …third_party/ffmpeg/macos-universal`**: run
  `scripts/build-ffmpeg.sh` from the repository root, then configure again.
- **The first build seems stuck:** JUCE is being downloaded (once for the CMake
  build, once for the Cargo build), which can take several minutes.
- **Tracks are skipped with "Folder not available … Cannot resolve the bookmark:
  The file couldn't be opened because it isn't in the correct format":** the app
  bundle was rebuilt with a different signature. Add the folder again, or sign
  with a certificate (see [The app bundle](#the-app-bundle)).
- **"Folder not available" for a folder on an external or network drive:** the
  drive isn't mounted. Mount it and try again; the library keeps its tracks
  meanwhile.
- **No sound:** check that the output device shown under "Developer tools" (at
  the bottom of the sidebar) is the one
  you are listening on, that it isn't muted, and that the volume slider in the
  player bar isn't at zero.

## Repository layout

```
core/          C++ audio core (static library), its C API and Catch2 tests
app/           Tauri app: Svelte frontend (src/) and Rust backend (src-tauri/)
scripts/       build-ffmpeg.sh, the format scripts and other tooling
cmake/         CMake helpers (FFmpeg imported targets, TagLib)
third_party/   locally built FFmpeg (git-ignored)
PLAN.md        roadmap, decisions, risks and release plan
CLAUDE.md      conventions and notes for AI-assisted development
```

## License

Proprietary; all rights reserved. Third-party components keep their own licenses:
JUCE is used under a commercial license, FFmpeg under the LGPL 2.1 or later as
shared libraries, and TagLib under the MPL 1.1. See [PLAN.md](PLAN.md) §4.
