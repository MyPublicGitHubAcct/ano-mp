# ano-mp — Implementation Plan

Status as of 2026-10-03: Phases 0–6c are built on macOS. Phases 0–3 are
complete; the exit checks of Phases 4, 5, 6, 6b and 6c wait to be done in
the app (each phase says what). Phase 7 (hardening) is under way: every
step of its "Order of work" that needs no owner decision is done, and the
signed release waits on the owner's §8.1 decisions
(`docs/release-decisions.md`). Phase 7b (themes, effects, visualizations
and recommendations; P3, added 2026-10-03) follows Phase 7, before the
ports. Finished work's design notes and records are in `docs/design/`,
linked from each phase.


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

Where each part lives. Each phase's design doc in `docs/design/` says what it
holds and how it was built.

- **Core** (`core/`): the C API (`core/include/anomp/anomp.h`); decoding
  (`FFmpegAudioFormat`, FFmpeg built by `scripts/build-ffmpeg.sh`), tags
  (`TagReader`, TagLib), playback (`PlayerEngine`, `AudioEngine`: gapless,
  async loads, crossfade, `Equaliser`, `Crossfeed`), visualizer analysis
  (`SignalTap`, `SpectrumAnalyser`, `AnalysisThread`), `FileAnalyser`, and
  platform code (`MediaControls`, `FolderAccess`, `FileStatus`,
  `VolumeWatcher`, `DockMenu`). Fuzz targets in `core/fuzz/`.
- **Rust** (`app/src-tauri/src/`): `anomp.rs` (the C API's wrappers),
  `audio.rs` (the main-thread engine host), `library/` (DB, migrations,
  scanner, browse, search, art and thumbnails, playlists, features),
  `queue/`, `media.rs`, `metadata/` (online sources and their worker),
  `history/`, `remote/`, `shell/` (menus, mini player, opened files),
  `settings.rs`, `visualizer.rs`, `logging.rs`, `diagnostics.rs`,
  `bindings.rs`, `self_test.rs`, `dev.rs` (debug builds only).
- **Frontend** (`app/src/`): `routes/`, `lib/components/`, `lib/state/`,
  `lib/visualizer/`, `lib/i18n/`, generated types in `lib/generated/`.
- **Tooling** (`scripts/`, `.github/`): `check-all.py` and the repo checks,
  formatters, release scripts, `bench.py`, the pre-commit hook, CI, release
  and fuzz workflows.
- **Docs** (`docs/`): checklists, release decisions, the smoke test, and
  finished work's design in `docs/design/`.

Test suites (`check-docs.py --counts` compares these with the suites):

| Suite | Location |
|---|---|
| 116 passing Catch2 tests, also clean under ASan, UBSan and TSan | `core/tests` |
| 439 passing `cargo test` tests, plus 7 ignored benchmarks (50,000 tracks) and 6 ignored live tests (one per online source) | `app/src-tauri/src` |
| 30 frontend tests (`npm test`, pure modules) | `app/tests/` |
| the scripts' 205 pytest tests (`test-python.py`) | `scripts/tests/` |

## 3. Prerequisites

Needed for Phases 0–7 (macOS only; the Command Line Tools are enough). All
installed on the dev machine as of 2026-09-25 (versions noted).
`scripts/doctor.py` checks them against the repo's pins and minimums:

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

1. **Licenses: JUCE commercial; ano-mp's own code MIT (decided; MIT since
   2026-10-03, closed source before).** JUCE 8+ is AGPLv3 or commercial. We
   use the commercial license, starting on the free "Starter" tier and moving
   to a paid tier before revenue passes its cap (check the current JUCE 9
   tiers then). This keeps the released app off the AGPL and the App Store
   open to us. The code written here is MIT (`LICENSE`); the components it is
   built on keep their own licences (`THIRD_PARTY_NOTICES`), so anyone else
   distributing a build needs their own JUCE licence or must ship it under
   the AGPLv3. FFmpeg is used under the LGPL (see #3), which the app
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
8. **Personalisation, effects and discovery: added 2026-10-03 at P3.**
   X1–X5 (Phase 7b): themes, real-time effects, five uncommon
   visualizations, and recommendations from inside and outside the
   library. Low priority: after Phase 7's exit and the first release, but
   before the cross-platform work (Phases 8–10), so the ports carry them.
   #6's rules apply; X2 and X5 are off by default (they change what is
   heard and go online).

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

