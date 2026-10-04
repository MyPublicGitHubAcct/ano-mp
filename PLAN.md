# ano-mp — Implementation Plan

Status as of 2026-10-04: Phases 0–6c are built on macOS. Phases 0–3 are
complete; the exit checks of Phases 4, 5, 6, 6b and 6c wait to be done in
the app (each phase says what). Phase 7 (hardening) is under way: every
step of its "Order of work" that needs no owner decision is done, and the
signed release waits on the owner's §8.1 decisions
(`docs/release-decisions.md`). Phase 7b (themes, effects,
visualizations, recommendations, recording and similar artists; P3,
added 2026-10-03) follows Phase 7, before the ports; X1 (themes), X2
(effects), X3 (visualizations), X4 (library recommendations), X5
(recommendations from outside the library), X6 (recording) and X7
(similar artists) were built early, at the owner's request. Phase 7c (user and developer
documentation and a dictionary of classes, added 2026-10-04) is
planned; its user guide is part of the first release. Finished work's design notes and records
are in `docs/design/`, linked from each phase.


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
  (`SignalTap`, `SpectrumAnalyser`, `AnalysisThread`), recording
  (`Recorder`, `FFmpegEncoder`, X6), `FileAnalyser`, and
  platform code (`MediaControls`, `FolderAccess`, `FileStatus`,
  `VolumeWatcher`, `DockMenu`). Fuzz targets in `core/fuzz/`.
- **Effects** (`effects/`): `anomp_effects`, the real-time effects (X2), a
  dependency-free C++20 library the core links: `EffectChain` (the one
  public header) and the eight effects behind it.
- **Rust** (`app/src-tauri/src/`): `anomp.rs` (the C API's wrappers),
  `audio.rs` (the main-thread engine host), `library/` (DB, migrations,
  scanner, browse, search, art and thumbnails, playlists, features,
  recommendations),
  `queue/`, `media.rs`, `metadata/` (online sources and their worker),
  `history/`, `remote/`, `shell/` (menus, mini player, opened files),
  `settings.rs`, `recording.rs`, `visualizer.rs`, `logging.rs`, `diagnostics.rs`,
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
| 159 passing Catch2 tests (136 of the core's, 23 of the effects library's), also clean under ASan, UBSan and TSan | `core/tests`, `effects/tests` |
| 474 passing `cargo test` tests, plus 8 ignored benchmarks (50,000 tracks) and 7 ignored live tests (one per online source) | `app/src-tauri/src` |
| 59 frontend tests (`npm test`, pure modules) | `app/tests/` |
| the scripts' 208 pytest tests (`test-python.py`) | `scripts/tests/` |

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
     decoders, plus libswresample; no video or network, and only the
     encoders and muxers recording writes, X6, with LAME for MP3 linked
     into libavcodec, LGPL too). This adds a
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
   X1–X7 (Phase 7b): themes, real-time effects, ten uncommon
   visualizations (X3, five more added 2026-10-04), recommendations from inside and outside the
   library, recording what is playing to a file (X6, added
   2026-10-04), and similar artists on artist pages. Low priority: after Phase 7's exit and the first release, but
   before the cross-platform work (Phases 8–10), so the ports carry them.
   #6's rules apply; X2, X5 and X6 are off by default (they change what is
   heard, go online and write large files).

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
Windows (Phase 10). Phase 7b (themes, effects, visualizations,
recommendations, recording and similar artists, P3) comes after Phase 7 and before the ports, so those
features are settled on macOS first. Phase 7c (documentation) runs
alongside: its user guide (D1) is needed for the first release, and the
developer guide and dictionary of classes follow. Xcode is not needed until Phase 8.
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
X1–X7 is needed for a release, and none starts before Phase 7's exit. They
come before the ports (Phases 8–10), so each is built and settled once on
macOS and then ported with the rest of the app. Each is an optional feature
under §4 #6's rules: a switch in `FeatureSettings` that it checks where it
acts, off by default if it changes what is heard or goes online.

| # | Feature | Touches | Size |
|---|---|---|---|
| X1 | Themes: the user changes the look and feel | UI, settings | M |
| X2 | Real-time effects on what is playing (reverb, chorus, spectral freeze) | core DSP, C API, Rust, settings, UI | L |
| X3 | Ten more visualizations, all uncommon in music players (two of them combinations) | core (analysis), UI | M–L |
| X4 | Recommendations from the library | Rust, UI | M |
| X5 | Recommendations from outside the library | Rust (metadata), UI | M (after X4) |
| X6 | Record what is playing to a file (WAV, AIFF, FLAC, ALAC, AAC, MP3) | core, C API, Rust, settings, UI | M |
| X7 | Similar artists on artist pages | Rust, UI | S (after X4; X5 for outside ones) |

- [x] **X1 Themes.** The UI's colours, fonts, density, corner radius and
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
  - **Built** 2026-10-03, ahead of Phase 7's exit at the owner's request.
    `AppSettings.appearance` (`theme.rs`) holds the theme in use, the
    user's saved themes (by name, at most 50) and whether the system's
    "Increase contrast" swaps in the high-contrast colours (on by
    default). A theme is a light and a dark palette of twelve `#rrggbb`
    colours, a scheme (system, light, dark), a font from a fixed list
    (system, rounded, serif, mono, Avenir), a text size (12–22 px), a
    density (compact, regular, roomy), a corner radius (0–16 px) and
    "accent from the cover". The twelve built-ins (Standard, Light, Dark,
    High contrast, Paper, Midnight, Forest, Ocean, Rose, Graphite, Sunset,
    Meadow) are `app/src/lib/themes.json`,
    which Rust reads for the default and `contrast.test.mjs` checks for
    AA in both schemes. `lib/theme.ts` turns a theme into custom
    properties (hover, selection and shadow derived from the colours) and
    the cover's colour into a readable accent; `state/appearance.svelte.ts`
    sets them on `:root` in every window, first before the first paint.
    Radii and control padding read the tokens; list rows scale with text
    size and density (`appearance.rowHeight`). The editor previews colours
    and sliders as they move and saves when let go, flags each pair under
    AA, and shows the palette it edits in a sample. Theme files are
    `{"format": "ano-mp theme", "version": 1, "theme": …}`, imported as
    leniently as stored settings (`theme_import`). Switch: `themes`, on by
    default; off, the standard theme shows and Appearance is hidden.
    Left to check in the app: the exit's theme round trip between Macs,
    VoiceOver over the editor, and layouts at 22 px text.
