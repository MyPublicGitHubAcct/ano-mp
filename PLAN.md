# ano-mp — Implementation Plan

Status as of 2026-09-25: repository skeleton in place, C++ core builds and its
Catch2 suite passes on macOS. The Phase 0–7 toolchain (§3) is installed.
Phase 0 complete: the Tauri app links the core, JUCE plays a test tone inside
the Tauri process, and device-change events reach the UI. Phase 1 complete:
FFmpeg decodes every format through `FFmpegAudioFormat`, and `PlayerEngine`
plays, pauses, seeks and hands off gaplessly to a queued next track, checked
by offline tests and by ear in the app. Phase 2 complete (2026-09-26): the
core reads tags and embedded art with TagLib, the Rust library database and
incremental folder scanner are in place, the library is browsed a page at a
time under configurable sort/grouping rules, and library folders are kept as
security-scoped bookmarks in a sandboxed app bundle. Phase 3 complete
(2026-09-26): the queue (with gapless hand-off across it, shuffle, repeat
and persistence), full-text search, cover art, the responsive player UI,
and macOS Now Playing and media keys through `MediaControls`. Phase 4
started (2026-09-26): steps 4.1–4.6 (source settings, HTTP client, folder
art, MusicBrainz matching, Cover Art Archive and the image cache, the
metadata worker that enriches the library in the background, and the album
details, "Find details", "Choose cover" and "Find artist" dialogs and the
Online sources panel) are done, and 4.7: artist pages with Wikipedia
biographies, and Wikipedia descriptions of albums. 4.8 done (2026-09-26):
every candidate source's terms were read and each decision recorded in the
sources table; Discogs ships, off by default, as a second album-details
source that keeps only its matches (details fetched when shown, the
user's token in the keychain), and the rest are excluded or wait for
written consent. What remains of Phase 4 is checking its exit in the app.
Phase 5 built (2026-09-26): the core analyses what the player plays on a
thread of its own while the visualizer is open, Rust streams the frames
to the webview in a compact binary form, and eight visualizations draw
them, including a wall of covers from the current track's year or
artist; what remains is checking them by eye with music playing.
Phase 6 built (2026-09-26): one Settings screen (⌘,) for the library's
folders, the sort rules, what lists and album pages show, playback (the
output device, its buffer size, ReplayGain) and the visualizer, plus the
online sources; the settings have a typed schema in Rust with generated
TypeScript types. What remains is checking device switching and
ReplayGain by ear. Nineteen optional features (O1–O19, §4.6) were
proposed on 2026-09-27, and all were accepted and built the same day
(Phase 6b): each can be turned on or off in Settings › Features. What
remains of them is checking them in the app. A review of the repo
on 2026-09-27 added a prioritised backlog: features users expect of any
library player that the app lacks (F1–F21, §4.7), and hardening for
security, robustness and maintenance (H1–H21, Phase 7). Their P1 items
are part of Phase 7's exit. All of F1–F21 were built the same day (Phase
6c), with H3's command permissions; what remains of them is checking
them in the app. On 2026-10-02 the order of the remaining work was set
(Phase 7, "Order of work"):
1. a CI baseline (done 2026-10-02: `scripts/check-all.py` and a GitHub
   Actions macOS job);
2. the quick P1 security fixes;
3. the P1 items that protect user data, including H22 (handling library
   folders that can't be found at launch);
4. the exit checks in a sandboxed bundle;
5. the larger core items (done 2026-10-02: fuzzing, opening files off
   the main thread, cloud placeholders);
6. alongside all of these, the owner's §8.1 decisions (briefs in
   `docs/release-decisions.md`, 2026-10-02);
7. the release setup that needs no certificate (done 2026-10-02: one
   version number, third-party notices, the release workflow, the macOS
   build, bundle and notarization scripts); the signed release waits on
   Step 6;
8. the signed release as far as the owner's decisions allow (begun
   2026-10-02: none decided yet, so its signing parts wait; done meanwhile:
   the clean-Mac smoke-test checklist and H14, the sandboxed bundle's
   self-test; then, 2026-10-03, the CI failure fixed, H18's budgets
   checked by `bench.py`, the design records moved to `docs/design/`
   (H20), typed IPC (H15), the queue stored as rows and sent as edits
   (H16) and cover thumbnails (H17)).

## 1. Architecture

```
┌──────────────────────────────────────────────┐
│ Frontend (web UI, TypeScript)                │  app/src
│ library views, now-playing, admin screen,    │
│ visualizer (canvas/WebGL)                    │
└───────────────▲──────────────────────────────┘
                │ Tauri commands + events/channels
┌───────────────┴──────────────────────────────┐
│ Tauri backend (Rust)                         │  app/src-tauri
│ library DB (SQLite), settings, MusicBrainz / │
│ Cover Art Archive client, file scanning,     │
│ bridges UI <-> core                          │
└───────────────▲──────────────────────────────┘
                │ C ABI (include/anomp/anomp.h), static link
┌───────────────┴──────────────────────────────┐
│ anomp_core (C++20, JUCE + FFmpeg)            │  core/
│ decoding, playback, queue/gapless, tag       │
│ reading, FFT/levels for visualization,       │
│ OS media integration (Now Playing etc.)      │
└──────────────────────────────────────────────┘
```

Why this split:

- **The core is linked in as a static library, not run as a sidecar process.**
  iOS does not allow spawning helper processes, so an in-process static library
  is the only design that works on every target. Rust's `build.rs` builds it
  with the `cmake` crate and links it.
- **The boundary is a plain C API** (no `cxx` crate or C++ types crossing).
  It is simple, stable and easy to test from both sides. Events such as
  position, track-ended and spectrum frames travel through registered C
  callbacks that Rust forwards as Tauri events or channels.
- **FFmpeg decodes every format on every platform**; JUCE does everything
  after decoding (resampling to the device, mixing, output, devices). FFmpeg
  is wrapped as a single JUCE `AudioFormat` (`FFmpegAudioFormat`), so the rest
  of the core doesn't know FFmpeg exists. One decoder means identical format
  support and gapless behaviour on all four OSes, and one test matrix.
- **Rust owns the non-audio services** (DB, HTTP, settings). This keeps the C++
  core small and testable, and those crates (`rusqlite`, `reqwest`, `serde`)
  are mature.

## 2. Current state (done)

| Item | Location |
|---|---|
| git repo, `.gitignore` | `/` |
| Top-level CMake with JUCE 9.0.2 + Catch2 v3.16.0 via FetchContent, each pinned by commit (an archive of the commit plus its SHA-256) | `CMakeLists.txt` |
| Presets `debug` / `release` (Ninja), and `asan` (Address and Undefined) / `tsan` sanitizer presets with workflow presets that build and run the Catch2 suite | `CMakePresets.json` |
| `anomp_core` static lib; `FormatRegistry` registers `FFmpegAudioFormat` only | `core/src` |
| `FFmpegAudioFormat`: FFmpeg-backed JUCE reader (float output, gapless trimming, exact seeks and lengths) | `core/src/FFmpegAudioFormat.*` |
| 21 committed audio fixtures (750 KB) of one deterministic chirp (two of them tagged, with cover art), and their generator | `core/tests/fixtures/`, `scripts/make-test-fixtures.py` |
| C API: `anomp_version`, `anomp_can_decode_extension`, `anomp_read_tags`, `anomp_engine_*` (device, player, asynchronous loads, events, advance count), `anomp_file_is_dataless`, `anomp_media_controls_*`, `anomp_set_log_callback`, `anomp_volume_watcher_*` | `core/include/anomp/anomp.h` |
| TagLib 2.3.2 (MPL, static, from the pinned release tarball, built with only the formats FFmpeg plays) and `TagReader`: tags, MusicBrainz IDs, embedded art | `cmake/TagLib.cmake`, `core/src/TagReader.*` |
| `PlayerEngine`: load/play/pause/stop/seek/volume, gapless next track, resampling to the device rate | `core/src/PlayerEngine.*` |
| `MediaControls`: OS Now Playing info and remote commands (Apple: `MPNowPlayingInfoCenter`/`MPRemoteCommandCenter`; no-op fallback elsewhere) | `core/src/MediaControls*` |
| Visualizer analysis: `SignalTap` (lock-free tap on the player's output), `SpectrumAnalyser` (bands, chroma, levels, triggered waveform, beats), `AnalysisThread`, `anomp_engine_set_analysis_callback` | `core/src/SignalTap.h`, `core/src/SpectrumAnalyser.*`, `core/src/AnalysisThread.*` |
| Output devices (list, open by name with a buffer size, device info), per-track gain switched sample-exactly at the hand-off, ReplayGain and R128 tags | `core/src/AudioEngine.*`, `core/src/PlayerEngine.*`, `core/src/TagReader.*` |
| 115 passing Catch2 tests, also clean under ASan, UBSan and TSan, including the fuzzer's past findings | `core/tests` |
| Fuzzing (H5): libFuzzer targets for the decoder and the tag reader, the `fuzz` preset (Homebrew `llvm@22`, ASan, UBSan, an instrumented static FFmpeg), 60 s each in `check-all.py`, 30 min weekly in CI | `core/fuzz/`, `scripts/run-fuzzers.py`, `.github/workflows/fuzz.yml` |
| Files opened off the main thread (H11): asynchronous loads in the engine and the C API, the queue's loading state and timeout; cloud placeholders (H12) recorded unread by scans, left by the analysis, downloaded when played | `core/src/PlayerEngine.*`, `app/src-tauri/src/queue/opening.rs`, `core/src/FileStatus*`, `app/src-tauri/src/library/scanner.rs` |
| Tauri 2 app (SvelteKit + `adapter-static`, Svelte 5, TS) under a strict Content Security Policy; Rust and Node pinned by `rust-toolchain.toml` and `.nvmrc` | `app/`, `app/src-tauri/tauri.conf.json` |
| `build.rs` builds `anomp_core` with the `cmake` crate and links it plus the Apple frameworks | `app/src-tauri/build.rs` |
| Safe Rust wrappers over the C API | `app/src-tauri/src/anomp.rs` |
| Library: SQLite schema and migrations, folders, incremental parallel scanner, sort/grouping rules, paged browsing, FTS5 search, cover art (`anomp-art` URI scheme), `library_*` commands | `app/src-tauri/src/library/` |
| Play queue: order, shuffle, repeat, gapless hand-off across it, persistence, `queue_*` commands and `queue-changed` event | `app/src-tauri/src/queue/` |
| OS media integration host: Now Playing kept in step with the queue and player, remote commands routed to the queue, artwork | `app/src-tauri/src/media.rs` |
| Metadata sources (Phase 4, in progress): source settings and order, HTTP client with rate limits, backoff and response cache, folder-image art, MusicBrainz search/lookup and album matching, Cover Art Archive covers and listings, the on-disk image cache, the metadata worker (job queue, priorities, background enrichment, offline pause, `metadata-changed` and `metadata-progress` events, calls for the dialogs), release/cover/artist candidates and the user's picks, Wikipedia artist biographies and album descriptions, Discogs as an opt-in second album-details source (only matches stored, the token in the keychain) | `app/src-tauri/src/metadata/` |
| Visualizer stream: frames encoded and sent over a Tauri `Channel` while subscribed (`visualizer_*` commands); the cover wall's albums (`library_cover_wall`) | `app/src-tauri/src/visualizer.rs`, `app/src-tauri/src/library/covers.rs` |
| Visualizer UI: eight canvas visualizations, picker, full screen, colours from the cover | `app/src/lib/visualizer/`, `app/src/lib/components/Visualizer*.svelte` |
| Settings: typed `AppSettings` (display, playback, output, visualizer) stored under `app`, lenient reading, applied on save; TypeScript types generated with ts-rs and checked by `cargo test` | `app/src-tauri/src/settings.rs`, `app/src/lib/generated/settings.ts` |
| Settings screen: library folders, sort rule editor, displayed fields, output device and buffer size, ReplayGain, visualizer, online sources | `app/src/lib/components/SettingsPage.svelte`, `app/src/lib/components/settings/` |
| Optional features O1–O19 (Phase 6b), each switched in Settings › Features: file analysis, parts of files, loops, tempo and pitch, crossfeed, signal path in the core; analysis, history, radio, discovery, health, lyrics, preferences and the LAN remote in Rust; their views in the UI | `core/src/FileAnalyser.*`, `core/src/Crossfeed.*`, `app/src-tauri/src/{library,history,queue,remote}/`, `app/src/lib/components/` |
| Expected features F1–F21 (Phase 6c): playlists and smart playlists, favourites and ratings, moves kept, credits and compilations, substring and field search, user-data export and import, folder watching and rescans at launch | `app/src-tauri/src/library/`, `app/src-tauri/src/collection.rs`, migrations 007–009 |
| Menus, Dock menu, menu-bar controls, mini player, files opened from the Finder, track-change notifications | `app/src-tauri/src/shell/`, `core/src/DockMenu*` |
| Crossfade and a 10-band equaliser in the engine; tag ratings, credits and full file info from the tag reader | `core/src/PlayerEngine.*`, `core/src/Equaliser.*`, `core/src/TagReader.*` |
| UI text in a typed message catalogue; coded errors from Rust | `app/src/lib/i18n/`, `app/src-tauri/src/coded.rs` |
| Library DB safety (H10): a copy before each migration, a check at launch with the restore or rebuild offer, `PRAGMA optimize` at exit, the response cache pruned | `app/src-tauri/src/library/db.rs`, `app/src-tauri/src/library/recovery.rs`, `app/src/lib/components/DbRepairDialog.svelte` |
| Missing folders (H22): each folder's state, scans that never empty a folder, the launch message, the queue passing over unavailable tracks, volumes watched so a drive that comes back is rescanned, unreadable folders' tracks dimmed in lists and left out of radio, smart playlists' play and Home's suggestions | `app/src-tauri/src/library/access.rs`, `app/src-tauri/src/library/availability.rs`, `core/src/VolumeWatcher*`, `app/src/lib/components/MissingFolders.svelte`, `app/src/lib/folders.ts` |
| Logs (H9): a rotating, redacted log file, the panic hook, the core's log (JUCE's Logger and failed assertions), the webview's errors, Settings › About with "Show logs" and "Copy diagnostics" | `app/src-tauri/src/logging.rs`, `app/src-tauri/src/diagnostics.rs`, `core/src/Log.*`, `app/src/lib/components/settings/AboutOptions.svelte` |
| 30 frontend tests (`npm test`: the queue's numbered edits, frame decoding, key estimation, selection, equaliser presets, the visualizer's flash guard, the message catalogue, theme contrast, web links, unreadable folders) | `app/tests/` |
| 433 passing `cargo test` tests (cover thumbnails and their index; the saved queue's rows, its migration and its numbered edits; the commands' generated TypeScript bindings; the first paint's timing; the bundle's self-test stages; the bundled notices; files opened off the main thread: loading, timeouts and skips; cloud placeholders in scans and the analysis; a drive unmounted mid-scan; database copies, checks and recovery, folder states and the scanner keeping a folder's tracks, unreadable folders left out of radio, smart playlists and suggestions, logs, redaction and diagnostics, playlists, smart playlists, favourites and ratings, moves, credits and compilations, substring search, sleep timer and stop after, crossfade arming, resume, data export and import, coded errors, settings and their bindings, ReplayGain gains, C API wrappers, schema, folders, scanner, sort keys, genres, rules, browsing, search, art sources and candidates, album details, queue, Now Playing sync, metadata settings and keys, HTTP client, MusicBrainz parsing and matching, Cover Art Archive, image cache, metadata worker, candidates and choices, Wikipedia, discographies, Discogs, visualizer frames and subscribers, cover walls), plus 7 ignored benchmarks (on 50,000 tracks: search, browsing, the queue's list and storage, inserts, a scan; cover thumbnails) and 6 ignored live tests (MusicBrainz, Cover Art Archive, biographies, descriptions, discographies, Discogs) | `app/src-tauri/src` |
| `AudioEngine` + `anomp_engine_*` C API: default output device, test tone, device-change event | `core/src/AudioEngine.*` |
| Pinned LGPL audio-only FFmpeg 9.0.2 (universal dylibs) and `FFmpeg::*` CMake targets | `scripts/build-ffmpeg.sh`, `cmake/FFmpeg.cmake` |
| Main-thread engine host; `audio_device_name`, test-tone and `player_*` commands; `player-*` events | `app/src-tauri/src/audio.rs` |
| Player UI: sidebar (views, folders, scanning, online sources), browser with album details, search, queue panel, now-playing bar, artist pages, the metadata dialogs and the Online sources panel; responsive down to 360 px, light and dark | `app/src/routes/+page.svelte`, `app/src/lib/` |
| Developer page (debug builds only, with its commands): device name, test tone, loading typed paths straight into the engine, event log | `app/src/lib/components/dev/DevPage.svelte`, `app/src-tauri/src/dev.rs` |
| Tauri dialog plugin (`dialog:allow-open`) for the dev UI's file picker | `app/src-tauri/src/lib.rs`, `app/src-tauri/capabilities/default.json` |
| One version number: CMake's `project(VERSION)`, compiled into `anomp_version()`, the other copies set and checked by `version.py` | `CMakeLists.txt`, `scripts/version.py` |
| Third-party notices, generated and checked, in the bundle and in Settings › About | `THIRD_PARTY_NOTICES`, `scripts/make-notices.py`, `scripts/licenses/`, `app/src/lib/components/NoticesDialog.svelte` |
| Release tooling (unsigned until the Developer ID exists): the tag-triggered workflow, the universal build, the bundle check, notarization, checksums and notes from `CHANGELOG.md`, certificate expiry | `.github/workflows/release.yml`, `scripts/{build-app,check-bundle,notarize,release,check-signing}.py`, `CHANGELOG.md` |
| One entry point for every check (`check-all.py`, `--quick` without the builds), the repo checks (C API bindings, core source lists, migrations), the scripts' 132 pytest tests (`test-python.py`), `ruff check`, gitleaks, clippy, `cargo deny`, ESLint and Prettier, the sanitizer runs, a pre-commit hook, Dependabot, and a GitHub Actions macOS job that runs `check-all.py` (by hand, and called by the release workflow) | `scripts/`, `scripts/tests/`, `scripts/hooks/`, `.github/` |
| The sandboxed bundle's self-test (H14): `--self-test` behind the `self-test` feature (scan, bookmark, covers, decoding and a gapless hand-off in the sandbox), built and run by `self-test-bundle.py` from `check-all.py` on CI | `app/src-tauri/src/self_test.rs`, `scripts/self-test-bundle.py` |
| The clean-Mac smoke test for each release (§8.3, §8.7 step 6), with H18's owner checks | `docs/release-smoke-test.md` |
| Finished phases' design notes and known limits (H20), the docs' paths, links and §2's counts checked (`check-docs.py`) | `docs/design/`, `scripts/check-docs.py` |
| Benchmarks against H18's budgets and a committed baseline (`bench.py`): the Rust ones on 50,000 tracks and the core's (`bench` preset) | `scripts/bench.py`, `scripts/bench-baseline.json`, `core/tests/BenchTests.cpp` |

Build and test:

```sh
cmake --preset debug && cmake --build --preset debug && ctest --preset debug
```

## 3. Prerequisites

Needed for Phases 0–7 (macOS only; the Command Line Tools are enough). All
installed on the dev machine as of 2026-09-25 (versions noted):

- [x] CMake ≥ 3.25 and Ninja (4.4.3, 1.13.2 via Homebrew); Apple clang (21.0.0,
      Command Line Tools, macOS SDK 27.0)
- [x] **Rust**: `curl https://sh.rustup.rs -sSf | sh` (rustc/cargo 1.98.1)
- [x] **Node.js**: `brew install node` (or fnm/nvm) (v26.10.0, npm 11.19.1 —
      the "Current" line until Node 26 enters LTS in October 2026; fall back to
      24 LTS if Tauri tooling objects)
- [x] Tauri CLI: `cargo install tauri-cli --version "^2" --locked`, or use the
      npm `@tauri-apps/cli` (tauri-cli 2.11.5)
- [x] FFmpeg build dependencies: `brew install nasm pkg-config` (nasm 3.02,
      pkgconf 3.0.7; the project builds its own FFmpeg; see Phase 1)
- [x] Check tools (H7, 2026-10-02): `brew install gitleaks uv` (gitleaks
      8.30.1) and `cargo install cargo-deny --version 0.20.2 --locked`.
      Rust and Node are pinned by `rust-toolchain.toml` and `.nvmrc`
      (rustup installs the pinned Rust by itself).

Not needed until Phase 8 (iOS/iPadOS):

- [ ] **Xcode** (full app, not just the Command Line Tools), then
      `sudo xcode-select -s /Applications/Xcode.app`
- [ ] iOS Rust targets: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
- [ ] Apple Developer account, needed to sign and run on physical iPhone/iPad devices

Not needed until Phases 9–10. Tauri apps can't practically be cross-compiled,
so each OS needs a native machine, VM or CI runner:

- [ ] **Linux (Phase 9):** Ubuntu 24.04+ machine or VM with Rust, Node.js, and
      `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
      (Tauri) plus `libasound2-dev libfreetype-dev libfontconfig1-dev` (JUCE)
- [ ] **Windows (Phase 10):** Windows 11 machine or VM with Rust (MSVC
      toolchain), Node.js, Visual Studio 2022 Build Tools (C++ workload), and
      WebView2 (preinstalled on Windows 11)

## 4. Decisions

All decided 2026-09-25 except the two legal checks flagged below, which are
release gates (§8.1) rather than engineering blockers. The optional
features in #6 were proposed and accepted on 2026-09-27. #7 (2026-09-27)
sets priorities rather than open questions.

1. **Licenses: JUCE commercial, closed source (decided).** JUCE 8+ is AGPLv3
   or commercial. We use the commercial license, starting on the free
   "Starter" tier and moving to a paid tier before revenue passes its cap
   (check the current JUCE 9 tiers then). This keeps the source closed and the
   App Store open to us. FFmpeg is used under the LGPL (see #3), which the app
   satisfies by shipping FFmpeg as replaceable shared libraries plus a license
   notice and source offer. That is standard on desktop.
   - **Open point: FFmpeg LGPL on the App Store.** Dynamic `.xcframework`s
     keep the libraries replaceable, but Apple's usage terms are the usual
     objection. Get a legal opinion before the first App Store submission.
2. **Frontend framework: Svelte 5 + TypeScript + Vite (decided).** Small
   bundle, and runes handle high-frequency state (position, levels) without
   re-render tuning. The visualizer draws on a canvas outside the framework.
3. **Decoder: FFmpeg for all formats (decided).**
   - Formats: MP3, FLAC, WAV, AIFF, Ogg Vorbis, Opus, AAC/M4A, ALAC and WMA on
     every platform. More (APE, WavPack, DSD) are a configure flag away.
   - Build: our own pinned, **LGPL, audio-only, shared** FFmpeg build (no
     `--enable-gpl`/`--enable-nonfree`, only the needed demuxers and
     decoders, plus libswresample; no video, network or encoders). This adds a
     few MB per platform. We don't use distro or Homebrew FFmpeg, whose builds
     vary (some strip AAC, Homebrew's is GPL).
   - **AAC: FFmpeg's decoder everywhere (decided), pending a licensing
     opinion before release.** Apple's and Microsoft's own decoders are
     covered by their vendors' licenses; FFmpeg's AAC decoder is not. Many
     early AAC patents have expired, but Via LA's pool still lists active ones
     in some countries.
   - **Fallback:** if the AAC or App Store opinions go badly, Apple builds
     route AAC/ALAC to CoreAudio (and Windows to Media Foundation). Only
     `FFmpegAudioFormat`'s registration changes.
4. **Tag library: TagLib 2.x under the MPL (decided).** JUCE reads audio but
   only minimal metadata. TagLib is dual-licensed LGPL-2.1 / MPL-1.1; we use
   it under the MPL so static linking on iOS is OK. It covers ID3v2, Vorbis
   comments, MP4 atoms and embedded art, and lives in the core behind
   `anomp_read_tags` (Phase 2).
5. **Minimum OS targets: modern (decided).** Platform order is macOS,
   iOS/iPadOS, then Linux, then Windows.

   | Platform | Minimum |
   |---|---|
   | macOS | 14 (Sonoma) |
   | iOS / iPadOS | 17 |
   | Linux | Ubuntu 24.04 / Fedora 40 (WebKitGTK 4.1) |
   | Windows | 11 (Windows 10 support ended October 2025) |
6. **Optional features: proposed and accepted 2026-09-27.** O1–O19 below
   are features few players have that fit this one's design. None of them
   is needed for a release or blocks a phase. All nineteen were accepted
   and built on 2026-09-27 (Phase 6b); each item's **Decision** line says
   how, and where it differs from the proposal. The user can turn every
   one of them on or off in Settings › Features (`FeatureSettings`); the
   ones that cost hours of CPU time, change what is heard, go online or
   listen on the network are off by default. They follow the rules that
   already apply:
   - The app never writes the user's files. Results go in the library DB,
     in tables keyed by track, album or artist id, as `album_links` is.
     Those ids survive rescans because the scanner upserts them.
   - Anything online goes through `http::Client`, is off by default, and
     is named in the privacy policy (§8.1).
   - Anything that decodes library files outside playback runs off the
     main thread with its own reader, never through the engine, and opens
     files through `access::open_folder` first.
   - Platform code (headphone detection, sample-rate switching) goes
     behind a small core interface with one implementation per OS.
7. **Expected features and hardening: prioritised 2026-09-27.** F1–F21
   (§4.7) are what users expect of any library player and this one lacks
   (playlists, favourites, multi-select, menus, accessibility and so on),
   unlike O1–O19, which few players have. H1–H22 (Phase 7) make the app
   and its development more secure, robust and efficient. Both use one
   scale:
   - **P1**: before the first public release. Part of Phase 7's exit.
   - **P2**: in the first updates after it, or when a later item needs it.
   - **P3**: later, as time allows.

   The rules in #6 apply to F1–F21 too: the user's files are never
   written, and the user's data lives in the library DB. Change a
   priority here with its date and reason.

   All twenty-one features were built on 2026-09-27, whatever their
   priority (Phase 6c); each item's **Decision** line in §4.7 says how,
   and where it differs from the proposal. What remains is checking them
   in the app (Phase 6c's exit).

Release-only decisions (distribution channels, packaging, signing) are in §8.1.

#### Optional features (§4.6)

| # | Feature | Touches | Size | Decision (2026-09-27) |
|---|---|---|---|---|
| O1 | Loudness analysis for files without ReplayGain tags | core, Rust, migration | M | Built; off by default |
| O2 | Waveform seek bar | Rust, UI | S (after O1) | Built; on |
| O3 | Segue-aware shuffle and silence handling | Rust (queue), core for trimming | S–M (after O1) | Built; segue shuffle on, skipping silence off |
| O4 | Library health report | Rust, UI (analysis checks after O1) | M | Built; on |
| O5 | Cue sheets and chapters as tracks | every layer, migration | L | Built; on |
| O6 | Classical works and movements | core (tags), migration, browse, UI | M | Built from tags; on |
| O7 | Per-track and per-album playback preferences | Rust, migration, UI | S | Built; on |
| O8 | Local listening history, opt-in ListenBrainz | Rust, migration, UI | M | Built; history on, ListenBrainz off |
| O9 | Library radio (endless queue from the library) | Rust (queue) | M (after O8) | Built; on ("keep playing when the queue ends" off) |
| O10 | Signal path panel and sample-rate matching | core, Rust, UI | S (panel), M (matching) | Built; panel on, matching off |
| O11 | Headphone crossfeed | core DSP, settings | S | Built; off by default |
| O12 | Practice mode: A–B loop, tempo without pitch change | core (new dependency), UI | M | Built; off by default |
| O13 | Local synced lyrics (tags and `.lrc` files) | core (tags), Rust, UI | S–M | Built, local only; on |
| O14 | LAN remote control from a phone's browser | Rust (server), entitlements, UI | M | Built; off by default, security review before release |
| O15 | Recently added | Rust, migration, UI | S | Built; on |
| O16 | Recently played | Rust, UI | S (after O8) | Built; on |
| O17 | Albums released on this day | core (tags), Rust, migration, UI | S–M | Built; on |
| O18 | Five more albums in this genre, at random | Rust, UI | S | Built; on |
| O19 | Top 20 played by year or month | Rust, UI | S (after O8) | Built; on |

- **O1 Loudness analysis for files without ReplayGain tags.** ReplayGain
  applies only to tagged files today. None of the dev library's files are
  tagged, and most players either ignore untagged files or rewrite their
  tags, which this app never does. A background pass decodes each track
  once and measures its integrated loudness and true peak (EBU R128 /
  ITU-R BS.1770), per track and per album. It stores them as computed
  gains. `PlaybackSettings::gain` uses the tag values first, then the
  computed ones, then the preamp for untagged files.
  - Core: a thread-safe `anomp_analyse_file` with its own
    `FFmpegAudioFormat` reader, separate from the engine, with progress
    and cancel callbacks. It returns everything O2–O4 need, so each file
    is decoded once.
  - Rust: an `analysis` worker thread shaped like the metadata worker,
    running at low priority a few files at a time. It can resume where it
    stopped and has a switch in Settings. It writes a `track_analysis`
    table (new migration) invalidated by the track's size and mtime.
  - Tests: EBU Tech 3341 test signals, or synthetic tones of known
    loudness, rendered like the chirp fixtures.
  - Cost: decoding a 50,000-track library takes hours of CPU time once.
  - **Decision:** accepted and built 2026-09-27, off by default for its
    CPU cost. `anomp_analyse_file` (`core/src/FileAnalyser.*`) measures
    a whole file or one part of it; the worker is `library/analysis.rs`
    (a few tracks at a time, the playing track first); results are in
    `track_analysis` and `album_analysis` (migration 006). An album is
    gated over 0.5 LU histograms of its tracks' 400 ms blocks, stored
    per track, so a new track doesn't mean decoding the album again.
    Computed gains are used only while the feature is on
    (`library::playback`).
- **O2 Waveform seek bar.** The analysis also keeps a coarse min/max
  envelope of about 1,000 points a track (about 2 KB as bytes).
  `SeekBar.svelte` draws it, with the played part in the accent colour.
  This shows quiet intros, drops and hidden tracks at a glance, and
  helps O12 set loop points. Until a track is analysed, the seek bar is
  the plain bar it is today. **Decision:** accepted and built
  2026-09-27, on by default. The envelope is 1,000 (min, max) byte
  pairs; the playing track is analysed ahead of the rest, even with O1
  off, so its waveform appears.
- **O3 Segue-aware shuffle and silence handling.** The analysis records
  each track's leading and trailing silence (below −60 dBFS). It marks a
  segue where one track's last 50 ms and the next track's first 50 ms on
  the same album both have sound. Shuffle keeps each run of segued tracks
  together, in order, as a single unit in `queue/model.rs`. Concept
  albums, live albums and DJ mixes then stop cutting mid-phrase, which
  plain shuffle does in almost every player. An optional "skip long
  silence" setting ends a track after N seconds of trailing silence (the
  gap before a hidden track). That needs the engine to accept an end
  position, which O5 adds too. **Decision:** accepted and built
  2026-09-27. Segue shuffle is on by default (`queue::album_units` gives
  each run a shuffle unit; `Queue::shuffle_from` moves runs whole).
  Skipping silence is off by default and differs from the proposal: a
  long silence *inside* a track (a hidden track's gap) is jumped over
  with a one-time skip region in the engine
  (`anomp_track_options.skip_from`/`skip_to`), so the hidden track still
  plays; silence that runs to the end ends the track (the range's end).
- **O4 Library health report.** A read-only view of problems the app can
  detect but otherwise hides. Each row can reveal the file in Finder.
  - Files that fail to decode, or whose decoded length falls short of
    the header's (truncated), found by the O1 pass.
  - Suspected lossy transcodes: "lossless" files (FLAC, ALAC, WAV) whose
    spectrum stops at 16–19 kHz for the whole track, as MP3 or AAC encoders
    leave it. This is a heuristic and is labelled "suspected".
  - Inconsistent albums: tracks of one album with different album
    artists, years or disc totals, and missing or repeated track numbers.
    These checks read only the DB and don't need O1.
  - Likely duplicates: the same recording MBID, or the same normalized
    title and artist with a length within 2 s. Audio fingerprinting stays
    deferred with AcoustID (Phase 4).
  - **Decision:** accepted and built 2026-09-27, on by default:
    `library/health.rs` and the Library health view, with "Reveal in
    Finder" (`opener:allow-reveal-item-in-dir`).
- **O5 Cue sheets and chapters as tracks.** Shows single-file albums
  (FLAC or APE with a `.cue` sheet, or a FLAC with an embedded cue sheet)
  and chaptered files (MP4 chapters, ID3 `CHAP`) as separate tracks.
  - Scanner: reads the `.cue` next to the file through the folder's
    bookmark (UTF-8, or a legacy code page detected) and makes a track
    for each `INDEX 01`, with its start and end in samples. The cue's
    `TITLE` and `PERFORMER` override the file's tags.
  - DB: a track becomes (folder, path, start), not (folder, path). That
    changes the unique key, so the migration rebuilds `tracks` and has to
    keep the FTS triggers (migration 002) in step.
  - Engine: `load` and `set_next` take an optional start and end. The
    engine treats the end as the track's end, so a hand-off to the next
    range of the same file is gapless and sample-exact (the reader's
    seeks are exact).
  - **Decision:** accepted and built 2026-09-27, on by default.
    Migration 005 rebuilds `tracks` keyed by (folder, path, range start)
    and keeps the FTS triggers; the scanner reads a `.cue` next to the
    file (UTF-8, else Windows-1252) or a CUESHEET tag
    (`library/cue.rs`), else chapters FFmpeg finds (MP4, ID3 CHAP, Ogg,
    FLAC cue sheet blocks; `FFmpegAudioFormat::readChapters`). Turning
    it on or off re-reads every file.
- **O6 Classical works and movements.** Most players show "Symphony No. 5
  in C minor, Op. 67: I. Allegro con brio" as one flat title. Tags
  already carry the structure: work, movement name and movement number
  (ID3 `TIT1`/`MVNM`/`MVIN`, MP4 `©wrk`/`©mvn`/`©mvi`, Vorbis
  `WORK`/`MOVEMENTNAME`/`MOVEMENT`), plus composer and conductor.
  - `TagReader` reads them from TagLib's property map. Check TagLib
    2.3's key names when building this.
  - A migration adds the columns, and triggers for any that search
    indexes.
  - Composer and work become browse grouping keys (`library/rules.rs`).
  - Album pages group movements under their work, with the composer and
    performers.
  - "Play work" queues a whole work, and shuffle treats a work as one
    unit, as O3 does segues.
  - Later, MusicBrainz work relationships can fill works for untagged
    files through the metadata worker.
  - **Decision:** accepted and built 2026-09-27 from tags, on by
    default: work, movement, composer (an artist) and conductor in
    `tracks`; Composer and Work browse levels and a default "Composer"
    rule; works grouped on album pages with "Play work"; a work is a
    shuffle unit. MusicBrainz works for untagged files are still later.
- **O7 Per-track and per-album playback preferences.** The user's own
  rules, set from the context menu, shown as a badge in lists, and stored
  in the DB, never in the files. They cascade from their track or album.
  - Skip a track in album and shuffle play (intros, skits, bonus
    tracks). It still plays when chosen directly.
  - Never shuffle an album: shuffle plays it whole, in order.
  - A gain offset in dB added to ReplayGain, passed with the track's
    gain as now.
  - A start or end trim, using O5's ranges.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `track_prefs` and `album_prefs` (migration 006), `library/prefs.rs`,
    the preferences dialog from track and album menus, and a badge in
    lists. Trims use O5's ranges.
- **O8 Local listening history, with opt-in ListenBrainz.** The queue
  adds a row to a `plays` table (track, start time, seconds played) once
  a track passes half its length or 4 minutes, the usual scrobbling
  rule. By default this history stays on the machine.
  - Views few local players offer: albums once played often but not for a
    year, "a year ago today" and never played. O16 and O19 are views on
    this table too. Play count and last played become `TrackColumn`
    options.
  - Opt-in ListenBrainz submission. It is MetaBrainz's service, like
    MusicBrainz, and the MBIDs from tags make its matches exact. The
    user's token lives in the keychain through `metadata::keys`. Listens
    are queued while offline and sent through `http::Client`. Its terms
    are checked before release like every other source (§8.1).
  - **Decision:** accepted and built 2026-09-27. Local history is on by
    default (`history/`: a main-thread tracker, a history thread writing
    `plays`); ListenBrainz is off by default, with the token in the
    keychain (`keys::Account::ListenBrainz`) and listens queued in
    `listens_pending`. Its terms still need checking (§8.1). Plays and
    last played are track columns.
- **O9 Library radio.** When the queue runs out, or on "Start radio from
  this", the app keeps adding tracks from the local library that fit the
  seed track. It scores tracks by shared or related genres, nearby
  years, the same label, and artists linked by MusicBrainz relationships
  (members, collaborations). Tracks played recently in O8's history are
  weighted down. Each pick shows why it was chosen ("same label, 1994").
  Everything is local and no service is called. The logic lives in
  `queue/` as a source of next items, tested against the fake engine.
  **Decision:** accepted and built 2026-09-27, on by default:
  `queue/radio.rs` scores genre, era, label and linked artists;
  MusicBrainz artist lookups now include `artist-rels` (members,
  collaborations, subgroups). "Keep playing when the queue ends" is a
  separate switch, off by default.
- **O10 Signal path panel and sample-rate matching.** Clicking the format
  in the now-playing bar shows the signal path as it actually is:
  1. The file's codec, bit depth, rate and bitrate.
  2. The ReplayGain gain applied.
  3. Resampling from the file's rate to the device's, or none.
  4. The volume.
  5. The device's name, rate and buffer size.

  The engine knows every step, so the panel is small. An opt-in setting
  switches the device to the file's rate when the device supports it, to
  avoid resampling (the known limit in Phase 6). Constraints:
  - Changing the rate interrupts output. It happens only at a track
    start that isn't a gapless hand-off; when the next track's rate
    matches the current one, nothing changes.
  - On macOS the rate is the device's, so it also changes for other apps
    using that device.
  - `AudioEngine` needs to reopen the device with a given rate.

  **Decision:** accepted and built 2026-09-27: the panel is on by
  default (`anomp_engine_signal_path`, from the format badge in the
  playing bar); sample-rate matching is off by default
  (`anomp_engine_set_device_sample_rate`, only at a load, never at a
  gapless hand-off).
- **O11 Headphone crossfeed.** Hard-panned stereo (much of the 1960s)
  tires the ears on headphones. Crossfeed blends a delayed, low-passed
  part of each channel into the other (Bauer's stereo-to-binaural
  method, as in bs2b).
  - A few biquads and a short delay in `PlayerEngine`, after the tap so
    the visualizer still shows the mix. Written in-house, with no new
    dependency.
  - Off and three strengths in the playback settings.
  - Optionally on by itself when the output is headphones, as the OS
    reports: the data source of Core Audio's built-in output, or the
    `AVAudioSession` route on iOS. This goes behind a small platform
    interface.
  - Tested offline through `getNextAudioBlock`, as the other engine tests
    are.
  - **Decision:** accepted and built 2026-09-27, off by default:
    `core/src/Crossfeed.*` after the tap; "only with headphones" (on by
    default) uses `OutputRoute` (Core Audio's data source on the
    built-in output, the audio route on iOS); Bluetooth headphones can't
    be told from speakers.
- **O12 Practice mode.** For musicians learning a part or transcribing
  one, which few library players support.
  - An A–B loop set on the seek bar (easier with O2's waveform). The
    engine jumps back at B on the exact sample, as the gapless hand-off
    does, not on a UI timer.
  - Tempo from 50% to 150% without changing pitch, and optionally a pitch
    shift in semitones. This needs a time-stretcher before the
    resampler, and its licence decides which one:
    - Signalsmith Stretch (MIT, header-only C++) fits the closed-source
      and iOS static-link rules.
    - Rubber Band is GPL or paid.
    - SoundTouch is LGPL, so it would have to ship as a shared library
      like FFmpeg.
  - **Decision:** accepted and built 2026-09-27, off by default.
    Signalsmith Stretch 1.4.0 and its FFT library (MIT, header-only) are
    pinned in `cmake/Signalsmith.cmake`; the A–B loop keeps a second
    reader of the file waiting at A and swaps the two at B. Its quality
    at 50% is still to be checked by ear.
- **O13 Local synced lyrics.** Shows lyrics already on disk.
  - Unsynced lyrics: ID3 `USLT`, Vorbis `LYRICS`/`UNSYNCEDLYRICS`, MP4
    `©lyr`.
  - Synced lyrics: ID3 `SYLT`, LRC text in a lyrics tag, or a `.lrc` file
    next to the track, read through the folder's bookmark as folder
    images are.
  - Synced lines highlight with the position (already sent every 50 ms),
    and clicking a line seeks there.
  - Lyrics are read from the file when shown (a flag on
    `anomp_read_tags`) and never stored in the DB.
  - Online lyrics (LRCLIB and others) stay out of scope. Adding one would
    be a sources-table decision, as in Phase 4.
  - **Decision:** accepted and built 2026-09-27, local only, on by
    default: `library/lyrics.rs` and the Now Playing view.
- **O14 LAN remote control.** Controls the desktop app from a phone's
  browser on the same network, with nothing to install.
  - An opt-in HTTP and WebSocket server in Rust serves a compact remote
    page: now playing, cover, play/pause, next, seek, volume, queue and
    search. Its commands go through the same queue functions as the UI
    and the media keys.
  - Pairing uses a code or QR code shown in Settings and a random token
    for each pairing. The server listens on LAN addresses only, and is
    off by default.
  - Costs: the sandbox's `network.server` entitlement, macOS's
    local-network prompt, and an entry in the privacy policy.
  - A listening socket is attack surface, so it needs a security review:
    token checks, rate limits, and no file access beyond cover art and
    the page itself.
  - The Phase 8 iOS build could later act as a richer remote.
  - **Decision:** accepted and built 2026-09-27, off by default, and
    still needs its security review before a release (§8.1). It differs
    from the proposal: plain HTTP with the page polling once a second
    instead of a WebSocket (no new dependency), and a pairing code with
    the page's address instead of a QR code. The safeguards are listed
    in `remote/mod.rs`; the `network.server` entitlement and the
    local-network usage text are in place.
- **O15 Recently added.** A sidebar view of albums ordered by when their
  newest track arrived, grouped into this week, this month and earlier.
  - `scanned_at` can't serve: every rescan rewrites it. A migration adds
    `tracks.added_at`, which the scanner sets on insert only (the upsert
    leaves it alone). Existing rows take the file's mtime, the nearest
    thing to an arrival date, because their `scanned_at` values are all
    the same first scan.
  - "Date added" also becomes a sort key for the browse rules.
  - Limit: a track's identity is its path, so moving or renaming a file
    makes it look newly added.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `tracks.added_at` (migration 005), a "Date added" sort key and album
    order, and the Home view.
- **O16 Recently played.** Needs O8's `plays` table. Lists what was
  played, newest first. Consecutive plays from one album collapse into a
  single album row ("11 tracks of …"), so an album doesn't fill the list,
  and one click plays it again. It differs from the queue, which holds
  only what is queued now. **Decision:** accepted and built 2026-09-27,
  on by default: `history::views::recently_played`, on the Home and
  History views.
- **O17 Albums released on this day.** Albums whose original release
  date falls on today's month and day, labelled with the anniversary
  ("30 years ago today"). It is shown as a card in a home or sidebar
  view, not as a notification.
  - The date comes first from the MusicBrainz release group's first
    release date, which the stored album details already hold, so a
    reissue counts on the original's date.
  - Otherwise it comes from a full `DATE` tag. `TagReader` keeps only the
    year today, so the full date needs a new column, re-read at the next
    scan as migration 004 did.
  - Albums dated only to a year or month never match. A 29 February
    release shows on 28 February in other years.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `tracks.release_date` from ORIGINALDATE or DATE (migration 005
    re-reads every file), preferring the MusicBrainz release group's
    first date.
- **O18 Five more albums in this genre, at random.** A "More in <genre>"
  row on album pages: five random albums that share a genre with the
  current one, excluding the current album, with a button to draw again.
  - The genres come from the tags through `anomp_has_genre`. An album
    with several genres gets a chip for each, and the chips switch the
    row.
  - The five stay the same while the page is open. Drawing again picks a
    new five.
  - With O8, albums played recently are drawn less often.
  - A random order over the matching albums is cheap at 50,000 tracks
    (a few thousand albums). Check it with the ignored benchmarks.
  - **Decision:** accepted and built 2026-09-27, on by default:
    `library::discover::more_in_genre`, on album pages.
- **O19 Top 20 played by year or month.** Needs O8. The 20 most played
  tracks, albums and artists for a chosen year or month (a "year in
  review"), counted as plays that passed O8's rule. "Play these 20" puts
  them in the queue, and the period can step back and forward.
  **Decision:** accepted and built 2026-09-27, on by default:
  `history::views::top_played`, on the History view.

#### Expected features (§4.7)

Found by a review of the repo on 2026-09-27. Priorities follow §4.7's
scale (P1 before the first release). Several close known limits recorded
in Phases 2–6, as noted.

| # | Feature | Touches | Size | Priority |
|---|---|---|---|---|
| F1 | Playlists, with M3U8 import and export | Rust, migration, UI | M | P1 |
| F2 | Smart playlists (saved rules) | Rust, UI | M (after F1) | P2 |
| F3 | Favourites, then ratings | Rust, migration, UI, core (tags) | S | P1 favourites, P2 ratings |
| F4 | Multi-select, drag and drop | UI, Rust | M | P1 |
| F5 | Open files from Finder ("Open with", file associations) | Rust, bundle config, queue | S–M | P2 |
| F6 | Menu bar with a Controls menu, shortcuts sheet, Dock menu | Rust, UI | S | P1 |
| F7 | Mini player and menu-bar controls | Rust (second window), UI | S–M | P2 |
| F8 | First run and empty states | UI | S | P1 |
| F9 | Rescan at launch, then file watching | Rust | M | P1 at launch, P2 watching |
| F10 | Keep the user's data when files move or are renamed | Rust (scanner), migration | M | P1, with F1 and F3 |
| F11 | Compilations and multiple artists | core (tags), Rust, migration | M | P2 |
| F12 | Substring and field search | Rust (FTS), migration | S–M | P2 |
| F13 | Sleep timer, stop after this track | Rust (queue), UI | S | P2 |
| F14 | Crossfade | core, Rust, settings | M–L | P3 |
| F15 | Equaliser | core DSP, settings, UI | M | P3 |
| F16 | Track info panel | Rust, UI | S | P2 |
| F17 | Resume where the user left off | Rust (queue, media) | S | P2 |
| F18 | Accessibility, and a safe visualizer | UI | S–M | P1 |
| F19 | Localisation groundwork | UI, Rust errors | M | P2 |
| F20 | Export and import the user's data | Rust, UI | M | P2 |
| F21 | Track-change notifications | Rust | S | P3 |

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

## 5. Phased plan

Each phase ends with a demonstrable result and green tests. Platform order:
macOS (Phases 0–7), then iOS/iPadOS (Phase 8), then Linux (Phase 9), then
Windows (Phase 10). Xcode is not needed until Phase 8. Phase 11 (Bandcamp
streaming) is a feature, not a platform, and depends on Bandcamp's
permission. Its engineering starts after Phase 7. The phases cover making
the app work on each platform; packaging, signing and shipping it are all
in §8.

Rules from the start, so the later ports stay cheap:
- Keep platform code behind small interfaces in the core (media controls,
  file access, audio session) with one implementation per OS. No AppKit or
  CoreAudio calls outside those files.
- Store folder access as bookmarks/opaque handles, not raw paths.
- UTF-8 everywhere across the C API; never assume `/` separators or
  case-sensitive paths.
- The core links only JUCE modules that avoid GUI dependencies where
  possible (`juce_audio_utils` pulls in `juce_gui_basics`, which means X11
  and freetype on Linux). Done in Phase 1: the core links only
  `juce_audio_formats` and `juce_audio_devices` (plus their dependencies).

### Phase 0 — Toolchain and integration spike (highest risk first)
Design notes, the steps as built, tests and known limits: [docs/design/phase-0-toolchain.md](docs/design/phase-0-toolchain.md).

- [x] **Exit:** a tone plays from a Tauri app on macOS. Confirmed by ear
  2026-09-25. **Phase 0 complete.**

### Phase 1 — Playback engine (C++ core)
Design notes, the steps as built, tests and known limits: [docs/design/phase-1-playback-engine.md](docs/design/phase-1-playback-engine.md).

- [x] **Exit:** a library of MP3/FLAC files plays through the app with
  working seek and volume, and a gapless album plays without clicks at track
  boundaries. Confirmed by ear 2026-09-26 (MP3 album with continuous tracks,
  queued one at a time through the dev UI). **Phase 1 complete.**

### Phase 2 — Metadata and library
**Phase 2 complete.** Design notes, the steps as built, tests and known limits: [docs/design/phase-2-metadata-library.md](docs/design/phase-2-metadata-library.md).

### Phase 3 — Frontend: core player UI
**Phase 3 complete.** Design notes, the steps as built, tests and known limits: [docs/design/phase-3-player-ui.md](docs/design/phase-3-player-ui.md).

### Phase 4 — Online metadata services
Goal: enrich the library with album details, artist information and cover
art from online sources and local files, with **more than one source for
each kind of data and the user choosing between them**: an ordered list of
sources per kind (the first with a result wins), and a per-album choice
that pins a specific source's match or picture. Every service can be turned
off, and the app works offline on what it has already fetched.

**Sources considered** (checked 2026-09-26, and settled in 4.8 from the
terms read first-hand that day unless the row says otherwise; "terms" is
about a closed-source commercial app, §4.1, and every online source is
re-checked before release, §8.1):

| Source | Provides | Access and limits | Terms | Plan |
|---|---|---|---|---|
| Embedded art (tags) | Album/track art | Local, already read by the core | — | On (exists) |
| Folder images | Album art (`cover`, `folder`, `front`, `album`, `albumart*` .jpg/.png/.webp next to the tracks) | Local; readable through the folder's bookmark | — | On |
| MusicBrainz | Release, recording and artist metadata (dates, label, catalogue number, country, release type, genres), and links to Wikidata, Discogs etc. | No key; a `User-Agent` with contact details; **~1 request/s per IP**, 503 when exceeded | Core data CC0; MetaBrainz asks commercial users to become supporters | On, the primary source |
| Cover Art Archive | Album art by release or release-group MBID; 250/500/1200 px thumbnails | No key; no limits today; images redirect (307) to archive.org | Images belong to their owners; showing them in a player is the norm | On |
| Wikidata + Wikipedia | Artist and album descriptions, reached through MusicBrainz URL relationships | No key; `User-Agent` | Text CC BY-SA: show attribution and a link with it | On |
| Discogs | Release metadata (credits, labels, catalogue numbers, formats, styles) and images | 60 requests/min with a token, 25 without; searching needs a token. The user's own personal access token, since a secret shipped in a desktop app isn't secret | API Terms of Use, last updated 2025-05-27 (read through the Help Center's article API, as the page answers 403 to scripts): release data (titles, dates, formats, track lists, identifiers, credits, artist and label names) is CC0; images, user and marketplace data are "Restricted Data", **not for any commercial purpose**. Commercial use is "generally permitted", but "charging a fee to use or access any part of Your application that integrates with Our API" needs their written permission when Discogs gives that access free. Nothing may be shown more than 6 hours behind discogs.com, nor cached or stored longer than needed. "Data provided by Discogs" directly next to its data, linked to the discogs.com page with it; a non-affiliation notice shown prominently (may be in the terms or documentation) | **Ships (4.8), off by default.** Details only: the match (release id) is stored, details are fetched when shown and kept in memory for at most 5 hours, no offline copy, no pictures anywhere; credit and notice shown. Token in the OS keychain. If the app is sold, Discogs' written permission is a release gate (§8.1) |
| fanart.tv | Artist images, logos, backgrounds; album covers; keyed by MBIDs | A project key (ours) required, a personal key per user optional (fresher images) | Its terms page answers a bot check; the archived copy (2025-01-26) says images stay their owners' and rests its fair-use case partly on being "a completely free service". The API docs say nothing on commercial use; a third-party API listing quotes "Do not use the API for commercial use without written consent" (not found first-hand) | **Not shipped.** Ask fanart.tv for written consent before any work; it would add artist images (a new `ArtistImage` kind) |
| TheAudioDB | Artist bios and images, album descriptions | Free test key "123", 30 requests/min; premium $8/month (Patreon), 100/min | Terms of use: with the free key "you cannot publish apps to an appstore unless you are a paid subscriber"; paid users may build apps within their rate limit and must name TheAudioDB as the source; images are mostly fan uploads, Creative Commons only where marked | **Not shipped.** A project key would put every user under one paid 100/min limit; a user-supplied key asks each user to pay for biographies Wikipedia already gives; image rights unclear |
| iTunes Search API | Large album art, release dates | No key; about 20 calls/min | Apple Services Performance Partners terms: album art and other promotional content only "for the purposes of promoting" the item, next to an Apple store badge linking to it, and not for "independent entertainment value apart from its promotional purpose" | **Excluded**: a player's covers are the non-promotional use the terms rule out |
| Deezer | Album art up to 1000 px, search | No key | API terms: use "strictly limited for a non-commercial purpose", with no money made "in connection with the use of" the services or their content | **Excluded** |
| AcoustID + Chromaprint | Identifies untagged files by audio fingerprint | API key; 3 requests/s; Chromaprint is LGPL and a new native dependency | **Free for non-commercial use only**; commercial use is a paid plan (not re-read in 4.8) | Deferred (after Phase 4): needs a paid plan and a new native dependency |
| Last.fm | Artist bios, tags, similar artists | API key | **Non-commercial only** without written permission, 100 MB storage cap, mandatory branding | Excluded |
| Bandcamp | Details and art for albums bought there | No API for fans (only label and merch-partner APIs); the Acceptable Use Policy forbids scraping | Personal, non-commercial use only | Only under the Phase 11 agreement |
| Spotify | — | OAuth; endpoints cut back in 2024 | Terms don't fit enriching a local library | Excluded |

Lyrics (e.g. LRCLIB) are out of scope for Phase 4.

**Design decisions:**
- **Tags stay the library's identity.** Titles, artists, albums and the
  grouping come from the files, which the app never writes. Online data adds
  fields (release date, label, catalogue number, country, release type,
  genres, descriptions) and pictures, shown next to the tag values with
  their source. A per-field "prefer online value" display option belongs to
  the Phase 6 fields screen.
- **Providers behind traits.** Each source is a provider declaring what it
  can supply: `Release` (album match and details), `AlbumArt`,
  `ArtistInfo`, `ArtistImage`. Matching, art and the UI see only the traits,
  so adding a source is one module. Album details went behind
  `albums::ReleaseSource` with Discogs (4.8); the other kinds still have one
  online source each and get their trait with a second.
- **Choosing a source.** The settings (`metadata.services` in `settings`,
  read with the same keep-what's-usable fallback as `library.sort`) hold a
  master "online services" switch, per-service enabled flags and API keys,
  "match automatically after a scan", and an ordered source list per kind.
  Automatic matching walks that order. Per album, the user can open "Find
  details" or "Choose cover" to see candidates from every enabled source,
  labelled by source, and pick one. A pick is stored as `chosen_by = 'user'`
  and automatic runs never replace it; "Use automatic" clears it.
- **Storage (migration 003):** `album_links` (album, source, status,
  external ID, score, `chosen_by`, the normalized details as JSON,
  checked-at; one row per album and source, so an album can be linked to
  MusicBrainz and Discogs at once), `artist_links` likewise, and
  `album_art` (the user's chosen picture: source and reference).
  All cascade from their album or artist. They are separate from the
  scanner's columns, which a rescan overwrites; album and artist ids survive
  rescans because the scanner upserts them. A "not found" result is stored
  too, so it isn't retried on every launch (retried after 30 days, or on
  request).
- **HTTP:** `ureq` 3 (blocking: the work runs on its own thread at ≤1
  request/s, so async adds nothing), rustls with *ring* and the OS trust
  store through `rustls-platform-verifier`, so the TLS stack is the same on
  all four OSes, iOS included, with no OpenSSL on Linux. `User-Agent`:
  `ano-mp/<version> ( <contact> )`. A token-bucket limiter per host (MB 1/s,
  Discogs 1/s, others per their limits), timeouts, and backoff on
  503/429/`Retry-After`. Responses are cached in `mb_cache` (used for every
  source despite its name, keyed by source and URL) with a time-to-live per
  kind (lookups 30 days, searches 7 days).
- **Downloaded images** go to a size-capped folder in the app cache dir,
  named by a hash of the URL, not into SQLite, and can always be fetched
  again.
- **The art URI handler never goes online.** It serves the user's choice,
  then local sources (embedded, folder), then downloaded images, and says
  404 otherwise. Fetching is done by the metadata worker, which emits
  `metadata-changed` (album and artist ids) so the UI reloads that art.
- **One metadata worker thread** owns the HTTP client and the limiters and
  takes jobs from a queue: user requests (a candidates dialog, the playing
  album) before background enrichment. It opens its own DB connection, like
  the scanner, and never holds the shared one while waiting on the network.
- **Offline:** a connection failure marks the service unreachable and
  backs off (1 min, doubling to 30 min); background work pauses, cached
  data keeps showing, and user requests fail at once with "offline". The UI
  shows the status per service.
- **Tests never touch the network.** Providers take a `Transport` trait;
  tests use a fake that serves recorded responses committed as fixtures.
  Each service gets one `#[ignore]`d live smoke test.