Each item's proposal and its **Decision** line (how it was built, and
where that differs from the proposal) are in
[docs/design/phase-6b-optional-features.md](docs/design/phase-6b-optional-features.md).

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

Each item's proposal and its **Decision** line are in
[docs/design/phase-6c-expected-features.md](docs/design/phase-6c-expected-features.md).

## 5. Phased plan

Each phase ends with a demonstrable result and green tests. Platform order:
macOS (Phases 0–7), then iOS/iPadOS (Phase 8), then Linux (Phase 9), then
Windows (Phase 10). Phase 7b (themes, effects, visualizations and
recommendations, P3) comes after Phase 7 and before the ports, so those
features are settled on macOS first. Xcode is not needed until Phase 8.
Phase 11 (Bandcamp streaming) is a feature, not a platform, and depends
on Bandcamp's permission. Its engineering starts after Phase 7. The phases
cover making the app work on each platform; packaging, signing and
shipping it are all in §8.

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

The sources considered and which ship, the design decisions, the steps as built, tests and known limits: [docs/design/phase-4-online-metadata.md](docs/design/phase-4-online-metadata.md).

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

**Order of work** (set 2026-10-02). Each step's full record (what was
done, the suites after it, the owner's answers that shaped it, what was
found) is in [docs/design/phase-7-hardening.md](docs/design/phase-7-hardening.md).

1. A repeatable baseline: `check-all.py` and the CI job. **Done
   2026-10-02.**
2. Quick P1 security and lint fixes: H1, H2, H3's opener, H6, H7, H8,
   H13's P1 part. **Done 2026-10-02.**
3. P1 items that protect user data: H10, H22, H9. **Done 2026-10-02.**
4. Exit checks in a sandboxed bundle (`docs/step4-checklist.md`). **Probe
   checks done 2026-10-02; the owner's checks wait.**
5. The larger core P1 items: H5, H11, H12. **Done 2026-10-02**; the
   owner's checks are § 5 of the checklist.
6. Owner decisions (§8.1), alongside the rest: briefs in
   `docs/release-decisions.md` (2026-10-02). None decided yet.
7. Release setup without a certificate (§8.2, §8.3's parts that need
   none; M6). **Done 2026-10-02.**
8. The signed release (§8.3 with a Developer ID, §8.7), as far as the
   owner's decisions allow. **Begun 2026-10-02**: the smoke test
   (`docs/release-smoke-test.md`) and H14; Part 5 (2026-10-03): CI fixed,
   H15–H18 and H20; Part 6 (2026-10-03): H21, `doctor.py`, M5's
   dependency tools and the weekly audit, M3's fixture tools. Its
   signing parts wait on Step 6.

Waiting on the owner, in the order the owner takes them (set 2026-10-03):

1. ~~the CI log of run #9 (and #6)~~ fixed 2026-10-03 (Step 8's record);
   the fix is pushed (asked 2026-10-03), and CI on `main` waits for the
   owner to run it, now with Part 6 (the audit workflow's first run
   too);
2. the distribution channel and where releases are hosted (Part 1,
   the updater, if a direct download). The owner made the
   repository public on 2026-10-03, so its GitHub Releases can host
   downloads and Actions minutes are free. Decided 2026-10-03: a
   direct download from GitHub Releases (§8.1), with an update check
   rather than the updater (§8.2); iPhone and iPad wait for the last
   steps;