- [x] **X2 Effects.** An effects chain in the core, after decoding and
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
  - **Built** 2026-10-03, ahead of Phase 7's exit at the owner's request,
    as a library of its own: `effects/` (`anomp_effects`), plain C++20
    with no dependencies, which the core links privately. **Decided:**
    not `juce_dsp`'s effects: JUCE's CMake modules compile their sources
    into every target that links them, so a second JUCE library beside
    the core would carry JUCE twice, and a library without dependencies
    builds, tests and benchmarks alone and ports with nothing else. It
    takes the core's warning flags (JUCE's flag targets, which compile no
    JUCE code) and has its own Catch2 suite (`effects/tests`).
    `EffectChain.h` is its one public header: the catalogue (each
    effect's id, default mix, and parameters with unit, range, default
    and whether they move logarithmically) and the chain.
  - Eight effects, run in a fixed order: spectral freeze, lo-fi
    (bits, sample rate), tremolo and auto-pan (rate, depth, stereo),
    phaser (six allpasses; rate, depth, feedback), flanger (rate, depth,
    feedback), chorus (two voices a channel; rate, depth), echo (time,
    feedback, tone in the feedback, spread from straight to ping-pong; a
    new time crossfades over 50 ms rather than bending the pitch) and
    reverb (Freeverb's combs and allpasses, public domain, scaled to the
    rate; size, damping, width, pre-delay). Convolution with impulse
    responses wasn't built. The freeze keeps the magnitudes of the last
    0.15 s (an 8192-point Hann FFT at 44.1 and 48 kHz, longer above) and
    resynthesises them every quarter window with random phases, each
    bin keeping the phase difference between the channels so the image
    stays put, at the input's loudness; holding and letting go crossfade
    at equal power over its `fade`.
  - **The chain**: each effect's switch, mix and parameters are atomics,
    set from any thread and read by the audio thread every 64 samples;
    every change glides. An effect switched on fades in over 50 ms; one
    switched off stops taking input and lets its tail (reverb, echo)
    ring out before its wet signal fades. With every effect off the
    signal is untouched (the engine's exact-output tests still pass).
    The chain runs after the resampler and before the analysis tap, so
    the visualizer shows what the effects make; the equaliser,
    crossfeed and volume follow. Tails carry across a gapless hand-off
    and a crossfade; a held freeze lets go when a track takes over or
    another is loaded. While paused nothing is processed, so a tail
    resumes with the music.
  - **C API**: `anomp_effect_describe`, `anomp_engine_set_effect`
    (switch, mix, parameters; out of range refused, compared as the
    floats the core keeps), `anomp_engine_set_freeze`,
    `anomp_engine_freeze_held`; the signal path gains `effects` (a bit
    per effect on or ringing out) and `freeze_held`.
  - **Rust** (`effects.rs`): `AppSettings.effects`, each effect's switch,
    mix and parameters by id, validated against the core's catalogue;
    the `effects` feature switch, off by default (it changes what is
    heard), turns them all off. Commands: `effects_catalog`,
    `effects_preview` (plays settings without saving, while a slider
    moves), `effects_freeze`, `effects_status`.
  - **UI**: Settings › Effects, always listed (hidden at first while
    off, it couldn't be found), its first switch the feature's own (also
    in Features), the rest greyed out while off: a preset
    (eleven, `lib/effects.ts`, each replacing every effect's settings),
    then each effect in chain order with its switch, mix and parameters
    (logarithmic sliders for rates and times), heard as they move and
    saved when let go; the freeze's Hold button follows a new track
    letting go. While the freeze is on, a snowflake button in the
    now-playing bar holds and lets go too (`state/effects.svelte.ts`,
    shared with Settings, asking the engine again as the track changes
    and every second while held, since the core lets go by itself). The
    signal path panel lists the effects in use and a held freeze. Global settings only; per-track ones (O7) weren't asked
    for.
  - **CPU** (`core.effects.all_192k`, H18): all eight on, the freeze
    held, at 192 kHz in 512-sample blocks, 13.9 times real time
    on the baseline machine; its budget is at least 10.
  - Left to check in the app: each effect by ear, including during a
    gapless hand-off, a crossfade and a seek, with no clicks or dropouts
    at the smallest buffer size.
- [x] **X3 Ten visualizations** that few players have, beside Phase 5's.
  The five candidates, which the owner took, and on 2026-10-04 five more
  at the owner's request, two of them combinations of the others:
  - a **Tonnetz**: the harmonic lattice, lit by the chroma Phase 5
    already computes, so chords and modulations show as moving shapes;
  - a **recurrence plot** that builds up over the track, showing its
    repeats (verses, choruses) as a self-similarity matrix;
  - **cymatics**: Chladni plate figures driven by the dominant partials;
  - a **phase portrait**: a delay-embedded attractor of the waveform,
    whose shape follows timbre;
  - a **pitch spiral**: the spectrum wrapped one octave per turn, so
    notes line up along spokes and harmonics form patterns;
  - a **harmonograph**: damped-pendulum figures tuned, in just
    intonation, to the interval between the two loudest notes;
  - **rhythm rings**: one bar a turn, each bar a ring, the onsets marked
    where they fall, so a repeating rhythm lines up from ring to ring;
  - a **stereo stage**: each band placed between the speakers by its
    balance, bass at the bottom;
  - **Resonance** (a combination): cymatics with the phase portrait
    over it;
  - **Harmony** (a combination): the pitch spiral beside the Tonnetz.

  Each takes the cover's colours and obeys the flash guard and
  `prefers-reduced-motion` (F18). New analysis (the recurrence plot's
  features) goes in the core's analysis thread and the frame encoding,
  with tests on synthetic signals as in Phase 5. Check each one's CPU
  cost at Retina size.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request.
    **New analysis** (`SpectrumAnalyser`, tests in `AnalysisTests.cpp`):
    `notes`, 84 semitones from C2 (MIDI 36) to B8, each the loudest
    component within half a semitone on the bands' scale, read from the
    chroma's 8192-point transform; and `balance`, per band, where its
    sound sits from left (-1) to right (+1), from 2048-point transforms
    of each channel, 0 for a band under the floor. Both cross the C API
    at the end of `anomp_analysis_frame` (`note_count`, `lowest_note`,
    `notes`, `balance`) and the frame format, now version 2 (`encode`:
    the notes as bytes, the balance as signed bytes; 2.3 KB a frame).
    **Decided:** the recurrence plot's features stay in the frontend, not
    the core: the chroma and bands each frame carries are its features,
    so the core has nothing new to compute for it.
  - **Pure logic** (`lib/visualizer/music.ts`, `recurrence.ts`; tests in
    `tests/visualizations.test.mjs`): the strongest triad (its weakest
    note above the mean of the other nine), the strongest interval named
    the consonant way round, the Tonnetz's lattice (fifths along a row,
    major thirds up), the Chladni mode each note rings (the square
    plate's modes in order of m² + n²), the embedding delay (the
    autocorrelation's first zero), and the tempo (`TempoTracker`): the
    autocorrelation of the last 8 s of onset strength, spread ±40 ms,
    over 50–200 bpm, leaning gently towards 120 bpm, with the beats where
    the onsets line up best on that period. A first version fitted the
    core's discrete beats and, on real music, never found a tempo:
    onsets are single-frame spikes and real beats are irregular. Compared
    on five synthetic grooves at 60–180 bpm, this finds 158 of 180, every
    miss an octave off at the extremes (60 read as 120, 180 as 90). Its
    threshold is low, so music without a pulse may show a tempo. The
    recurrence plot sums a feature (the chroma, and the bands pooled
    into eight about their mean) over half-second steps in a 256-step
    grid, and when it fills merges pairs and doubles the step, so the
    whole track always fits; it starts again with each track.
  - **Renderers** (`lib/visualizer/renderers/`): `tonnetz`,
    `recurrence`, `cymatics`, `portrait`, `spiral`, `harmonograph`,
    `rhythm`, `stage`, `resonance`, `harmony`, listed after Phase 5's.
    The combinations are `combine.ts`'s `overlay` (the top one drawn into
    a layer of its own, where `clearStage` fades to transparent; the
    scene's `layer`) and `sideBySide` (one above the other on a tall
    stage). Names, descriptions and their few
    words (chords, intervals, tempo) are in `en.json`.
  - **CPU** at Retina size (640 × 400 points at 2×), measured in an
    offscreen WKWebView on synthetic frames, mean per draw including the
    canvas's flush: under 2 ms for the Tonnetz, recurrence plot, phase
    portrait, harmonograph, rhythm rings and stereo stage; 7 to 10 ms
    for cymatics, the pitch spiral and both combinations, with 95% of
    draws within 5 ms. The stereo stage's 64 glows are sprites painted
    once per palette: drawn as gradients they cost 52 ms.
  - Left to check in the app: each one on real music (the recurrence
    plot on a song with a chorus, the rhythm rings' tempo against a
    known one), in calm mode, full screen on a Retina display, and in
    the auto-cycle.
- [x] **X4 Recommendations from the library.** "More like this" for a
  track, album or artist, and a Home row of library items the user hasn't
  played lately that resemble what they have. Offline and local: scores
  from shared genres, artists and credits (MusicBrainz relations already
  fetched), era, the analysis (O1's loudness, Phase 5's key and tempo if
  stored), and co-listening in the history (O8). It extends what radio
  (O9) and O18 already pick, sharing their code, and leaves out
  unreadable folders (`availability::unreadable`). The scoring is a pure
  function, tested on a fixture library; run the benchmarks (H18) on a
  50k-track library.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    in `library/similar.rs`. An item (a track, an album, an artist, or
    the user's taste) is a **profile** of its tracks, each weighted: the
    share of them in each genre (folded, so "jazz" is "Jazz"), its
    artists (every credited one and the album artist, never Various
    Artists), composers and its albums' MusicBrainz labels (the heaviest
    few of each), its era (the weighted median year, only while the
    middle half spans at most ten years, so an artist across decades has
    none), and its mean loudness where O1 has analysed it. **Decided:**
    key and tempo aren't used: Phase 5 computes them live for the
    visualizer and stores neither, and storing them would mean a second
    analysis pass; loudness is the analysis that counts.
  - **The score** (`score`, pure, tested on hand-made profiles): a
    genre, 3 points times the share both have in common, up to one more
    for a second; the era, 2 within two years, 1 within five; a label, 3;
    an artist linked to the seed's (radio's: band members, subgroups and
    collaborations on MusicBrainz either way, or sharing an album in the
    library), 4; a shared artist, 2 between albums and 1 between tracks
    ("more by"); a shared composer who isn't a shared artist, 2;
    listening sessions together (plays no more than 30 minutes apart,
    O8), 2 for one, rising to 4 at three; and a point for loudness within
    1.5 LU, only beside something else. Recommendations need 3 (a genre
    alone is enough, an era alone isn't); the two strongest reasons worth
    a point each go to the UI as data (`Reason`) and are worded there
    (`lib/similar.ts`, `similar.reason.*`).
  - **What is recommended**: for a track, tracks on other albums (one an
    album, two an artist); for an album, albums (two an album artist);
    for an artist, artists, which X7's library half can show as they
    are. Candidates are made of their tracks outside unreadable folders,
    so an album only there is never one; a seed is all of its tracks
    wherever they are. **Home's "You might like"** takes the taste from
    the last 90 days' plays (a point a play) and the favourites (a track
    2, an album or an artist 3 spread over its tracks), or the last 200
    plays when nothing was played lately, and suggests albums none of
    whose tracks were in it, nor favourite albums, two an album artist,
    each raised by up to 15% by the day so the row changes daily.
    **Radio** (O9) now scores with the same function, keeping its
    randomness, its penalty for last week's plays and its limits, so it
    gains composers and sessions; its reasons stay its own few English
    words, as the queue shows them. O18 stays random by design.
  - Commands `library_similar_tracks`, `_albums`, `_artists` and
    `library_for_you`, behind the `recommendations` switch (on by
    default: local and cheap). UI: "More Like This" on tracks', albums'
    and artists' context menus and on the artist page opens a dialog
    (tracks with Play All and Add All to Queue; album cards; artists);
    album pages show a "More like this" row under "More in this genre";
    Home shows "You might like" first. Each hides while it has nothing.
  - **Benchmarks** (`bench_recommendations`, `similar.*`, budget 300 ms)
    on the 50k-track library with 20,000 plays: 58 to 82 ms for each
    kind, Home and a radio refill, every one reading the whole library
    (no cache to keep in step).
  - Left to check in the app: recommendations that make sense on the
    owner's library (the exit), and whether the weights want tuning.
- [x] **X5 Recommendations from outside the library.** Artists and
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
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    in `metadata/outside.rs` and `metadata/listenbrainz.rs`. **Terms**
    (read 2026-10-04): ListenBrainz's data is MetaBrainz's, "available for
    commercial use" with the same supporter tiers as MusicBrainz (§8.1's
    MetaBrainz decision covers both); its API asks each client for at most
    one call a second, which `http.rs` now applies to both ListenBrainz
    hosts. The sources table in `docs/design/phase-4-online-metadata.md`
    and the privacy table in `docs/release-decisions.md` have its row.
  - **Seeds**: the artists of the user's taste as X4 reads it (the last
    90 days' plays, else the last 200; a favourite track 2, a favourite
    album or artist 3), the heaviest five with a MusicBrainz id (the
    artist's match, else the tags'; never Various Artists). Only those
    ids go out, to the Labs API's `similar-artists` query (the algorithm
    ListenBrainz's own artist pages show), cached 30 days in the response
    cache like MusicBrainz's. Settings › Features lists the artists whose
    ids are sent. While online services are off, only cached lists are
    used.
  - **Candidates**: each seed's 30 best similar artists, scored by their
    place in its list (its score over the list's best) times the seed's
    share of the taste, summed across seeds; and MusicBrainz's relations
    stored with the seed's match (members, subgroups, collaborations; X4
    reads the same) at 0.8 of a list's best, with no request. Left out:
    anything the library has by MBID (tags or match) or by folded name,
    Various Artists, and dismissed ones (`outside_dismissed`, migration
    013, carried by F20's export). The two strongest reasons go to the UI
    ("like Radiohead on ListenBrainz", "Radiohead member"). A seed whose
    list fails still counts for its relations; the failure shows only
    when nothing was found.
  - **UI**: Home's "Beyond your library" after "You might like" (loaded
    apart from Home's other rows, as Labs can be slow); "More Like This"
    for an artist gains "Not in your library" (and shows while either
    switch is on). A suggestion's name opens a menu of links: MusicBrainz
    and ListenBrainz always, the homepage and Bandcamp page from a
    MusicBrainz lookup of that artist (`Artist::bandcamp`, new) while
    MusicBrainz may be contacted; each through `openWebLink`. "Not
    Interested" (the row's ×, or the menu) dismisses; Settings offers to
    suggest the dismissed again.
  - Commands `outside_for_you`, `outside_like_artist` (one seed: what X7's
    outside half will call), `outside_links`, `outside_dismiss`,
    `outside_status` and `outside_forget_dismissed`, behind the
    `outsideRecommendations` switch (off by default: it goes online).
    Fixtures: ListenBrainz's list for Radiohead, and Radiohead's
    MusicBrainz artist re-recorded with its Bandcamp link, a member and a
    subgroup (`record-fixtures.py` now selects from a response that is a
    list, path `""`); `live_similar_artists` checks the real service.
  - **Decided:** ListenBrainz's per-user recommendations
    (`/1/cf/recommendation/user/<name>/recording`) aren't used: they need
    the user's ListenBrainz name (known only with O8's token) and return
    recordings, each needing a MusicBrainz lookup at one a second to name
    its artist, while the seeds' similar artists already give artists
    the user doesn't own. Releases the user doesn't own are Phase 4's
    discography on the artist page (an owned artist's missing release
    groups), so X5 suggests artists only.
  - **Known limits:** Labs is ListenBrainz's experimental API: answers
    took 1 to 40 s on 2026-10-04, and one failed. A timeout marks the
    host unreachable (`http.rs`), so the other seeds fail at once and
    the cached lists and MusicBrainz's relations still show; the
    algorithm's name may change, which `live_similar_artists` would
    catch (§6).
  - Left to check in the app: suggestions that make sense on the owner's
    library, none of them owned (the exit), the links, and how long a
    first Home takes with ListenBrainz slow.
