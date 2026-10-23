# ano-mp — Implementation Plan

Status as of 2026-10-11: Phases 0–6c are built on macOS. Phases 0–3 are
complete; the exit checks of Phases 4, 5, 6, 6b and 6c wait to be done in
the app (each phase says what). Phase 7 (hardening) is under way: every step
of its "Order of work" that needs no owner decision is done, and the signed
release waits on the owner's §8.1 decisions (`docs/release-decisions.md`).
Phase 7b (P3, added 2026-10-03) is built ahead of Phase 7's exit, at the
owner's request: X1–X7 (themes, effects, visualizations, recommendations
from the library and from outside it, recording, similar artists), X8 (the
effects workbench) and X9 (search that finds features), the last two checked
in the app (2026-10-08). Phase 7c (documentation, added 2026-10-04) is under
way: D1's user guide is written, checked and bundled (Help › ano-mp Help)
and waits on its screenshots and exit check; D2's developer guide is written
and checked and waits on its exit check; D3's dictionary of classes has
passed its exit check; D4 (documentation upkeep, added 2026-10-08) has its
stale text fixed, the smoke test's pass over Phase 7b done and Phase 7b's
and D3's records moved to `docs/design/`, its checks of links' anchors, of
Mermaid diagrams and of the shortcuts appendix added, the index of plan
items (`docs/plan-items.md`) generated and checked (2026-10-09), a guide to
the documents (`docs/README.md`) written, and a porting chapter added to the
developer guide ahead of Phases 8–10, with the Windows, Linux and iOS
problems its inventory found fixed (2026-10-10). Phase 7d (added 2026-10-08)
proposes forty more features (N1–N40) for the owner to choose from; none is
accepted yet. Phase 7e (playing music from a Jellyfin server, added
2026-10-10) is built ahead of Phase 7's exit, at the owner's request
(2026-10-11): 7e.2–7e.6 are done and tested offline against hand-made
responses; 7e.1's test server, and the exit check in the app, are still to
do. Phase 7f (playing music from a Plex server, added 2026-10-10) is
built the same way at the owner's request (2026-10-11): 7f.2–7f.6 are done
and tested offline against hand-made responses; 7f.1 read Plex's terms
(nothing rules the app out; a §8.1 gate for the owner) and waits on a test
server, as does the exit check. Its switch is withdrawn from Settings ›
Features for now (2026-10-11, the owner's request; shown only while still
on). Finished work's design notes and records are in `docs/design/`,
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

- **The core is linked in as a static library, not run as a sidecar
  process.** iOS does not allow spawning helper processes, so an in-process
  static library is the only design that works on every target. Rust's
  `build.rs` builds it with the `cmake` crate and links it.
- **The boundary is a plain C API** (no `cxx` crate or C++ types crossing).
  It is simple, stable and easy to test from both sides. Events such as
  position, track-ended and spectrum frames travel through registered C
  callbacks that Rust forwards as Tauri events or channels.
- **FFmpeg decodes every format on every platform**; JUCE does everything
  after decoding (resampling to the device, mixing, output, devices). FFmpeg
  is wrapped as a single JUCE `AudioFormat` (`FFmpegAudioFormat`), so the
  rest of the core doesn't know FFmpeg exists. One decoder means identical
  format support and gapless behaviour on all four OSes, and one test
  matrix.
- **Rust owns the non-audio services** (DB, HTTP, settings). This keeps the
  C++ core small and testable, and those crates (`rusqlite`, `reqwest`,
  `serde`) are mature.

## 2. Current state (done)

Where each part lives. Each phase's design doc in `docs/design/` says what
it holds and how it was built.

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
  `servers/` (music servers as library sources, Phases 7e and 7f),
  `history/`, `remote/`, `shell/` (menus, mini player, opened files),
  `settings.rs`, `recording.rs`, `visualizer.rs`, `logging.rs`,
  `diagnostics.rs`, `bindings.rs`, `self_test.rs`, `dev.rs` (debug builds
  only).
- **Frontend** (`app/src/`): `routes/`, `lib/components/`, `lib/state/`,
  `lib/visualizer/`, `lib/i18n/`, generated types in `lib/generated/`.
- **Tooling** (`scripts/`, `.github/`): `check-all.py` and the repo checks,
  formatters, release scripts, `bench.py`, the pre-commit hook, CI, release
  and fuzz workflows.
- **Docs** (`docs/`): the user guide (`docs/user-guide/`, Phase 7c D1, its
  settings reference generated by `guide.rs`), the developer guide
  (`docs/developer-guide/`, D2, its repository map checked by
  `check-developer-guide.py`), the dictionary of classes and types
  (`docs/class-dictionary/`, D3, checked against the sources by
  `class-dictionary.py`), the index of plan items (`docs/plan-items.md`, D4,
  generated by `plan-items.py`), checklists, release decisions, the smoke
  test, and finished work's design in `docs/design/`; `docs/README.md`
  (D4) says what each is for, who reads it and whether it is current.

Test suites (`check-docs.py --counts` compares these with the suites):

| Suite | Location |
|---|---|
| 159 passing Catch2 tests (136 of the core's, 23 of the effects library's), also clean under ASan, UBSan and TSan | `core/tests`, `effects/tests` |
| 559 passing `cargo test` tests, plus 8 ignored benchmarks (50,000 tracks), 10 ignored live tests (one per online source, a Jellyfin server, plex.tv and a Plex server) and the exit test's ignored helper (run in a child process) | `app/src-tauri/src` |
| 93 frontend tests (`npm test`, pure modules) | `app/tests/` |
| the scripts' 268 pytest tests (`test-python.py`) | `scripts/tests/` |

## 3. Prerequisites

Needed for Phases 0–7 (macOS only; the Command Line Tools are enough). All
installed on the dev machine as of 2026-09-25 (versions noted).
`scripts/doctor.py` checks them against the repo's pins and minimums:

- [x] CMake ≥ 3.25 and Ninja (4.4.3, 1.13.2 via Homebrew); Apple clang
      (21.0.0, Command Line Tools, macOS SDK 27.0)
- [x] **Rust**: `curl https://sh.rustup.rs -sSf | sh` (rustc/cargo 1.98.1)
- [x] **Node.js**: `brew install node` (or fnm/nvm) (v26.10.0, npm 11.19.1 —
      the "Current" line until Node 26 enters LTS in October 2026; fall back
      to 24 LTS if Tauri tooling objects)
- [x] Tauri CLI: `cargo install tauri-cli --version "^2" --locked`, or use
      the npm `@tauri-apps/cli` (tauri-cli 2.11.5)
- [x] FFmpeg build dependencies: `brew install nasm pkg-config` (nasm 3.02,
      pkgconf 3.0.7; the project builds its own FFmpeg; see Phase 1)
- [x] Check tools (H7, 2026-10-02): `brew install gitleaks uv` (gitleaks
      8.30.1) and `cargo install cargo-deny --version 0.20.2 --locked`.
      Rust and Node are pinned by `rust-toolchain.toml` and `.nvmrc`
      (rustup installs the pinned Rust by itself).

Not needed until Phase 8 (iOS/iPadOS):

- [ ] **Xcode** (full app, not just the Command Line Tools), then
      `sudo xcode-select -s /Applications/Xcode.app`
- [ ] iOS Rust targets:
      `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`
- [ ] Apple Developer account, needed to sign and run on physical
      iPhone/iPad devices

Not needed until Phases 9–10. Tauri apps can't practically be
cross-compiled, so each OS needs a native machine, VM or CI runner:

- [ ] **Linux (Phase 9):** Ubuntu 24.04+ machine or VM with Rust, Node.js,
      and `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
      (Tauri) plus `libasound2-dev libfreetype-dev libfontconfig1-dev`
      (JUCE)
- [ ] **Windows (Phase 10):** Windows 11 machine or VM with Rust (MSVC
      toolchain), Node.js, Visual Studio 2022 Build Tools (C++ workload),
      and WebView2 (preinstalled on Windows 11)

## 4. Decisions

All decided 2026-09-25 except the two legal checks flagged below, which are
release gates (§8.1) rather than engineering blockers. The optional
features in #6 were proposed and accepted on 2026-09-27. #7 (2026-09-27)
sets priorities rather than open questions.

1. **Licenses: JUCE commercial; ano-mp's own code MIT (decided; MIT since
   2026-10-03, closed source before).** JUCE 8+ is AGPLv3 or commercial. We
   use the commercial license, starting on the free "Starter" tier and
   moving to a paid tier before revenue passes its cap (check the current
   JUCE 9 tiers then). This keeps the released app off the AGPL and the App
   Store open to us. The code written here is MIT (`LICENSE`); the
   components it is built on keep their own licences
   (`THIRD_PARTY_NOTICES`), so anyone else distributing a build needs their
   own JUCE licence or must ship it under the AGPLv3. FFmpeg is used under
   the LGPL (see #3), which the app satisfies by shipping FFmpeg as
   replaceable shared libraries plus a license notice and source offer. That
   is standard on desktop.
   - **Open point: FFmpeg LGPL on the App Store.** Dynamic `.xcframework`s
     keep the libraries replaceable, but Apple's usage terms are the usual
     objection. Get a legal opinion before the first App Store submission.
2. **Frontend framework: Svelte 5 + TypeScript + Vite (decided).** Small
   bundle, and runes handle high-frequency state (position, levels) without
   re-render tuning. The visualizer draws on a canvas outside the framework.
3. **Decoder: FFmpeg for all formats (decided).**
   - Formats: MP3, FLAC, WAV, AIFF, Ogg Vorbis, Opus, AAC/M4A, ALAC and WMA
     on every platform. More (APE, WavPack, DSD) are a configure flag away.
   - Build: our own pinned, **LGPL, audio-only, shared** FFmpeg build (no
     `--enable-gpl`/`--enable-nonfree`, only the needed demuxers and
     decoders, plus libswresample; no video or network, and only the
     encoders and muxers recording writes, X6, with LAME for MP3 linked
     into libavcodec, LGPL too). This adds a few MB per platform. We don't
     use distro or Homebrew FFmpeg, whose builds vary (some strip AAC,
     Homebrew's is GPL).
   - **AAC: FFmpeg's decoder everywhere (decided), pending a licensing
     opinion before release.** Apple's and Microsoft's own decoders are
     covered by their vendors' licenses; FFmpeg's AAC decoder is not. Many
     early AAC patents have expired, but Via LA's pool still lists active
     ones in some countries.
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
   X1–X9 (Phase 7b): themes, real-time effects, ten uncommon
   visualizations (X3, five more added 2026-10-04), recommendations
   from inside and outside the library, recording what is playing to a
   file (X6, added 2026-10-04), similar artists on artist pages, an
   effects workbench that plays one file through the effects and
   records it (X8, added 2026-10-04), and search that finds the app's
   features (X9, added 2026-10-08). Low priority: none is needed for a
   release, and none starts before Phase 7's exit (all nine were built
   ahead of it, 2026-10-03 to 2026-10-08, at the owner's request).
   They come before the cross-platform work (Phases 8–10), so the ports
   carry them. #6's rules apply; X2, X5 and X6 are off by default (they
   change what is heard, go online and write large files).
9. **More features: proposed 2026-10-08, none accepted yet.** N1–N40
   (Phase 7d) came from a review of the feature set for what users would
   enjoy: common features the app still lacks (folder view, internet
   radio, podcasts, AirPlay, custom shortcuts, importing from the Music
   app) and uncommon ones (tempo and key analysis, ABX comparison, a
   music quiz, karaoke, a year in review). The owner picks which to
   build; each picked item gets a priority on #7's scale. #6's rules
   apply, and N10 (library corrections) is the answer to tag editing
   that keeps them.

Release-only decisions (distribution channels, packaging, signing) are in
§8.1.

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

Each phase ends with a demonstrable result and green tests. A feature or
setting the user can reach is done only when the user guide covers it (Phase
7c D1; `check-user-guide.py` and `guide.rs`'s test check it). Platform
order: macOS (Phases 0–7), then iOS/iPadOS (Phase 8), then Linux (Phase 9),
then Windows (Phase 10). Phase 7b (themes, effects, visualizations,
recommendations, recording, similar artists, the effects workbench and
search that finds features, P3) comes after Phase 7 (but was built ahead of
its exit, at the owner's request) and before the ports, so those features
are settled on macOS first. Phase 7c (documentation) runs alongside: its
user guide (D1) is needed for the first release, and the developer guide and
dictionary of classes follow. Phase 7d proposes more features (N1–N40) for
the owner to choose from; those chosen follow Phase 7b's placement. Phase 7e
(Jellyfin servers) follows Phase 7's exit and comes before the ports, and
builds the library sources and audio cache that Phase 11 reuses. Phase 7f
(Plex servers) follows Phase 7e, reusing what it builds, and also comes
before the ports. Xcode is not needed until Phase 8. Phase 11 (Bandcamp
streaming) is a feature, not a platform, and depends on Bandcamp's
permission. Its engineering starts after Phase 7. The phases cover making
the app work on each platform; packaging, signing and shipping it are all in
§8.

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
Design notes, the steps as built, tests and known limits:
[docs/design/phase-0-toolchain.md](docs/design/phase-0-toolchain.md).

- [x] **Exit:** a tone plays from a Tauri app on macOS. Confirmed by ear
  2026-09-25. **Phase 0 complete.**

### Phase 1 — Playback engine (C++ core)
Design notes, the steps as built, tests and known limits:
[docs/design/phase-1-playback-engine.md](docs/design/phase-1-playback-engine.md).

- [x] **Exit:** a library of MP3/FLAC files plays through the app with
  working seek and volume, and a gapless album plays without clicks at track
  boundaries. Confirmed by ear 2026-09-26 (MP3 album with continuous tracks,
  queued one at a time through the dev UI). **Phase 1 complete.**

### Phase 2 — Metadata and library
**Phase 2 complete.** Design notes, the steps as built, tests and known
limits:
[docs/design/phase-2-metadata-library.md](docs/design/phase-2-metadata-library.md).

### Phase 3 — Frontend: core player UI
**Phase 3 complete.** Design notes, the steps as built, tests and known
limits:
[docs/design/phase-3-player-ui.md](docs/design/phase-3-player-ui.md).

### Phase 4 — Online metadata services
Goal: enrich the library with album details, artist information and cover
art from online sources and local files, with **more than one source for
each kind of data and the user choosing between them**: an ordered list of
sources per kind (the first with a result wins), and a per-album choice
that pins a specific source's match or picture. Every service can be turned
off, and the app works offline on what it has already fetched.

The sources considered and which ship, the design decisions, the steps as
built, tests and known limits:
[docs/design/phase-4-online-metadata.md](docs/design/phase-4-online-metadata.md).

- **Exit:** a library of tagged and untagged albums gets details and covers
  from MusicBrainz and the Cover Art Archive; the user can reorder or turn
  off sources, pick another source's cover or match for an album, and the
  app behaves the same with the network off, showing what it cached.

### Phase 5 — Visualization
Design notes, the steps as built, tests and known limits:
[docs/design/phase-5-visualization.md](docs/design/phase-5-visualization.md).

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
Design notes, the steps as built, tests and known limits:
[docs/design/phase-6-settings.md](docs/design/phase-6-settings.md).

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
Design notes, the steps as built, tests and known limits:
[docs/design/phase-6b-optional-features.md](docs/design/phase-6b-optional-features.md).

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
Design notes, the steps as built, tests and known limits:
[docs/design/phase-6c-expected-features.md](docs/design/phase-6c-expected-features.md).

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

**Order of work** (set 2026-10-02). Each step's full record (what was done,
the suites after it, the owner's answers that shaped it, what was found) is
in [docs/design/phase-7-hardening.md](docs/design/phase-7-hardening.md).

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

- Allowed, not fixed (each in its item's record): 44.1 kHz files resampled
  at 30× real time (H18; the budget is 20×); browsing by year or genre at
  56–73 ms (H18; 100 ms budget); restore's 290 ms of track details (H16);
  `queue/model.rs` not split (H19), having changed little.
- Next: the owner's items above, in order; then Parts 0–3 of the signed
  release as decisions arrive. Engineering left that needs no decision: H19
  as modules are touched, M2's `sync-ffmpeg-frameworks.py`, H13's P2 tests
  and H8's edition move.

**Hardening items** (from the 2026-09-27 review; priorities as in
§4.7). CI runs each check through `check-all.py` (M4), so a local run
matches it.

| # | Item | Area | Size | Priority | Status |
|---|---|---|---|---|---|
| H1 | Content Security Policy | security | S | P1 | Done 2026-10-02 |
| H2 | Keep developer commands and `/dev` out of release builds | security | S | P1 | Done 2026-10-02 |
| H3 | Narrow the URL opener and per-window command permissions | security | S | P1 | Done (permissions 2026-09-27, opener 2026-10-02) |
| H4 | Contain folder pictures on unsandboxed platforms | security | S | P2, before Phase 9 | Done 2026-10-09 |
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
X1–X9 is needed for a release, and none starts before Phase 7's exit (all
nine were built ahead of it, 2026-10-03 to 2026-10-08, at the owner's
request). They come before the ports (Phases 8–10), so each is built and
settled once on macOS and then ported with the rest of the app. Each is an
optional feature under §4 #6's rules: a switch in `FeatureSettings` that it
checks where it acts, off by default if it changes what is heard or goes
online.
Each item's design, steps as built, the owner's answers, checks and known
limits: [docs/design/phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md).

| # | Feature | Touches | Size |
|---|---|---|---|
| X1 | Themes: the user changes the look and feel | UI, settings | M |
| X2 | Real-time effects on what is playing (reverb, chorus, spectral freeze) | core DSP, C API, Rust, settings, UI | L |
| X3 | Ten more visualizations, all uncommon in music players (two of them combinations) | core (analysis), UI | M–L |
| X4 | Recommendations from the library | Rust, UI | M |
| X5 | Recommendations from outside the library | Rust (metadata), UI | M (after X4) |
| X6 | Record what is playing to a file (WAV, AIFF, FLAC, ALAC, AAC, MP3) | core, C API, Rust, settings, UI | M |
| X7 | Similar artists on artist pages | Rust, UI | S (after X4; X5 for outside ones) |
| X8 | Effects workbench: pick a file, play it through the effects, record the result | UI, Rust (core and C API only for offline rendering) | M |
| X9 | Find a feature: search finds settings, views and switches and opens them | UI | S–M |

- [x] **X1 Themes.** Built 2026-10-03; to check in the app (the phase's
  exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x1-themes).
- [x] **X2 Effects.** Built 2026-10-03; to check in the app (the phase's
  exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x2-effects).
- [x] **X3 Ten visualizations.** Built 2026-10-04; to check in the app
  (the phase's exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x3-ten-visualizations).
- [x] **X4 Recommendations from the library.** Built 2026-10-04; to check
  in the app (the phase's exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x4-recommendations-from-the-library).
  - [x] **To do** (found 2026-10-09, writing the smoke test's pass over
    Phase 7b): "More Like This" is on an album's menu in the library's
    views and in search results only (`albumFeatureItems`). Add it to
    album cards' menu (`AlbumCards.svelte`: Home, Favourites, album
    pages' rows, the More Like This dialog) and the artist page's
    albums (`ArtistPage.svelte`'s `albumMenu`), as the user guide says;
    then the smoke test may right-click any album. Done 2026-10-09: both
    menus add `albumFeatureItems`; a menu opened in a dialog moves into it,
    or the modal left it inert (`ContextMenu.svelte`); and a dialog opened
    from another's menu no longer closes as the old one goes
    (`Dialog.svelte`).
- [x] **X5 Recommendations from outside the library.** Built 2026-10-04;
  to check in the app (the phase's exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x5-recommendations-from-outside-the-library).
- [x] **X6 Record the output to a file.** Built 2026-10-04; to check in
  the app (the phase's exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x6-record-the-output-to-a-file).
- [x] **X7 Similar artists on artist pages.** Built 2026-10-04; to check
  in the app (the phase's exit). Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x7-similar-artists-on-artist-pages).
- [x] **X8 Effects workbench.** Built 2026-10-04; checked in the app
  2026-10-08. Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x8-effects-workbench).
- [x] **X9 Find a feature.** Built 2026-10-08; checked in the app
  2026-10-08. Record:
  [phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md#x9-find-a-feature).
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
  without X5; X8's exit (passed 2026-10-08); X9's exit (passed 2026-10-08).

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
| D4 | Documentation upkeep: stale text, unchecked links, gaps | everyone who reads the repo | P2 (P1 for the smoke test's coverage) |

They live in the repo as Markdown, beside the code they describe, so a
change and its documentation land in one commit and `check-docs.py`
checks their paths and links: D1 in `docs/user-guide/` (one page per
chapter, with an index), D2 in `docs/developer-guide/` (one page per
chapter, with an index), D3 in `docs/class-dictionary/` (one page per
layer, with a README holding the indexes). They describe the app as it is,
not its history: the reasons and the record of how each part was built stay
in `docs/design/`, and D2 links to them rather than repeating them.

- [ ] **D1 User guide.** Written 2026-10-04: `docs/user-guide/`, an
  index, the fourteen chapters below and four appendices, with the
  settings reference generated by `app/src-tauri/src/guide.rs` (its
  `cargo test` fails while the file is stale or a setting in the defaults
  has no row) and `scripts/check-user-guide.py` in `check-all.py`'s quick
  checks (every sidebar item, built-in library view, Settings section,
  `FeatureSettings` field, page menu item and `error.*` message is in the
  guide). `CLAUDE.md` and §5 make the guide part of done. Open:
  - the screenshots, marked `<!-- Screenshot: … -->` where each goes,
    taken last against the release build with a demo library;
  - ~~Help › "ano-mp Help"~~: done 2026-10-04, bundled (the owner's
    decision): `app/src/routes/help` renders the guide's Markdown,
    compiled into the frontend, in a window of its own
    (`shell/help.rs`, `capabilities/help.json`: the settings and https
    links only). `lib/guide.ts` parses the guide's subset of Markdown
    into a tree drawn with ordinary elements, never as HTML, and
    `tests/guide.test.mjs` parses every page and checks each link and
    anchor resolves. No menu shortcut: AppKit matches ⇧⌘/ unreliably;
  - the exit check with someone new to the app, and the owner's read;
  - found while writing: the Finder offers ano-mp for .wv, .ape and
    .alac files (`tauri.conf.json`'s `fileAssociations`), which no
    decoder or registered extension plays; and a file with more than two
    channels plays only its first two.

  How the app is meant to be used, in plain
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
- [ ] **D2 Developer guide.** Written 2026-10-04: `docs/developer-guide/`,
  an index and the thirteen chapters below, with Mermaid diagrams of the
  layers, the threads, a track's journey, the effect chain, the queue,
  the metadata worker and each worked sequence (play, hand-off and
  crossfade, a scan, a search, a MusicBrainz lookup, a setting,
  quitting). The repository map names every top-level folder and every
  module of `core/src`, `effects/src`, `app/src-tauri/src` and `app/src`
  (263 today), checked by `scripts/check-developer-guide.py` in
  `check-all.py`'s quick checks (a script of its own rather than part of
  `check-user-guide.py`, since it reads the tree, not the app's UI);
  `check-docs.py` checks its paths and links. `CLAUDE.md` points to it and
  makes keeping the map current part of each change. Open:
  - the exit check: a developer new to the repo finds three real bugs'
    code from the guide alone, and adds a setting by its recipe;
  - found while writing, both looked into on 2026-10-04:
    - *the log level can't be raised in a release build* (debug lines
      need a debug build), which limited what a user's log can show:
      **decided and done** 2026-10-04. Of three designs (a switch writing
      debug lines into `ano-mp.log` until the next launch; the same kept
      across launches for up to 24 hours; debug lines in a file of their
      own), the owner chose the third. **Detailed logging** in Settings ›
      About writes every line, debug included and redacted as ever, to
      `ano-mp-detailed.log` (one old copy past 8 MB) until switched off or
      the app quits; switching it off deletes the file, and so does the
      next launch, so it holds one session at most but outlives a quit.
      `ano-mp.log` never takes debug lines in a release build. "Copy
      diagnostics" says only whether it is on (a "Logging" section) and
      still leaves debug lines out. It is a switch for this run, not a
      setting, so it has no stored value, `validate` rule or settings
      reference row: commands `logging_detailed` and
      `logging_set_detailed`.
    - *the workers at quit*: **done** 2026-10-04. Tauri calls
      `process::exit` as soon as `RunEvent::Exit` is handled. Harm found:
      the history thread was never told to stop, the play in progress
      never got its final listened time, and a play queued behind a
      ListenBrainz request (up to 30 s) was lost; the analysis could
      still be inside the core while `exit` ran its static destructors;
      and `analysis::store` wrote a track and its album's loudness as two
      statements, so a cut between them left the album's loudness stale
      for good. Not harmful: the metadata worker (single-statement
      writes, pictures renamed into place, unfinished work queued again
      at launch), the update checker (writes nothing), the remote, the
      library watcher and scans (batches are transactions, the launch
      rescans). Now quitting waits, at most `quitting::WAIT` (1 s) for
      all, for the history, analysis and metadata threads, with playback
      paused first and the engine still dropped last; the history thread
      writes everything waiting before each ListenBrainz request, and if
      it is still in one at the deadline, the main thread writes what it
      left (a listened time names its play by track and start, not by a
      row only that thread knew); `store` is one transaction. The tests
      found that a user's request under way when the metadata worker
      stopped was never answered (its next step was queued after the
      stop): the worker now drops what is queued as it stops. Tests
      stop each worker mid-job.
    - *a crash at quit* (found with the above): any thread still in the
      core when `exit` ran the C++ static destructors (a scan or a cover
      reading tags, Get Info, a file opening) could run over the core's
      format registry or TagLib's and JUCE's singletons as they were
      destroyed. Making scans stoppable wouldn't cover the other threads,
      or TagLib's statics, so instead: **done** 2026-10-04,
      `quitting::skip_static_destructors`, last on `RunEvent::Exit`,
      registers an `atexit` handler that runs before those destructors
      (handlers run in reverse order of registration), flushes the log
      and calls `_exit` with the exit code asked for. Tauri's cleanup and a
      restart's relaunch come before `exit`, so they still run. A test
      runs the handler in a child process and checks that what was
      registered before it doesn't run.

  How the code is organised and how it works,
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
- [x] **D3 Dictionary of classes.** Written 2026-10-04; exit check
  passed 2026-10-04. Record:
  [phase-7c-documentation.md](docs/design/phase-7c-documentation.md#d3-dictionary-of-classes).
- [ ] **D4 Documentation upkeep.** Added 2026-10-08 at the owner's
  request, from a review of every document in the repo against the tree
  (the checks all pass: 57 docs, 273 modules in the map, 714 dictionary
  entries, §2's counts). What the checks can't see:
  - **Stale text** (small, do first). **Done 2026-10-09:** each finding
    below fixed, §7 replaced by a link to the repository map; the
    **Order** line was already fixed. What the review found:
    - this plan's status line says "as of 2026-10-04" but records
      2026-10-08's work; `CLAUDE.md`'s "Project" section says "Phases
      0–6c are built; Phase 7 … is under way" and names neither 7b nor
      7c;
    - the developer guide's README cites "X1–X7" (X8 and X9 are built),
      and this phase's **Order** says "X7 when it lands";
    - `README.md`'s status paragraph lists none of Phase 7b's features
      (themes, effects, the workbench, recording, recommendations,
      similar artists, finding a feature), and its repository layout's
      `docs/` line leaves out the class dictionary;
    - §7 "Proposed repo layout" describes a layout that now exists, and
      repeats `README.md`'s and the developer guide's repository map:
      replace it with a link to `03-repository-map.md`.
  - **Phase 7b's records to `docs/design/`.** **Done 2026-10-09:**
    X1–X9's records moved word for word to
    [docs/design/phase-7b-personalisation.md](docs/design/phase-7b-personalisation.md)
    and D3's to
    [docs/design/phase-7c-documentation.md](docs/design/phase-7c-documentation.md),
    each item keeping its status line and a link here; X4's open to-do
    stays here. What the review found: X1–X9 are built, but their
    designs, steps as built and known limits are still here (about 800
    lines, over a third of this plan). Move them word for word into
    docs/design/phase-7b-personalisation.md (one section per X item),
    leaving the table, each item's status line and a link, as Phases
    0–6c and Phase 7's H items were; D3's record goes to
    docs/design/phase-7c-documentation.md the same way. `CLAUDE.md`'s
    rule on moving finished work names whole phases and Phase 7's steps
    and H items only: add Phase 7b's X items and Phase 7c's D items
    (done 2026-10-09, with the stale text).
  - **The release smoke test** (`docs/release-smoke-test.md`) checks
    nothing of Phase 7b: add a short pass over themes, an effect on and
    off (bit-identical when off is the tests' job; here, by ear), the
    workbench, a recording in each container, a recommendation, similar
    artists and a feature found from the search box. P1, since the
    features ship in the first release. **Done 2026-10-09:** its
    section 5 checks a theme, an effect by ear, recording in all six
    formats and after a relaunch, the workbench, two new
    visualizations, More Like This, outside recommendations, similar
    artists and finding a feature.
  - **Checks that miss things:**
    - `check-docs.py` drops a link's `#anchor` (`link_problem`), so a
      renamed heading breaks links silently. Only the user guide's
      (`tests/guide.test.mjs`) and the dictionary's links into the
      developer guide (`class-dictionary.py`) are checked today. Resolve
      every relative link's anchor against the target's headings, as
      GitHub slugs them, with a test. **Done 2026-10-09:** `check-docs.py`
      checks each `#anchor` into a Markdown file (a folder's means its
      README.md, a bare `#x` the doc's own) against its headings' slugs,
      repeats numbered as GitHub does (`heading_anchors`); all 1,967
      anchored links resolved, so no doc changed;
    - the keyboard shortcuts appendix (`appendix-a-shortcuts.md`) wasn't
      compared with anything: the menus' accelerators (`shell/`) and the
      frontend's key handlers could change without it. Add the comparison to
      `check-user-guide.py`, or generate the appendix from one list that the
      menus, the shortcuts sheet (F6) and the guide share. **Done
      2026-10-09:** compared, not generated, since the appendix carries a
      Menu column, notes, prose, a "no shortcut" list and the Help window's
      keys that a generator would have to rebuild, and the menu's
      accelerators live in `install`'s builder chains. `check-user-guide.py`
      now fails when a menu accelerator (`PAGE_ITEMS`' or a builder's
      `.accelerator(…)`) or a pair of `menu::shortcuts()` isn't in the
      appendix's first table with the same keys under the item's name, when
      that table lists a shortcut or item the menus don't have, when its "no
      shortcut" list misses a `PAGE_ITEMS` item without one or names one
      with one, and when its other tables and `ShortcutsDialog.svelte`'s
      `PAGE` differ, row for row, either way. Keys are written as the
      sheet's `keys()` writes them on macOS; a key its `names` map renames
      that the script doesn't know fails too. Seven tests, and the real
      tree's. It also catches a menu accelerator missing from `shortcuts()`
      (the sheet). An inventory of the keydown handlers found keys neither
      listed: ↑ and ↓ alone, Page Up, Page Down, Home and End in lists
      (`VirtualList`), 0–5 and the arrows on a track's stars (`Stars`), the
      context menu's keys (arrows, Home, End, Tab, → and ←, type-ahead), Esc
      closing a dialog or popover, Return and Esc in a playlist's name, Esc
      in **Filter artists** and Space in the mini player. ↑/↓ and 0–5 are
      rows of the sheet and the appendix now (three `en.json` messages), the
      rest the appendix's prose, and chapter 4's ratings name the stars'
      keys. Known limit: the handlers themselves aren't read, since telling
      a shortcut from a field's or a control's own keys would mean scraping
      each component; a key added to a handler without a `PAGE` row still
      goes unnoticed, and the appendix's prose and chapters 3 and 8's key
      tables aren't checked;
    - a Mermaid diagram with a syntax error shows as an error box on GitHub,
      and no check parses them. Parse every ```` ```mermaid ```` block in
      `check-all.py` with a pinned `@mermaid-js/mermaid-cli` run through
      npm, as Prettier is (a dev dependency in `app/package.json`).
      **Checked by hand 2026-10-09:** all 16 diagrams (11 flowcharts, 5
      sequence diagrams, all in the developer guide) parse with Mermaid
      11.4.1, and the sequence messages' generics (`Vec<TrackInfo>`,
      `Promise<Folder>`) are drawn as plain text, so no doc changed.
      mermaid-cli's headless Chromium couldn't start in the sandbox, so none
      was rendered. **Done 2026-10-09:** `scripts/check-mermaid.py`, in
      `check-all.py`'s quick checks, finds every ```` ```mermaid ```` block
      in the repo's Markdown (indented ones too) and parses each with
      `mermaid.parse` under jsdom (Mermaid 11.17.2 and jsdom 30.1.0, dev
      dependencies pinned in `app/package.json`); parsing needs no browser,
      so mermaid-cli wasn't needed. Mermaid sanitises labels with DOMPurify,
      which needs jsdom's window. All 16 diagrams parse; a diagram that
      parses but draws badly still isn't caught.
  - **New pages:**
    - *an index of plan items*: the O, F, H, X and D numbers are cited
      everywhere (`CLAUDE.md`, the developer guide, the dictionary, commit
      messages), but defined only across this plan and the design notes. A
      generated docs/plan-items.md (number, name, status, where it is
      documented), with `--check` in `check-all.py`'s quick checks, lets a
      reader look one up in seconds. **Done 2026-10-09:**
      `scripts/plan-items.py` writes `docs/plan-items.md` (`--check` in
      `check-all.py`'s quick checks): a table per series in plan order, O,
      F, H, X, D, N and M (§9.2's steps), 121 items in all, each with its
      name, status, the section of this plan that defines it and the heading
      in `docs/design/` holding its record, linked so that `check-docs.py`
      checks the anchors. The status is the item's table's status column (O,
      H), else its item line (X, D, M; "Proposal" for an unticked N), else,
      for F, whose table has none, its record's **Decision** line. A number
      defined twice, an item missing its series' table row or line, a done
      item with no record, an item with no status, and a table status that
      disagrees with the item's tick each fail it. Twelve tests, and the
      real tree's. Found: every done item has a record; those without one
      are open (H19, D1, D2, D4) or proposals (N1–N40). No status disagrees
      within this plan, but M6's record (the §9.2 section of
      `docs/design/phase-7-hardening.md`, moved word for word) is still
      unticked, "Done 2026-10-02 except `release.py`'s updater manifest",
      which this plan has since found unneeded; a record is history, so the
      script doesn't compare its tick, and the record stays as moved. F's
      statuses are only in the design notes, and M's item lines aren't bold
      as the others' are. Known limit: O, F, H and M items' records are list
      items under one heading, so their links reach the section, not the
      item. Fixed in passing: the recipes chapter's "Two steps" (it listed
      three);
    - *a docs/README.md* saying what each document under `docs/` is for, who
      reads it and whether it is current or a record (the Step 4 checklist
      becomes a record once Step 4 is done). **Done 2026-10-10:**
      [docs/README.md](docs/README.md), its entries grouped by reader
      (people who use the app, developers, the owner at release, reviewers)
      after a lead-in pointing to `README.md`, `CLAUDE.md`, this plan and
      the index of plan items for what they cover. Each entry says what the
      document is for, who reads it and whether it is current, naming the
      check that keeps it so, or a record. The Step 4 checklist is current
      until the owner's checks are done; the release decisions are current
      while decisions are open. `README.md`'s layout, `CLAUDE.md` and the
      repository map now point to it, and `CLAUDE.md` asks for an entry for
      each new document. Found: the design records aren't quite word for
      word (Phase 7b's record corrects three slips, and says so), and Phase
      7's and 7c's records still grow as items finish; the bundle check,
      release decisions and smoke test have no check beyond their links;
    - *a porting chapter in the developer guide* ahead of Phases 8–10:
      what is macOS-only today (the `_apple` platform files and their
      `_none` fallbacks, the sandbox and folder access, media controls,
      `tauri.conf.json`'s macOS settings, `build-ffmpeg.sh`'s platform
      folders, the macOS-only CI), each seam's interface, and a recipe
      for adding a platform's implementation.
      **Done 2026-10-10:**
      [docs/developer-guide/14-porting.md](docs/developer-guide/14-porting.md),
      linked from the guide's README, its overview, chapter 4's platform
      code, the recipes chapter (which points to its recipe) and
      `docs/README.md`. Inventoried from the tree: the core's six seams
      (each header, `_apple` file and fallback, chosen by
      `core/CMakeLists.txt`'s `if(APPLE)`; `FolderAccess`'s fallback is
      `FolderAccess_unsandboxed.cpp`, not a `_none` file), every platform
      `cfg` in the Rust crate, the keychain, `shell/`, `tauri.conf.json`,
      the entitlements and capabilities, the build and release scripts, CI
      and the frontend's keys. A table per layer gives what iOS, Linux and
      Windows each need, with what the fallbacks lose as a user would notice
      it and the recipe (also in the recipes chapter, by link). One Mermaid
      diagram, 17 in all. Found: `audit.yml` already runs on Linux, so three
      of the four workflows are macOS-only; iOS needs an audio-session seam
      the core lacks; Windows keeps OneDrive placeholders `FileStatus` would
      have to see. Portability bugs outside the seams, none affecting macOS,
      all fixed the same day at the owner's request without changing what
      macOS does: Windows paths escaped the log's path scrubbing
      (`logging::path_at` now knows drive, share and verbatim paths, H9);
      `add_folder` stored `canonicalize`'s verbatim `\\?\` path on Windows
      and `open_folder_of` compared paths exactly (now `library/paths.rs`:
      `canonical` and `within`, which ignores case on Windows, used by
      `add_folder`, `relocate_folder`, the overlap check, `open_folder_of`,
      `external::library_track` and `playlists::resolve`); an M3U8 entry
      with a drive letter was found on Windows only by its tail
      (`playlists::entry_path`); the Help window's Back key was ⌘[ only (now
      Alt+← off macOS and iOS, `lib/keys.ts`); `access::in_trash` missed a
      Linux volume's `.Trash-<uid>` and Windows' `$Recycle.Bin`; `shell/`
      used Tauri's desktop-only menu and tray unconditionally (now
      `#[cfg(desktop)]`, with `notify-rust` a desktop-only dependency and
      the items only menus call allowed to be unused elsewhere; both
      branches pass Clippy, by flipping the cfg on macOS); messages named
      the Mac, the Finder, iCloud, the keychain or ⌘ (now `<key>@windows`,
      `@linux` and `@ios` forms that `t` prefers by `lib/platform.ts`,
      shortcuts as a `{keys}` parameter, and `i18n.test.mjs` requiring the
      forms); and the core's asynchronous C API tests were skipped off macOS
      (now pumping JUCE's own queue there). Tests: 5 Rust, 3 frontend. The
      Linux, Windows and iOS branches that only those systems compile (the
      core tests' pump, the iOS build) wait for Phases 8–10. No hard-coded
      `/` was found: paths are joined by component. The repository map
      gained `library/paths.rs`, `lib/platform.ts` and `lib/keys.ts`, and
      the dictionary `Platform`.
  - **`README.md`'s troubleshooting** repeats the user guide's chapter
    14 and the developer guide's chapter 10 in part: keep the build
    failures, link to those two for the rest.
  - **Owner decisions** (§8.1, before the repo is public): whether
    `README.md` keeps its opening line about testing Claude Code; a
    SECURITY.md (how to report a vulnerability, with a contact the
    owner chooses; the LAN remote and the parsers make it worth having)
    and a CONTRIBUTING.md (pointing to the developer guide,
    `check-all.py` and the pre-commit hook), or neither while the
    project takes no outside contributions.
  - **Exit:** every item above done or decided against; the new checks
    in `check-all.py` with their tests, each against the real tree.
- **Order:** D2's overview and repository map first (they shape the rest
  and help anyone reviewing §8.1's decisions), then D1 (needed for the
  release), then the rest of D2 and D3 together, since writing the
  dictionary exposes what D2 misses. Screenshots are taken last, against
  the release build's UI. Phase 7b's features are documented as each is
  built. D4's stale text and smoke-test pass come before the release;
  the rest of D4 before Phase 8 starts.
- **Decide first:** ~~whether the user guide is bundled or hosted~~
  (bundled, the owner's decision of 2026-10-04);
  the screenshot library (a demo library of free-licensed music, so
  covers and names can be published); and whether D1 is translated
  when the UI is (F19), or English only at first.
- **Exit (to check)**: D1–D4's exits, `check-all.py` green with the new
  checks, and the owner reading D1 end to end on the release build.

### Phase 7d — More features users will enjoy (proposals)
Added 2026-10-08 at the owner's request: a review of the feature set
(O1–O19, F1–F21, X1–X9 and Phase 11) for what is still missing. It
proposes forty features, some that users expect of any player ("common")
and some that few players have but users will enjoy ("uncommon"). **None
is accepted yet**: the owner picks which to build, and each picked item
gets a priority on §4 #7's scale and, once built, a **Decision** line, as
O1–O19 did. Until then they are P3, after Phase 7's exit; the ones the
owner wants on every platform come before the ports (Phases 8–10), as
Phase 7b's did.

§4 #6's rules apply to every item: a switch in `FeatureSettings`, checked
where it acts, off by default if it costs a lot, changes what is heard,
goes online or listens on the network; the user's files are never
written (N10 and N38 say how they keep to this); anything online goes
through `http::Client` and needs a row in Phase 4's sources table first;
anything that decodes library files in the background checks for cloud
placeholders and opens folders through `library::access` first; platform
code goes behind a small core interface.

| # | Feature | Kind | Touches | Size |
|---|---|---|---|---|
| | **Playing and the queue** | | | |
| N1 | Spoken-word mode: speed per book, skip 15/30 s, smart rewind, "Continue listening" | common | core (speed range), Rust, UI | M |
| N2 | Better shuffle: shuffle albums, spread artists, weight by rating and time since played | common | Rust (queue) | S–M |
| N3 | Named queues the user switches between | uncommon | Rust (queue, store), migration, UI | M |
| N4 | Soft pause: fade out on pause, stop and skip, fade in on resume | common | core, settings | S |
| N5 | Balance, mono and swapped channels | common | core, settings, UI | S |
| N6 | Loudness compensation that follows the volume, and a night mode | uncommon | effects, settings, UI | M |
| N7 | Alarm: start music at a time, fading in; with the sleep timer, a bedtime pair | uncommon | Rust, UI | S–M |
| N8 | Beat-matched transitions between unrelated tracks (Auto DJ) | uncommon | core, Rust | L (after N13) |
| N9 | Preview on a second output (headphones) while the main one plays | uncommon | core, C API, Rust, UI | M–L |
| | **Library and browsing** | | | |
| N10 | Library corrections: fix titles, artists, genres and years without touching files | common | Rust, migration, UI | M |
| N11 | Genre clean-up: merged spellings and a genre tree | uncommon | Rust, UI | M |
| N12 | Folder view: browse the library as its folders | common | Rust (browse), UI | S |
| N13 | Tempo, key and energy from the audio: columns, filters, smart-playlist rules | uncommon | core (analysis), Rust, migration, UI | L |
| N14 | Personal labels and notes on tracks, albums and artists | uncommon | Rust, migration, UI | S–M |
| N15 | Prefer the best copy of a duplicated recording | uncommon | Rust, UI | S–M (after O4) |
| N16 | Discography with what the library lacks, and new releases by the user's artists | uncommon | Rust (metadata), UI | M |
| N17 | Credits: producers, composers and players, each linked to their tracks | uncommon | core (tags), Rust, migration, UI | M |
| N18 | Play a random album, optionally within a genre, decade or the unplayed | common | Rust (queue), UI | S |
| | **Discovery and fun** | | | |
| N19 | Listening statistics and a year in review, computed locally | common | Rust, UI | M (after O8) |
| N20 | Album of the day, played through | uncommon | Rust, UI | S |
| N21 | Music quiz: name the track from a short clip of the library | uncommon | Rust, UI | S–M |
| N22 | Blind comparison (ABX) of two versions of a recording | uncommon | core, Rust, UI | M |
| N23 | Karaoke mode: large synced lyrics, vocal reduction, key shift | uncommon | effects, UI | M (after O13) |
| N24 | Share a now-playing card as an image | uncommon | UI, Rust | S |
| N25 | Ambient display: a dim, slow full-screen Now Playing when idle | uncommon | UI | S |
| N26 | Desktop lyrics: a small floating window with the current line | uncommon | Rust (window), UI | S–M |
| | **Control and integration** | | | |
| N27 | Custom shortcuts, and global hotkeys that work from other apps | common | Rust, UI | S–M |
| N28 | Automation: a URL scheme, then AppleScript and Shortcuts actions | common | Rust, bundle config | M |
| N29 | AirPlay speakers from the player bar | common | core (platform), UI | M |
| N30 | Spoken announcements of the next track | uncommon | Rust or UI (speech), core (ducking) | S–M |
| | **Online (off by default)** | | | |
| N31 | Internet radio, with a station directory | common | core (stream input), Rust, migration, UI | L |
| N32 | Podcasts | common | Rust, migration, UI | L (after N31's input, with N1) |
| N33 | Online synced lyrics (LRCLIB) when a track has none | common | Rust (metadata), UI | S–M |
| N34 | Last.fm scrobbling | common | Rust (metadata), UI | S–M |
| | **The user's data** | | | |
| N35 | Import ratings, play counts and playlists from the Music app | common | Rust, UI | M |
| N36 | Import listening history from ListenBrainz or Last.fm exports | uncommon | Rust, UI | S |
| N37 | Automatic backups of the user's data | uncommon | Rust, UI | S |
| N38 | Copy a playlist to a device or folder, converting it, and keep it in sync | common | core (offline encode), Rust, UI | M–L |
| | **Sound quality** | | | |
| N39 | Exclusive (hog) mode, and "bit-perfect" in the signal path | uncommon | core (platform), settings, UI | M |
| N40 | Headphone correction: a parametric equaliser that imports AutoEq profiles | uncommon | effects, Rust, UI | M |

#### Playing and the queue

- [ ] **N1 Spoken-word mode.** Audiobooks, lectures and long mixes play
  differently from albums. An album or folder marked as spoken word (or
  detected from its genre, or from chapters, O5) gets: a speed of its own,
  remembered per book, from 0.75× to 2.5× at the same pitch (practice
  mode's stretcher, O12, beyond its 150%); skip back 15 s and forward 30 s
  on the player bar, the menu and the media keys' skip buttons; a smart
  rewind of a few seconds after a pause of more than a minute; "Mark as
  finished"; and "Continue listening" on Home, from F17's positions.
  Crossfade, segue shuffle and radio leave it out.
- [ ] **N2 Better shuffle.** Three choices beside today's shuffle:
  *shuffle albums* (whole albums in order, the albums in random order);
  *spread artists* (no artist twice within the next few tracks where the
  queue allows); and *weighted* (higher-rated and long-unplayed tracks
  come up more often, skipped ones less, using F3's ratings and O8's
  history). Segue groups (O3), works (O6) and "Never shuffle it" (O7) keep
  working. Tested in `queue/model.rs` against the fake engine, with fixed
  seeds.
- [ ] **N3 Named queues.** Several queues ("Work", "Running", "Dinner"),
  each with its own tracks, position, shuffle and repeat, switched from
  the queue panel; switching pauses one and resumes the other where it
  was. The saved queue (H16) gains a queue id; a migration moves today's
  rows into a default queue.
- [ ] **N4 Soft pause.** Pause, stop and a manual skip fade out over a
  set time (0.1–2 s, default 0.3 s), and resume fades in, so music never
  stops with a click or a jolt. A gain ramp in the engine after the
  effects; gapless hand-offs and crossfades are unchanged.
- [ ] **N5 Balance, mono and swapped channels.** For listeners who hear
  better in one ear, a single speaker, or speakers wired the wrong way
  round: a balance slider, a mono switch (summed at -3 dB) and a channel
  swap, after the equaliser and crossfeed. Settings › Playback; the signal
  path (O10) lists them. Accessibility as much as sound.
- [ ] **N6 Loudness compensation and night mode.** At low volumes ears
  lose bass and treble. Loudness compensation lifts them by an
  equal-loudness contour (ISO 226) that follows the volume slider, so
  quiet listening keeps its shape; it replaces the equaliser's fixed
  "Loudness" preset for those who want it. Night mode is a gentle
  compressor and limiter that keeps quiet passages audible and loud ones
  from waking anyone. Both are effects in `effects/` (bit-identical when
  off), off by default.
- [ ] **N7 Alarm.** Start a playlist, an album or library radio at a set
  time on chosen days, fading in from silence over a few minutes to a
  set volume; with the sleep timer (F13), a "bedtime and wake-up" pair.
  It works while the app is open and the Mac awake; **decide first**
  whether to keep the Mac awake for it (a power assertion while an alarm
  is due) or to say plainly that a sleeping Mac won't ring.
- [ ] **N8 Beat-matched transitions.** Between tracks that aren't from
  one album, an "Auto DJ" transition: the outgoing track's tempo glides to
  the incoming one's (the time-stretcher, O12), and the crossfade starts
  on a downbeat, using N13's tempo and beat grid; harmonic mixing prefers
  next tracks in a compatible key for radio. Off by default (it changes
  what is heard). Offline render tests in the style of
  `PlayerEngineTests.cpp`.
- [ ] **N9 Preview on a second output.** Audition a track on headphones
  (or any second device) while the main output keeps playing: DJs, party
  hosts and anyone choosing what plays next. A second, simple player on a
  second device, its own small transport in the track's menu. **Decide
  first**: two JUCE devices at once on macOS, and what the ports can do.

#### Library and browsing

- [ ] **N10 Library corrections.** Users want to fix a wrong title, artist,
  genre or year, but the app never writes their files (§4 #6). A
  corrections table overrides tag fields per track or album (title,
  artist, album artist, album, genre, year, track and disc number),
  applied wherever tags are read, shown with a mark and "Use the tag's
  value" to undo; a selection edits many at once. Corrections survive
  rescans and moves (F10), travel with export (F20), and export as a CSV
  a tag editor can apply. **Decide first**: whether writing tags into
  files is ever wanted (TagLib can), which would change §4 #6 and is the
  owner's decision.
- [ ] **N11 Genre clean-up.** Genre tags are messy ("Hip-Hop", "Hip Hop",
  "hiphop"; "Rock; Alternative"). Spellings are merged by a normalised key
  with an alias list the user can edit, and a genre tree (parent genres,
  from an offline copy of MusicBrainz's genre list) lets the user browse
  "Rock" and see its sub-genres' albums. Smart playlists and O18 use the
  merged genres. Nothing is written to files; the tree ships with the app.
- [ ] **N12 Folder view.** A sidebar item that browses the library as
  its folders on disk, for people who organise music by folder: play a
  folder, add it to the queue, show it in the Finder. Unreadable folders
  dim as elsewhere (H22).
- [ ] **N13 Tempo, key and energy.** The analysis worker (O1's reader,
  the cloud-placeholder rule) also computes tempo (BPM), musical key (with
  Camelot notation for DJs), energy and a "danceability" figure. They
  become columns, browse filters and smart-playlist rules ("120–130 BPM,
  high energy" for running), and radio can follow a tempo or key.
  Tags (BPM, INITIALKEY) are read first where present. Off by default
  (hours of CPU, like O1); X3's beat detector is a starting point.
- [ ] **N14 Personal labels and notes.** The user's own labels ("road
  trip", "rainy Sunday", "for Dad") on tracks, albums and artists, from
  the menu or by dragging onto a label in the sidebar; browsable, and a
  smart-playlist rule. A free-text note per album or artist (where it
  was bought, which pressing, a memory). Both kept in the library DB and
  exported with F20.
- [ ] **N15 Prefer the best copy.** The health report (O4) finds
  duplicates; this acts on them. For each recording held more than once,
  one copy is preferred (lossless over lossy, then the higher rate, then
  the one without errors, or the user's pick), and the others are hidden
  from lists, shuffle and radio, still reachable from Get Info. Plays,
  ratings and favourites merge onto the preferred copy.
- [ ] **N16 Discography and new releases.** An artist page lists the
  artist's official albums from MusicBrainz, marking those in the library
  and those it lacks (with links out only). "New from your artists" on
  Home checks, weekly, for releases by artists the user plays most.
  MusicBrainz is already a source; the weekly check is its own switch,
  off by default.
- [ ] **N17 Credits.** Producer, engineer, composer, lyricist, conductor
  and performers, from tags (COMPOSER, CONDUCTOR, the ID3 involved-people
  frames) and MusicBrainz's relationships, on album pages and in Get
  Info. Each name opens the library's tracks it is credited on. Fits
  classical works (O6) and jazz line-ups.
- [ ] **N18 Play a random album.** A menu item, shortcut and Home button
  that plays a random whole album, optionally within a genre, a decade,
  the unplayed or the long-unplayed. "Another one" replaces it.

#### Discovery and fun

- [ ] **N19 Listening statistics and a year in review.** From O8's local
  history: hours listened, top artists, albums, tracks and genres by
  week, month and year, a "listening clock" (hour of day by weekday),
  streaks, first listens and most-repeated days. In December, a year in
  review page with the year's highlights, saved as an image if the user
  asks (N24). Nothing goes online.
- [ ] **N20 Album of the day.** One album a day from the library on
  Home, chosen with a bias to the unplayed and long-unplayed, with
  "Listen through": the whole album in order, no shuffle, no crossfade.
  A gentle way into a large library.
- [ ] **N21 Music quiz.** "Name that tune" from the user's own library:
  a few seconds from a random point of a track, four titles (or artists,
  or years) to choose from, score and streaks, harder rounds with shorter
  clips. Picks favour played tracks so the answers are guessable. Later,
  players on phones through the LAN remote (O14; a change to `remote/`
  needs §8.1's security review).
- [ ] **N22 Blind comparison (ABX).** Compare two versions of a
  recording (two masterings, a lossless file and its MP3, a remaster and
  the original): both level-matched by O1's loudness, switched seamlessly
  at the same position, with a blind ABX test and its score. For the
  curious and the audiophile; pairs come from N15's duplicates or any two
  tracks the user picks.
- [ ] **N23 Karaoke mode.** Synced lyrics (O13) full screen in large
  type, the current line highlighted, with a vocal-reduction effect
  (centre-channel cancellation that keeps the bass) and a key shift
  (practice mode's pitch, O12). Vocal reduction is a new effect in
  `effects/`, off by default; true stem separation is out of scope.
- [ ] **N24 Share a now-playing card.** An image of the cover, title,
  artist and, optionally, a lyric line or the waveform (O2), in the
  theme's colours, copied to the clipboard or saved where the user
  chooses. Nothing goes online; drawn on a canvas.
- [ ] **N25 Ambient display.** After a few idle minutes in full screen
  (or from the View menu), Now Playing becomes a calm display: a slow
  gradient from the cover's colours, a large clock, title and lyrics,
  slowly drifting so nothing burns in. Honours reduced motion (F18).
- [ ] **N26 Desktop lyrics.** A small floating, click-through window
  that shows the current synced line over other apps, as many players in
  East Asia do; moved and sized by the user, hidden when the track has no
  synced lyrics. Its window gets only the commands it needs (H3).

#### Control and integration

- [ ] **N27 Custom shortcuts and global hotkeys.** Rebind any command in
  a Settings section, with conflicts flagged, and optional system-wide
  hotkeys (play/pause, next, love, volume) that work while another app is
  in front, through Tauri's global-shortcut plugin. The shortcuts sheet
  (F6) and the user guide's appendix A show the user's bindings.
- [ ] **N28 Automation.** A URL scheme (play, pause, next, play a
  playlist or album by name, search) so launchers, Stream Decks and
  scripts can drive the app; then AppleScript's standard media verbs and
  Shortcuts actions. **Decide first**: what a link from a web page may do
  (control playback only, never read or change library data), reviewed
  like the LAN remote.
- [ ] **N29 AirPlay speakers.** Choose AirPlay speakers and TVs from the
  player bar with macOS's route picker, beside the output device list,
  and compensate AirPlay's delay so lyrics and the visualizer stay in
  time. Platform code in the core, behind the device interface; the ports
  get their own (Phase 9 has none).
- [ ] **N30 Spoken announcements.** Say the next track's title and artist
  as it starts, in the system voice, the music ducked under it: for blind
  users, for workouts, and for a radio-DJ feel with library radio (O9).
  Choices: every track, only after a pause, or only in radio. Off by
  default.

#### Online (off by default)

Each needs a row in Phase 4's sources table (terms, rate limits, what is
stored) before it is built, and a line in the privacy policy (§8.1).

- [ ] **N31 Internet radio.** Play internet radio streams (Icecast,
  SHOUTcast, HLS) with a station directory (radio-browser.info, open
  data), favourite stations, the playing song's title from the stream's
  metadata, and recording (X6). FFmpeg is built without networking (§4
  #3), so Rust fetches the stream through `http::Client` and feeds the
  core through a stream input: the same seam Phase 11 needs, so this
  follows it.
- [ ] **N32 Podcasts.** Subscribe by RSS feed or OPML import, download
  episodes into the app's own container (never the user's folders), play
  them with N1's spoken-word controls, and delete played ones after a
  set time. After N31's stream input, for streaming episodes unplayed.
- [ ] **N33 Online synced lyrics.** When a track has no lyrics in its
  tags or an `.lrc` file, fetch synced lyrics from LRCLIB (open, no key),
  cache them, and show their source. O13 left online lyrics out pending
  a sources-table decision; this is that decision.
- [ ] **N34 Last.fm scrobbling.** Many users keep their history on
  Last.fm. Phase 4 excluded Last.fm as a metadata source (non-commercial
  terms); scrobbling the user's own plays to their own account is a
  separate terms question. **Owner decision**, with O8's ListenBrainz
  code as the model (a token in the keychain, a retry queue).

#### The user's data

- [ ] **N35 Import from the Music app.** Ratings, play counts, last
  played, loved tracks and playlists from the Music app's exported
  library file, matched to the library by path, then by tags and length.
  A preview shows what will match before anything changes. Closes the
  FAQ's "Ratings and play counts from the Music app aren't read".
- [ ] **N36 Import listening history.** Load a ListenBrainz or Last.fm
  export into O8's history, matched to the library's tracks, so
  statistics (N19), Top 20 (O19) and recommendations (X4) start full.
- [ ] **N37 Automatic backups.** F20's export on a schedule (weekly by
  default), keeping the last few, in the app's container or a folder the
  user chooses (a writable bookmark of its own, like recording's folder);
  restore from a list in Settings › Library.
- [ ] **N38 Copy to a device.** Send a playlist, smart playlist or
  selection to a folder (a USB stick for the car, a phone, an old player),
  converting to MP3 or AAC at a chosen quality, with tags and covers, and
  keep it in step later (copy the new, remove the gone). It writes new
  files only, in a folder the user picks, never the library's. Uses X6's
  encoders, fed by an offline decode off the main thread (the offline
  render X8 decided against; **decide first** whether it is built here).

#### Sound quality

- [ ] **N39 Exclusive mode.** macOS's hog mode keeps other apps' sounds
  out of the output and, with O10's sample-rate matching, lets the
  device play each file's samples unchanged; the signal path shows
  "bit-perfect" when nothing (volume, levelling, effects, resampling)
  touches them. **Decide first**: whether JUCE's CoreAudio device can
  take hog mode, and how it behaves when another app wants the device.
- [ ] **N40 Headphone correction.** A parametric equaliser (up to ten
  filters of any type, frequency, gain and Q) beside F15's ten bands,
  importing AutoEq profiles and Equalizer APO text files, one profile per
  headphone, switched with F15's headphone detection. A new effect in
  `effects/`, bit-identical when off.

- **Suggested order** (for the owner's choice): the small common ones
  first (N4, N5, N12, N18, N27), then the most asked-for (N10, N2, N35,
  N19, N1, N29), then the uncommon ones the owner likes best. The online
  items wait on their sources-table rows; N31 and N32 follow Phase 11's
  stream input.
- **Exit (per item)**: built under §4 #6's rules, covered by the user
  guide, the developer guide and the class dictionary in the same commit,
  and checked in the app as each item's exit says.

### Phase 7e — Jellyfin servers
Added 2026-10-10 at the owner's request. Goal: play the music on the user's
own **Jellyfin server** next to the local library, streamed on demand,
through the same browser, search, queue, gapless hand-off and Now Playing as
local files. Jellyfin is free, self-hosted software whose REST API any
client may use, so unlike Phase 11 nothing waits on a third party's
permission.

Scheduling: after Phase 7's exit and before the ports (Phases 8–10), as
Phase 7b's features were, so the design settles on macOS first. Built ahead
of Phase 7's exit at the owner's request (2026-10-11), as Phase 7b was. It
is built platform-neutral, so the ports carry it. It builds the
library-source kind, the audio cache and the download-first playback that
Phase 11's design describes; Phase 7f, Phase 11 and N31/N32 then reuse them.

**Where things stand** (to check in 7e.1):
- The API: `POST /Users/AuthenticateByName` (or Quick Connect, which needs
  no password typed into the app) returns an access token and user id; every
  request sends
  `Authorization: MediaBrowser Client=…, Device=…, DeviceId=…, Version=…, Token=…`.
  `/Items` (with `IncludeItemTypes=Audio`, `Recursive=true`, paging and
  `Fields`) lists the music; `/Audio/{id}/stream?static=true` serves the
  original file and `/Audio/{id}/universal` transcodes to a container the
  client names; `/Items/{id}/Images/Primary` serves art; `/Sessions/Playing`
  and its `Progress` and `Stopped` calls report plays; the server is found
  on the LAN by a UDP broadcast to port 7359.
- Jellyfin 10.9 and later are supported (`jellyfin::MIN_VERSION`, checked
  at sign-in; the user guide says the same); which calls each version has
  (lyrics, for one) is checked in 7e.1 against a real server.

**Design:**
- **A Jellyfin server is a library source.** A migration adds a `kind` to
  `folders` (`local`, `jellyfin`), as Phase 11 proposes, so tracks, albums,
  artists, browse, FTS search, smart playlists and the queue work unchanged.
  A Jellyfin track's `path` is its item id, never a URL; the server's
  address and user id are the folder's, in place of a bookmark. Everything
  that opens a track (`library::playback::track_play`, `queue/opening.rs`,
  the scanner, art, Get Info) branches on the kind; a local track goes
  through `access::open_folder` as now. Settle in 7e.2 whether 11.2 keeps
  this model or separate tables, since both phases share it.
- **Sync instead of scan.** "Refresh" pages through the server's music and
  upserts tracks with the server's metadata (title, artists, album artist,
  album, disc and track numbers, length, year, genres, the file's container,
  bit rate and sample rate, and MusicBrainz ids where the server has them,
  so Phase 4's links work without a lookup). Later syncs ask only for items
  changed since the last; items gone from the server are removed, as a
  rescan removes deleted files. The scanner's H22 rule holds: a sync that
  would remove all of a server's tracks, or most of them, keeps them and
  fails the source.
- **Rust fetches, the core decodes** (as Phase 11): FFmpeg stays built
  without networking (§4 #3) and the core never goes online. Rust downloads
  the file through `http::Client` (a streaming GET on the same `Transport`,
  sharing the offline backoff and the test fake) into the audio cache, and
  the engine opens the cached file like any other, so gapless hand-off,
  ReplayGain, effects and analysis work unchanged. The queue arms the next
  track ahead, so it downloads while the current one plays. A track opens
  through `Opening::Pending` (H11), with its own timeout for the download.
- **Original or transcoded.** The original file (`static=true`) by default;
  a transcode only when its format isn't one FFmpeg's build plays, or when
  the user sets a maximum bit rate for connections away from home (a
  setting, off by default). The cached copy is named by item id and the form
  fetched.
- **Audio cache** as Phase 11 describes it: in the app's container, kept
  apart from the image cache but built like it (SHA-256 names, `.part` then
  rename, least recently used evicted first), with its own budget, 1 GB by
  default. The server is the user's own, so cached files may outlive the
  session; "Keep available offline" on an album or playlist pins its files
  in the cache (7e.6).
- **Sign-in and secrets.** The user enters the server's address (or picks
  one the LAN discovery found) and signs in with a password or Quick
  Connect. The password is never stored; the access token goes in the
  keychain through `metadata::keys` (so logging redacts it), keyed by
  server, and the settings record only that one exists. The `DeviceId` is a
  random id made once per install. Signing out calls `/Sessions/Logout`,
  deletes the token, the source's rows and its cached audio.
- **Plain http only on the LAN.** Tokens and audio cross the network, so an
  `http://` address is accepted only for a local address (as the LAN remote
  decides what is local), and anything else needs `https://`. Servers with
  self-signed certificates are refused (owner's decision, 2026-10-11):
  only certificates the system trusts.
- **Unreachable is not empty** (H22): a server that doesn't answer gives its
  source a `FolderState` saying so; its tracks stay and show dimmed unless
  cached, and anything that picks tracks to play leaves the uncached ones
  out, as `availability::unreadable` does.
- **Art and details** go through the image cache and thumbnails (H17, with
  `Art::origin` set), so the `anomp-art` handler still serves only pictures
  already downloaded and the CSP doesn't change. Details are labelled with
  the server's name, as Phase 4 labels its sources.
- **Plays and favourites.** Reporting plays to the server
  (`/Sessions/Playing…`) is a switch, on by default once the source exists
  (the server is the user's own), so the server's play counts and other
  clients stay current. Local history (O8), ratings and favourites stay
  local, and nothing syncs back from the server (owner's decision,
  2026-10-11: one way). The server's playlists come in as read-only
  playlists.
- **A feature switch** in `FeatureSettings`, off by default (it goes
  online), checked where it acts; the Settings panel and the sidebar's
  server entry hide while it is off.
- **Logs and diagnostics**: the server's address and user name are written
  at debug only (the formatter scrubs URL paths at info and above); "Copy
  diagnostics" says how many servers there are and their versions, never
  their addresses.
- **Platforms.** macOS's sandbox already allows outgoing connections
  (`com.apple.security.network.client`), and `Info.plist` already has an
  `NSLocalNetworkUsageDescription` (for the LAN remote): reword it to cover
  finding and reaching a server. iOS (Phase 8) needs the same Local Network
  permission and the multicast entitlement for discovery, or offers address
  entry only.
- **Tests never touch the network**, as in Phase 4: the fake `Transport`
  serves recorded responses from a test server (`metadata/fixtures/`, each
  with its `manifest.json` entry, re-recorded by `record-fixtures.py`) and a
  committed audio fixture. A `live_` test, `#[ignore]`d, reads a test
  server's address and token from environment variables. The self-test (H14)
  gets a stage that plays a cached Jellyfin track inside the sandbox.

**Steps:**
- [ ] 7e.1 Set up a test server (Jellyfin in Docker, with the fixtures'
  chirp files and a few tagged albums) and check the API above against it:
  authentication and Quick Connect, paging and changed-since queries, static
  and transcoded streams, art, play reporting, lyrics, and the versions to
  support. Add a row to Phase 4's sources table (what is fetched and stored,
  no rate limit beyond `http::Client`'s) and a line to the privacy policy
  (§8.1). *Partly done (2026-10-11):* the API was built from Jellyfin
  10.10's documentation and is tested against hand-made responses in
  `metadata/fixtures/jellyfin/` (`made_by_hand` in `manifest.json`); 10.9 is
  the minimum (`jellyfin::MIN_VERSION`); the sources table's row and
  `docs/release-decisions.md`'s privacy row are added. The owner signed in
  to their own Jellyfin 10.11.10 server in `tauri dev` and synced it (42,806
  tracks, 12 playlists). Still to do, needing Docker: the test server,
  recording its responses over the hand-made ones (and correcting what
  differs: `MinDateLastSaved`, which provider id holds the recording, how
  `/Sessions/Playing/Stopped` counts a play, the transcode URL), and running
  `cargo test live_jellyfin -- --ignored` against it (`JELLYFIN_URL`,
  `JELLYFIN_USER`, `JELLYFIN_PASSWORD`). Lyrics from the server are left for
  later.
- [x] 7e.2 Design against what 7e.1 found: the library model shared with
  Phase 11, two-way sync of favourites and play counts or not, self-signed
  certificates. Write the migration. *Done 2026-10-11:* a `kind` on
  `folders` (`local`, `jellyfin`; `plex` allowed for 7f), with
  `server_sources`, `server_tracks`, `server_playlists` and
  `offline_albums`/`offline_playlists` (migration 014); Phase 11 can add its
  kind the same way. One-way sync and no self-signed certificates, as the
  owner decided.
- [x] 7e.3 Sign-in (address, LAN discovery, password and Quick Connect),
  token storage and sync. The server's music shows up in browse, search and
  the queue. *Done 2026-10-11* (`servers/address.rs`, `discovery.rs`,
  `jellyfin.rs`, `sync.rs`, `commands.rs`): tracks are written through the
  scanner's own `write_unfiled_track`, so browse, search, smart playlists
  and MusicBrainz links work unchanged.
- [x] 7e.4 Playback: download to the audio cache, then load through H11's
  pending path; download the next track ahead; transcoding; failures and
  offline behaviour as designed; play reporting. *Done 2026-10-11*
  (`servers/cache.rs`, `fetch.rs`, `playing.rs`, `queue/opening.rs`).
- [x] 7e.5 UI: the server in the sidebar with its sync status, dimmed tracks
  while it is unreachable, the server's panel in Settings, its error
  messages in `en.json`; the user guide, developer guide and class
  dictionary in the same commits. *Done 2026-10-11:* Settings › Servers, the
  sidebar's Servers, chapter 15 of the user guide, the developer guide's
  "Music servers".
- [x] 7e.6 "Keep available offline" for albums and playlists. *Done
  2026-10-11* (`servers/offline.rs`, the album and playlist menus).
- [ ] 7e.7 Optional: progressive playback (Phase 11's 11.6, built once for
  both), if download-first start-up is too slow away from home.
**As built** (2026-10-11): a server, as one user sees it, is a library
folder of kind `jellyfin` whose `path` is a key naming no file
(`jellyfin:<server id>/<user id>`) and whose tracks' `relative_path` is the
server's item id. Code that walks or opens folders on disk asks for `kind =
'local'`, and `access::open_folder` refuses any other kind. The token is in
the keychain (`Account::Server`), the device id in the `servers.deviceId`
settings row. `ServerSettings` (`app.servers`): reporting plays (on), a bit
rate for servers away from home (0, the originals) and the cache's size (1
GB). Servers have their own `http::Client`, with no rate limit to their
host. Tests: 35 in `servers/` and the migration's, all offline; the
self-test's "server track" stage decodes a track from the audio cache in the
sandbox.
**Known limits** (2026-10-11): lists dim a server's tracks while it can't be
reached even when they're downloaded (which tracks are cached is on disk,
not in the database), though the queue plays them; radio, smart playlists'
play and Home's suggestions leave out every track of an unreachable server;
Get Info reads a server track's file only once it has been played; play
reports are best effort and aren't retried; a server whose address changes
needs signing in again.

**To do** (left on 2026-10-11, in this order):
- [ ] Run a test server: Jellyfin 10.10 in Docker (the official
  jellyfin/jellyfin image), its library the fixtures' chirp files and a few
  tagged albums, Quick Connect turned on, and a test user. Docker isn't
  installed on the dev machine yet.
- [ ] Record the server's responses over the hand-made ones in
  `app/src-tauri/src/metadata/fixtures/jellyfin/`, giving each a `url` entry
  in `manifest.json` in place of `made_by_hand` (`record-fixtures.py` would
  need the test server's address and token, which it doesn't take yet:
  extend it, or record by hand and say how).
- [ ] Correct what the real server does differently, each with its test:
  whether `MinDateLastSaved` finds changed tracks (else a full sync each
  time); which provider id holds the recording's MBID; whether
  `/Sessions/Playing/Stopped` with a position counts a play; the MP3
  transcode URL (`Jellyfin::mp3_url`); Quick Connect's calls; the minimum
  version (`MIN_VERSION`, 10.9).
- [ ] Run the live test against it, with `JELLYFIN_URL`, `JELLYFIN_USER`
  and `JELLYFIN_PASSWORD` set:
  `cargo test live_jellyfin -- --ignored`.
- [ ] Check the exit (below) in a sandboxed bundle, with
  `docs/bundle-checks.md`: it uses the real container and so the owner's
  library, so ask first. Check too that the Local Network prompt appears for
  discovery, and the self-test's "server track" stage passes there.
- [ ] When 7e.1 and the exit are done: move this phase's design, steps as
  built and known limits word for word to a new
  docs/design/phase-7e-jellyfin.md, and link it here.
- **Exit (to check in the app):** after signing in to a Jellyfin server, its
  music appears in the library and plays through the queue alongside local
  tracks, gapless between streamed tracks, with Now Playing and media keys
  working, inside the sandboxed bundle. A server that goes away leaves its
  tracks dimmed, not gone, and cached ones still play. Every test runs
  offline.

### Phase 7f — Plex servers
Added 2026-10-10 at the owner's request. Goal: play the music on the user's
**Plex Media Server** next to the local library, as Phase 7e does for
Jellyfin: streamed on demand, through the same browser, search, queue,
gapless hand-off and Now Playing as local files. Unlike Jellyfin, Plex is
proprietary and signing in goes through the user's plex.tv account, so
Plex's terms and plex.tv's part are checked first (7f.1).

Scheduling: after Phase 7e and before the ports (Phases 8–10). It reuses
7e's library-source kind, audio cache, download-first playback, sync,
unreachable-source handling and server UI, so it is mostly a second client
behind them; anything 7e built for Jellyfin alone that Plex also needs is
made general here, not copied.

**Where things stand** (to check in 7f.1):
- Sign-in is plex.tv's PIN flow: `POST https://plex.tv/api/v2/pins` (with
  `strong=true`) returns a code, the user approves it in their browser at
  `https://app.plex.tv/auth` (opened with `openLink`, so no password is
  typed into the app), and polling the pin returns the account's token.
  `https://plex.tv/api/v2/resources` then lists the servers the account can
  reach, each with its own access token and its connections (local, remote
  and Plex's relay). Every request sends `X-Plex-Client-Identifier`,
  `X-Plex-Product`, `X-Plex-Version` and `X-Plex-Token` headers.
- A server's music: `/library/sections` (music libraries have type
  `artist`), then `/library/sections/{id}/all?type=10` for its tracks, paged
  by `X-Plex-Container-Start` and `X-Plex-Container-Size` and filtered by
  `updatedAt`. A track's `Media` and `Part` give the file's container, codec
  and bit rate, and the part's `/library/parts/…` key serves the original
  file; `/music/:/transcode/universal` transcodes. Art comes from each
  item's `thumb` (sized through `/photo/:/transcode`), plays are reported
  through `/:/timeline` and `/:/scrobble`, ratings through `/:/rate`, and
  lyrics are a stream of the track where the server has them.
- Servers are found on the LAN by GDM (a UDP broadcast to port 32414) as
  well as through plex.tv's list.
- Each server has certificates for its `*.plex.direct` names, so https works
  on the LAN without a self-signed certificate.
- **Plex Pass.** Since April 2025 Plex has required Plex Pass (or a Remote
  Watch Pass) to stream personal media from outside the server's network in
  its own apps. Whether and how that applies to a third-party client is
  checked in 7f.1; the app says so plainly rather than failing silently.
- **Terms.** Plex documents its server API for developers, but whether a
  commercial third-party client may use it and the plex.tv sign-in, may
  cache audio, and what it must show, is read in 7f.1 and recorded as a §8.1
  gate.

**Design** (Phase 7e's, with these differences):
- **A Plex server is a library source** of kind `plex` (7e.2's migration
  allows for it, or a new one adds it). A track's `path` is its rating key,
  never a URL. The folder holds the server's machine identifier rather than
  an address, since a server's connections change: the address is chosen at
  each sync and playback from plex.tv's list (local first, then remote, then
  the relay if the user allows it) and remembered until it fails. There is
  one source per music library the user picks, so a server with several can
  be added in part.
- **Sync** pages through each picked library's tracks changed since the last
  sync, as 7e's does, and H22's rule holds. Plex's MusicBrainz ids (its
  `Guid` entries, where the server's agent set them) feed Phase 4's links.
- **Playback** downloads the part's original file into the audio cache, or a
  transcode under 7e's rules, and opens it through H11's pending path.
  Plex's relay is slow and capped, so it is used only when nothing else
  answers, and the user is told.
- **Two kinds of token.** The plex.tv account's token and each server's
  access token go in the keychain through `metadata::keys`, keyed by account
  and by server; the settings record only that they exist. The
  `X-Plex-Client-Identifier` is a random id made once per install. Tokens go
  in headers, never in URLs, and logging's redaction learns `X-Plex-Token`
  (in a header or a query, with a test). Signing out removes the device from
  the account and deletes the tokens, the sources' rows and their cached
  audio.
- **Https first.** A server's `plex.direct` connection is used when it has
  one; plain http only for a local address, as in 7e, and only when the
  server allows insecure connections.
- **plex.tv is a third party.** Signing in and listing servers contact
  plex.tv and nothing else does: the music comes from the user's own server,
  or through Plex's relay. The privacy policy names it (§8.1).
- **Plays, ratings and playlists** as 7e decides for Jellyfin: reporting
  plays is a switch, on once the source exists; the server's audio playlists
  come in read-only; Plex's 0–10 ratings map to the app's own only if 7e.2
  chose two-way sync.
- **A feature switch** of its own in `FeatureSettings`, off by default,
  apart from Jellyfin's.
- **Logs, diagnostics, platforms and tests** as 7e: addresses and user and
  server names at debug only; "Copy diagnostics" counts servers and their
  versions; GDM needs the same Local Network permission (and iOS's multicast
  entitlement) as Jellyfin's discovery; tests use recorded responses from a
  test server and from plex.tv in `metadata/fixtures/`, and `live_` tests
  are `#[ignore]`d. The self-test (H14) gets a Plex stage only if its
  playback path differs from 7e's.

**What 7f.1's reading found** (2026-10-11, from Plex's Terms of Service and
its API documentation at developer.plex.tv; no test server yet):
- **Terms.** Plex's Terms of Service call software that calls a Plex Media
  Server "Interfacing Software": its maker is liable for it and grants Plex
  a licence to use, host, link to and promote it, its name included. Nothing
  in them rules the app out, so the phase goes on. Three points are the
  owner's to settle before a release that includes Plex (§8.1): the user's
  licence to Plex is personal and non-commercial; Plex's marks need its
  written permission (the app names Plex only to say which servers it plays
  from, and shows no logo); and getting Plex Pass features without a Plex
  Pass is forbidden, so the app never works around a server's refusal.
- **Plex Pass.** Since 2025-04-29 Plex's own apps need a Plex Pass or a
  Remote Watch Pass to stream video from a server away from home, with
  Plexamp's music exempt; in November 2025 Plex said third-party clients
  streaming remotely through its API would follow in 2026. The server
  enforces it, so the app shows a server away from home refusing a track as
  `plexPassNeeded` and plays at home as before. What the server answers is
  checked against a test server.
- **Sign-in.** Plex now recommends a JWT flow (a device key registered with
  the PIN, tokens that last 7 days, refreshed by signing a nonce). The older
  PIN flow still works, and the app uses it first: `POST
  https://plex.tv/api/v2/pins?strong=true`, the user approving the code at
  `https://app.plex.tv/auth#?clientID=…&code=…`, and `GET /api/v2/pins/{id}`
  until it has an `authToken`; 7f.7 moves to JWT.
  `https://clients.plex.tv/api/v2/resources?includeHttps=1&includeRelay=1`
  lists the servers, each with its own `accessToken` and `connections`
  (`uri`, `local`, `relay`), and `https://plex.tv/api/v2/user` names the
  account. Every request sends `X-Plex-Client-Identifier`, `X-Plex-Product`,
  `X-Plex-Version` and, once signed in, `X-Plex-Token`. Any `X-Plex-` header
  may be a query parameter instead, which the app uses for paging alone
  (`X-Plex-Container-Start` and `-Size`), never for the token. The API is
  versioned (`X-Plex-Pms-Api-Version`; 1.0.0 needs PMS 1.41.9); the app
  sends none, so it gets the long-standing responses.
- **Signing out.** No documented call removes one device from an account, so
  signing out deletes the tokens here, and the user guide says where to
  remove the device at plex.tv.

**Design as built** (7f.2–7f.6, 2026-10-11):
- **Headers.** `http::Transport` sends a list of headers in place of one
  `Authorization` value (`Client::get_with_headers`, `post_with_headers`,
  `download_with_headers`; the `_with_auth` calls stay), so Plex's
  `X-Plex-*` headers go through the same client, backoff and test fake.
  Logging redacts `X-Plex-Token` as a header and as a query parameter.
- **One source per music library** (migration 015): `server_sources` gains
  `section` (the library's key; empty for Jellyfin) and `section_name`,
  unique on server, user and section, and `connections` (the server's
  connections as plex.tv last listed them, as JSON); `server_tracks` gains
  `media_path` (the path that serves the file, which a Plex track's id
  doesn't give). A Plex source's folder key is `plex:<machine id>/<account
  id>/<section>`; the server's token is kept under `plex:<machine
  id>/<account id>` and the account's under `plex-account:<account id>`.
- **One switch per kind of server.** `FeatureSettings::plex`, off by
  default; everything 7e checks against `features.jellyfin` asks whether the
  source's own kind is on (`servers::kind_on`), and the Settings panel and
  sidebar show while either is.
- **A client per kind behind one interface** (`servers/api.rs`,
  `ServerApi`): who the server is, the headers a fetch needs, where a
  track's original file and MP3 transcode are, where a picture is, and
  reporting plays, for Jellyfin and Plex alike, so `commands`, `fetch` and
  `playing` don't branch on the kind beyond choosing the client.
- **Connections** (`servers/plex.rs`): the address that last answered is
  tried first, then the stored connections: local ones, then remote https
  ones, then Plex's relay only while "Use Plex's relay" is on
  (`ServerSettings::plex_relay`, off). Plain http only to a local address
  and only when the server doesn't require https. A connection counts only
  when `/identity` answers with the server's machine id. Each sync asks
  plex.tv for the connections again; without plex.tv the stored ones are
  used. A source playing through the relay says so in Settings.
- **Pictures** are stored under a key naming no address (`plex://<machine
  id><thumb path>`) and fetched through the server's `/photo/:/transcode` at
  the connection in use, so a changed address doesn't download them again.
- **Plays** go to `/:/timeline` (playing, then stopped with the position);
  the server counts a play by its own rule, as Jellyfin's does.
- **Discovery** by GDM (`servers/discovery.rs`) adds a server's address on
  the local network to the connections plex.tv gave, for a server whose
  local connections plex.tv doesn't list.

**Plan of work** (2026-10-11, in this order):
- [ ] 7f.1 Read Plex's terms and check the API against a test server.
  *Partly done (2026-10-11):* the terms and the documentation are read
  (above), the §8.1 gate and the privacy row are written, and Phase 4's
  sources table has a row. Still to do, needing Docker and a Plex account: a
  test server (Plex Media Server in Docker, claimed by a test account, with
  the fixtures' chirp files and a few tagged albums), recording its and
  plex.tv's responses over the hand-made ones in `metadata/fixtures/plex/`,
  correcting what differs (each with its test: the `updatedAt>=` filter,
  which `Guid` holds the recording's MBID, the MP3 transcode's URL and
  profile, whether a timeline report counts a play, what a server away from
  home answers without Plex Pass), and running `cargo test live_plex --
  --ignored` against it.
- [x] 7f.2 Make 7e general: the header list in `http`; migration 015;
  `FolderKind::Plex`; the `plex` switch and `kind_on`; the `ServerApi`
  interface with Jellyfin behind it; the sources' new columns, keys and
  tokens. Every 7e test still passes unchanged in meaning. *Done
  2026-10-11.*
- [x] 7f.3 The Plex client (`servers/plex.rs`, `servers/plextv.rs`): the PIN
  sign-in, the account, resources and connections, choosing a connection,
  music libraries, tracks a page at a time and changed since a time, track
  ids, audio playlists, original and transcoded files, pictures, play
  reports and errors; GDM. Tested against hand-made responses in
  `metadata/fixtures/plex/` (`made_by_hand` in `manifest.json`). *Done
  2026-10-11.*
- [x] 7f.4 Commands: start and finish the PIN sign-in, list the account's
  servers with their music libraries, add the libraries the user picks (each
  a source, synced at once), and sign out of a source (the server's token
  goes with its last source, the account's with its last server). Sync,
  playback, play reports, checks and "Keep available offline" work for Plex
  sources through `ServerApi`. *Done 2026-10-11*
  (`servers/plex_commands.rs`, `servers/commands.rs`).
- [x] 7f.5 UI: Settings › Servers gains "Sign in with Plex" (the code, the
  browser opened with `openWebLink`, then the servers and their music
  libraries to pick), the relay switch, and Plex's sources in its list; the
  sidebar lists them; error messages in `en.json`. The user guide (chapter
  15, the errors appendix, the settings reference), the developer guide, the
  class dictionary, the privacy chapter, `Info.plist`'s Local Network text
  and `CHANGELOG.md` in the same change. *Done 2026-10-11*
  (`settings/PlexSignIn.svelte`, `ServerOptions.svelte`).
- [x] 7f.6 Run every check (`check-all.py --quick`, `cargo test`, clippy,
  `npm run check`, `npm test`, `npm run lint`) and update §2's counts. *Done
  2026-10-11:* all pass; the C++ suites and the full `check-all.py`
  (sanitizers, fuzzers, bundle) weren't run, since nothing in `core/` or
  `effects/` changed.
- [ ] 7f.7 Optional: move to Plex's JWT sign-in (an Ed25519 device key in
  the keychain, tokens refreshed before their 7 days are up), once 7f.1's
  test server shows what it changes.
**As built** (2026-10-11): 18 more tests, in `servers/` (the Plex and
plex.tv clients, connections, GDM, `ServerApi`, a Plex library's sync, a
Plex Pass refusal) and migration 015's, with log redaction's test covering
`X-Plex-Token`; two `live_` tests (`live_plex_tv` with `PLEX_TOKEN`,
`live_plex` with `PLEX_URL` and `PLEX_SERVER_TOKEN`). The self-test gets no
Plex stage: a Plex track plays from the audio cache exactly as a Jellyfin
one does.
**Known limits** (2026-10-11): everything is checked against Plex's
documentation and hand-made responses, not a real server (7f.1); signing out
deletes the tokens here but doesn't remove the device from the account at
plex.tv (no documented call; the user guide says where); the legacy PIN
sign-in, whose token doesn't expire, is used until 7f.7; a track's sample
rate isn't in Plex's track lists, so it shows once played; Plex's loudness
analysis isn't read (ReplayGain comes from the app's own analysis); 7e's
known limits hold for Plex sources too.
**Withdrawn for now** (2026-10-11, the owner's request): Settings ›
Features offers no Plex servers switch (`FeaturesOptions.svelte`'s
`NOT_OFFERED`), unless it is still on, so it can be turned off; Find lists
it only then. The code, setting and tests stay; §8.7 lists the steps to
offer it again.
- **Exit (to check in the app):** as Phase 7e's, with a Plex server: after
  signing in through plex.tv, its music plays through the queue alongside
  local and Jellyfin tracks, gapless, with Now Playing and media keys
  working, inside the sandboxed bundle. A server that goes away leaves its
  tracks dimmed, not gone, and cached ones still play. Every test runs
  offline.

### Phase 8 — iOS and iPadOS
- Install Xcode, the iOS Rust targets, and set up the Apple Developer
  account (§3).
- Run `cargo tauri ios init`. In `build.rs`, cross-compile `anomp_core` for
  iOS with CMake (`CMAKE_SYSTEM_NAME=iOS`, keyed off `CARGO_CFG_TARGET_OS`)
  and link UIKit instead of AppKit.
- Repeat the Phase 0 spike on the iOS simulator: JUCE audio inside Tauri.
- Build FFmpeg for iOS device and simulator as dynamic frameworks packaged
  in an `.xcframework`, embedded in the app bundle (LGPL on iOS; see §4.1).
- Audio session: `AVAudioSession` category `.playback`, background audio
  mode, interruption and route-change handling.
- File access: import via document picker / Files app with security-scoped
  bookmarks.
- Metadata keys (`metadata/keys.rs`, 4.8): `apple-native-keyring-store`'s
  `protected` store, which iOS requires and which needs the provisioning
  profile's keychain entitlement.
- Layouts: iPhone (compact) and iPad (split view); touch targets and
  gestures.
- Lock screen and Control Center via the Phase 3 Now Playing code.
- Add an iOS simulator build to CI. Release steps (TestFlight, App Store)
  are in §8.4.
- Found by D4's porting inventory and fixed 2026-10-10 (the developer
  guide's [porting chapter](docs/developer-guide/14-porting.md) has the
  rest): `shell/`'s menus, tray, extra windows and notifications are
  `#[cfg(desktop)]`, since Tauri's `menu` and `tray` exist on desktop only,
  and `notify-rust` is a desktop-only dependency; messages that name the Mac
  have `@ios` forms where iOS differs. To do here: build that branch for iOS
  (it passes Clippy only with the cfg flipped on macOS), give notifications
  an iOS route, and add the audio session above as a new core seam.
- **Exit:** the app plays a library on a physical iPhone and iPad, including
  in the background.

### Phase 9 — Linux
- Set up the Linux machine/VM (§3) and add an `ubuntu-latest` CI job
  (CMake + ctest, `cargo test`, frontend checks, `cargo tauri build`).
- `build.rs`: link `asound`, `pthread`, `dl`, plus freetype/fontconfig if
  JUCE GUI modules are still linked.
- Audio: JUCE uses ALSA (and optionally JACK). On PipeWire/PulseAudio
  desktops this goes through the ALSA compatibility layer; verify device
  selection, device switching and latency there.
- Formats: build FFmpeg for Linux x86_64 (and arm64 if targeted) with the
  same script and flags. Bundle it in the packages with
  `RPATH=$ORIGIN`-style lookup so the system FFmpeg is never picked up.
- Media keys / desktop integration: MPRIS over D-Bus (the `mpris-server` or
  `souvlaki` crate on the Rust side, or `sdbus` in the core).
- File access: plain paths; the library scanner must handle symlinks and
  case-sensitive names.
- Metadata keys (`metadata/keys.rs`, 4.8): the Secret Service over D-Bus
  (a `keyring-core` store), with a clear message when no keyring daemon
  runs.
- WebKitGTK rendering check: the visualizer (Phase 5) may need a canvas
  fallback if WebGL is slow or disabled.
- Found by D4's porting inventory and fixed 2026-10-10 (see the [porting
  chapter](docs/developer-guide/14-porting.md)): `access::in_trash` knows a
  volume's `.Trash-<uid>`; the Help window's Back key is Alt+←; messages
  that name the Mac, the Finder, iCloud or the keychain have `@linux` forms;
  `CApiTests.cpp`'s asynchronous tests pump JUCE's own queue. To do here:
  compile and run that pump (it hasn't been built on Linux), and check
  JUCE's event loop is pumped in the app (§6).
- **Exit:** a release build plays a library on Ubuntu and Fedora, with media
  keys working. Packaging is in §8.5.

### Phase 10 — Windows
- Set up the Windows machine/VM (§3) and add a `windows-latest` CI job.
- CMake/MSVC: build the core with the dynamic CRT
  (`CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded$<$<CONFIG:Debug>:Debug>DLL`) so
  it matches Rust's default and links cleanly. `build.rs` links the Windows
  system libs JUCE needs (`winmm`, `ole32`, `uuid`, `ws2_32`, `version`,
  `shlwapi`, etc.).
- Audio: JUCE WASAPI (shared mode by default; exclusive mode as an option in
  the admin screen). ASIO is out of scope (SDK licensing).
- Formats: build FFmpeg for Windows x64 (MSVC toolchain via MSYS2, or
  cross-compiled with MinGW from the Linux CI job). Ship the DLLs next to
  the executable.
- Media keys / lock-screen overlay: System Media Transport Controls (SMTC),
  via `souvlaki` or the `windows` crate.
- Paths: long paths (`\\?\` prefix), UTF-16 ↔ UTF-8 at the OS boundary,
  case-insensitive de-duplication in the library DB.
- Metadata keys (`metadata/keys.rs`, 4.8): the Windows Credential Manager
  (a `keyring-core` store).
- Found by D4's porting inventory and fixed 2026-10-10 (see the [porting
  chapter](docs/developer-guide/14-porting.md)): folders' paths are stored
  without the verbatim `\\?\` prefix and files matched to them ignoring case
  (`library/paths.rs`); the log scrubs drive, share and verbatim paths (H9);
  an M3U8 entry with a drive letter or a share is a path here
  (`playlists::entry_path`); `access::in_trash` knows `$Recycle.Bin`; the
  Help window's Back key is Alt+←; messages that name the Mac have
  `@windows` forms. Still to do: OneDrive's placeholders need a `FileStatus`
  implementation (H12), background scans a lower priority
  (`watch::lower_priority`), and `CApiTests.cpp`'s pump of JUCE's queue a
  first build here.
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
for the owner and can take months to answer, so start it early. Phase 7e
(Jellyfin servers) builds the library-source kind, the audio cache and
download-first playback described below first; this phase reuses them.

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
  artist, album, track number, length, release date, tags as genres, and
  art. It stops at the first purchase already synced. Items no longer in the
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
  `bandcamp.com/developer`. Describe the app: a commercial player (its own
  code MIT) that streams only the signed-in user's purchases, keeps audio
  only in a bounded cache, and links back to Bandcamp. Ask for API access to
  a fan's collection and streams, and for their rules on caching and offline
  play, on iOS and the App Store, on attribution, and on any commercial
  terms. Record the answer and its date here. No engineering starts without
  a yes.
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
| Jellyfin's API changes between server versions, or a server is reached over plain http | Phase 7e supports stated server versions, checked against a test server and recorded fixtures; `http://` only for local addresses, `https://` otherwise; the Jellyfin code is one Rust module behind the library-source kind |
| Plex's terms rule out a commercial third-party client, Plex Pass limits streaming away from home, or plex.tv's sign-in changes | Phase 7f reads the terms first (7f.1) and stops if they rule the app out; Plex Pass limits are shown plainly, never as a failure; the Plex code is one Rust module behind the library-source kind, so it can be dropped without touching local or Jellyfin playback |
| Bandcamp refuses, limits or withdraws access (no API for fans; scraping forbidden) | Phase 11 starts only with written permission; the Bandcamp code is one Rust module behind the library-source kind, so it can be dropped without touching local playback; fallback 11.7 (purchases as local files) |
| iOS sandbox limits on music files | Document picker import and bookmarks; the Apple Music library (DRM) is out of scope |
| Losing the user's own data (picks, and later playlists, favourites and history) to a corrupt DB, a bad migration, or moved files | Copies before migrations and integrity checks (H10); ids kept across moves and renames (F10); export and import (F20) |
| Library folders in iCloud Drive, on a NAS or on sleeping disks: whole-library downloads during a scan, UI freezes on open | Dataless files detected and not read (H12); files opened off the main thread (H11) |
| Failures in users' hands leave no trace (`eprintln!` output is lost in a bundle, and release builds abort on panic) | Log file, panic hook and "Copy diagnostics" (H9) before the first release |
| Visualizations triggering photosensitive reactions | Flash limits, `prefers-reduced-motion`, and a note on first use (F18) |

## 7. Proposed repo layout

The layout proposed here now exists. The developer guide's
[repository map](docs/developer-guide/03-repository-map.md) describes it
file by file, and [README.md](README.md#repository-layout) in brief.

## 8. Release

Everything needed to get the app into users' hands, collected in one place.
Platform subsections apply once the matching phase in §5 is done.

### 8.1 Gates before any public release
These need answers first; most need the owner rather than engineering.
Each open item has a brief (options, what it blocks, what was checked) in
`docs/release-decisions.md` (2026-10-02).

- [ ] **JUCE license** (§4.1): commercial license in place (Starter tier to
      start; upgrade before revenue passes its cap).
- [ ] **FFmpeg LGPL compliance** (§4.1): notice, source offer and
      replaceable shared libraries in every package. Done for the macOS
      direct download (2026-10-02): `THIRD_PARTY_NOTICES` has the LGPL, the
      configure flags and the source tarball, and the dylibs are separate in
      `Contents/Frameworks` (`check-bundle.py`). For the App Store, confirm
      this is acceptable before the first iOS or Mac App Store submission.
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
  - [ ] Then pick the bundle identifier (e.g. `com.<owner>.anotracks`, same
        on every platform) and a publisher name. Neither can change after
        the first store release without losing users' installs.
- [ ] **Distribution channels:**
  - macOS: notarized direct download (DMG) and/or Mac App Store. The store
    requires the App Sandbox and does not allow the built-in updater.
    Proposal: direct download first. **Decided 2026-10-03 (owner):**
    builds are distributed from this repository's GitHub Releases page
    (the DMG, the zipped app and `SHA256SUMS` that `release.py` makes);
    the Mac App Store is not planned for now.
  - Linux: `.deb` and AppImage first (Tauri builds both, plus `.rpm`).
    Flatpak/Flathub reaches more distros but adds sandbox work; later.
  - Windows: NSIS installer (proposal) and/or MSI. Microsoft Store and
    winget optional, later.
- [ ] **Privacy policy and support page:** required by the App Store, and
      the app contacts MusicBrainz and Cover Art Archive. Host a short
      policy and support URL.
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
- [ ] **Plex's terms** (Phase 7f, if it ships): 7f.1 (2026-10-11) found
      that Plex's Terms of Service allow "Interfacing Software" that calls
      a Plex Media Server, at its maker's liability and with a licence to
      Plex to use and promote it. Settle: whether the user's personal,
      non-commercial licence fits a paid app; Plex's permission for the
      app to name Plex ("Plex servers", no logo shown); and that the app
      never works around Plex Pass (it shows a server's refusal as
      `plexPassNeeded`). The privacy policy names plex.tv (sign-in and the
      list of servers) and Plex's relay (`docs/release-decisions.md` has
      both rows).
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

- **Single version number:** CMake `project(VERSION)` is the source of
  truth; generate `anomp_version()` from it instead of the hard-coded
  string, and keep `Cargo.toml`, `tauri.conf.json` and `package.json` in
  sync with `scripts/version.py`, checked in CI (§9.2 M6). MusicBrainz
  `User-Agent` includes this version and a contact address. **Done**, but
  the contact is a placeholder until the owner gives one (§8.1).
- **Release workflow** (GitHub Actions, triggered by a version tag): build
  and test on every platform, sign, package, attach the artifacts plus
  SHA-256 checksums to a draft GitHub Release, and generate release notes
  from `CHANGELOG.md`. **Done for macOS** (`.github/workflows/release.yml`):
  it calls `ci.yml`, builds the universal app and DMG, and runs
  `release.py`; signing and notarization steps are skipped, with a warning,
  until the secrets exist. Linux and Windows join with their phases.
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
- **App icon and metadata:** icon set for every platform
  (`cargo tauri icon`), app description, screenshots. Not done: it needs the
  name (§8.1).

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
- `.deb` and AppImage via the Tauri bundler, with FFmpeg bundled and found
  via `$ORIGIN` RPATH (never the system copy).
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
- NSIS installer (and/or MSI) via the Tauri bundler, with the FFmpeg DLLs
  next to the executable.
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

To offer Plex again (Phase 7f, withdrawn for now):
1. Remove `"plex"` from `NOT_OFFERED` (`FeaturesOptions.svelte`).
2. Remove the `needs` condition from its Find entry (`find.ts`).
3. Undo the two user-guide notes (chapters 11 and 15).
4. Restore the `CHANGELOG.md` entry.

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