3. the Developer Program and the Developer ID certificate (Part 2),
   then the App Store Connect API key and the six secrets (Part 3);
4. the JUCE licence, the AAC opinion, the privacy policy and support
   URL, the `User-Agent` contact and MetaBrainz plan, crash
   reporting;
5. the checklist's owner entries (`docs/step4-checklist.md`) and,
   per release, `docs/release-smoke-test.md`;
6. the name, bundle identifier and publisher (Part 0's rename, then
   the icon set from the artwork). Still blocks the first signed
   release (the identifier is in the signature and the container)
   and an App Store record. Asked 2026-10-03: the trademark,
   domain and App Store checks not done, the account type
   undecided, no artwork. Checked then: no DNS records for
   `anotracks`, `anotone` or `anotraks` under .com, .app or .dev;
   WHOIS "No match" for the three .com names; .app and .dev have no
   WHOIS server any more (IANA lists none; Google's registry
   answers RDAP only), so a registrar's search settles those.

- Allowed, not fixed (each in its item's record): 44.1 kHz files resampled at
  30× real time (H18; the budget is 20×); browsing by year or genre at
  56–73 ms (H18; 100 ms budget); restore's 290 ms of track details
  (H16); `queue/model.rs` not split (H19), having changed little.
- Next: the owner's items above, in order; then Parts 0–3 of the signed
  release as decisions arrive. Engineering left that needs no
  decision: H19 as modules are touched, M2's
  `sync-ffmpeg-frameworks.py`, H4 (before Phase 9), H13's P2 tests and
  H8's edition move.

**Hardening items** (from the 2026-09-27 review; priorities as in
§4.7). CI runs each check through `check-all.py` (M4), so a local run
matches it.

| # | Item | Area | Size | Priority | Status |
|---|---|---|---|---|---|
| H1 | Content Security Policy | security | S | P1 | Done 2026-10-02 |
| H2 | Keep developer commands and `/dev` out of release builds | security | S | P1 | Done 2026-10-02 |
| H3 | Narrow the URL opener and per-window command permissions | security | S | P1 | Done (permissions 2026-09-27, opener 2026-10-02) |
| H4 | Contain folder pictures on unsandboxed platforms | security | S | P2, before Phase 9 | Open (below) |
| H5 | Fuzz the tag reader and decoder | security, robustness | M | P1 targets and CI run, P2 long runs | Done 2026-10-02; the weekly long runs' results to come |
| H6 | Sanitizer presets for the core | robustness | S | P1 | Done 2026-10-02 |
| H7 | Supply chain: exact pins, `cargo deny`, update bot, secret scanning | security, maintenance | S | P1 | Done 2026-10-02; GitHub secret scanning is the owner's setting |
| H8 | Rust and C++ lint gates, pinned toolchains | maintenance | S | P1 | Done 2026-10-02 (`doctor.py` 2026-10-03) but the edition move (below) |
| H9 | Logs, panic capture and "Copy diagnostics" | robustness, support | M | P1 | Done 2026-10-02 |
| H10 | Library DB safety: backups before migrations, checks, pruning | robustness | S–M | P1 | Done 2026-10-02 |
| H11 | Open files off the main thread | robustness | M | P1 | Done 2026-10-02 |
| H12 | Cloud, network and removable folders | robustness | M | P1 | Done 2026-10-02; SMB and real placeholders are owner checks |
| H13 | Frontend lint, format and tests | maintenance | M | P1 lint, P2 tests | P1 done 2026-10-02; P2 open (below) |
| H14 | A self-test of the sandboxed bundle in CI | robustness | M | P2 | Done 2026-10-02 |
| H15 | Typed IPC end to end | maintenance | S–M | P2 | Done 2026-10-03 |
| H16 | Queue storage and updates that scale | efficiency | M | P2 | Done 2026-10-03 |
| H17 | Cover thumbnails and an on-disk art cache | efficiency | M | P2 | Done 2026-10-03 |
| H18 | Performance budgets | efficiency | S | P2 | Done 2026-10-03; three budgets are owner checks |
| H19 | Split the largest modules | maintenance | S each | P3, as touched | Open, as touched (below) |
| H20 | Move design records out of `PLAN.md` | maintenance | S | P2 | Done 2026-10-03 |
| H21 | C++ static analysis | maintenance | S | P3 | Done 2026-10-03 |
| H22 | Missing folders at launch | robustness | M | P1 | Done 2026-10-02 (H22a and H22b) |

Each finished item's proposal and record (what was built, its tests,
what was found, what is left) is in
[docs/design/phase-7-hardening.md](docs/design/phase-7-hardening.md).
The open items and parts:

- **H4 Folder pictures on unsandboxed platforms.**
  `folder_art::is_relative_path` rejects `..` lexically, but a symlink
  inside a library folder can still point outside it. The macOS sandbox
  blocks that, but Linux and Windows have no sandbox. Canonicalise the
  picture's path and require it under the folder's root, with a test
  using a symlink.
- **H8's remainder.** Move to Rust edition 2024 in one separate commit.
- **H13's P2 part.**
  - P2: Vitest for the rune modules (`state/*.svelte.ts`), which plain
    `node --test` can't compile. IPC is faked with
    `@tauri-apps/api/mocks` (`mockIPC`), using payloads recorded from
    the Rust tests.
  - P2: Playwright smoke tests against `vite dev` with the same mocks,
    run in WebKit: browse, search, queue edits, settings. This is §6's
    mitigation for WebKitGTK and WebView2 differences, and they run on
    each OS's CI job from Phases 9–10.
- **H19 Split the largest modules**, when they are next changed, with no
  change in behaviour:
  - `metadata/jobs.rs` (2,347 lines), by job kind;
  - `library/browse.rs`;
  - `queue/model.rs`;
  - `anomp.rs`, one file per area of the C API.

- **Exit:** the macOS app is ready for the first release in §8.3, with
  the P1 items of §4.7 and of H1–H22 done.

### Phase 7b — Personalisation, effects and discovery (macOS)
Added 2026-10-03 at the owner's request. **Priority: P3 (§4 #8)**: none of
X1–X5 is needed for a release, and none starts before Phase 7's exit. They
come before the ports (Phases 8–10), so each is built and settled once on
macOS and then ported with the rest of the app. Each is an optional feature
under §4 #6's rules: a switch in `FeatureSettings` that it checks where it
acts, off by default if it changes what is heard or goes online.

| # | Feature | Touches | Size |
|---|---|---|---|
| X1 | Themes: the user changes the look and feel | UI, settings | M |
| X2 | Real-time effects on what is playing (reverb, chorus, spectral freeze) | core DSP, C API, Rust, settings, UI | L |
| X3 | Five more visualizations, all uncommon in music players | core (analysis), UI | M–L |
| X4 | Recommendations from the library | Rust, UI | M |
| X5 | Recommendations from outside the library | Rust (metadata), UI | M (after X4) |

- [ ] **X1 Themes.** The UI's colours, fonts, density, corner radius and
  the cover-derived accent become design tokens (CSS custom properties on
  `:root`), which every component already reads or is moved to read. The
  user picks a built-in theme (light, dark, high contrast, and a few
  others) or edits one in Settings › Appearance, with a live preview. A
  theme is stored in `AppSettings` as token values, each validated
  (colours, a fixed list of fonts, numbers in range), never as raw CSS,
  so the CSP and `validate` stay meaningful. Themes export and import as
  JSON. High contrast and the system's light/dark and larger text sizes
  keep working (F18); a theme that fails WCAG AA contrast for text is
  flagged in the editor.