Design notes, the steps as built, tests and known limits: [docs/design/phase-4-online-metadata.md](docs/design/phase-4-online-metadata.md).

- **Exit:** a library of tagged and untagged albums gets details and covers
  from MusicBrainz and the Cover Art Archive; the user can reorder or turn
  off sources, pick another source's cover or match for an album, and the
  app behaves the same with the network off, showing what it cached.

### Phase 5 — Visualization
Design notes, the steps as built, tests and known limits: [docs/design/phase-5-visualization.md](docs/design/phase-5-visualization.md).

- **Checked**: the core's analysis on synthetic signals (tones at three
  rates, a chord's pitch classes, levels, the trigger, kicks found and a
  steady tone not, the thread's frames and single silent frame, the
  player's pre-volume tap, the C API); the frame encoding against a byte
  string the frontend test decodes too; subscribers; cover walls. The app
  starts and shows the Visualizer view's entries. Each visualization was
  rendered in headless WebKit from synthetic frames and SVG covers and
  looked at, which found and fixed a clipped ring, an overshooting
  vectorscope, seams in the kaleidoscope and the VU calibration.
- **Not checked**: anything with real music in the app: that frames
  arrive through the `Channel` at 60 fps without stutter, how
  responsive and how busy each visualization feels, the beat detector
  on real music, full screen, and the CPU cost (the canvas at Retina
  size, the wall's hundred tiles). WebKitGTK and WebView2 (Phases 9–10)
  may need a lighter mode.

### Phase 6 — Admin / settings screen
Design notes, the steps as built, tests and known limits: [docs/design/phase-6-settings.md](docs/design/phase-6-settings.md).

- **Checked**: core tests for per-track gains (switching at the hand-off,
  changes by file, clamping), ReplayGain tags in all ten formats and R128
  conversion, the device API's null and unknown-name cases; Rust tests for
  settings (round trip, validation, keeping the usable parts of stored
  JSON), gain calculation, the migration, the queue's open tracks and the
  bindings. In the app: every section renders with the real library and
  output device, and a change is saved and read back.
- **Not checked**: switching output devices and buffer sizes by ear,
  unplugging and replugging the chosen device, and ReplayGain on tagged
  music (none of the dev library's files are tagged), including a mode
  change mid-track.

### Phase 6b — Optional features (O1–O19)
Design notes, the steps as built, tests and known limits: [docs/design/phase-6b-optional-features.md](docs/design/phase-6b-optional-features.md).

- **Exit (to check in the app)**: the loudness analysis over the whole
  library and ReplayGain from it by ear; waveforms; a live album's
  segues in shuffle; a hidden track's gap skipped; a single-file album
  with a cue sheet playing gaplessly; an audiobook's chapters; classical
  works on album pages; each preference; plays counting and ListenBrainz
  receiving them with a real token; radio's picks; the signal path with
  sample-rate matching on a device that offers several rates; crossfeed
  with headphones plugged and unplugged; practice mode at 50% and 150%
  (Signalsmith's quality) and a tight loop; lyrics from an `.lrc` and
  from SYLT; pairing a phone and controlling playback from it, including
  the macOS local-network prompt in a sandboxed bundle.

### Phase 6c — Expected features (F1–F21)
Design notes, the steps as built, tests and known limits: [docs/design/phase-6c-expected-features.md](docs/design/phase-6c-expected-features.md).

- **Exit (to check in the app)**: M3U8 round trips with another player
  (Music, VLC), including legacy code pages; dragging within and between
  lists; a folder dropped from the Finder becoming a library folder in a
  sandboxed bundle; Open With and double-clicking an `.m3u8` in the
  bundle; every menu item and shortcut, the Dock menu, the menu-bar item,
  and the mini player on top of a full-screen app; the welcome page on a
  fresh library; unplugging a drive, then "Locate…"; watching while files
  are copied in, renamed and moved; a compilation and a duet in the
  browser; crossfade by ear, including skipping during a fade; the
  equaliser by ear and switching profiles with headphones; resuming an
  audiobook and the media keys after a relaunch; VoiceOver over the main
  views, and the layouts at larger text sizes; the flash guard on a
  strobing track; export on one Mac and import on another after a scan;
  notifications with covers.

### Phase 7 — Hardening (macOS)
- CI (GitHub Actions, macOS runner): CMake build + ctest, `cargo test`,
  frontend lint/type-check/tests, all through `scripts/check-all.py`
  (§9.2 M4), plus a weekly job for outdated pins and advisories (M5).
- Performance: library of 50k+ tracks; scan time; memory use.
- Robustness: corrupt/truncated files, missing files on disk, unplugged
  output devices, offline services.
- Hardening items H1–H22 below, and the P1 features of §4.7.

**Order of work** (set 2026-10-02). Phases 0–6c are built, but the exit
checks of Phases 4, 5, 6, 6b and 6c are not done yet, and only part of
H3 and H7 among the P1 hardening items is done. Work through these steps
in order. Step 6 runs alongside all of them.

1. **A repeatable baseline (about 1 day). Done 2026-10-02.**
   - Every suite passed on a clean tree: ctest 99, `cargo test` 342 (9
     ignored), `npm test` 18, `svelte-check` 0 errors.
   - `check-all.py` runs them, after the formatters, the three M2 checks
     and the scripts' tests. It also builds the frontend before
     `cargo test`, since Tauri embeds `app/build` when the crate compiles
     and a fresh clone has none.
   - `check-c-api.py` found two C functions Rust doesn't bind
     (`anomp_track_options_default`, `anomp_media_controls_perform`);
     both are deliberate and listed in its `NOT_BOUND`.
   - The CI job (`macos-15`) hasn't run yet: it runs on the first push.
   - Run every suite (ctest, `cargo test`, `npm test`, `npm run check`)
     to confirm they pass.
   - Write `check-all.py` (M4), with the M1 pytest harness and the cheap
     M2 checks (`check-c-api.py`, `check-sources.py`,
     `check-migrations.py`).
   - Add a GitHub Actions macOS job that runs only `check-all.py`.
   - This comes first because everything after it is hardening, and
     hardening without CI tends to slip back.
2. **Quick P1 security and lint fixes (1–2 days). Done 2026-10-02.**
   - Every suite passed on a clean tree: ctest 100 (also under the `asan`
     and `tsan` presets), `cargo test` 342 (9 ignored), `npm test` 21,
     `svelte-check` 0 errors, script tests 30. `check-all.py` has 21
     steps; each item's entry below says what it added.
   - The CI job still hasn't run: Step 1's commit isn't pushed yet.
   - Left for the owner: turning on GitHub secret scanning with push
     protection (and Dependabot security updates) in the repository's
     settings, and installing the hook in each clone
     (`git config core.hooksPath scripts/hooks`).
   - H1 (CSP), H2 (developer surface out of release builds).
   - H3's remaining half: the opener still allows `http://*`.
   - H8 (clippy, `[lints]`, toolchain pins), H7's remainder (SHA pins for
     JUCE and Catch2, `cargo deny`, the update bot, secret scanning).
   - H13's P1 part (ESLint and Prettier), H6 (sanitizer presets).
3. **P1 items that protect user data (3–5 days). Done 2026-10-02
   (H22b with Step 4).**
   - Every suite passed on a clean tree: ctest 102 (also under the `asan`
     and `tsan` presets), `cargo test` 383 (9 ignored), `npm test` 21,
     `svelte-check` 0 errors, script tests 30. `check-all.py` (still 21
     steps) ran in full after each item.
   - The CI job still hasn't run: Steps 1–3 aren't pushed yet. Secret
     scanning and the hook are still the owner's to turn on.
   - H10 (DB backups and checks): the DB now holds playlists, ratings,
     history and picks that a rescan can't recreate.
   - H22 (missing folders at launch): a folder whose path exists but is
     empty was emptied by the launch rescan. Split in two: H22a (the
     data loss, the folder states, the launch message, the queue, the
     workers, drives coming back) is done; H22b (the lists) is left.
   - H9 (logs, panics, diagnostics): release builds aborted on a panic
     and left no trace.
   - Wording drafted for the owner to review: the missing-folders message
     (`missing.*`, `folderState.*`, `folderShort.*` and
     `error.folderUnavailable.*` in `en.json`), the database repair offer
     (`dbRepair.*`) and Settings › About (`about.*`).
   - What only a sandboxed bundle shows is in Step 4's list.
4. **Exit checks in a sandboxed bundle. Probe checks done 2026-10-02;
   the owner's checks wait.**
   - H22b came first (H22's entry). After it every suite passed:
     ctest 102 (also under `asan` and `tsan`), `cargo test` 387 (9
     ignored), `npm test` 25, `svelte-check` 0 errors, script tests 30,
     `check-all.py` all 21 steps.
   - The checklist is `docs/step4-checklist.md`, ordered by risk, each
     entry saying how to check it, what passing looks like, and who.
   - The real container was protected by moving the app's own files out
     of it and back (macOS won't let another process move or fully copy
     a container); see "Running a bundle check safely" in `CLAUDE.md`.
   - A temporary probe, compiled into the bundle and removed afterwards,
     ran stages against a scratch library of `core/tests/fixtures`
     written into the container:
     - scan, bookmark and covers in the sandbox, a gapless hand-off, the
       diagnostics without paths: passed;
     - folder states with real bookmarks (empty and kept through a
       scan, in a Trash and not followed, deleted, back), H22b, a queue of
       unreadable tracks not spinning: passed after a fix (H22);
     - the log in the container (format, no paths): passed;
     - a panic in a release build: passed;
     - the database repair, restore and rebuild: passed (the dialog is the
       owner's to see).
   - Found: every quit crashed (H9's entry), a deleted folder read as "no
     access" (H22's entry). Each fixed with a test; after the fixes every
     suite passed: ctest 104 (also under `asan` and `tsan`), `cargo test`
     387 (9 ignored), `npm test` 25, `check-all.py` all 21 steps.
   - Left for the owner (the checklist's unticked entries): Finder drops,
     Open With, USB, SMB, the Trash and deletion outside the container, a
     rebuilt bundle's folders, "Show logs", "Copy diagnostics", the repair
     dialog, the local-network prompt, the keychain, playback by ear, the
     rest of the app, and the visualizers.
   - Merge the exit lists of Phases 4, 5, 6, 6b and 6c into one
     checklist, ordered by risk:
     - sandbox-only behaviour first (folder drops, Open With, the LAN
       remote's local-network prompt, missing folders);
     - then gapless playback and cue sheets, crossfade, the equaliser and
       ReplayGain by ear;
     - the visualizers last.
   - This comes after H9 so that failures leave logs. Each failure
     becomes a fix with a test.
   - From Step 3, only a sandboxed bundle shows these:
     - H22's own list (a USB drive unplugged at launch, then plugged in;
       an SMB share not mounted; a folder moved to the Trash; a folder
       deleted), with what each shows: the folder's state and reason,
       the launch message, the queue skipping, the drive found again
       without a restart (the `VolumeWatcher`'s NSWorkspace notifications
       under the sandbox).
     - A rebuilt bundle's folders: their bookmarks don't resolve, so they
       should read as "no access" (`noPermission`), keep their tracks,
       and come back with "Locate…". `access::unresolved` tells that from
       "missing" by the bookmark error's text, or by the stored path still
       being a folder, which the sandbox may hide.
     - The log in the container (`~/Library/Containers/<id>/Data/Library/
       Logs/<id>/ano-mp.log`), "Show logs" revealing it in the Finder,
       and "Copy diagnostics" (`navigator.clipboard` in the webview).
     - A panic in a release build: the log ends with it (a temporary
       probe).
     - The database repair offer: restore and rebuild, each restarting
       the app (a damaged copy of a scratch library).
5. **The larger core P1 items (about 1 week). Done 2026-10-02; the
   owner's checks are § 5 of `docs/step4-checklist.md`.**
   - Every suite passed on a clean tree after each part: ctest 114 (also
     under `asan` and `tsan`), `cargo test` 398 (9 ignored), `npm test`
     25, `svelte-check` 0 errors, script tests 33, `check-all.py` all 22
     steps (the new one: "core fuzzing", each target for 60 s).
   - The owner's answers that shaped it: Steps 3 and 4 pushed (the Actions
     runs' results not yet seen here); none of Step 4's owner checks known
     to have failed; Homebrew LLVM for the fuzzers (`llvm@22`), with
     FFmpeg instrumented too; the CI cache and the weekly fuzzing job; the
     wording "Opening…", "The file took more than {seconds} s to open",
     "Downloading from iCloud…" and "{count} in iCloud, not downloaded".
   - The probe in the checklist's § 5 ran on 2026-10-02 with Step 7 and
     passed (Step 7's entry).
   - No bundle check was run in Step 5: H11 holding a folder open while
     another thread opens the file, and H12's placeholders, are in the
     checklist's § 5 (one probe, the rest the owner's).
   - H5 (fuzzing).
   - H11 (opening files off the main thread). This is the riskiest
     change to the engine, so fuzzing and the sanitizers come first.
   - H12 (cloud placeholders), which builds on H11.
6. **Owner decisions (§8.1), alongside steps 1–5.** These block the
   first release however far the engineering gets:
   - the name and trademark check, then the bundle identifier, which
     can't change after the first release and which signing needs;
   - the JUCE licence;
   - the AAC opinion;
   - the privacy policy and support URL;
   - the MetaBrainz plan and a real `User-Agent` contact;
   - whether to have crash reporting.

   **Briefs written 2026-10-02** (`docs/release-decisions.md`), one per
   item, in the order they block: the name, then the bundle identifier
   and publisher; the distribution channels; the Developer Program and
   Developer ID; the JUCE licence; the AAC opinion; the privacy policy
   (with a table, from the code, of what the app sends where); the
   MetaBrainz plan and the `User-Agent` contact; crash reporting. None is
   decided yet. Read-only lookups: no DNS records for the candidate
   domains, and WHOIS shows `anotracks.com`, `anotone.com` and
   `anotraks.com` unregistered; the `.app` registry's WHOIS didn't answer.
7. **Release setup without a certificate (§8.2, and §8.3's parts that
   need none; M6). Done 2026-10-02; the signed release waits on Step 6.**
   - Every suite passed on a clean tree after each part: ctest 115 (also
     under `asan` and `tsan`), `cargo test` 399 (9 ignored), `npm test`
     25, `svelte-check` 0 errors, script tests 72, `check-all.py` all 24
     steps (new: "version number", quick, and "third-party notices").
   - The owner's answers that shaped it: the Actions runs haven't run yet;
     which of the checklist's owner entries are done isn't known, and
     none is known to have failed; no §8.1 decision made yet; the crates'
     licences from `cargo metadata` rather than a new tool; the About
     wording ("Third-party notices" and its hint), with Discogs' notice
     there too; the release workflow as proposed.
   - Step 5's bundle probe (the checklist's § 5) passed in a sandboxed
     bundle, with the real container's files moved out and back (identical
     after): the 21 fixtures played through with 20 gapless hand-offs,
     every load handed to the engine with its folder held, none failed;
     then 9 skips back to back, 5 superseded while opening, no folder left
     held. The probe's first run stopped on its own mistake (it skipped
     while the short fixtures kept playing), not the app's.
   - One version number: `anomp_version()` is compiled from CMake's
     `project(VERSION)` (`ANOMP_VERSION`), and `scripts/version.py` sets
     or checks the copies the tools need (`Cargo.toml` and `Cargo.lock`,
     `tauri.conf.json`, `package.json` and both versions in
     `package-lock.json`). The `User-Agent` and ListenBrainz already took
     the version from the core; the contact is a marked placeholder
     until the owner gives one. Tests: 9 script tests, the C API test
     against `project(VERSION)`, a bump round trip through the core.
   - Third-party notices: `scripts/make-notices.py` writes
     `THIRD_PARTY_NOTICES` (374 KB): JUCE (its licensing statement, and
     zlib, the only code it vendors into the linked modules; FLAC, Ogg,
     Vorbis, Oboe and ASIO are listed as not in the app, and a new
     vendored library fails the script), FFmpeg (LGPL 2.1, version,
     configure flags, source tarball and SHA-256), TagLib under the MPL
     with utfcpp, Signalsmith Stretch and Linear, Catch2 as not shipped,
     264 crates and the 6 npm packages the frontend bundles (recorded by
     a Vite plugin at build time). Each licence expression must be
     satisfiable from `deny.toml`'s list; identical texts are printed
     once. 16 crates and the 4 Tauri npm packages ship no licence text,
     so standard texts are committed in `scripts/licenses/`. The bundle
     carries the file; Settings › About opens it in a dialog and shows
     Discogs' notice. Tests: 15 script tests, a Rust test that the bundle
     maps the file under the name the command reads.
   - The release workflow (`release.yml`): on a `v*` tag it calls
     `ci.yml` (now also `workflow_call`, and run on branch pushes only;
     since Step 8, by hand and through `release.yml` only),
     builds the universal app and DMG with `build-app.py`, notarizes when
     the secrets exist, and `release.py` checks the bundle, zips the app,
     writes `SHA256SUMS` and cuts the notes from the new `CHANGELOG.md`;
     the files go to a draft GitHub Release with `gh`. By hand, on a
     branch, it is a trial with the files as an artifact. It hasn't run
     yet.
   - macOS without a certificate: `build-app.py` (the hardened-runtime
     rule), `check-bundle.py`, `notarize.py` (dry run without
     credentials), `check-signing.py` (no identities yet), and the
     entitlements reviewed (§8.3). `rust-toolchain.toml` adds the x86_64
     target. A universal build here passed `check-bundle.py`: arm64 and
     x86_64 in the executable and the four FFmpeg dylibs, `@rpath` install
     names and the `@executable_path/../Frameworks` rpath, no library from
     outside the bundle and the OS, `codesign --verify --deep --strict`,
     the entitlements and notices as committed; the DMG (14.5 MB) verified,
     and `release.py`'s trial run wrote files whose `SHA256SUMS` check.
     Tauri's DMG step drives the Finder by AppleScript unless `CI=true`,
     and waits on the Automation permission locally, so local runs set it.
     The hardened runtime with a real identity, notarization and
     Gatekeeper wait for the certificate.
   - Found: fuzzing found undefined behaviour in TagLib's Shorten reader
     in the first full run; fixed with a test and a fixture (H5's entry).
   - Waiting on the owner, in the order they block: the decisions in
     `docs/release-decisions.md`; then the Developer ID certificate, the
     notarization key and the six secrets; the updater (after the
     distribution decision); the icon set (after the name); a first trial
     of `release.yml`; the wording of `CHANGELOG.md`'s first entry and of
     `error.noticesUnreadable`; the About page by eye.
8. **The signed release (§8.3 with a Developer ID, §8.7), as far as the
   owner's decisions allow. Begun 2026-10-02; its signing parts wait on
   Step 6.**
   - The owner's answers: Step 7 committed and pushed (`fd3aeae`); none
     of the eight decisions in `docs/release-decisions.md` made; which of
     the checklist's owner entries are done not known (none known to have
     failed); `CHANGELOG.md`'s first entry and `error.noticesUnreadable`
     kept as drafted; no identifier chosen, so no container move.
   - CI (from the owner's screenshot of the Actions page): runs #6
     ("completed step 5") and #9 ("completed phase 7 in six parts")
     failed on `main`, #9 after 7 min; Dependabot's runs based on them fail
     too. `check-all.py` passed here in full, all 24 steps, on the same
     tree, so the cause is in the runner; the failing step's log is
     awaited, and the fix comes first once it is in. Meanwhile the owner
     set CI to run by hand and through `release.yml` only (`ci.yml`,
     README's "When GitHub Actions run"), so a push no longer runs it.
   - Done: `scripts/__pycache__` and `scripts/tests/__pycache__` (17
     committed `.pyc` files) untracked and ignored.
   - Done: Part 4's checklist, `docs/release-smoke-test.md`: checksums,
     Gatekeeper on the DMG and at first launch, import and relaunch, MP3,
     FLAC and AAC, seeking, a gapless album, media keys, a MusicBrainz
     lookup, the log, and the update from the previous version (once the
     updater exists). The owner runs it on each release's DMG. The dry
     run of §8.7 waits on a signed trial of `release.yml`; `bench.py`
     (§8.7 step 4) doesn't exist yet (M4).
   - Done: H14, the sandboxed bundle's self-test (its entry), proposed as
     Part 5's first item and agreed. Every suite passed afterwards:
     ctest 115 (also under `asan` and `tsan`), `cargo test` 405 (9
     ignored), `npm test` 25, `svelte-check` 0 errors, script tests 79,
     `check-all.py` all 25 steps (new: "bundle self-test", which skips
     itself outside CI).
   - Waiting on the owner, in the order they block the first public
     release:
     1. ~~the CI log of run #9 (and #6)~~ fixed 2026-10-03 (below);
        CI on `main` itself waits for the owner to push the fix and run
        it;
     2. the name, bundle identifier and publisher (Part 0's rename, then
        the icon set from the artwork);
     3. the distribution channel and where releases are hosted (Part 1,
        the updater, if a direct download);
     4. the Developer Program and the Developer ID certificate (Part 2),
        then the App Store Connect API key and the six secrets (Part 3);
     5. the JUCE licence, the AAC opinion, the privacy policy and support
        URL, the `User-Agent` contact and MetaBrainz plan, crash
        reporting;
     6. the checklist's owner entries (`docs/step4-checklist.md`) and,
        per release, `docs/release-smoke-test.md`.
   - **Part 5 done 2026-10-03:** the CI fix first, then `bench.py` (M4)
     and H18's budgets, H20 (with `check-docs.py`, M2), H15, H16 and H17,
     each in its entry. After each part `check-all.py` ran in full; after
     the last, all 27 steps passed (new: "docs' paths and links", quick,
     and "docs' test counts"): ctest 115 (also under `asan` and `tsan`),
     `cargo test` 433 (12 ignored), `npm test` 30, `svelte-check` 0
     errors, script tests 132.
   - The owner's answers that shaped Part 5: Step 8 committed and pushed
     (`be6e6ad`); `gh` to read CI's logs; no §8.1 decision made; whether
     any checklist entry failed not known, so the container's log wasn't
     read; Homebrew `lld@22`, then (when it failed) Xcode 26.3 for the
     fuzz build only, and the downloads in CI's cache; a scratch branch
     pushed and CI run on it, then deleted; the budgets, the margin, the
     core benchmark, the owner checks' wording and `bench.py` as a release
     step; browse pages at 100 ms; playback at 20× real time; the split of
     this file, Phase 4's decisions kept here, `--counts`; the syn
     generator over tauri-specta; the test-only derive in `remote/`; the
     `image` crate over ImageIO, and the thumbnails' index.
   - CI (runs #6 and #9): two causes, both fixed.
     - The fuzz build failed on every run: the macos-15 runner's default
       Xcode 16.4 links with an ld that can't read clang 22's objects
       ("invalid r_symbolnum"); here the newer ld can. Homebrew's
       `lld@22` linked it, but broke C++ exceptions in it (the tags
       fuzzer died on an exception TagLib catches itself), so it was
       dropped and uninstalled. Now `run-fuzzers.py` passes
       `ANOMP_FUZZ_DEVELOPER_DIR` to its own builds as `DEVELOPER_DIR`,
       and `ci.yml` and `fuzz.yml` set it to the runner's Xcode 26.3;
       every other build keeps the default Xcode. 4 script tests.
     - Run #9 also lost the asan and tsan presets to 502s from GitHub's
       archive downloads, made likely by each preset downloading every
       tarball again. `cmake/Fetch.cmake` (`anomp_fetch_declare`) now
       downloads each pinned tarball once into `build/_downloads`, checks
       its SHA-256, retries three times with backoff, and hands FetchContent
       the file; `ci.yml` caches the folder. 6 script tests (`cmake -P`
       over `file://` URLs).
     - Verified on a scratch branch (ci/fuzz-linker, deleted after): CI
       passed all 25 steps it had then, the fuzzers built with Xcode 26.3,
       and H14's bundle self-test passed every stage on the runner for the
       first time, playback included ("handed off after 0.6 s on Apple
       Virtual Sound Device").
   - Allowed, not fixed (each in its entry): 44.1 kHz files resampled at
     30× real time (H18; the budget is 20×); browsing by year or genre at
     56–73 ms (H18; 100 ms budget); restore's 290 ms of track details
     (H16); `queue/model.rs` not split (H19), having changed little.
   - Next: the owner's items above, in order; then Parts 0–3 of the signed
     release as decisions arrive. Engineering left that needs no
     decision: H19 as modules are touched, H21 (clang-tidy), M3 and M5's
     scripts, `doctor.py`.

Then the signed release (§8.3 with a Developer ID, §8.7), then Phase 8. H20 (design
records out of this file) is P2, but it can be pulled forward whenever
this file gets in the way.

**Hardening items** (from the 2026-09-27 review; priorities as in
§4.7). CI runs each check through `check-all.py` (M4), so a local run
matches it.

| # | Item | Area | Size | Priority |
|---|---|---|---|---|
| H1 | Content Security Policy | security | S | P1 |
| H2 | Keep developer commands and `/dev` out of release builds | security | S | P1 |
| H3 | Narrow the URL opener and per-window command permissions | security | S | P1 |
| H4 | Contain folder pictures on unsandboxed platforms | security | S | P2, before Phase 9 |
| H5 | Fuzz the tag reader and decoder | security, robustness | M | P1 targets and CI run, P2 long runs |
| H6 | Sanitizer presets for the core | robustness | S | P1 |
| H7 | Supply chain: exact pins, `cargo deny`, update bot, secret scanning | security, maintenance | S | P1 |
| H8 | Rust and C++ lint gates, pinned toolchains | maintenance | S | P1 |
| H9 | Logs, panic capture and "Copy diagnostics" | robustness, support | M | P1 |
| H10 | Library DB safety: backups before migrations, checks, pruning | robustness | S–M | P1 |
| H11 | Open files off the main thread | robustness | M | P1 |
| H12 | Cloud, network and removable folders | robustness | M | P1 |
| H13 | Frontend lint, format and tests | maintenance | M | P1 lint, P2 tests |
| H14 | A self-test of the sandboxed bundle in CI | robustness | M | P2 |
| H15 | Typed IPC end to end | maintenance | S–M | P2 |
| H16 | Queue storage and updates that scale | efficiency | M | P2 |
| H17 | Cover thumbnails and an on-disk art cache | efficiency | M | P2 |
| H18 | Performance budgets | efficiency | S | P2 |
| H19 | Split the largest modules | maintenance | S each | P3, as touched |
| H20 | Move design records out of `PLAN.md` | maintenance | S | P2 |
| H21 | C++ static analysis | maintenance | S | P3 |
| H22 | Missing folders at launch | robustness | M | P1 |

- **H1 Content Security Policy.** `tauri.conf.json` has `"csp": null`.
  Set a strict policy:
  - `default-src 'self'`
  - `img-src 'self' anomp-art: data: blob:`
  - `connect-src ipc: http://ipc.localhost`
  - `style-src 'self' 'unsafe-inline'`
  - `object-src 'none'` and `frame-src 'none'`

  Add a `devCsp` for Vite's HMR. Check that covers still load, that the
  visualizer can still read a cover's pixels (CORS on `anomp-art`), and
  that the visualizer `Channel` still works. Nothing renders remote HTML
  today (no `{@html}`; biographies are text), so this is defence in
  depth. It is needed before any richer remote content, Bandcamp (Phase
  11) or O14.
  - **Done 2026-10-02.** `tauri.conf.json` has the policy above, plus
    `script-src 'self'`, `font-src 'self'`, `base-uri 'none'` and
    `form-action 'none'`. Tauri adds the hash of SvelteKit's inline boot
    script to `script-src`.
    - `dangerousDisableAssetCspModification: ["style-src"]`: otherwise
      Tauri puts a nonce in `style-src`, and a nonce makes browsers
      ignore `'unsafe-inline'`. That would block every `style` attribute
      (Svelte's, and `app.html`'s).
    - Checked in a built bundle with a temporary probe, run against a
      scratch library of `core/tests/fixtures`:
      - no violations;
      - a cover from `anomp-art` drawn into a canvas and its pixels read;
      - a `data:` image;
      - 149 visualizer `Channel` frames in 2.5 s of playback;
      - inline styles applied.
      A remote image was blocked and reported, so the policy is
      enforced.
    - `devCsp` (the same policy, plus `ws:` for Vite's HMR) does nothing
      on desktop: in `tauri dev`, Tauri loads Vite's URL directly and
      sets no CSP there. It applies only where Tauri proxies the dev
      server (iOS, Phase 8). The LAN remote's page (`remote/`) already
      sends its own CSP.
- **H2 Developer surface out of release builds.** Release builds
  register `player_load` and `player_set_next` (any typed path), the
  test-tone commands, and the `/dev` route.
  - Register those commands only under `cfg(debug_assertions)`, or a
    `dev-tools` Cargo feature. Leave `/dev` out of the release frontend.
  - On Linux and Windows (Phases 9–10) there is no sandbox. There,
    `player_load` would hand any readable file to FFmpeg.
  - **Done 2026-10-02.** The /dev page's ten commands (`core_version`,
    `audio_device_name`, the test tone, `player_load`/`set_next`/`play`/
    `pause`/`stop`/`seek`) are in `src/dev.rs`, compiled and registered
    under `cfg(debug_assertions)` only. `build.rs` still lists them in the
    main window's permission set, so that file is the same in every
    profile; in a release build there is nothing to call.
    - The page is `components/dev/DevPage.svelte`, imported by
      `routes/dev/+page.ts` only when `__DEV_TOOLS__` (a Vite `define`)
      is true: under `vite dev`, or in the Tauri CLI's debug builds
      (`TAURI_ENV_DEBUG`). Otherwise the import is dead code and its chunk
      isn't emitted; `/dev` redirects to `/`. A release frontend build
      holds none of those command names.
- **H3 URL opener and command permissions.**
  - `opener:allow-open-url` allows every `http://` and `https://` URL.
    The links come from MusicBrainz URL relations and Wikipedia, which
    anyone can edit. Allow `https` only, and upgrade or show as plain
    text any `http` link.
  - Use Tauri 2's app-command permissions
    (`tauri_build::Attributes::app_manifest`), so a window gets only the
    commands it needs. This is required before a second window (F7), and
    before any remote-facing surface (O14).
  - **Done 2026-09-27 (the command permissions, with F7):** `build.rs`
    declares every app command in Tauri's app manifest and writes the
    main window's permission set (`permissions/main-window.toml`, from
    `generate_handler!` in `lib.rs`); the mini player's
    (`permissions/mini-window.toml`) lists the commands it uses.
  - **Done 2026-10-02 (the URL opener):** `capabilities/default.json`
    allows `https://*` only.
    - Every link goes through `lib/openLink.ts` (`openLink` for an `<a>`,
      `openWebLink` for Get Info's MusicBrainz links). It opens only what
      `lib/links.ts`'s `webLink` allows: https, with `http` upgraded.
    - Every `href` built from service data (MusicBrainz homepages,
      Wikipedia articles and licences, release pages, credits) is
      `webLink(…)`. Any other scheme gives no `href`, so it shows as plain
      text.
    - Tested in `tests/links.test.mjs`. The six copies of `openLink` in
      the components are now one.
- **H4 Folder pictures on unsandboxed platforms.**
  `folder_art::is_relative_path` rejects `..` lexically, but a symlink
  inside a library folder can still point outside it. The macOS sandbox
  blocks that, but Linux and Windows have no sandbox. Canonicalise the
  picture's path and require it under the folder's root, with a test
  using a symlink.
- **H5 Fuzzing.** Users' libraries hold arbitrary files. FFmpeg's
  demuxers and decoders and TagLib are the app's largest attack surface
  (§6).
  - libFuzzer targets in `core/fuzz/`:
    - the tag reader, through an internal overload that takes a
      `TagLib::IOStream` (a `ByteVectorStream`), not a path;
    - `FFmpegAudioFormat`: open, read, and seek at random positions,
      over a `juce::MemoryInputStream`.
  - Seed the corpus from `core/tests/fixtures/`, and build with ASan and
    UBSan (a `fuzz` preset).
  - CI runs each target for 60 s (on each version tag, through
    release.yml, or by hand; not on every push since 2026-10-02). A weekly job runs them
    for longer and keeps the corpus as an artifact.
  - Each crash found becomes a regression test with a committed fixture.
  - **Done 2026-10-02 (the P1 targets and the CI run; the weekly long run
    is set up, its results to come).**
    - Toolchain: Apple's clang has no libFuzzer runtime
      (`libclang_rt.fuzzer_osx.a`), so the `fuzz` preset compiles
      everything with Homebrew's `llvm@22` (`brew install llvm@22`, also
      in CI). LLVM 21's ASan hangs at start-up on macOS 26 (Darwin 25):
      `get_dyld_hdr` mallocs while ASan initialises. LLVM 22 and 23 work.
    - `core/fuzz/`: `DecoderFuzzer` opens the input from a
      `juce::MemoryInputStream`, reads its start and four positions the
      input picks (from its last bytes, so a seed loses little);
      `TagReaderFuzzer` runs `readTags` and `readFileInfo` over a
      `TagLib::ByteVectorStream`, through new overloads taking a
      `TagLib::IOStream` (`TagReader.h` forward-declares it, so no TagLib
      header leaks). The file overloads now go through them, and a test
      checks a stream gives the file's tags.
      `FFmpegAudioFormat::silenceLog` quiets FFmpeg's stderr for the
      decoder target, which includes no FFmpeg header.
    - FFmpeg is instrumented too: `build-ffmpeg.sh --fuzz` builds the same
      pin and formats as static arm64 libraries with libFuzzer coverage,
      ASan and UBSan into `third_party/ffmpeg/macos-arm64-fuzz/`, which
      `cmake/FFmpeg.cmake` imports when `ANOMP_BUILD_FUZZERS` is on.
      Without it libFuzzer would get no coverage from inside FFmpeg.
    - `scripts/run-fuzzers.py` builds both and runs each target (60 s by
      default) over `build/fuzz/corpus/<target>`, seeded from
      `core/tests/fixtures/`; failing inputs land in `build/fuzz/crashes/`.
      `check-all.py` runs it as "core fuzzing" (not `--quick`), so CI does.
      CI caches `build/fuzz`. `.github/workflows/fuzz.yml` runs each target
      for 30 min on Mondays (and by hand), keeping the corpus in the cache
      and uploading it with any failing inputs.
    - Found: no crashes, leaks, hangs or UBSan reports, in 10 min per
      target locally (32,681 decoder and 276,768 tag runs, 2,444 and 4,749
      new corpus units) and in every 60 s run since. One slow tag input
      (a passing stall; 0.12 s when run alone) was not a failure.
    - Found 2026-10-02 (Step 7's check-all run): UBSan in TagLib's
      Shorten reader (`shortenfile.cpp:135`, a signed left shift too far),
      reached through `FileRef`'s detection by content from a 40-byte
      input. Shorten is a format the app can't play. Fixed by building
      TagLib with only the formats FFmpeg plays (`cmake/TagLib.cmake`:
      Shorten, TrueAudio, DSF, tracker modules and Matroska off; APE kept
      for MP3's APE tags), which also takes their parsers out of the
      attack surface. The input is the first fuzz fixture
      (`core/tests/fixtures/fuzz/tags-shorten-shift.bin`), run by "Inputs
      the fuzzer found stay harmless", which failed under the asan preset
      before the fix and passes after. Not reported upstream (the owner's
      call).
- **H6 Sanitizer presets.** Add CMake presets `asan` (Address and
  Undefined) and `tsan` that build and run the Catch2 suite. `tsan`
  covers `SignalTap`, `AnalysisThread`, and the audio thread's hand-off
  and events. Both run in `check-all.py` (not `--quick`) and in CI.
  - **Done 2026-10-02.** `CMakePresets.json` has `asan` (`-fsanitize=
    address,undefined`, UBSan errors fatal) and `tsan`, each in its own
    build directory. Workflow presets (`cmake --workflow --preset asan`)
    configure, build and run ctest, with `ASAN_OPTIONS`/`UBSAN_OPTIONS`/
    `TSAN_OPTIONS` set to stop at the first report.
    - Both are steps in `check-all.py`. TSan takes about 4.5 min locally.
    - Both found nothing (100 of 100 each).
    - No test ran the engine on two threads: every `PlayerEngine` test
      renders on the thread that calls it. "PlayerEngine hands off while
      an audio thread renders" now renders on its own thread while the
      message thread queues next tracks, seeks, changes gains and
      dispatches events, for 20 hand-offs.
    - A racy program built with the same flags is caught, so the presets
      would report a real race.
- **H7 Supply chain.**
  - Pin JUCE and Catch2 by commit SHA, not tag, since a tag can move
    (TagLib and FFmpeg are already pinned by SHA-256).
  - CI installs with `npm ci`, and GitHub Actions are pinned by commit
    SHA.
  - `cargo deny` (`deny.toml`) checks advisories, licences, bans and
    allowed sources. M5's `audit-deps.py` runs it in place of
    `cargo audit`, and its licence list feeds `make-notices.py` (M6).
  - Dependabot or Renovate for Cargo, npm and Actions, grouped into the
    monthly update in §9.1.
  - GitHub secret scanning with push protection, and `gitleaks` in the
    pre-commit hook. The app handles keys now (the Discogs token), and
    signing secrets arrive with §8.2.
  - **Done 2026-09-27 (Tauri's pins):** `tauri`, `tauri-build` and the
    dialog and opener plugins are pinned exactly in `Cargo.toml`, and
    their npm packages (`@tauri-apps/api`, `cli` and the plugins)
    exactly in `package.json`. The loose `2` ranges had let `cargo`
    move `tauri` to 2.12 while npm kept `@tauri-apps/api` at 2.11,
    and `tauri build` refuses mismatched major.minor versions. Each
    crate moves together with its npm package.
  - **Done 2026-10-02 (the rest):**
    - JUCE and Catch2: `URL` GitHub's archive of the commit each release
      tag names (JUCE `7278278`, Catch2 `317ac1e`, from `git ls-remote`),
      plus its `URL_HASH`, as TagLib and Signalsmith are pinned. A
      shallow git clone can't fetch a commit by hash, and a full clone of
      JUCE is large. Each archive was hashed twice and matched the
      previous git checkout file for file.
    - Actions in `ci.yml` are pinned by commit with the release in a
      comment: checkout v5.1.0, setup-node v5.0.0, cache v4.3.0. Newer
      majors exist (v7, v7, v6); Dependabot's first run offers them.
    - `app/src-tauri/deny.toml`: `cargo deny check` (cargo-deny 0.20.2,
      installed `--locked` in CI) is a `check-all.py` step, not
      `--quick`, since it fetches the advisory database.
      - Licences: the permissive ones in the tree, plus MPL-2.0
        (cssparser and selectors, through Tauri).
      - Crates from crates.io only; wildcard versions denied; duplicate
        versions only warn.
      - It found RUSTSEC-2024-0370 (proc-macro-error unmaintained). That
        is ignored with its reason: Linux only, through Tauri's gtk-rs
        0.18. It also found a yanked `yoke-derive` 0.8.3, moved to 0.8.4.
    - `.github/dependabot.yml`: Cargo, npm and Actions, monthly, one
      grouped pull request each, with a 7-day cooldown. Tauri's crates
      and npm packages get patch updates only, since the two sides must
      move together by hand.
    - Secrets: gitleaks is a quick `check-all.py` step over the history
      (CI checks out every commit for it). `scripts/hooks/pre-commit`
      runs gitleaks over the staged changes, then `check-all.py --quick`.
      It is the pre-commit hook M4 describes, installed with
      `git config core.hooksPath scripts/hooks`. GitHub secret scanning
      with push protection is a repository setting the owner turns on.
- **H8 Lint gates and pinned toolchains.**
  - `cargo clippy --all-targets -- -D warnings` runs in `check-all.py`.
  - A `[lints]` table in `Cargo.toml`, including `unsafe_op_in_unsafe_fn`
    and `clippy::undocumented_unsafe_blocks`, so each `unsafe` block in
    `anomp.rs` says why it is sound.
  - `rust-toolchain.toml` pins §3's Rust version, and `.nvmrc` with
    `engines` pins Node, so local and CI builds match. `doctor.py` (M4)
    reads them.
  - Move to Rust edition 2024 in one separate commit.
  - **Done 2026-10-02 (all but the edition move and `doctor.py`):**
    - `[lints]` in `Cargo.toml` sets `unsafe_op_in_unsafe_fn` and
      `clippy::undocumented_unsafe_blocks` to warn, which `-D warnings`
      makes errors. `check-all.py` runs `cargo clippy --all-targets -- -D
      warnings`.
    - Fixed: 13 unsafe operations in `anomp.rs`'s unsafe fns, now in
      their own blocks with `SAFETY:` comments; a missing one on
      `FolderAccess`'s `Sync`; and 11 small lints (slices from
      references, `as_chunks`, `is_multiple_of`, `Range::contains`,
      `sort_by_key`, …).
    - Allowed:
      - `clippy::type_complexity`, crate-wide: its 15 cases are SQL rows
        read as tuples where they're queried.
      - `drop_non_drop` at one statement in `remote/mod.rs`
        (`drop(library)` on a `State`, which only marks the end of its
        use); the code itself is unchanged.
      - In release builds, `dead_code` on the four `Engine` wrappers
        only the debug-only /dev commands call (H2).
    - `rust-toolchain.toml` (repo root) pins Rust 1.98.1 with clippy and
      rustfmt; CI runs `rustup toolchain install`.
    - `.nvmrc` pins Node 26.10.0, which CI's setup-node reads.
      `package.json` `engines` (`>=26.10.0 <27`) with `engine-strict` in
      `app/.npmrc` makes npm refuse other versions.
    - C++ lint is H21. `doctor.py` (M4) doesn't exist yet; it should read
      both pins when written.
- **H9 Logs, panics and diagnostics.** The Rust code reported problems
  with `eprintln!` (54 calls on 2026-10-02, none now), which a bundled app sends nowhere. With
  `panic = "abort"` in the release profile, a panic leaves no trace.
  - Use the `log` facade with `tauri-plugin-log`, writing a rotating file
    of a few MB in the app's log directory.
  - A panic hook writes the message and a backtrace there before the
    abort.
  - The core logs through a C callback (`anomp_set_log_callback`),
    including JUCE's `Logger` and failed assertions.
  - Redact keys, `Authorization` headers and `token=` query parameters.
    Library paths are logged at debug level only.
  - Settings → About gets "Show logs" and "Copy diagnostics": versions,
    OS, output device, and library counts, with no paths or titles.
  - This is needed whatever the crash-reporting decision in §8.1 is, and
    a crash reporter would build on it.
  - **Done 2026-10-02.**
    - `logging.rs`: tauri-plugin-log 2.10.0 (the crate and its npm
      package pinned exactly; MIT or Apache-2.0, `cargo deny` passes)
      writes `ano-mp.log` in the app's log directory
      (`~/Library/Logs/dev.anomp.player`, inside the container when
      sandboxed): 2 MB a file, the current one and two old ones.
      - Release builds write info and above; debug builds debug too, and
        copy it to stderr. Other crates log only warnings.
      - Lines are `<UTC time> <LEVEL> <target>: <message>`. Targets are
        module paths, "core" for the C++ core, "webview" for the page,
        "panic".
    - Redaction, at every level: keys the app holds (`keep_secret`,
      which `metadata::keys` calls for each key it reads or stores, the
      ListenBrainz token included), `Authorization` values, and the
      `token`, `access_token`, `api_key`, `apikey`, `key`, `secret` and
      `password` query parameters.
    - At info and above (the owner's decision): absolute paths become
      `<path>`, and URLs keep only their scheme and host. Messages give
      ids and counts; titles, artists and paths go at debug.
    - Panics: `logging::install_panic_hook`, the first thing `run` does,
      writes the message, where, the thread and a backtrace, and
      flushes before the default hook and the abort. In release builds
      (`strip = true`) the backtrace has addresses only.
    - The core: `anomp_set_log_callback` (`core/src/Log.*`). JUCE's
      `Logger` goes there, and `JUCE_LOG_ASSERTIONS=1` sends failed
      assertions there in every build.
    - The webview's uncaught errors and unhandled rejections
      (`lib/logErrors.ts`, both windows, `log:allow-log`).
    - Settings › About (`AboutOptions.svelte`): the versions, "Show logs"
      (reveals the file in the Finder) and "Copy diagnostics"
      (`diagnostics.rs`). The diagnostics are versions, OS, build,
      whether sandboxed, the output device, library counts, folder states
      (counts), the feature switches and online sources that are on, the
      schema version, the DB's size, its copies, the launch check, and
      the log's last 100 lines at info and above. No paths or titles;
      the owner approved the additions to the list above.
    - The `eprintln!` calls are log calls. 51 were swapped mechanically:
      the "[tag]" prefix dropped (the target names the module), setup
      failures as errors, other failures as warnings, the rest as
      information. Two are in `remote/mod.rs`, for the §8.1 review to see.
      The queue's skipped-track message logs the track's id, and the art
      handler's error its picture's path only at debug.
    - Tests: redaction, scrubbing, levels, timestamps, the log's last
      lines, the panic hook, the diagnostics' text, switches and counts,
      and the core's callback (Catch2 and Rust).
    - **Fixed 2026-10-02 (Step 4): every quit crashed.** In a bundle, each
      quit logged "JUCE Assertion failure in juce_Timer.cpp:99", then
      aborted about 16 s later with a crash report (a pure virtual call in
      `juce::Logger::~Logger` from `exit`).
      - The core's logger (`Log.cpp`'s forwarder, a static) was destroyed
        with the process's statics while still JUCE's current logger; the
        host never clears its callback. JUCE's destructor asserted, and
        logged that through the half-destroyed logger. The forwarder now
        stops being the current logger first.
      - The assertion itself: `AudioEngine`'s JUCE runtime was a member, so
        it shut JUCE down before the engine's `Timer` base let go of JUCE's
        timer thread. The runtime is now the engine's first base
        (`JuceRuntime`).
      - Tests: "C API log may stay set when the process exits" (aborted at
        exit before the fix) and "C API engine shuts JUCE down after its
        timer" (no assertion logged).
    - Left: readable release backtraces need symbols (`strip =
      "debuginfo"`, or a saved dSYM), a size question for §8.2. FFmpeg's
      own log (`av_log`) isn't routed; it would be noisy on damaged files.
- **H10 Library DB safety.** The DB now holds work users can't recreate
  by rescanning: their picks, and soon playlists and history (F1, O8).
  - Before applying migrations, write a copy with `VACUUM INTO`
    (`library.db.pre-<n>`), keeping the last two.
  - Run `PRAGMA quick_check` in the background at launch. On failure,
    offer to restore the last copy, or to rebuild by rescanning after
    exporting what F20 can save.
  - Run `PRAGMA optimize` at exit.
  - Prune `mb_cache` by age and size. 4.5 records that nothing prunes it.
  - **Done 2026-10-02.**
    - Copies: before applying migration n to an existing database,
      `db::open` writes `library.sqlite3.pre-<n>` with `VACUUM INTO` (to
      a `.partial` file, then renamed), and keeps the newest two. (The
      file is `library.sqlite3`, not `library.db`.) A copy that can't be
      written stops the migration: the database stays as it was, and the
      library doesn't open until there is room.
    - The check: `library/recovery.rs` runs `PRAGMA quick_check(20)` on
      a thread of its own at launch (`commands::upkeep`). A database that
      can't be opened at all fails the same way.
    - The offer: on failure the UI shows `DbRepairDialog`, to restore the
      newest copy or to rebuild, or to do neither until the next launch.
      - Either choice is written to `library.recovery.json`, and the app
        restarts; the next launch carries it out before anything opens the
        database.
      - The damaged database (with its WAL files) is moved aside as
        `library.sqlite3.damaged-<unix seconds>`, never deleted.
      - A rebuild first exports what F20 can save to
        `library-rescue-<unix seconds>.json` next to the database; if
        that fails, the dialog asks before going on without it. It starts
        a new database with the old one's folders (bookmarks included)
        and settings, scans every folder, then imports the export.
    - `PRAGMA optimize` at exit (`commands::shutdown`).
    - `mb_cache` is pruned after a passing launch check: copies older
      than 180 days (`cache::MAX_AGE`; the longest freshness is 30 days),
      then the oldest until the rest fit in 64 MB (`cache::MAX_BYTES`).
    - Tests: `db` (a copy before migrating, none for a new database, the
      newest two kept, a copy that can't be written stops the migration),
      `recovery` (a sound and a damaged file, restore, rebuild, a request
      carried out once, a restore names only a file next to the
      database), `cache` (age and size).
    - Left: no database has a copy until migration 010 ships; until then
      a damaged one can only be rebuilt. A copy at other times (say, once
      a week) would close that; not planned.
- **H11 Open files off the main thread.** Opens run synchronously on the
  main thread (Phase 1's known limit: 7 ms for a local MP3). A sleeping
  USB disk, a NAS, or a cloud placeholder (H12) can take seconds, which
  freezes the UI, the media keys and the queue's hand-off arming.
  - Open and prime the reader on a worker thread, holding the folder's
    access (`open_folder`) while it opens. Hand the ready reader to the
    engine on the main thread.
  - The C API gets an asynchronous load with a completion event. The UI
    shows the track as loading, and a timeout fails it with a reason
    that the queue skips as it skips a missing file.
  - **Done 2026-10-02.**
    - Core: `PlayerEngine::loadAsync`/`setNextAsync` open the reader and
      build the track, its read-ahead prefilled, on a thread of their own
      (one per request, so one stuck on a disk that went away doesn't
      hold up the next); `dispatchEvents` hands ready tracks over on the
      message thread, in request order, and reports each request once
      (`onLoadFinished`: loaded, failed or cancelled). A load supersedes
      earlier requests; a next waits for an earlier load; `setNextAsync`
      clears the next track at once, so the current one can't hand off to
      the track being replaced; the synchronous commands cancel requests
      too. `AudioEngine` dispatches as soon as a track is ready
      (`AsyncUpdater`), not at the next 50 ms tick. The engine stays
      main-thread only. As it goes, it waits 2 s for opening threads, then
      lets them finish alone; they own what they share and give up once
      cancelled.
    - C API: `anomp_engine_load_track_async`,
      `anomp_engine_set_next_track_async`, `anomp_engine_cancel_load`, and
      `ANOMP_EVENT_LOAD_FINISHED` (`request`, `result`, `error`, the event's
      new fields). Bound in `anomp.rs` (`Event::LoadFinished`).
    - Rust: `queue/opening.rs` reads the track's row and resolves its
      folder's bookmark on a blocking thread, asks the engine on the main
      thread, holds the folder (`open_folder`) until the engine reports,
      and passes the outcome to the queue. The queue (`model.rs`) gets
      `Opening::Pending` from `Player::load`/`set_next`: the item is
      current and shown as loading (`QueueState::loading`), play, pause and
      seek apply when it's ready, the engine plays on what it had, and
      `check_loads` fails one still opening after `LOAD_TIMEOUT` (20 s)
      with `openTimedOut`, skipped as a missing file is. A
      `folderUnavailable` failure is still "not now". The UI says
      "Opening…" once an open has taken 300 ms.
    - Fixed on the way: the skipped-track toast showed a coded error as
      JSON; it goes through `errorText` now.
    - Tests: Catch2 for the async load with and without read-ahead, a
      failure, cancellation, supersession, the gapless hand-off to a next
      track opened on another thread, callbacks requesting loads, and the
      engine going with files opening (all under ASan and TSan too); the C
      API's requests and events; queue tests for loading, commands while
      loading, the timeout, skips, a folder out of reach, and removing or
      clearing a track as it opens.
    - Open time, 5-minute VBR MP3, 20 opens: `load()` took 21.5–22.2 ms on
      the main thread (Debug and Release alike; most of it the read-ahead
      prefill, which polls in 5 ms sleeps, beyond the reader's 7 ms open).
      `loadAsync()` takes 0.06 ms there (Release; 0.12 ms Debug), and
      21.6 ms from asking to loaded: no slower end to end. The benchmark
      is the hidden test "PlayerEngine open time" (`ANOMP_BENCH_FILE`).
- **H12 Cloud, network and removable folders.**
  - With "Optimize Mac Storage", iCloud Drive keeps dataless placeholder
    files. Reading their tags downloads them, so one scan could download
    a whole library.
    - The scanner checks for dataless files (`SF_DATALESS` in
      `st_flags`), records them without reading them, and says so in
      the scan report.
    - Playing one downloads it first, through H11's asynchronous open.
    - The check lives behind a small core interface next to
      `FolderAccess`.
  - Test SMB folders, and a drive unmounted in the middle of a scan. A
    missing folder keeps its tracks, and so do an empty mount point and
    files that go during a scan (H22).
  - **Done 2026-10-02 (SMB and real placeholders are the owner's checks,
    `docs/step4-checklist.md` § 5).**
    - Core: `FileStatus::isDataless` (`stat()`'s `SF_DATALESS`, which
      doesn't download), next to `FolderAccess`, one file per platform;
      `anomp_file_is_dataless` in the C API.
    - Migration 010: `tracks.dataless`, a file that was a placeholder when
      last seen. The scanner checks new and changed files only: a
      placeholder is recorded without reading it (a track with no tags) or,
      if known, keeps its rows; either is marked and counted in the scan
      report (`dataless`). Every scan checks marked ones again and reads
      them once downloaded, which changes neither size nor time; reading a
      file clears the mark. `ScanOptions::placeholders` lets tests fake the
      check.
    - The analysis worker and embedded art check before reading: the
      worker marks a placeholder and leaves it until a scan finds it
      downloaded; art tries the album's other files and sources.
    - Playing one downloads it through H11's open: `opening` sees the file
      is a placeholder, and the queue shows "Downloading from iCloud…" and
      allows `DOWNLOAD_TIMEOUT` (5 min). Settings › Library shows "N in
      iCloud, not downloaded" per folder.
    - Tests: the scanner with a fake check (recorded unread, checked again,
      read once downloaded; a changed placeholder keeping its tags); a
      drive unmounted between the walk and the reads keeping every track
      (and adding nothing), then catching up; the analysis leaving a
      marked track; the queue's longer timeout; the C API's check.
- **H13 Frontend lint, format and tests.** The frontend has
  `svelte-check` and 5 tests of pure modules. There is no linter or
  formatter.
  - P1: ESLint (`eslint-plugin-svelte`, `typescript-eslint`) and
    Prettier (with its Svelte plugin), through a `format` script with a
    `--check` mode like the others.
    - **Done 2026-10-02.** ESLint 10 (`app/eslint.config.js`: the
      recommended sets of ESLint, typescript-eslint and
      eslint-plugin-svelte, with eslint-config-prettier) and Prettier 3
      (`app/.prettierrc.json`: width 120, the Svelte plugin), each pinned
      exactly in `package.json`.
    - `scripts/format-frontend.py [--check]` runs app/'s own Prettier
      and is a quick `check-all.py` step. `npm run lint` is a full one.
      The tree was reformatted once (77 files).
    - ESLint found 25 problems:
      - Fixed: a `$state` kept in step by an `$effect` became a writable
        `$derived` (`PlaylistView`); an unused `svelte-ignore`
        (`Dialog`).
      - Allowed at each line, with the reason:
        - `prefer-svelte-reactivity` (7): maps and sets that must not be
          reactive (scratch, bookkeeping, copies into `$state.raw`, the
          drop-target registry).
        - `no-useless-mustaches` (2): `{" "}`, a space Svelte would trim
          at the start of a block.
      - Configured, for 13 links: `no-navigation-without-resolve` skips
        links. They are web links that `openLink` opens, and the app has
        no base path.
    - Type-aware rules (`recommendedTypeChecked`) are not on; they would
      need `svelte-kit sync` first and would find more.
  - P2: Vitest for the rune modules (`state/*.svelte.ts`), which plain
    `node --test` can't compile. IPC is faked with
    `@tauri-apps/api/mocks` (`mockIPC`), using payloads recorded from
    the Rust tests.
  - P2: Playwright smoke tests against `vite dev` with the same mocks,
    run in WebKit: browse, search, queue edits, settings. This is §6's
    mitigation for WebKitGTK and WebView2 differences, and they run on
    each OS's CI job from Phases 9–10.
- **H14 Self-test of the sandboxed bundle.** Sandbox mistakes show only
  in a bundle (`CLAUDE.md`), and `tauri-driver` doesn't support macOS.
  - A `--self-test <folder>` flag, compiled into test builds only, runs a
    script inside the ad-hoc-signed sandboxed bundle and exits with a
    status: use a temporary data directory, add the fixtures folder,
    scan, play two tracks through a gapless hand-off, and read the
    covers.
  - CI runs it on the macOS runner after building the bundle.
  - **Done 2026-10-02 (Step 8).** `app/src-tauri/src/self_test.rs`,
    compiled in only with the `self-test` Cargo feature (no new crate):
    `ano-mp --self-test` runs before Tauri starts, with no window and
    none of the app's files. It writes the 21 core fixtures (embedded)
    into the temporary folder (the container's when sandboxed), then:
    the sandbox (`APP_SANDBOX_CONTAINER_ID`), a scratch library with the
    folder added (a security-scoped bookmark) and scanned (every fixture a
    track, none failed), the bookmark resolved, the two tagged fixtures'
    covers through `art::lookup`, every file decoded (`analyse_file`, its
    folder held), and the two shortest tracks through a gapless hand-off
    at volume 0 (`advance_count`). Without an output device playback is
    skipped (`--require-audio` fails instead); `--require-sandbox` fails
    an unsandboxed run. One line per stage; nothing with a path.
  - `scripts/self-test-bundle.py` builds the ad-hoc bundle with the
    feature into `target/self-test` (apart from `build-app.py`'s), runs
    the executable with `--require-sandbox` and checks the lines. It is
    `check-all.py`'s "bundle self-test" step, which runs only on CI (`CI`
    set) or with `--local`, since a sandboxed bundle runs in the app's
    real container. No workflow change: CI's `check-all.py` runs it.
  - Tests: 6 Rust tests (every stage outside the sandbox but playback, the
    sandbox required, a folder that can't be written, the command line,
    the lines, every fixture embedded) and 7 script tests. Run here:
    unsandboxed with `cargo run --features self-test -- --self-test`, and
    in the sandboxed bundle with the owner's go-ahead (the container's
    temporary folder only; left as it was): every stage passed, the
    hand-off after 0.5 s. Not yet seen on CI's runner, which may have no
    output device (playback would be skipped there).
- **H15 Typed IPC end to end.** Phase 6 notes that payloads other than
  the settings are still hand-written in `api.ts`. Derive their
  TypeScript types with ts-rs as the settings do. Then generate the
  command wrappers too (tauri-specta, or a small generator), so a
  renamed command or argument fails `npm run check` instead of failing
  at run time.
  - **Done 2026-10-03 (Step 8).** `app/src-tauri/src/bindings.rs`, a
    test like `settings::bindings`, writes two files and fails while
    either is stale (`ANOMP_WRITE_BINDINGS=1 cargo test bindings`):
    - `app/src/lib/generated/ipc.ts`: 97 types, every payload and event
      the commands send and every type inside them (ts-rs's dependency
      walk from the list in `declare_types`), derived with
      `#[cfg_attr(test, derive(ts_rs::TS))]`. Where the frontend already
      had another name, a test-only `ts(rename)` keeps it (`Item` is
      `QueueItem`, `TrackSummary` is `Track`, and eight more). The two
      prefs types take `ts(optional_fields = nullable)`, since serde
      fills in missing fields.
    - `app/src/lib/generated/commands.ts`: one wrapper per command in
      `generate_handler!` (134), read from its signature with syn (a
      new direct dev-dependency, `=2.0.119`, MIT OR Apache-2.0, already
      in `Cargo.lock` through the macros; the owner chose it over
      tauri-specta, whose 2.0 is still a release candidate). Arguments
      in camelCase, Tauri's own (`AppHandle`, `State`, …) left out,
      `Option<T>` arguments optional, `Result<T, _>` resolving to `T`.
      Each type is resolved through the command file's `use` items to its
      full path, then to its TypeScript name, so two Rust types of one
      name stay apart.
    - `app/src/lib/api.ts` lost its 75 hand-written payload types (801
      lines out, 163 in) and calls `commands.*` instead of `invoke`
      with a string. Two hand-written types had described another Rust
      type than their name said: the album header's `AlbumLink` was
      `SourcedLink`, the signal path panel's `SignalPath` was
      `SignalPathPayload`; the components now name them so.
    - With the owner's agreement, `remote/`'s `RemoteStatus` and
      `RemoteDevice` take the test-only derive; nothing the remote does
      changed.
  - Pass: renaming `library_remove_folder`'s `folder_id` made `npm run
    check` fail ("'folderId' does not exist… Did you mean 'folder'?"),
    and so did renaming the command; both put back.
  - Tests: 8 unit tests of the generator (handler list, camelCase, the
    type mapping, `use` resolution, ambiguous and unknown types), plus the
    up-to-date check; svelte-check 0 errors, ESLint clean.
  - Found in Part D's check-all run: ts-rs visits a type's dependencies in
    an order that changes between builds, so `ipc.ts` came out shuffled
    and the check failed with nothing changed. The declarations are now
    sorted by name.
- **H16 Queue storage and updates.** Phase 3's known limits: the saved
  queue is one JSON value rewritten on every change (350 KB for 50,000
  tracks), and every change sends the whole list (6.6 MB).
  - Store the items as rows (a migration).
  - Send changes as numbered edits (insert, remove, move ranges), which
    the UI applies to its copy. It asks for the whole list only when it
    misses a number.
  - **Done 2026-10-03 (Step 8).**
    - Storage: migration 011 adds `queue_items` (uid, track id, `ord`,
      `original`), with sparse REAL keys for the play order and, while
      shuffled, the order before shuffling, so an insert or a move writes
      only its rows; keys too close to split are renumbered. The old list
      moves out of the `player.queue` setting in the migration itself
      (SQLite's JSON functions; 71 ms for 50,000), which keeps the current
      index, position, repeat, the volume and a new `shuffled` flag (an
      empty queue can be shuffled). `queue/store.rs` mirrors the rows on
      the main thread; restoring keeps the rows' uids and deletes rows
      whose track left the library. Files opened from outside the library
      take a place in the mirror but no row, as before.
    - Updates: the model logs `Edit`s (insert, remove, move, update) for
      the frontend and the store separately; a replace, clear, shuffle or
      unshuffle (and each new pass of shuffle with repeat all) is a reset,
      sent and written whole. `QueueState` carries `listVersion` and
      either `items` or `edits`; `app/src/lib/queueEdits.ts` applies the
      edits to the UI's copy and asks for the whole state when a version
      is missing or an edit doesn't fit. A dragged block is sent as its
      ranges leaving and coming back, and the store keeps its places in
      the order before shuffling. The remote still gets the whole state.
    - `bench.py` (§9.2 M4), with 50,000 items in a file database:
      | | before | after |
      |---|---|---|
      | "Play next" (state and save) | 17.8 ms | 1.6 ms |
      | its state | 8.6 MB | 642 bytes |
      | next track (state and save) | 1.3 ms | 1.4 ms (one small write) |
      | restore: the saved list | a 350 KB JSON value | 5.3 ms of rows |
      Restore as a whole is the tracks' details (`track_infos`, 290 ms on
      the file database, 180 ms in memory), which H16 doesn't change.
      A state no longer walks every item for unavailable ones when there
      are none.
    - `queue/model.rs` changed in under a tenth of its lines, so H19's
      split waits; `move_items` now finds its items through a set.
    - Tests: 5 model tests (edits replayed on 50,000 items, runs of
      removals and updates, resets and the full state's version, the
      original order of an insert while shuffled, restored uids), 6 store
      tests (rows following 50,000 items edit by edit, only touched rows
      written, renumbering, outside files, a block moved while shuffled,
      dropped rows), the migration on a file database with its `pre-11`
      copy, a queue saved before the rows restored shuffled where it was,
      an empty shuffled queue, 5 frontend tests (`queueEdits.test.mjs`).
- **H17 Cover thumbnails.** Phase 3's known limit: art is served at full
  size from a cache in memory only, so it is re-read after each launch,
  and Now Playing artwork is sent at full size.
  - Make thumbnails at two sizes, for lists and for the album header, in
    the on-disk image cache (`metadata/images.rs`), keyed by the
    source's hash.
  - Serve full size only where it is shown full size, and cap Now
    Playing's artwork.
  - Pick a JPEG/PNG decoder and check what it adds to the binary.
  - **Done 2026-10-03 (Step 8).**
    - Decoder (the owner's choice over ImageIO in the core): the `image`
      crate, `=0.25.10`, default features off, JPEG, PNG and WebP. +650 KB
      per architecture in a stripped release binary (about 1.3 MB in the
      universal app); 8 new crates (image, image-webp, zune-jpeg,
      zune-core, moxcms, pxfm, byteorder-lite, quick-error), all under
      licences in `deny.toml`'s list; `THIRD_PARTY_NOTICES` regenerated
      (391 KB). GIF and anything it can't decode is served as it is.
    - `library/thumbs.rs`: 128 px for lists and 512 px for the album
      header, grids, the visualizer and the OS's Now Playing (which no
      longer gets the full picture), JPEG at quality 85 (PNG when the
      picture has transparency), never scaled up. Files in the image
      cache (`metadata/images.rs`, same budget and eviction), named by the
      SHA-256 of the picture they came from, so albums with one cover
      share them.
    - Migration 012 adds `art_thumbs`: which picture an album or track
      shows, where it came from (`art::Origin`: a file in a library folder,
      or a download) and a stamp of it (the file's size and time and its
      folder's time, or the URL). A launch checks the stamp with `stat`
      (which never downloads a cloud placeholder) and serves the
      thumbnail unread. Choosing or downloading a cover deletes the
      album's row; changing the sources deletes all; a scan deletes none,
      since a launch rescan would empty the index every time, and the
      stamps catch changed files and pictures added to a folder.
    - The UI asks `?size=list` or `?size=header` (`artUrl`, `Art.svelte`'s
      `quality`); full size only in the Now Playing view and the "Choose
      cover" dialog. The memory cache keeps each size apart.
    - Made only when the UI shows a cover, on the scheme's blocking
      threads, never in the background; `art::lookup` already passes over
      placeholders (H12).
    - `bench.py`: from a 1,500 px cover (204 KB), a list thumbnail is 11 KB
      made in 11 ms, a header one 76 KB in 21 ms, once per picture.
    - Tests: 5 (scaling and what is left alone, made once and indexed,
      the index served unread and a folder change caught, small pictures
      not indexed, forgetting), and size URLs parsed.
- **H18 Performance budgets.** Phase 7's performance bullet needs
  numbers. Commit budgets and check them with `bench.py` (M4):
  - launch to first paint;
  - memory at rest with 50,000 tracks;
  - scan time;
  - browse and search latency;
  - CPU while playing, with and without the visualizer.
  - **Done 2026-10-03 (Step 8).** The budgets are in
    `scripts/bench-baseline.json` with the baseline, measured on an Apple
    M1 Ultra (Virtual): 6 cores, 12 GB, macOS 26. `scripts/bench.py`
    checks them (M4); the owner agreed each budget, how it is measured,
    the margin and where it runs:

    | Budget | Ceiling | Measured by | Baseline |
    |---|---|---|---|
    | Search, any query, all kinds | 100 ms | Rust benchmark | 49 ms ("k") |
    | Browse, first page of a grouping or of its first group | 100 ms | Rust benchmark (new) | 73 ms (year) |
    | Browse, a whole-library node's tracks in order | 300 ms | Rust benchmark | 198 ms |
    | First scan of 50,000 files | 90 s | Rust benchmark (new: the fixtures as APFS clones) | 9.4 s |
    | Unchanged rescan | 10 s | the same | 0.27 s |
    | Playback through the engine, any file, effects included | ≥ 20× real time | C++ benchmark (new, `bench` preset) | 27× (MP3 with crossfeed and EQ) |
    | The visualizer's analysis | ≥ 50× real time | C++ benchmark (new) | 403× |
    | Launch to first paint | 1.5 s | owner check (`first paint after N ms` in the log) | not yet run |
    | Memory at rest, 50,000 tracks | 400 MB, app and WebKit processes | owner check | not yet run |
    | Whole-app CPU while playing | 5%, 25% with the visualizer | owner check | not yet run |

    - Past its budget, or more than 25% worse than the baseline (and over
      the unit's floor: 2 ms, 0.05 s, 0.1 MB), a result fails. On another
      machine only the budgets count. `--update` rewrites the results.
    - Not in `check-all.py`: it needs a release build of the crate and
      the `bench` preset's JUCE build, and shared runners are too noisy
      for the margin. It is §8.7 step 4, and §9.1's row after scanner,
      browse, search or DB changes.
    - The owner checks are § 6 of `docs/release-smoke-test.md`. The app
      logs `first paint after N ms` once (`diagnostics_first_paint`, two
      frames after the main page mounts; a count only).
    - Found and agreed, not fixed: the core resamples 44.1 kHz files to
      a 48 kHz device at about 30× real time (JUCE's windowed sinc, Phase
      1's design), against 700–1,200× for files at the device rate, so the
      playback budget is 20× for any file, not 100×. Browsing by year or
      genre and a folder's first page scan every track (each album's
      earliest year; `anomp_genres` per row): 56–73 ms, so browse pages
      share search's 100 ms. A stored album year and a genre table
      (migrations) would make them fast when browsing is next worked on.
    - Tests: 13 script tests (`test_bench.py`, over saved output), a
      Rust test of the first-paint timing; the new benchmarks are ignored
      (Rust) or hidden (`[.][bench]`, Catch2), so suite counts don't
      change.
- **H19 Split the largest modules**, when they are next changed, with no
  change in behaviour:
  - `metadata/jobs.rs` (2,347 lines), by job kind;
  - `library/browse.rs`;
  - `queue/model.rs`;
  - `anomp.rs`, one file per area of the C API.
- **H20 Design records out of `PLAN.md`.** The plan is over 3,000 lines
  and grows with every step. It mixes the roadmap with finished design
  notes. Move each finished phase's design and known limits to
  `docs/design/phase-<n>-<name>.md` (`docs/` is empty). Keep status,
  decisions, open steps, the backlogs (§4.6–§4.7, H1–H22) and links
  here. `check-docs.py` (M2) checks the links.
  - **Done 2026-10-03 (Step 8).** Phases 0–6c's design notes, steps as
    built, tests and known limits moved word for word (1,489 lines, checked
    line by line against the old file) to
    `docs/design/phase-{0-toolchain,1-playback-engine,2-metadata-library,3-player-ui,4-online-metadata,5-visualization,6-settings,6b-optional-features,6c-expected-features}.md`,
    each with a heading and a line saying where it came from. Each phase
    here keeps its heading, a link, and its status lines (**Exit**,
    **Checked**, **Not checked**, "Phase N complete"); Phase 4 also keeps
    its goal, the sources table and its design decisions (the owner's
    choice). Phase 7, Phases 8–11, §1–§4 and §6–§9 are unchanged. This
    file went from 4,411 lines to about 2,900.
  - `CLAUDE.md`'s two pointers to Phase 1's design now name its file, and
    it says where finished phases' design goes. The code's "PLAN.md Phase
    N" comments still lead here, then to the file.
  - `scripts/check-docs.py` (M2): every relative link and backquoted repo
    path in `PLAN.md`, `CLAUDE.md`, `README.md` and `docs/` exists (742
    paths and 7 links before the move, none broken). A shortened path
    (`library/access.rs`) matches the tail of a file git lists; built,
    fetched and run-time files and scripts §9.2 plans are skipped or
    listed in its `NOT_IN_REPO`. A quick check-all step. `--counts`
    compares §2's four test counts with the suites (`ctest -N`, `cargo
    test -- --list` less the ignored, `npm test`, pytest's collection), a
    full-run step. Tests: 30 in `test_check_docs.py`, one against the real
    tree.
- **H21 C++ static analysis.** `clang-tidy` with a small set of checks
  (`bugprone-*`, `performance-*`, `concurrency-*`) over `core/src`, in
  `check-all.py` but not `--quick`.
- **H22 Missing folders at launch.**
  - **Today:**
    - F8 marks a folder whose bookmark doesn't resolve as unavailable in
      the sidebar and settings, with "Locate…".
    - The launch rescan (F9) fails that folder alone. The failure goes
      into the scan report and `eprintln!`, so nothing tells the user
      at launch.
  - **Gaps:**
    - **A folder that exists but is empty is emptied.**
      - The path of an unmounted network share, or a mount point left
        under `/Volumes`, can still be a directory.
      - `walk_folder` checks only `is_dir()`, so the rescan finds no
        files and removes every track in the folder.
      - That loses everything keyed by those tracks: plays, ratings,
        playlist entries, analysis. `kept_albums` keeps only album and
        artist picks.
    - **A folder moved to the Trash is followed there.** The bookmark
      resolves to its new place, and `open_folder` quietly records the
      Trash path.
    - **The restored queue and resume position** (F17, Phase 3) can
      point at tracks in a missing folder. It isn't defined what plays,
      or how the queue skips, when every item is unavailable.
    - **Nothing re-checks a folder when its drive returns.** The watcher
      leaves out folders that are unavailable at launch and doesn't pick
      them up until the settings change or the app restarts.
    - **The workers can treat missing files as failures.** The
      analysis worker, metadata worker and health view may record a
      missing file as unreadable or broken.
  - **Do:**
    - **Check every folder before the rescan, off the main thread.**
      Classify each as:
      - available;
      - missing (the bookmark doesn't resolve, or the path is gone);
      - empty where it had tracks;
      - in the Trash;
      - permission lost (including the ad-hoc-signing bookmark error in
        `CLAUDE.md`).

      Record the state and its reason with the folder, through
      `library_folders`.
    - **Never let a scan empty a folder.**
      - When a folder that had tracks reads as empty, or would lose most
        of its tracks in one scan, keep the tracks and mark the folder
        "empty, possibly not mounted".
      - Ask before removing them. Test it with a fake empty directory.
    - **Ask about a folder in the Trash.** Offer to locate it again or
      remove it, rather than following it there.
    - **Show it once at launch.** One message that isn't a dialog:
      - "N folders can't be found";
      - each folder's reason;
      - "Locate…", "Remove" and "Keep" buttons.

      It must not block playback of the other folders. When every folder
      is missing (a library on an external drive that isn't plugged in),
      show this in the main view in place of an empty library.
    - **Missing tracks in lists.**
      - Tracks of an unavailable folder stay in browse, search,
        playlists and history, shown as unavailable.
      - Radio, shuffle refills, smart playlists' "play" and Home's
        suggestions skip them.
    - **The queue at launch.**
      - A restored current track that can't be opened becomes "not
        available" with its position kept, and nothing plays by itself.
      - When playing, the queue skips unavailable items, as H11's
        timeout does, without spinning when all of them are unavailable.
      - Media keys and Now Playing show the stopped state.
    - **When drives come back.**
      - Watch for volumes mounting and unmounting (`NSWorkspace`
        notifications behind a core interface next to `FolderAccess`, or
        FSEvents on `/Volumes`).
      - A folder that becomes available is rescanned and watched, and
        the sidebar updates without a restart.
    - **Workers.** Analysis, metadata and health treat tracks of an
      unavailable folder as "not now", not "failed". They retry when the
      folder returns.
    - **Errors.** Use coded errors (`folder_unavailable` with a reason
      code) and `en.json` messages for each state. H9 logs each state
      change.
  - **Tests:**
    - unit tests over the folder states with a fake `FolderAccess`;
    - a scanner test where a folder that had tracks is now an empty
      directory, and keeps them;
    - queue tests for a restored queue with all items unavailable.
  - **In the step 4 checklist:**
    - launch with a USB drive unplugged, then plug it in;
    - an SMB share that isn't mounted;
    - a folder moved to the Trash;
    - a folder deleted.
  - **Done 2026-10-02 (H22a), all but the lists (H22b).**
    - States: `access::FolderState` (available, missing, empty,
      mostlyGone, inTrash, noPermission), with the system's words.
      - `access::check_folder` finds them: it resolves the bookmark
        through the `Bookmarks` trait (`System` over `FolderAccess`; a
        fake in tests), lists the folder, and calls it empty only if it
        had tracks.
      - `availability::FolderStates` keeps them in memory: they are found
        again at each launch, so no migration. `library_folders` returns
        them (`Folder.status`); each change is logged and announced
        (`library-folders`).
    - Never emptied: `scanner::holds` keeps a folder's tracks when a
      scan would remove all of them, or more than half of a folder of 20
      or more. Moves to other folders are matched first. The folder fails
      as `empty` (nothing found) or `mostlyGone`.
      - `library_remove_missing` removes them when the user says so
        (after a confirmation).
      - A file that fails to read because it went during the scan keeps
        its tracks too.
    - The Trash: `open_folder` refuses a bookmark that resolves into a
      Trash (`.Trash`, `.Trashes`, a freedesktop `Trash/files`) and leaves
      the stored path alone; the user locates or removes the folder.
    - At launch: every folder is checked off the main thread, on a
      connection of its own, before the launch rescan. Only folders that
      are there are rescanned and watched.
    - The message (`MissingFolders.svelte`) sits above the main view and
      isn't a dialog: "N folders can't be found", each folder's reason,
      and "Locate…", "Remove…" (or "Remove missing tracks…" for empty and
      mostlyGone) and "Keep" (hidden until the next launch).
      - When every folder is missing, it takes the place of the library
        and Home views.
      - The sidebar and Settings › Library show each folder's reason.
    - The queue: `Queue::set_unavailable_tracks` marks the items of
      unavailable folders (not reported as skipped) whenever folders
      change. When nothing opens, the current item stays with its
      position, and nothing spins. Now Playing shows a restored current
      item that can't be opened as stopped.
    - Drives coming back: the core's `VolumeWatcher`, next to
      `FolderAccess` (NSWorkspace's mount and unmount notifications;
      nothing on iOS or elsewhere yet), through `anomp_volume_watcher_*`,
      hosted on the main thread by `availability::watch_volumes`.
      - Each event checks the folders again.
      - A folder that came back is rescanned and watched again, its
        queue items are tried again, and its tracks analysed.
    - Workers:
      - The analysis worker leaves a folder it can't read alone until a
        scan or the folder's return, storing no failure. It retries rows
        earlier versions stored; the health view and the failure count
        leave those out.
      - The metadata worker doesn't open library files, so it needed no
        change.
    - Errors: `coded::folder_unavailable(path, reason, detail)`, and
      `errorText` shows `error.<code>.<reason>` when the catalogue has it.
    - Tests:
      - folder states with fake bookmarks (each state, the Trash not
        followed, states through errors);
      - `availability` (a folder going, then coming back through empty; a
        held folder staying held; tracks of folders);
      - the scanner (an empty directory keeps its tracks and playlist
        entries, then removes them when told; most of a folder held, half
        not; files moved to another folder not held);
      - the queue (a restored queue with nothing available plays nothing
        and keeps its position, then plays when it comes back; a folder
        going while playing is passed over);
      - Now Playing showing it stopped;
      - the volume watcher (Catch2 and Rust).
  - **Fixed 2026-10-02 (Step 4): a deleted folder read as "no access".**
    Under the sandbox, a deleted folder's security-scoped bookmark fails
    with "isn't in the correct format", the words `access::unresolved`
    took for a rebuilt bundle's bookmark. It now goes by the stored path
    first: nothing there is `missing`; a folder there, or a path the
    sandbox won't stat, is `noPermission`; only otherwise does the error's
    text decide. Test: the case in `a_folder_the_app_may_not_read_has_no_permission`.
    Checked in the bundle for a folder inside the container; one outside
    it is in the owner's list.
  - **Done 2026-10-02 (H22b):** tracks of unavailable folders shown as
    such in browse, search, playlists and history, and left out by radio
    (and its refills, which is what "shuffle refills" meant: shuffle only
    reorders the queue), smart playlists' "play" and Home's suggestions.
    - Which folders: `FolderStates::unreadable()` (every unavailable state
      but `mostlyGone`, whose remaining files still play), through
      `availability::unreadable(app)`. Their ids are bound as JSON and read
      with `json_each` (`availability::json_ids`), never formatted in.
    - Left out:
      - `radio::picks` (start radio and the refills);
      - `smart::track_ids`, which `playlists_track_ids` (play, add to queue)
        uses: a limited smart playlist plays its limit of tracks that open.
        Lists of tracks keep them; the queue passes over them. M3U export
        keeps them;
      - Home: recently added, released on this day, the highlights
        (forgotten, a year ago, never played) and "More in this genre"
        leave out albums none of whose tracks can be opened
        (`discover::playable_album`). Recently added dates an album by
        its readable tracks. Home and "More in this genre" reload when the
        set of unreadable folders changes (`library.unreadableKey`).
    - Shown: `TrackSummary` has `folder_id` (so browse, search, playlists
      and favourites have it), recently played's tracks `folder_id`, and a
      top entry its tracks' `folder_ids`. The UI works out the unreadable
      folders from `library.folders` (`lib/folders.ts`, the same rule as
      Rust) and dims a track (`TrackText`), a history entry or a Home
      card whose folders are all unreadable, with the folder's short reason
      (`folderShort.*`, the owner's choice: no new wording) as its tooltip.
      Hearts and stars stay usable. The sidebar's map of short reasons
      moved to `lib/folders.ts` (`FOLDER_SHORT`).
    - Tests: radio, a smart playlist's play (the limit filled from readable
      folders, and conditions joined by OR still needing the folder),
      recently added, on this day and more in genre, the highlights, the
      history's folder ids (`cargo test`, 4 new), and `tests/folders.test.mjs`
      (4).

- **Exit:** the macOS app is ready for the first release in §8.3, with
  the P1 items of §4.7 and of H1–H22 done.

### Phase 8 — iOS and iPadOS
- Install Xcode, the iOS Rust targets, and set up the Apple Developer account
  (§3).
- Run `cargo tauri ios init`. In `build.rs`, cross-compile `anomp_core` for iOS
  with CMake (`CMAKE_SYSTEM_NAME=iOS`, keyed off `CARGO_CFG_TARGET_OS`) and
  link UIKit instead of AppKit.
- Repeat the Phase 0 spike on the iOS simulator: JUCE audio inside Tauri.
- Build FFmpeg for iOS device and simulator as dynamic frameworks packaged in
  an `.xcframework`, embedded in the app bundle (LGPL on iOS; see §4.1).
- Audio session: `AVAudioSession` category `.playback`, background audio mode,
  interruption and route-change handling.
- File access: import via document picker / Files app with security-scoped
  bookmarks.
- Metadata keys (`metadata/keys.rs`, 4.8): `apple-native-keyring-store`'s
  `protected` store, which iOS requires and which needs the provisioning
  profile's keychain entitlement.
- Layouts: iPhone (compact) and iPad (split view); touch targets and gestures.
- Lock screen and Control Center via the Phase 3 Now Playing code.
- Add an iOS simulator build to CI. Release steps (TestFlight, App Store) are
  in §8.4.
- **Exit:** the app plays a library on a physical iPhone and iPad, including in
  the background.

### Phase 9 — Linux
- Set up the Linux machine/VM (§3) and add an `ubuntu-latest` CI job (CMake +
  ctest, `cargo test`, frontend checks, `cargo tauri build`).
- `build.rs`: link `asound`, `pthread`, `dl`, plus freetype/fontconfig if
  JUCE GUI modules are still linked.
- Audio: JUCE uses ALSA (and optionally JACK). On PipeWire/PulseAudio desktops
  this goes through the ALSA compatibility layer; verify device selection,
  device switching and latency there.
- Formats: build FFmpeg for Linux x86_64 (and arm64 if targeted) with the same
  script and flags. Bundle it in the packages with `RPATH=$ORIGIN`-style lookup
  so the system FFmpeg is never picked up.
- Media keys / desktop integration: MPRIS over D-Bus (the `mpris-server` or
  `souvlaki` crate on the Rust side, or `sdbus` in the core).
- File access: plain paths; the library scanner must handle symlinks and
  case-sensitive names.
- Metadata keys (`metadata/keys.rs`, 4.8): the Secret Service over D-Bus
  (a `keyring-core` store), with a clear message when no keyring daemon
  runs.
- WebKitGTK rendering check: the visualizer (Phase 5) may need a canvas
  fallback if WebGL is slow or disabled.
- **Exit:** a release build plays a library on Ubuntu and Fedora, with media
  keys working. Packaging is in §8.5.

### Phase 10 — Windows
- Set up the Windows machine/VM (§3) and add a `windows-latest` CI job.
- CMake/MSVC: build the core with the dynamic CRT
  (`CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>DLL`) so it
  matches Rust's default and links cleanly. `build.rs` links the Windows
  system libs JUCE needs (`winmm`, `ole32`, `uuid`, `ws2_32`, `version`,
  `shlwapi`, etc.).
- Audio: JUCE WASAPI (shared mode by default; exclusive mode as an option in
  the admin screen). ASIO is out of scope (SDK licensing).
- Formats: build FFmpeg for Windows x64 (MSVC toolchain via MSYS2, or
  cross-compiled with MinGW from the Linux CI job). Ship the DLLs next to the
  executable.
- Media keys / lock-screen overlay: System Media Transport Controls (SMTC),
  via `souvlaki` or the `windows` crate.
- Paths: long paths (`\\?\` prefix), UTF-16 ↔ UTF-8 at the OS boundary,
  case-insensitive de-duplication in the library DB.
- Metadata keys (`metadata/keys.rs`, 4.8): the Windows Credential Manager
  (a `keyring-core` store).
- **Exit:** a release build plays a library on Windows 11, with media
  keys working. Installer and signing are in §8.6.

### Phase 11 — Bandcamp streaming
Goal: play the user's **Bandcamp collection** (what they have bought there)
next to the local library, streamed on demand. It uses the same browser,
search, queue, gapless hand-off and Now Playing as local files. Previewing
albums the user doesn't own is a later extension, and only if Bandcamp's
terms allow it.

Scheduling: this is a feature, not a platform. Engineering starts after
Phase 7, so it doesn't hold up the first macOS release. It is built
platform-neutral, so it works on every platform ported by then. Step 11.1 is
for the owner and can take months to answer, so start it early.

**Where things stand** (checked 2026-09-26):
- **Bandcamp has no API for fans.** `bandcamp.com/developer` lists only the
  Account, Sales Report and Merch Orders APIs, for labels and merchandise
  fulfilment partners. Access is granted on request and uses OAuth 2 client
  credentials.
- **Scraping is prohibited.** The Acceptable Use and Content Moderation
  Policy forbids scraping text, media or data by scripts, bots, scrapers or
  other automated means. Bandcamp's own pages and apps stream through
  private endpoints (signed, expiring 128 kbps MP3 URLs in the album page
  data), so calling those from ano-mp would be scraping. Some open-source
  players do that. We won't: the app is closed-source and commercial (§4.1),
  and a block or takedown would leave users with a broken feature.
- The Terms of Use (updated 2026-05-07) license content for personal,
  non-commercial use, and let fans preview it "by way of streaming" through
  the Service.
- **App Store Review Guideline 5.2.3** forbids saving or downloading media
  from third-party sources without their explicit authorization, and warns
  that streaming may break their terms. iOS therefore needs the same
  permission, in writing.

So the whole phase depends on Bandcamp's permission (11.1). The design
below assumes the permission covers a fan's collection and its streams.
Revisit it against whatever the agreement actually provides.

**Design:**
- **Rust fetches, the core decodes.** FFmpeg stays built without network
  protocols (§4.3), and the core never goes online. A `bandcamp` module in
  Rust downloads audio through the HTTP layer (a streaming GET on the same
  `Transport`, so it shares the offline backoff and the test fake) into an
  audio cache. The core opens the cached file like any other.
- **Download first, then play.** On open, the reader measures the length by
  decoding the last ~2 s and indexes MP3s (Phase 1), so it needs the whole
  file. A 5-minute track at 128 kbps is about 5 MB, roughly a second on a
  typical connection. The queue already arms the next track ahead of time,
  so the next streamed track downloads while the current one plays, and
  gapless hand-off works unchanged. Starting playback before the download
  finishes (progressive playback) is optional (11.6) and needs three things:
  a core `InputStream` over a growing file that waits for unread bytes; a
  reader mode that trusts the header length and skips the index scan until
  the file is complete; and a C API through which Rust reports bytes
  available, end of file or failure.
- **Library model: a Bandcamp account is a library source.** Proposal
  (settle it in 11.2): a new migration adds a `kind` column to `folders`
  (`local`, `bandcamp`). Tracks, albums, artists, the browse rules, FTS
  search and the queue then work unchanged. A Bandcamp track's `path` is its
  stable Bandcamp track id, never a URL. Everything that opens a track
  (`player_load`, `player_set_next`, the scanner, `art::lookup`) branches on
  the kind. A local track goes through `access::open_folder` as now. A
  Bandcamp track gets a fresh stream URL, is downloaded to the cache, and
  the engine receives the cached path.
- **Sync instead of scan.** For a Bandcamp source, "Refresh" lists the
  collection and upserts albums and tracks with Bandcamp's metadata: title,
  artist, album, track number, length, release date, tags as genres, and art.
  It stops at the first purchase already synced. Items no longer in the
  collection are removed, as a rescan removes deleted files.
- **Stream URLs are never stored.** They are signed and expire, so each one
  is fetched when the track is about to be downloaded.
- **Audio cache.** It is kept apart from the image cache but built the same
  way (`images.rs`): files named by SHA-256, written to `.part` and then
  renamed, least recently used evicted first, with its own budget
  (default 1 GB, set in the settings). The agreement decides whether cached
  audio may outlive the session and play offline. If it may not, the cache
  is cleared at exit and Bandcamp tracks show as unavailable while offline.
- **Sign-in** uses the flow the agreement provides (OAuth in the system
  browser redirecting back to the app, if offered). Tokens go in the OS
  keychain, never the settings JSON. Signing out deletes the tokens, the
  audio cache and the source's rows.
- **Art and details:** Bandcamp album art goes through the image cache and
  `anomp-art`, and details are labelled "Bandcamp", as Phase 4 labels its
  sources. Each album has a "View on Bandcamp" link, and any branding the
  agreement requires is shown. Later, Bandcamp could also be a Phase 4
  metadata source for local albums bought there, if the agreement allows
  it. Bandcamp downloads usually carry the album URL in their comment tag;
  check this.
- **Failures:** a track that can't be fetched (offline, session expired, no
  longer in the collection) fails to load with a reason. The queue skips it
  the way it skips a missing file.
- **Tests never touch the network**, as in Phase 4: the fake `Transport`
  serves recorded collection, track and audio responses (the audio is a
  committed fixture). An `#[ignore]`d live test reads a test account's token
  from an environment variable.

**Steps:**
- [ ] 11.1 **Permission (owner).** Contact Bandcamp through
  `bandcamp.com/developer`. Describe the app: a closed-source player that
  streams only the signed-in user's purchases, keeps audio only in a
  bounded cache, and links back to Bandcamp. Ask for API access to a fan's
  collection and streams, and for their rules on caching and offline play,
  on iOS and the App Store, on attribution, and on any commercial terms.
  Record the answer and its date here. No engineering starts without a yes.
- [ ] 11.2 Design against the real API: the library model (a `kind` on
  `folders`, or separate tables), the sign-in flow, the cache rules. Write
  the migration.
- [ ] 11.3 Sign-in, token storage and collection sync. Bandcamp albums show
  up in browse, search and the queue.
- [ ] 11.4 Playback: get the stream URL, download it to the audio cache,
  then `player_load`/`player_set_next`. Download the next track in the
  queue ahead of time. Failures and offline behaviour as designed.
- [ ] 11.5 UI: the Bandcamp account in the sidebar with its sync status,
  badges for tracks unavailable offline, "View on Bandcamp", and the
  account's settings in the Services panel (4.6) or admin screen (Phase 6).
- [ ] 11.6 Optional: progressive playback, if download-first start-up
  proves too slow.
- [ ] 11.7 **Fallback if 11.1 is refused:** Bandcamp purchases as local
  files. The user downloads their purchases (e.g. FLAC) from Bandcamp and
  puts them in a library folder. The app recognises Bandcamp downloads by
  their tags and links each album to its Bandcamp page. No Bandcamp
  endpoint is called.
- **Exit:** after signing in, the user's Bandcamp collection appears in the
  library and plays through the queue alongside local tracks. Hand-off
  between streamed tracks is gapless, and Now Playing and media keys work.
  Losing the network is handled as designed, and every test runs offline.
  With 11.7 instead: Bandcamp downloads in a library folder are recognised
  and linked to their Bandcamp pages.

## 6. Key risks

| Risk | Mitigation |
|---|---|
| JUCE message loop vs. Tauri's main-thread ownership | **Resolved on macOS (Phase 0):** JUCE's messages ride tao's main run loop; keep the engine on the main thread. Re-check on iOS (Phase 8), Linux and Windows |
| iOS problems found late (iOS work is deferred to Phase 8) | Keep the core platform-neutral and store bookmarks rather than paths from the start; if any late surprise would be costly, pull the Phase 8 simulator spike forward once Xcode is available |
| Licensing (JUCE commercial tier, FFmpeg LGPL, TagLib MPL) | JUCE license in place before any distribution (release gate, §8.1); FFmpeg always shipped as shared libs; App Store LGPL opinion before Phase 8 ships (§4.1) |
| AAC patent exposure from shipping FFmpeg's AAC decoder | Licensing opinion before release; CoreAudio fallback on Apple (§4.3) |
| FFmpeg build complexity across 4 OSes and several architectures | One script, pinned version, CI-cached artifacts; done per platform in its phase |
| FFmpeg and TagLib parser vulnerabilities (large attack surface; users' folders hold arbitrary files) | Minimal configure (only needed demuxers/decoders); fuzzing of the tag reader and decoder with sanitizers (H5, H6); a weekly CI job checks the pin against FFmpeg's security releases, and `bump-pin.py` makes the bump quick (§9) |
| MusicBrainz rate limits and bans | Strict limiter, caching, User-Agent with contact info |
| Decoder behaviour differing across platforms | Same FFmpeg version and flags everywhere; the Phase 1 format tests run on every CI OS |
| WebKitGTK (Linux) and WebView2 (Windows) behave differently from WKWebView | Keep the frontend to standard web APIs; run frontend smoke tests on each OS in CI |
| Bandcamp refuses, limits or withdraws access (no API for fans; scraping forbidden) | Phase 11 starts only with written permission; the Bandcamp code is one Rust module behind the library-source kind, so it can be dropped without touching local playback; fallback 11.7 (purchases as local files) |
| iOS sandbox limits on music files | Document picker import and bookmarks; the Apple Music library (DRM) is out of scope |
| Losing the user's own data (picks, and later playlists, favourites and history) to a corrupt DB, a bad migration, or moved files | Copies before migrations and integrity checks (H10); ids kept across moves and renames (F10); export and import (F20) |
| Library folders in iCloud Drive, on a NAS or on sleeping disks: whole-library downloads during a scan, UI freezes on open | Dataless files detected and not read (H12); files opened off the main thread (H11) |
| Failures in users' hands leave no trace (`eprintln!` output is lost in a bundle, and release builds abort on panic) | Log file, panic hook and "Copy diagnostics" (H9) before the first release |
| Visualizations triggering photosensitive reactions | Flash limits, `prefers-reduced-motion`, and a note on first use (F18) |

## 7. Proposed repo layout

```
ano-mp/
  CMakeLists.txt, CMakePresets.json
  core/            C++ JUCE engine (static lib) + C API
    include/anomp/ public C header
    src/
    tests/         Catch2
    tests/fixtures/ small audio files for tests
  scripts/         build-ffmpeg.sh and the Python tooling (§9)
    tests/         pytest for the scripts
  third_party/     built FFmpeg per platform (git-ignored, CI-cached)
  app/             Tauri 2 app
    src/           frontend (TS)
    src-tauri/     Rust backend, build.rs links core
  docs/
```

## 8. Release

Everything needed to get the app into users' hands, collected in one place.
Platform subsections apply once the matching phase in §5 is done.

### 8.1 Gates before any public release
These need answers first; most need the owner rather than engineering.
Each open item has a brief (options, what it blocks, what was checked) in
`docs/release-decisions.md` (2026-10-02).

- [ ] **JUCE license** (§4.1): commercial license in place (Starter tier to
      start; upgrade before revenue passes its cap).
- [ ] **FFmpeg LGPL compliance** (§4.1): notice, source offer and replaceable
      shared libraries in every package. Done for the macOS direct
      download (2026-10-02): `THIRD_PARTY_NOTICES` has the LGPL, the
      configure flags and the source tarball, and the dylibs are separate
      in `Contents/Frameworks` (`check-bundle.py`). For the App Store, confirm this is
      acceptable before the first iOS or Mac App Store submission.
- [ ] **AAC patents** (§4.3): licensing opinion obtained for FFmpeg's AAC
      decoder (the chosen path), or fall back to routing AAC to
      CoreAudio (Apple) / Media Foundation (Windows) / disabled (Linux).
- [ ] **Name and identity:** the candidate product name is **AnoTracks**
      (runner-up: Anotone). Before committing:
  - [ ] Search the USPTO and EUIPO trademark registers for "ANOTRACKS" and
        "ANOTRAKS", at least in class 9 (software). "Ano(t)raks" is an
        existing Japanese indie music label that uses the handle `anotraks`.
  - [ ] Check that the domains (e.g. anotracks.app / .com) and the App Store
        name are available.
  - [ ] Then pick the bundle identifier (e.g. `com.<owner>.anotracks`, same on
        every platform) and a publisher name. Neither can change after the
        first store release without losing users' installs.
- [ ] **Distribution channels:**
  - macOS: notarized direct download (DMG) and/or Mac App Store. The store
    requires the App Sandbox and does not allow the built-in updater.
    Proposal: direct download first.
  - Linux: `.deb` and AppImage first (Tauri builds both, plus `.rpm`).
    Flatpak/Flathub reaches more distros but adds sandbox work; later.
  - Windows: NSIS installer (proposal) and/or MSI. Microsoft Store and winget
    optional, later.
- [ ] **Privacy policy and support page:** required by the App Store, and the
      app contacts MusicBrainz and Cover Art Archive. Host a short policy and
      support URL.
- [ ] **Metadata service terms** (Phase 4): re-read the terms of every
      source that is on by default or selectable, for commercial use,
      attribution, caching and image display. Decide on a MetaBrainz
      supporter plan. Set the `User-Agent` contact to a real address.
  - [ ] **Discogs** (4.8): if the app is sold, get Discogs' written
        permission (their terms need it for "charging a fee to use or
        access any part of Your application that integrates with Our
        API"), or leave Discogs out of that build. Put the non-affiliation
        notice (`discogs::NOTICE`) in the About screen or the app's terms
        too, and name Discogs in the privacy policy: album titles and
        artists are searched there with the user's own token.
- [ ] **Bandcamp agreement** (Phase 11, if it ships): written permission
      covering every platform it ships on, including the App Store (Review
      Guideline 5.2.3), with its caching and attribution rules followed. The
      privacy policy covers the Bandcamp sign-in.
- [ ] **Crash reporting:** decide none vs. opt-in (e.g. Sentry). Anything
      opt-in must appear in the privacy policy.
- [ ] **ListenBrainz** (O8): read its terms for a commercial client, and
      name it in the privacy policy (listens are sent with the user's
      token when they turn it on).
- [ ] **LAN remote security review** (O14): review `remote/` (address
      checks, pairing limits, token hashing, request bounds, what it
      serves) before a release ships it, and describe it in the privacy
      policy.

### 8.2 Cross-platform release setup
Build once (after Phase 7), reused for every platform. **Done 2026-10-02
for macOS, except the signing secrets, the updater and the icon set,
which wait on §8.1** (Phase 7, Step 7): the version, the notices and the
release workflow are in place, unsigned until the secrets exist.

- **Single version number:** CMake `project(VERSION)` is the source of truth;
  generate `anomp_version()` from it instead of the hard-coded string, and
  keep `Cargo.toml`, `tauri.conf.json` and `package.json` in sync with
  `scripts/version.py`, checked in CI (§9.2 M6). MusicBrainz `User-Agent` includes this version and a
  contact address. **Done**, but the contact is a placeholder until the
  owner gives one (§8.1).
- **Release workflow** (GitHub Actions, triggered by a version tag): build and
  test on every platform, sign, package, attach the artifacts plus SHA-256
  checksums to a draft GitHub Release, and generate release notes from
  `CHANGELOG.md`. **Done for macOS** (`.github/workflows/release.yml`):
  it calls `ci.yml`, builds the universal app and DMG, and runs
  `release.py`; signing and notarization steps are skipped, with a
  warning, until the secrets exist. Linux and Windows join with their
  phases.
- **Signing secrets** (Apple certificates and notarization key, Windows
  signing, updater key) stored as CI secrets, with an offline backup. Losing
  the updater key strands existing installs. **Waits on the owner**:
  `release.yml`'s header lists the six Apple secrets it reads.
- **Auto-updates** for direct-download builds: Tauri updater plugin with a
  signed update manifest hosted with the releases. Store builds (App Store,
  Microsoft Store, Flathub) update through the store instead.
  **Waits on the distribution decision (§8.1)**; the plugin is a new crate
  and npm package, to be agreed with the owner first.
- **Third-party notices:** a generated `THIRD_PARTY_NOTICES` file shipped in
  every package and shown in the app's About screen. It covers JUCE; FFmpeg
  (LGPL text, exact version and configure flags, link to the matching
  source tarball); TagLib; Rust crates (`cargo-about`); and npm packages,
  generated by `scripts/make-notices.py` (§9.2 M6). CI fails if a
  dependency's license is unknown. **Done**: committed at the repo root,
  checked by `check-all.py`, in the bundle's Resources and in Settings ›
  About with Discogs' notice. The crates come from `cargo metadata`, not
  `cargo-about` (no new tool), and Signalsmith Stretch, the JUCE-vendored
  zlib and TagLib's utfcpp are covered too.
- **App icon and metadata:** icon set for every platform (`cargo tauri icon`),
  app description, screenshots. Not done: it needs the name (§8.1).

### 8.3 macOS
- Apple Developer Program membership and a **Developer ID Application**
  certificate (direct download); Mac App Store needs its own certificates.
- Universal binary (arm64 + x86_64), including the core and FFmpeg.
- Bundle the FFmpeg dylibs in `Contents/Frameworks`, set install names to
  `@rpath`, and sign all nested code with the hardened runtime. Done
  (Phase 2, and Phase 7 Step 7): the dylibs are embedded and signed;
  `scripts/build-app.py` turns the hardened runtime on exactly when a
  signing identity is given, and `tauri.conf.json` keeps it off for ad-hoc
  local builds. `scripts/check-bundle.py` checks a built bundle's slices,
  install names, rpath, nested signatures and entitlements.
- Entitlements: network client; for the sandbox, user-selected access and
  app-scoped bookmarks. Reviewed 2026-10-02 against the code: the sandbox,
  user-selected **read-write** (exporting M3U8 playlists and the user's
  data, F1 and F20, writes where the user picks), app-scoped bookmarks,
  network client, and network server for the LAN remote (O14), whose
  local-network prompt is `Info.plist`'s `NSLocalNetworkUsageDescription`,
  not an entitlement. No audio input is opened, so no microphone
  entitlement; the hardened runtime needs no exceptions.
- Notarize with `notarytool`, staple the ticket, ship as a DMG.
  `scripts/notarize.py` does it with an App Store Connect API key, and
  prints the plan without one (Step 7).
- Verify on a clean Mac: Gatekeeper accepts it (`spctl --assess`), first
  launch, library import, update from the previous version. The checklist
  is `docs/release-smoke-test.md` (Step 8), the owner's to run on each
  release's DMG; with §8.7 step 6 in it.

### 8.4 iOS and iPadOS
- App Store Connect record, bundle ID, distribution certificate and
  provisioning profiles; background audio capability enabled.
- FFmpeg `.xcframework` embedded and signed inside the app.
- **TestFlight** beta on iPhone and iPad before submission.
- App Store submission items:
  - privacy "nutrition label" (likely "data not collected"; confirm against
    the crash-reporting decision in §8.1);
  - privacy policy and support URLs;
  - export compliance: HTTPS only, so set `ITSAppUsesNonExemptEncryption` to
    `NO`;
  - screenshots for the required iPhone and iPad sizes, age rating,
    description and keywords;
  - review notes explaining background audio and how to add music (reviewers
    start with an empty library); if Bandcamp streaming ships, a demo
    Bandcamp account and Bandcamp's authorization (Guideline 5.2.3).

### 8.5 Linux
- `.deb` and AppImage via the Tauri bundler, with FFmpeg bundled and found via
  `$ORIGIN` RPATH (never the system copy).
- `.desktop` file, icons at the standard sizes, and AppStream metadata
  (needed later for Flathub and software centres).
- Test installs on clean Ubuntu 24.04 and Fedora 40+ VMs: launch, audio
  output under PipeWire, media keys, uninstall.
- Optional: GPG-sign artifacts, host an apt repository, submit to Flathub.
- AppImage builds can use the Tauri updater; `.deb` users update through the
  download page or the apt repository.

### 8.6 Windows
- **Code signing:** Authenticode certificate (OV/EV, or Azure Trusted
  Signing). Sign the executable, the FFmpeg DLLs and the installer; unsigned
  builds trigger SmartScreen warnings.
- NSIS installer (and/or MSI) via the Tauri bundler, with the FFmpeg DLLs next
  to the executable.
- WebView2: use the download bootstrapper (small installer) or the offline
  installer for machines without internet; Windows 11 already has it.
- Test on clean Windows 11 VMs: install, first launch, audio, media
  keys, upgrade, uninstall (library DB and settings handling).
- Optional: winget manifest, Microsoft Store listing.

### 8.7 Checklist for every release
1. All §8.1 gates still hold (new dependencies? new data sent anywhere?).
2. Bump the version (`scripts/version.py X.Y.Z`); rename `CHANGELOG.md`'s
   `[Unreleased]` section to the version and date.
3. CI green on every platform, including the format decode tests.
4. Regenerate third-party notices (`scripts/make-notices.py`); check the
   FFmpeg source link matches the pinned version. Run `scripts/bench.py`
   (H18's budgets; against the baseline on its machine)
   and `scripts/check-signing.py`, and do the §9.1 rows due "each release".
   A trial run of the release workflow (by hand, on the branch) shows the
   build passes `check-bundle.py` before tagging.
5. Tag `vX.Y.Z`; the release workflow builds, signs, notarizes and
   packages, and leaves a draft release to publish.
6. Smoke test each artifact on a clean machine: install/upgrade, play MP3,
   FLAC and AAC, seek, gapless album, media keys, MusicBrainz lookup
   (`docs/release-smoke-test.md` on macOS).
7. Publish the GitHub Release and the updater manifest; submit store builds
   (TestFlight → App Store review).
8. After release: watch crash reports (if enabled) and issue tracker; keep
   the previous version's artifacts available for rollback.

## 9. Maintenance

Work that recurs for as long as the app is developed, and the scripts that
keep it cheap and hard to get wrong. Scripts are Python, like the format
scripts, so they run the same on every OS (`py` on Windows) and in CI.
`build-ffmpeg.sh` stays in bash, since FFmpeg's `configure` needs a POSIX
shell everywhere (MSYS2 on Windows).

### 9.1 Recurring work

| Item | When | What it involves | Script (§9.2) |
|---|---|---|---|
| Native dependency pins: JUCE and Catch2 (commit archive + SHA-256, `CMakeLists.txt`), TagLib (tarball + SHA-256, `cmake/TagLib.cmake`), FFmpeg (version + SHA-256, checked against its GPG signature, in `build-ffmpeg.sh`) | Monthly check; at once for a security release | Find the new release, download it, hash it, rewrite the pin, rebuild, run every test suite. For FFmpeg, also re-check the configure output and `BUILD_INFO` | `check-pins.py`, `bump-pin.py` |
| FFmpeg dylib names in `tauri.conf.json` (`bundle.macOS.frameworks`, later the Linux and Windows lists) | Each FFmpeg bump that changes a major version | The names must match what the build produced, or the bundle step fails | `sync-ffmpeg-frameworks.py` |
| Formatter pins: clang-format (`format-cpp.py`), ruff (`format-python.py`) | A few times a year | Bump, reformat the tree in one separate commit | `check-pins.py`, `bump-pin.py` |
| Rust crates and npm packages | Monthly | `cargo update` / `npm update`; read the changelogs of Tauri, Svelte, rusqlite (its bundled SQLite version), ureq/rustls; run all tests | `check-pins.py` (reports; updating stays manual) |
| Security advisories | Weekly (scheduled CI job); FFmpeg security releases as announced | `cargo deny check` (RustSec advisories, licences; H7), `npm audit`, the pinned FFmpeg against ffmpeg.org's security page, TagLib and JUCE release notes | `audit-deps.py` |
| Toolchains: Rust, Node (Node 26 becomes LTS in October 2026, §3), CMake, Ninja, nasm, uv; Xcode and SDKs from Phase 8 | Each stable/LTS release; Xcode each year | Update §3's versions, check the build still passes, raise documented minimums | `doctor.py` |
| Recorded service responses (`app/src-tauri/src/metadata/fixtures/`) | Quarterly, whenever a live test fails, and for each new source (4.8) | Re-fetch the same URLs with the app's `User-Agent` at the services' rate limits, trim them the same way, and diff with the committed copies: a changed field means a parser needs work | `record-fixtures.py` |
| Audio fixtures (`core/tests/fixtures/`) | When a format is added or the test signal changes | Regenerate, update the lengths and lags in the tests' fixture table | `make-test-fixtures.py` (extend) |
| Library DB migrations | Every schema change | Numbered with no gaps, listed in `MIGRATIONS`, shipped ones unchanged, FTS triggers updated when an indexed column changes | `check-migrations.py` |
| C API surface | Every change to `anomp.h` | Each function has an FFI declaration and a safe wrapper in `anomp.rs` | `check-c-api.py` |
| Core source lists | Every new core file | Listed in `core/CMakeLists.txt` or `core/tests/CMakeLists.txt` (no globbing) | `check-sources.py` |
| Docs drift: `PLAN.md` status and §2, `CLAUDE.md`, `README.md` | Each finished step | Test counts match the suites; every repo path the docs mention exists | `check-docs.py` |
| Performance baselines (the ignored 50,000-track benchmarks) | Each release; after scanner, browse or DB changes | Run them in release mode, compare with committed numbers, flag regressions | `bench.py` |
| Fuzzing (H5) | Weekly (scheduled job) | Look at new crashes; fix them, add each crashing input as a regression fixture, and keep the corpus | none (CI job) |
| Formatter and linter pins: ESLint, Prettier, clippy's lints (H8, H13) | With the monthly npm and Rust updates | Bump, fix or allow the new warnings, and reformat in one separate commit | `check-pins.py` |
| Library DB copies (H10) | Each migration | Confirm the copy is written before the migration, and that old copies are pruned | none (tests) |
| Version number | Each release | One version everywhere (§8.2) | `version.py` |
| Third-party notices | Each release and each dependency change | Regenerate and check every licence is known (§8.2) | `make-notices.py` |
| Release artifacts | Each release | SHA-256 checksums, updater manifest, release notes from `CHANGELOG.md` (§8.2) | `release.py` |
| Signing material: Apple certificates (distribution and provisioning profiles yearly, Developer ID every five years), notarization key, Windows certificate, updater key | Monthly check once §8.3 is set up | Renew before expiry; keep the offline backups current | `check-signing.py` |
| Service terms and limits (MusicBrainz, Cover Art Archive, Wikimedia, later sources), the `User-Agent` contact, the MetaBrainz supporter plan | Yearly and before each release (§8.1) | Read the terms; update the sources table in Phase 4 | none (manual) |
| JUCE licence tier against revenue; App Store rules (SDK minimums, privacy manifests); minimum OS targets (§4.5) | Yearly (after WWDC for Apple) | Owner decisions; record them in §4 | none (manual) |

### 9.2 Scripts to develop and test

Conventions, following the existing scripts:
- One file per task in `scripts/`, named with hyphens, standard library
  only at run time (no virtualenv needed), a module docstring with usage,
  and `argparse`. A script that rewrites files has a `--check` mode that
  changes nothing and exits non-zero on a difference, for CI and a
  pre-commit hook.
- Logic in plain functions (parse a pin, rewrite a file, compare lists);
  network, subprocess and git calls in thin functions the tests replace.
  Tests never touch the network, as in the Rust tests.
- Tests with pytest in `scripts/tests/test_<script>.py`, with small input
  files in `scripts/tests/fixtures/` (excerpts of `CMakeLists.txt`,
  `anomp.h`, `tauri.conf.json` and so on, including broken ones).
  Scripts that check the repo also get one test against the real tree,
  which must pass.

Steps:
- [x] M1 Test harness (2026-10-02). `scripts/test-python.py` runs a pinned pytest
  through uvx (`uvx --from pytest==<version> pytest scripts/tests`), and
  `format-python.py` gains `ruff check` (lint) next to `ruff format`. Add
  tests for the existing scripts' pure logic (`format-cpp.py`'s file
  selection and batching; `make-test-fixtures.py`'s signal matching
  `TestSignal.h`'s constants).
- [ ] M2 Repo checks, each a read-only script that lists every problem it
  finds. `check-c-api.py`, `check-sources.py` and `check-migrations.py`
  are done (2026-10-02); the FTS warning covers migrations after 009,
  and a `-- fts:` comment acknowledges one that needs no trigger change.
  - `check-c-api.py`: parses the functions declared in `anomp.h` and the
    `extern "C"` block in `anomp.rs`; fails on any function missing from
    either side, and on declarations whose parameter counts differ.
  - `check-sources.py`: every `.cpp`/`.mm` under `core/src` and
    `core/tests` is listed in its `CMakeLists.txt`, and every listed file
    exists.
  - `check-migrations.py`: files in `library/migrations/` numbered from
    001 with no gaps, each listed in `MIGRATIONS` in order, and none that
    existed at the latest release tag changed since (`git diff` against
    the tag; before the first release there are no tags and this part is
    skipped). Warns when a migration alters `tracks`, `artists` or
    `albums` without touching the FTS triggers.
  - `sync-ffmpeg-frameworks.py`: rewrites `bundle.macOS.frameworks` from
    the dylibs in `third_party/ffmpeg/macos-universal/lib`; `--check`
    compares only.
  - `check-docs.py`: every backquoted repo path in `PLAN.md`, `CLAUDE.md`
    and `README.md` exists; with `--counts`, compares §2's test counts
    with `ctest --preset debug -N` and `cargo test -- --list` (needs
    builds, so not in the quick check). **Done 2026-10-03** (H20's entry),
    `docs/` and relative links included; `npm test`'s and the scripts'
    counts too.
- [ ] M3 Fixture tools (with 4.8, which adds sources and their fixtures):
  - `record-fixtures.py`: a manifest next to the metadata fixtures lists
    each file's URL and trim rule (which JSON fields to keep, how many
    list items). It fetches at one request a second with the app's
    `User-Agent`, trims, writes, and with `--check` shows the diff
    instead. Re-record the existing fixtures with it once and confirm
    `cargo test` still passes. Tests: trimming over saved raw responses,
    the rate limiting with a fake clock, and the manifest covering every
    committed file.
  - `make-test-fixtures.py`: pass fixed stream serials to `oggenc`
    (`--serial`), so regenerating changes no file unless the signal or
    encoders changed (the Vorbis fixtures change once when this lands);
    add `--only NAME`; print each fixture's length for the tests' table.
- [ ] M4 CI entry point (with Phase 7's CI). `check-all.py` is done
  (2026-10-02), with `.github/workflows/ci.yml` running it, and
  `scripts/hooks/pre-commit` runs its `--quick` mode (H7):
  - `check-all.py`: runs every formatter in `--check` mode, the M2
    checks and the Python tests, then (unless `--quick`) the C++, Rust
    and frontend builds and tests. CI calls this and nothing else, so a
    local run matches CI; `--quick` is the pre-commit hook.
  - `doctor.py`: checks the tools in §3 are installed at the minimum
    versions, that `third_party/ffmpeg/<platform>/BUILD_INFO` matches
    `build-ffmpeg.sh`, and on macOS that the Command Line Tools are
    selected. Prints what to install. Written for every OS from the
    start, since Phases 8–10 need it most.
  - `bench.py`: runs the ignored benchmarks in release mode, parses their
    timings and compares them with a committed baseline, failing past a
    set margin; `--update` rewrites the baseline. **Done 2026-10-03**
    (H18's entry): the Rust benchmarks and the core's (`bench` preset),
    H18's budgets, 13 tests.
- [ ] M5 Dependency tools (with Phase 7):
  - `check-pins.py`: reads every pin (JUCE, Catch2, TagLib, FFmpeg,
    clang-format, ruff) from its file and asks upstream for the latest
    release (GitHub releases, ffmpeg.org, PyPI); also summarizes
    `cargo update --dry-run` and `npm outdated`. Report only.
  - `bump-pin.py NAME VERSION`: downloads the release, computes its
    SHA-256 (and for FFmpeg verifies the GPG signature against the key
    recorded in `build-ffmpeg.sh`), rewrites the pin in place and prints
    the rebuild and test commands. Tests rewrite copies of the real files.
  - `audit-deps.py`: runs `cargo deny check` (H7) and `npm audit`, and
    checks the FFmpeg pin against ffmpeg.org's security page; a scheduled
    weekly CI job runs it with `check-pins.py`.
- [ ] M6 Release tools (with §8.2). Done 2026-10-02 except `release.py`'s
  updater manifest (after the updater, §8.2): `version.py`,
  `make-notices.py`, `release.py` and `check-signing.py`, plus
  `build-app.py`, `check-bundle.py` and `notarize.py` (§8.3), with 39
  tests in `test_version.py`, `test_make_notices.py` and
  `test_release_tools.py`.
  - `version.py`: `--check` fails unless `CMakeLists.txt`, `Cargo.toml`,
    `tauri.conf.json` and `package.json` agree (`anomp_version()` is
    generated from CMake by then); `version.py 0.2.0` sets them all.
  - `make-notices.py`: builds `THIRD_PARTY_NOTICES` from JUCE, FFmpeg
    (licence, version and configure flags from `BUILD_INFO`, source
    link), TagLib, `cargo-about` output and the npm licences; fails on an
    unknown licence.
  - `release.py`: checksums for the built artifacts, the Tauri updater
    manifest, and release notes cut from `CHANGELOG.md`.
  - `check-signing.py` (with §8.3): lists the signing certificates'
    expiry dates from the keychain and warns within 60 days.
- **Exit:** `check-all.py` runs in CI (by hand and on each version tag
  since 2026-10-02; the pre-commit hook runs `--quick`), the scheduled job
  reports outdated pins and advisories, and each §9.1 row either has its
  script or is marked manual.