- [x] **X6 Record the output to a file.** Added 2026-10-04 at the
  owner's request. A Record button (in the now-playing bar, and in the
  Controls menu) writes what is playing, as heard, to a WAV file until
  it is pressed again: across track changes, gapless hand-offs and
  crossfades, with the effects (X2), equaliser (F15) and crossfeed
  (O11) applied.
  - **Core**: a tap after crossfeed and before volume, so the recording
    doesn't depend on the volume slider (decide: before or after
    volume). The audio thread only pushes samples into a lock-free FIFO;
    a writer thread drains it to the file (JUCE's
    `AudioFormatWriter::ThreadedWriter` over `WavAudioFormat`, which
    `juce_audio_formats` already has). 32-bit float at the engine's
    rate by default, 24-bit as an option; RF64 past 4 GB. While paused
    nothing is written, so a pause leaves no silence. A change of the
    engine's rate (O10's sample-rate matching) starts a new file with
    a numbered suffix rather than resampling. A full disk or a write
    error stops the recording, keeps what was written (the header
    finalised) and reports it through an event. A FIFO overrun (the
    writer falling behind) is counted and reported, never blocks the
    audio thread.
  - **C API**: `anomp_engine_record_start` (UTF-8 path, format),
    `anomp_engine_record_stop`, `anomp_engine_recording` (state, frames
    written, overruns), and a `RecordingStopped` event with its reason;
    main-thread only, like the rest of the engine. The signal path
    panel (O10) shows a recording in progress.
  - **Rust** (a new `recording` module): the recordings folder, picked once with
    the folder picker and kept as a read-write security-scoped bookmark
    (the sandbox already has `files.user-selected.read-write`); a file
    named by date and time (`ano-mp 2026-10-04 21.15.03.wav`), never by
    title. Optionally a cue sheet beside it listing the tracks and their
    offsets, so the recording reopens as tracks (Tracks are parts of
    files). Logs follow H9: ids and counts at info, the file name at
    debug.
  - **UI**: the Record button with an elapsed time and a red dot while
    recording; Settings › Recording for the folder, the sample format
    and the cue sheet. Strings in `en.json`, errors through `errorText`.
  - Switch: `recording`, off by default (it writes large files: about
    23 MB a minute at 48 kHz float). Streams (Phase 11) are never
    recorded: the button is disabled while one plays, under Bandcamp's
    terms.
  - Tests: offline renders in the style of `PlayerEngineTests.cpp` that
    record a gapless hand-off and a crossfade and compare the file with
    the rendered output, sample for sample; pause, a rate change, a
    write error and an overrun; and a benchmark (H18) that the tap costs
    the audio thread nothing measurable. A bundle self-test stage (H14)
    writes into a picked folder under the sandbox.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request,
    who asked for a choice of output type ("wav, mp3, etc.") beyond the
    WAV above. **Decided:** six formats, all written by FFmpeg
    (`core/src/FFmpegEncoder.cpp`, the second file that includes its
    headers): WAV (16-bit, 24-bit or 32-bit float, the default; RF64 past
    4 GB), AIFF and FLAC (16 or 24), Apple Lossless and AAC in .m4a, and
    MP3, AAC and MP3 at 96–320 kbps. `build-ffmpeg.sh` now enables just
    those encoders and muxers (`FFmpegBuildTests` checks the exact list)
    and builds **LAME 4.0** (LGPL, July 2026; its decoder and programs
    left out) as a static library linked into libavcodec, so MP3 needs no
    new dylib; the pin is in `check-pins.py`/`bump-pin.py` (a
    `sourceforge` upstream) and in `THIRD_PARTY_NOTICES`. A lossy encoder
    that can't take the device's rate is resampled (MP3 to 48/44.1 kHz,
    AAC to 96/88.2 kHz); the rest record at the device's rate. The tap is
    **before the volume**, after crossfeed, so the slider doesn't change
    the recording; a pause fades it out and in with the same one-block
    ramp as what is heard, and while paused nothing is pushed.
  - **Core**: `Recorder` (`core/src/Recorder.*`) owns a 2^19-frame
    `AbstractFifo` (2.7 s at 192 kHz) the audio thread copies into, and a
    writer thread that wakes every 20 ms and drains it into the encoder;
    the audio thread never waits or allocates, and what doesn't fit is
    dropped and counted. A rate change (`prepareToPlay`) records a split
    at the frame it happened; the writer finishes the file there and opens
    "name 2.ext". A write error or ENOSPC stops the writer, which
    finalises the file; `PlayerEngine::dispatchEvents` reports it once
    (`onRecordingFailed`). Track marks are kept for the cue sheet: one at
    the start if a track is loaded, then at each `install`, gapless
    hand-off, and the *start* of a crossfade, accurate to the chunk the
    hand-off fell in. C API: `anomp_engine_record_start`/`_stop`,
    `anomp_engine_recording` (frames, seconds, overruns, files),
    `anomp_engine_recording_file`, `anomp_engine_recording_marks`,
    `anomp_record_format_available`/`_extension`,
    `ANOMP_EVENT_RECORDING_FAILED` (`result`: write failed or disk full),
    `anomp_signal_path.recording`, and `anomp_bookmark_create_writable`
    (a security-scoped bookmark without the read-only flag).
  - **Rust** (`recording.rs`): `AppSettings.recording` (format, bits,
    bitrate, cue sheet: on), the `recording` feature (off). The folder's
    bookmark is its own `settings` row (`recording.folder`), not part of
    `AppSettings`, so a data export never carries it; it is resolved (and
    refreshed when stale) for as long as a recording is written. Files are
    `ano-mp 2026-10-04 21.15.03.wav`, " (2)" if taken. The tracks that
    become current (`queue::publish`, once loaded) are matched to the
    core's marks, and on stop each file gets `<name>.cue` naming them
    (`library::cue` reads them back as tracks; a file after a rate change
    starts with the track playing). Commands `recording_status`,
    `_set_folder`, `_start`, `_stop`; events `recording` and
    `recording-failed`; coded errors `recordingNoFolder`,
    `recordingFolderUnavailable`, `recordingStartFailed`,
    `recordingFailed` (`.diskFull`), `recordingRunning`,
    `recordingNotRunning`. Turning the feature off, or quitting, stops and
    finishes the recording.
  - **UI**: Record in the now-playing bar (a red dot and the time recorded
    while it runs; the first press asks for a folder if there is none),
    Controls › Record (⌥⌘R, checked while recording), Settings ›
    Recording (folder, format, sample size or bitrate, cue sheet, the
    size a minute), and a Recording row in the signal path.
  - Tests: `RecorderTests.cpp` (a gapless hand-off and a crossfade
    recorded and compared with the rendered output sample for sample;
    pause, volume, a rate change and its numbered file and marks, write
    errors and a full disk, an overrun that never slows the audio thread,
    and each of the 14 format/size/rate cases decoded back), C API and
    writable-bookmark tests; Rust tests for names, settings, formats and
    cue sheets; a `recording` stage in the bundle self-test (a FLAC into a
    folder opened through a writable bookmark). Benchmarks:
    `core.record.tap_192k` (43,000× real time) and
    `core.play.flac_recording` (954× against 1,202× without) on
    2026-10-04.
  - **Known limits:** .m4a files (Apple Lossless, AAC) are readable only
    once finished: a crash or power loss while recording leaves them
    without their index, where WAV, AIFF, FLAC and MP3 keep what was
    written. AIFF has no 64-bit form, so it fails at 4 GB (about 4 hours
    at 48 kHz 24-bit). Cue marks are accurate to a chunk (about 10 ms), a
    crossfade cancelled by a seek keeps its mark, and a repeated track is
    named again by the track before it. Streams (Phase 11) don't exist
    yet; their rule above (Record disabled while one plays) lands with
    them. Left to check in the app: the exit's recording across a gapless
    album, a crossfade and a pause, played back elsewhere; the folder
    picked in a sandboxed bundle; a full disk; and Controls › Record.