- [ ] **X2 Effects.** An effects chain in the core, after decoding and
  before volume, next to the equaliser (F15) and crossfeed (O11): reverb
  (JUCE's `dsp::Reverb`, then perhaps convolution with bundled impulse
  responses), chorus (`dsp::Chorus`) and spectral freeze (an STFT that
  holds the current magnitudes with randomised phases while held). Each
  effect has a bypass, a wet/dry mix and a few parameters, smoothed so
  changes never click; the chain's order is fixed at first. Parameters
  cross the C API as plain values and reach the audio thread lock-free.
  Settings are global with optional presets; per-track settings (O7) only
  if asked for. The signal path panel (O10) lists active effects. Off by
  default (it changes what is heard). Decide first: whether `juce_dsp`
  joins the core's modules (it has no GUI dependency), the CPU budget at
  the highest sample rate (a benchmark in `BenchTests.cpp`, H18), and how
  effects behave across a gapless hand-off and a crossfade (reverb tails
  carry over; freeze releases on a track change). Offline render tests in
  the style of `PlayerEngineTests.cpp`.
- [ ] **X3 Five visualizations** that few players have, beside Phase 5's.
  Candidates, to settle with the owner before building:
  - a **Tonnetz**: the harmonic lattice, lit by the chroma Phase 5
    already computes, so chords and modulations show as moving shapes;
  - a **recurrence plot** that builds up over the track, showing its
    repeats (verses, choruses) as a self-similarity matrix;
  - **cymatics**: Chladni plate figures driven by the dominant partials;
  - a **phase portrait**: a delay-embedded attractor of the waveform,
    whose shape follows timbre;
  - a **pitch spiral**: the spectrum wrapped one octave per turn, so
    notes line up along spokes and harmonics form patterns.

  Each takes the cover's colours and obeys the flash guard and
  `prefers-reduced-motion` (F18). New analysis (the recurrence plot's
  features) goes in the core's analysis thread and the frame encoding,
  with tests on synthetic signals as in Phase 5. Check each one's CPU
  cost at Retina size.
- [ ] **X4 Recommendations from the library.** "More like this" for a
  track, album or artist, and a Home row of library items the user hasn't
  played lately that resemble what they have. Offline and local: scores
  from shared genres, artists and credits (MusicBrainz relations already
  fetched), era, the analysis (O1's loudness, Phase 5's key and tempo if
  stored), and co-listening in the history (O8). It extends what radio
  (O9) and O18 already pick, sharing their code, and leaves out
  unreadable folders (`availability::unreadable`). The scoring is a pure
  function, tested on a fixture library; run the benchmarks (H18) on a
  50k-track library.
- [ ] **X5 Recommendations from outside the library.** Artists and
  releases the user doesn't own, seeded from their library and history:
  candidates are ListenBrainz's similar-artist and recommendation data
  and MusicBrainz relations, checked first against each source's terms
  as in Phase 4's sources table (Last.fm stays excluded). Through
  `http::Client` and the metadata worker, off by default, named in the
  privacy policy (§8.1); what is sent (artist ids, not the library) is
  shown in Settings. Results are links out (MusicBrainz, the artist's
  site, Bandcamp's page; through `webLink`), never streams or downloads,
  and anything the user already owns is filtered out by MBID and folded
  names. Dismissed suggestions are remembered. Recorded fixtures in
  `metadata/fixtures/`, no network in tests.
- **Exit (to check in the app)**: a theme edited, saved, exported and
  imported on another Mac, with VoiceOver and high contrast still usable;
  each effect by ear, including during a gapless hand-off, a crossfade
  and a seek, with no clicks and no dropouts at the smallest buffer
  size; the five visualizations on real music, and their CPU cost;
  library recommendations that make sense on the owner's library;
  outside recommendations with ListenBrainz, none of them already owned.

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
  players do that. We won't: the app is commercial (§4.1),
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
  `bandcamp.com/developer`. Describe the app: a commercial player (its own code MIT) that
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
    Proposal: direct download first. **Decided 2026-10-03 (owner):**
    builds are distributed from this repository's GitHub Releases page
    (the DMG, the zipped app and `SHA256SUMS` that `release.py` makes);
    the Mac App Store is not planned for now.
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
for macOS, except the signing secrets and the icon set, which wait on
§8.1** (Phase 7, Step 7): the version, the notices, the release workflow
and the update check (2026-10-03) are in place, unsigned until the
secrets exist.

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
- **Updates** for direct-download builds: an update check, not the Tauri
  updater. **Done 2026-10-03 for macOS** (`app/src-tauri/src/updates.rs`):
  it reads the repository's latest GitHub Release (§8.1) through
  `http::Client`, compares versions, and Settings › About links to the
  release's page to download the DMG. Checking by hand always works;
  automatic checks (shortly after launch, then daily, with a toast once
  per newer version) are a feature switch, `updateCheck`, off by default
  since it goes online. The owner chose this over the plugin (2026-10-03):
  `tauri-plugin-updater` 2.13.1 installs on macOS by renaming the `.app`
  in place, falling back to an administrator AppleScript, and the App
  Sandbox allows neither; its check alone needs a signing key and a
  signed manifest, and verifies nothing until the download. So there
  is no updater key or manifest. Revisit if the plugin learns to install
  from a sandbox. Store builds (App Store, Microsoft Store, Flathub)
  update through the store instead.
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
7. Publish the GitHub Release (the update check finds it once it's
   neither a draft nor a pre-release); submit store builds
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
| Release artifacts | Each release | SHA-256 checksums, release notes from `CHANGELOG.md` (§8.2) | `release.py` |
| Signing material: Apple certificates (distribution and provisioning profiles yearly, Developer ID every five years), notarization key, Windows certificate | Monthly check once §8.3 is set up | Renew before expiry; keep the offline backups current | `check-signing.py` |
| Service terms and limits (MusicBrainz, Cover Art Archive, Wikimedia, later sources), the `User-Agent` contact, the MetaBrainz supporter plan | Yearly and before each release (§8.1) | Read the terms; update the sources table (`docs/design/phase-4-online-metadata.md`) | none (manual) |
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
- [x] M1 Test harness (2026-10-02): `scripts/test-python.py` (pinned
  pytest through uvx) and `ruff check` in `format-python.py`.
- [ ] M2 Repo checks. Done: `check-c-api.py`, `check-sources.py` and
  `check-migrations.py` (2026-10-02), `check-docs.py` (2026-10-03, H20).
  Open:
  - `sync-ffmpeg-frameworks.py`: rewrites `bundle.macOS.frameworks` from
    the dylibs in `third_party/ffmpeg/macos-universal/lib`; `--check`
    compares only.
- [x] M3 Fixture tools. Done 2026-10-03 (Step 8's Part 6):
  `record-fixtures.py` with `metadata/fixtures/manifest.json`, and
  `make-test-fixtures.py`'s fixed serials, `--only` and lengths.
- [x] M4 CI entry point. Done: `check-all.py` (2026-10-02; CI runs it,
  the pre-commit hook its `--quick` mode), `bench.py` (2026-10-03, H18)
  and `doctor.py` (2026-10-03, Step 8's Part 6).
- [x] M5 Dependency tools. Done 2026-10-03 (Step 8's Part 6):
  `check-pins.py`, `bump-pin.py`, `audit-deps.py` and the weekly
  `audit.yml`.
- [x] M6 Release tools (with §8.2). Done 2026-10-02: `version.py`,
  `make-notices.py`, `release.py`, `check-signing.py`, `build-app.py`,
  `check-bundle.py` and `notarize.py`. `release.py`'s updater manifest
  isn't needed: the update check reads GitHub's release (§8.2,
  2026-10-03).
- **Exit:** `check-all.py` runs in CI (by hand and on each version tag
  since 2026-10-02; the pre-commit hook runs `--quick`), the scheduled job
  reports outdated pins and advisories, and each §9.1 row either has its
  script or is marked manual.