- [x] **X7 Similar artists on artist pages.** A "Similar artists" section
  on the artist page (`ArtistPage.svelte`), under the biography: first
  the library's artists most like this one, by X4's artist scoring
  (`similar::similar_artists`, already built and shown in X4's dialog:
  shared genres, credits and MusicBrainz relations, co-listening), each
  opening its own artist page; then, while X5 is on, artists the user
  doesn't own from X5's sources (ListenBrainz's similar artists for the
  artist's MBID), as links out through `webLink`. It needs no data of its
  own: the library half is a query on X4's scores, the outside half is
  X5's fetch for one artist, run on the metadata worker and cached as X5
  caches. Each row says why it is there ("shares 3 genres", "played
  together often", "similar on ListenBrainz"). Unreadable folders' artists
  are left out (`availability::unreadable`), as are artists with too
  little in common (a score threshold, tested on X4's fixture library).
  Its own switch (`similar_artists`, on by default, as it is local and
  cheap; its outside half follows X5's switch). Hidden while the artist
  has no matches, rather than showing an empty section. Before building,
  decide: how many to show (a row of about eight, with "More"), and
  whether band members and member-of relations show here or stay in the
  artist's details.
  - **Built** 2026-10-04, ahead of Phase 7's exit at the owner's request.
    **Decided:** a row of eight of each half, and "More" shows the rest
    in place (up to X4's and X5's twelve each) rather than opening X4's
    dialog, whose switches differ; "More Like This" stays in the page's
    header. Band members, subgroups and collaborations show here as
    reasons ("Radiohead member"), scored as X4 scores them (a link is
    worth 4, enough alone); the page lists no members of its own, as
    MusicBrainz's relations are read only for scoring. Reasons are X4's
    and X5's as worded (`similar.reason.*`, `outside.reason.*`) rather
    than new counts ("shares 3 genres"), so the dialog and the page
    agree.
  - **Library half**: `library_artist_page_similar`, X4's
    `similar::similar_artists` behind X7's switch, so the section works
    with X4's off; X4's threshold (`MIN_SCORE`, 3) and unreadable folders
    as X4 applies them (`artists_need_enough_in_common_and_a_readable_folder`
    on X4's fixture library: a genre alone is enough, a year alone or
    another genre isn't, and an artist only in an unreadable folder is
    left out). **Outside half**: X5's `outside_like_artist` (cached and
    on the metadata worker as X5's), called only while both switches
    are on and loaded apart from the library half, so a slow
    ListenBrainz never holds it back; its rows are X5's
    (`OutsideArtists`: links out through `openWebLink`, "Not
    Interested").
  - **UI**: `ArtistSimilar.svelte` under the About section of
    `ArtistPage.svelte`, hidden while both halves are empty; the library
    half reloads with the library, the outside half only for another
    artist or a switch. Switch: `similarArtists` (X7 in Settings ›
    Features), on by default.
  - Left to check in the app: similar artists that make sense on the
    owner's library, with and without X5 (the exit).
- **Exit (to check in the app)**: a theme edited, saved, exported and
  imported on another Mac, with VoiceOver and high contrast still usable;
  each effect by ear, including during a gapless hand-off, a crossfade
  and a seek, with no clicks and no dropouts at the smallest buffer
  size; the ten visualizations on real music, and their CPU cost;
  library recommendations that make sense on the owner's library;
  outside recommendations with ListenBrainz, none of them already owned;
  a recording across a gapless album, a crossfade and a pause, played
  back in another app without gaps or clicks;
  similar artists that make sense on the owner's library, with and
  without X5.

### Phase 7c — Documentation
Added 2026-10-04 at the owner's request. The repo documents decisions
and designs well (this plan, `CLAUDE.md`, `docs/design/`), but that is
written for whoever is building the next step: it records why, in the
order things happened, and assumes the reader knows the codebase.
Nothing explains the app to someone who just wants to use it, and
nothing lets a developer new to the code find their way from "this
behaves oddly" to the file that does it. Phase 7c writes three
documents for human readers, judged first on being **complete** (every
screen, setting and component is covered) and **useful** (a reader with
a question finds the answer in a minute or two), then on polish.

| # | Document | Reader | Priority |
|---|---|---|---|
| D1 | User guide: using the app | anyone who plays music with it | P1 (part of the first release) |
| D2 | Developer guide: how the code is built and works | developers, testers, technically minded users | P2 (P1 for the parts §8.1's reviewers need) |
| D3 | Dictionary of classes and types | developers, alongside D2 | P2 |

They live in the repo as Markdown, beside the code they describe, so a
change and its documentation land in one commit and `check-docs.py`
checks their paths and links: D1 in a docs/user-guide folder (one page
per chapter, with an index), D2 in docs/developer-guide, D3 as
docs/class-dictionary.md (or a folder per layer, if one page grows too
long). They describe the app as it is, not its history: the reasons and
the record of how each part was built stay in `docs/design/`, and D2
links to them rather than repeating them.

- [ ] **D1 User guide.** How the app is meant to be used, in plain
  words, for someone who has never seen it and knows nothing of how it
  is built. Every feature the user can reach is described: what it is
  for, where to find it, what each control does, and what to expect
  (including when something is off by default, needs a service account,
  or goes online). No internal terms ("bookmark", "FTS", "migration"):
  where the user meets a concept, it is explained in their terms
  ("ano-mp remembers the folders you allowed it to read").
  - **Shape.** Task-first chapters ("Add your music", "Make a
    playlist"), each opening with what the reader can do there, then the
    steps, then the details. Short sentences, numbered steps, the
    controls' names exactly as the UI shows them (from `en.json`), and a
    screenshot (light theme, the standard theme, a demo library with no
    real artists' covers) wherever a screen is first introduced.
    Keyboard shortcuts beside each action and collected in an appendix
    (the same list as the shortcuts sheet, F6).
  - **Chapters**, at least:
    1. *Getting started*: what ano-mp is, the formats it plays, the
       first run (F8), adding a music folder and what the scan does, why
       macOS asks for permission, and a tour of the window (sidebar,
       browse pane, Now Playing bar, queue).
    2. *Finding music*: every sidebar view (Home and its suggestions,
       library, artists, albums, genres, Recently added, Recently played,
       On this day, favourites, history, playlists), browsing and
       grouping, sorting rules, search (words, parts of words, fields,
       F12), artist and album pages (biography, discography, works and
       movements, similar albums and artists, "more in this genre").
    3. *Playing music*: play, pause, seek (and the waveform seek bar),
       next and previous, the queue (adding, reordering, drag and drop,
       multi-select, F4), shuffle, segue-aware shuffle and repeat,
       library radio and "keep playing when the queue ends", gapless
       albums, crossfade, resume on launch, stop after this track and the
       sleep timer, per-track and per-album preferences (O7), volume
       levelling (ReplayGain and the loudness analysis, O1).
    4. *Playlists and favourites*: playlists, M3U8 import and export,
       smart playlists and their rules, favourites and ratings.
    5. *Track and album details*: Get Info, lyrics (synced and plain,
       `.lrc` files), cover art (choosing, replacing), cue sheets and
       chapters shown as tracks, compilations and multiple artists.
    6. *Online information*: what each service (MusicBrainz, Cover Art
       Archive, Wikipedia, Discogs, ListenBrainz) adds, what is sent to
       it and when, how to turn each on or off, adding a Discogs token,
       fixing a wrong match ("Find details", "Find artist"), and what
       happens offline.
    7. *Sound*: the equaliser, crossfeed, effects (reverb, chorus,
       spectral freeze), practice mode (A–B loop, tempo), sample-rate
       matching and the signal path panel: what each does to the sound
       and when to use it.
    8. *Visualizations*: each visualization with a picture, its settings,
       and the safe-visualizer options (F18).
    9. *Recording*: recording what is playing, the formats and what to
       choose, where files go, and what is and isn't recorded (effects,
       volume).
    10. *Other ways to control it*: the menu bar and Controls menu, the
        Dock menu, the mini player and menu-bar controls, media keys and
        the lock screen's Now Playing, notifications, opening files from
        Finder, and the LAN remote (setting it up on a phone and what it
        exposes).
    11. *Settings*: every Settings section (General, Library folders,
        Display, Sort rules, Playback, Equaliser, Effects, Visualizer,
        Recording, Appearance and themes, Features, Services, About),
        each option with what it changes and its default. Generated where
        possible (below), so no option is missed.
    12. *Keeping the library healthy*: the library health report,
        missing and unreadable folders (why tracks stay, dimmed),
        files in iCloud ("Optimize Mac Storage"), removing missing
        tracks, moving or renaming files, rescans and file watching,
        exporting and importing your data (F20), and the database repair
        dialog.
    13. *Privacy*: what stays on the Mac, what goes online and only when
        switched on, what the logs and "Copy diagnostics" contain.
    14. *Troubleshooting and FAQ*: no sound, a folder that won't scan, a
        file that won't play, wrong tags or covers, high CPU, the app not
        opening after an update; each with what to try and what to send
        with a bug report.
    - Appendices: keyboard shortcuts, supported formats, glossary of the
      words the UI uses, and every error message the UI can show
      (`error.*` in `en.json`) with what it means and what to do.
  - **Kept complete.** A check in `check-all.py` (a new script with its
    test, per §9.2's rules) compares the guide with the app: every
    sidebar view, Settings section and feature switch in
    `FeatureSettings` is named in the guide, and every `error.<code>`
    has an entry in the errors appendix. The Settings reference is
    generated from `AppSettings` (labels from `en.json`, defaults from
    Rust) and committed, like `THIRD_PARTY_NOTICES`, so it can't drift.
    A new feature or setting isn't done until the guide covers it (add
    to this plan's definition of done and `CLAUDE.md`).
  - **Shipped.** Help › "ano-mp Help" opens the guide. Decide first:
    bundle it in the app (HTML built from the Markdown, opened in its
    own window under the existing CSP, works offline) or host it and
    open it with `openWebLink` (smaller app, always current, needs the
    owner's site from §8.1). Bundling is the default proposal, since
    the app otherwise works offline.
  - **Exit:** someone who hasn't used the app follows the guide from a
    fresh install to a playing library, a playlist, an online lookup and
    a changed setting without help; every screen and setting is in it;
    the check passes.
- [ ] **D2 Developer guide.** How the code is organised and how it works,
  detailed enough that a developer can go straight to the part that
  interests them, understand what it does, and diagnose a problem there
  without reading the whole codebase. It assumes a programmer, not a
  JUCE, Rust or Svelte expert, and points to `CLAUDE.md`'s rules rather
  than restating them (and `CLAUDE.md` points back for explanations).
  - **Chapters**, at least:
    1. *Overview*: the three layers (§1), why the core is a static
       library and not a sidecar, the one C API, who owns which thread,
       and a diagram of a track's journey from a file on disk to the
       speaker and the visualizer.
    2. *Getting set up*: tools (`doctor.py`), first build, running the
       app, the tests and `check-all.py`, the presets (debug, ASan, TSan,
       fuzz, bench), and the common first-build failures and their fixes.
    3. *Repository map*: every top-level folder and every module in
       `core/src`, `effects/src`, `app/src-tauri/src` and `app/src`, one
       or two lines each: what it owns and what it must not do.
    4. *The core* (C++): `AudioEngine` and `PlayerEngine`, the FFmpeg
       reader, gapless hand-off and crossfade, tag reading, analysis for
       the visualizer, recording, folder access, media controls, logging,
       and the C API's conventions (errors, ownership, callbacks).
    5. *Effects*: the chain, how settings reach the audio thread, the
       bit-identical rule, adding an effect step by step.
    6. *The Rust backend*: start-up order (`lib.rs`), the engine on the
       main thread (`audio.rs`), the queue (model, opening off the main
       thread, the saved queue), the library (database, migrations,
       scanner, browse and search, availability and folder access,
       analysis, art and thumbnails), metadata services and the worker,
       settings and features, history and ListenBrainz, the shell (menus,
       Dock, tray, mini player, notifications), the LAN remote, logging
       and diagnostics, recovery at launch.
    7. *The frontend*: routes and windows, the state modules
       (`state/`), how a component calls a command (`api.ts`, the
       generated `commands.ts`) and receives events, i18n, themes,
       virtual lists, the visualizer's rendering.
    8. *How things flow*: worked sequences, each naming the functions in
       order across the layers: pressing play on a track; a gapless
       hand-off and a crossfade; adding a folder and its scan; a search
       keystroke; an album's details arriving from MusicBrainz; a setting
       changed in the UI reaching the engine; quitting.
    9. *Data*: the database schema table by table (what each column
       means, which code writes it), the settings file, the caches and
       where every file the app writes lives on disk (sandboxed and not).
    10. *Troubleshooting*: where the logs are and how to raise their
        level, reading "Copy diagnostics", the /dev page, running one
        test, reproducing a fuzzer crash, the bundle self-test, checking
        a sandboxed bundle safely, and a symptom table ("no sound",
        "folder shows as unavailable", "covers missing", "scan never
        ends", "bindings are stale", "clippy fails on unsafe") pointing
        to the code and the log lines to look at.
    11. *Recipes*: adding a C API function, a command, a setting, a
        feature switch, a migration, a metadata source, an effect, a
        visualization, a theme colour, a UI string; each the full list of
        places to touch, in the order `CLAUDE.md`'s rules require.
    12. *Testing*: what each suite covers, fixtures and how they're made,
        fakes (`FakeBookmarks`, the fake engine, `http::testing`),
        benchmarks and their budgets, fuzzing.
    13. *Building and releasing*: a pointer to §8 and the release
        scripts, with what each step checks.
  - **Diagrams** as text (Mermaid in the Markdown, which GitHub renders)
    so they are diffed and reviewed like code: the layers, the threads,
    the playback path, the scan, the metadata worker.
  - **Kept accurate.** Paths and links are checked by `check-docs.py`
    already; the repository map is checked against the tree (every
    module named, nothing named that's gone) by the same new check as
    D1's. A change that moves a responsibility updates D2 in the same
    commit.
  - **Exit:** a developer new to the repo, given three real bugs from
    the history, finds the responsible code from the guide alone, and
    adds a setting by following its recipe with no other help.
- [ ] **D3 Dictionary of classes.** An alphabetical reference of every
  named type a developer meets: C++ classes and structs in `core/` and
  `effects/` (public and private), the C API's types and enums, Rust
  structs, enums and traits in `app/src-tauri`, and the frontend's
  TypeScript types, state modules and Svelte components. Each entry
  gives: the name, its layer and file (linked), one or two sentences on
  what it is responsible for, its main collaborators (what it owns, what
  calls it), the thread it lives on where that matters, and a link to
  the D2 section that explains it in context. Generated types
  (`generated/`) are listed once, by where they come from.
  - **Generated skeleton, written descriptions.** A stdlib-only script
    lists the types from the sources (C++ `class`/`struct` declarations,
    Rust `pub struct`/`enum`/`trait` items, TS exported types, Svelte
    files) and checks the dictionary against it: a type without an entry,
    or an entry for a type that's gone, fails `check-all.py`. The
    descriptions are written by hand, since a generated one would only
    repeat the name; where a type has a doc comment, the entry and the
    comment say the same thing (the comment wins and the entry is
    copied from it). Tiny private helpers may be listed by name only,
    marked as such, so the list stays complete without padding.
  - Indexes beside the alphabetical list: by layer and by folder, so a
    reader can scan "everything in the queue" as easily as look up one
    name.
  - **Exit:** every type in the tree has an entry and the check passes;
    a sample of twenty entries, picked at random, read correctly
    against the code.
- **Order:** D2's overview and repository map first (they shape the rest
  and help anyone reviewing §8.1's decisions), then D1 (needed for the
  release), then the rest of D2 and D3 together, since writing the
  dictionary exposes what D2 misses. Screenshots are taken last, against
  the release build's UI. Phase 7b's features are documented as each is
  built (X7 when it lands).
- **Decide first:** whether the user guide is bundled or hosted (above);
  the screenshot library (a demo library of free-licensed music, so
  covers and names can be published); and whether D1 is translated
  when the UI is (F19), or English only at first.
- **Exit (to check)**: D1–D3's exits, `check-all.py` green with the new
  checks, and the owner reading D1 end to end on the release build.

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
| ListenBrainz's Labs API (X5's similar artists) is experimental: slow (1–40 s), sometimes down, and its algorithm names may change | Off by default; cached 30 days with a stale fallback; a timeout backs the host off, and MusicBrainz's relations still show; the ignored `live_similar_artists` test catches a renamed algorithm (run before each release with the other live tests) |
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
  effects/         C++ real-time effects (static lib, no dependencies), linked by core
    include/anomp/effects/ its public header
    src/
    tests/         Catch2
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
      Since 2026-10-04 the opinion covers FFmpeg's AAC encoder too (X6
      records to AAC); the fallback there is to leave AAC out of
      `build-ffmpeg.sh`'s encoders, which hides it from the format list.
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
      token when they turn it on). X5 (2026-10-04) read them for its
      similar artists: commercial use allowed under MetaBrainz's
      supporter tiers, as for MusicBrainz; the privacy policy must also
      name X5's requests (the MusicBrainz ids of the artists played most).
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
