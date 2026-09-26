# ano-mp — Implementation Plan

Status as of 2026-09-25: repository skeleton in place, C++ core builds and its
Catch2 suite passes on macOS. The Phase 0–7 toolchain (§3) is installed.
Phase 0 in progress: the Tauri app links the core and shows its version; the
JUCE-in-Tauri audio spike is next.

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
| Top-level CMake with JUCE 9.0.2 + Catch2 v3.16.0 via FetchContent | `CMakeLists.txt` |
| Presets `debug` / `release` (Ninja) | `CMakePresets.json` |
| `anomp_core` static lib, `FormatRegistry` (JUCE built-in decoders for now; replaced by FFmpeg in Phase 1) | `core/src` |
| C API: `anomp_version`, `anomp_can_decode_extension` | `core/include/anomp/anomp.h` |
| 5 passing Catch2 tests | `core/tests` |
| Tauri 2 app (SvelteKit + `adapter-static`, Svelte 5, TS) showing `anomp_version()` via the `core_version` command | `app/` |
| `build.rs` builds `anomp_core` with the `cmake` crate and links it plus the Apple frameworks | `app/src-tauri/build.rs` |
| Safe Rust wrappers over the C API, 2 `cargo test` tests | `app/src-tauri/src/anomp.rs` |

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
release gates (§8.1) rather than engineering blockers.

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

Release-only decisions (distribution channels, packaging, signing) are in §8.1.

## 5. Phased plan

Each phase ends with a demonstrable result and green tests. Platform order:
macOS (Phases 0–7), then iOS/iPadOS (Phase 8), then Linux (Phase 9), then
Windows (Phase 10). Xcode is not needed until Phase 8. The phases cover
making the app work on each platform; packaging, signing and shipping it are
all in §8.

Rules from the start, so the later ports stay cheap:
- Keep platform code behind small interfaces in the core (media controls,
  file access, audio session) with one implementation per OS. No AppKit or
  CoreAudio calls outside those files.
- Store folder access as bookmarks/opaque handles, not raw paths.
- UTF-8 everywhere across the C API; never assume `/` separators or
  case-sensitive paths.
- The core links only JUCE modules that avoid GUI dependencies where
  possible (`juce_audio_utils` pulls in `juce_gui_basics`, which means X11
  and freetype on Linux). Drop it in Phase 1 if nothing needs it.

### Phase 0 — Toolchain and integration spike (highest risk first)
- [x] Install the prerequisites in §3.
- [x] Scaffold `app/` with `npm create tauri-app` (Tauri 2, Svelte + TypeScript
  template; see §4.2). The template is SvelteKit with `adapter-static` (SPA,
  no SSR); kept for its routing. Bundle identifier `dev.anomp.player` is a
  placeholder — choose the real one before §8 (it is hard to change after
  the first store release).
- [x] `build.rs`: build `anomp_core` via the `cmake` crate, link the static lib plus
  the Apple frameworks (CoreAudio, AudioToolbox, CoreMIDI, Accelerate,
  AVFoundation, Foundation, AppKit).
- [x] Call `anomp_version()` from a Tauri command and show it in the UI.
- [ ] **Spike: JUCE inside a Tauri process.** Tauri (tao) owns the main thread and
  run loop. Verify that JUCE's `AudioDeviceManager` plays audio when
  initialised via `ScopedJuceInitialiser_GUI` without a `JUCEApplication`, and
  that device-change notifications still arrive.
- **Exit:** a tone plays from a Tauri app on macOS.

### Phase 1 — Playback engine (C++ core)
- `scripts/build-ffmpeg.sh`: fetch a pinned FFmpeg release, configure it
  LGPL/audio-only/shared (see §4.3), and install into
  `third_party/ffmpeg/<platform>-<arch>/`. It covers macOS arm64 and x86_64
  (merged with `lipo`) now; Phase 8–10 add iOS, Linux and Windows. CMake finds
  the result via an imported target; CI caches it by version and flags.
- `FFmpegAudioFormat` / `FFmpegAudioFormatReader`: a JUCE `AudioFormat` backed
  by libavformat + libavcodec, converting to float with libswresample.
  Requirements:
  - sample-accurate seeking;
  - encoder delay/padding trimming for gapless MP3/AAC/Opus;
  - accurate duration;
  - opens from a path (UTF-8) or a JUCE `InputStream` via a custom `AVIOContext`.
- Replace `registerBasicFormats()` in `FormatRegistry` with
  `FFmpegAudioFormat`. Keep JUCE's WAV writer only for generating test
  fixtures.
- `PlayerEngine`: `AudioDeviceManager` → `AudioSourcePlayer` →
  `AudioTransportSource` fed by `AudioFormatReaderSource` with a read-ahead
  background thread.
- Commands: load, play, pause, stop, seek, volume. State machine with
  thread-safe status reads.
- Queue with gapless transition: pre-open the next reader and hand it off at
  end of stream.
- C API: engine create/destroy, commands, `anomp_set_event_callback`
  (state/position/track-ended/error).
- **Tests:** decode every format in §4.3 from small fixture files (checking
  sample rate, channels, length and a checksum of the decoded samples), plus
  state transitions, seek accuracy, and the gapless handoff, run
  against generated WAV/FLAC fixtures with a null/offline device
  (`AudioProcessorGraph` or direct `getNextAudioBlock` calls, so no hardware
  is needed in CI).

### Phase 2 — Metadata and library
- Core: add TagLib (FetchContent) with `anomp_read_tags(path) → struct` for
  title, artist, album, album artist, track/disc, year, genre, duration,
  MusicBrainz IDs (if tagged), and embedded art.
- Rust: SQLite schema (tracks, albums, artists, folders, settings, mb_cache),
  migrations, and an incremental folder scanner (mtime/size change detection).
- Logical sort/grouping rules: by album artist → album → disc/track, by folder,
  by genre, by year. Rules are configurable (feeds the admin screen).
- macOS sandbox: user-selected folders plus security-scoped bookmarks. Store
  bookmarks, not raw paths, so the same model works on iOS later.
- **Tests:** Catch2 tests for tag reading over fixture files; `cargo test` for
  schema, scanner and sort rules.

### Phase 3 — Frontend: core player UI
- Library browser (artists / albums / tracks / folders), search, queue view,
  now-playing bar with transport and seek.
- Desktop window layout. Build it responsive (no fixed widths) so the
  iPhone/iPad layouts in Phase 8 are adjustments, not rewrites.
- OS media integration: macOS Now Playing info and remote commands
  (`MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`) implemented in Obj-C++
  in the core. This covers media keys and Control Center; the same APIs serve
  the iOS lock screen in Phase 8.

### Phase 4 — Online metadata services
- MusicBrainz client in Rust: a meaningful `User-Agent` (required), a
  **≤1 request/second** rate limiter, and results cached in SQLite.
- Matching: use MBIDs from tags when present; otherwise search on
  artist/album/title and score by duration and track count.
- Cover Art Archive for album art. Optionally AcoustID/Chromaprint
  fingerprinting later (needs an API key and adds a dependency).
- The user can enable or disable each service (admin screen) and the app
  degrades gracefully offline.

### Phase 5 — Visualization
- Core: lock-free FIFO tap on the output; FFT (`juce::dsp::FFT`) → log-spaced
  bins plus peak/RMS levels, published at ~30–60 Hz via callback.
- Rust forwards the frames over a Tauri `Channel` (binary/compact payload,
  not per-frame JSON if it proves costly).
- Frontend: canvas/WebGL renderers (spectrum bars, oscilloscope, VU) that
  users pick in preferences.

### Phase 6 — Admin / settings screen
- Settings are persisted in SQLite (or a Tauri store), with a typed schema in
  Rust and shared TS types (generated with `specta`/`ts-rs`).
- Sections: displayed fields and columns, enabled services, library folders
  and rescan, sort/grouping rules, visualization choice and parameters, audio
  output device and buffer size (desktop), and replay-gain on/off.

### Phase 7 — Hardening (macOS)
- CI (GitHub Actions, macOS runner): CMake build + ctest, `cargo test`,
  frontend lint/type-check/tests.
- Performance: library of 50k+ tracks; scan time; memory use.
- Robustness: corrupt/truncated files, missing files on disk, unplugged
  output devices, offline services.
- **Exit:** the macOS app is ready for the first release in §8.3.

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
- **Exit:** a release build plays a library on Windows 11, with media
  keys working. Installer and signing are in §8.6.

## 6. Key risks

| Risk | Mitigation |
|---|---|
| JUCE message loop vs. Tauri's main-thread ownership | Phase 0 spike before any other work; fall back to driving CoreAudio via JUCE's `AudioIODevice` without MessageManager-dependent features |
| iOS problems found late (iOS work is deferred to Phase 8) | Keep the core platform-neutral and store bookmarks rather than paths from the start; if any late surprise would be costly, pull the Phase 8 simulator spike forward once Xcode is available |
| Licensing (JUCE commercial tier, FFmpeg LGPL, TagLib MPL) | JUCE license in place before any distribution (release gate, §8.1); FFmpeg always shipped as shared libs; App Store LGPL opinion before Phase 8 ships (§4.1) |
| AAC patent exposure from shipping FFmpeg's AAC decoder | Licensing opinion before release; CoreAudio fallback on Apple (§4.3) |
| FFmpeg build complexity across 4 OSes and several architectures | One script, pinned version, CI-cached artifacts; done per platform in its phase |
| FFmpeg parser vulnerabilities (large attack surface) | Minimal configure (only needed demuxers/decoders); track FFmpeg security releases and bump the pin |
| MusicBrainz rate limits and bans | Strict limiter, caching, User-Agent with contact info |
| Decoder behaviour differing across platforms | Same FFmpeg version and flags everywhere; the Phase 1 format tests run on every CI OS |
| WebKitGTK (Linux) and WebView2 (Windows) behave differently from WKWebView | Keep the frontend to standard web APIs; run frontend smoke tests on each OS in CI |
| iOS sandbox limits on music files | Document picker import and bookmarks; the Apple Music library (DRM) is out of scope |

## 7. Proposed repo layout

```
ano-mp/
  CMakeLists.txt, CMakePresets.json
  core/            C++ JUCE engine (static lib) + C API
    include/anomp/ public C header
    src/
    tests/         Catch2
    tests/fixtures/ small audio files for tests
  scripts/         build-ffmpeg.sh and other tooling
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

- [ ] **JUCE license** (§4.1): commercial license in place (Starter tier to
      start; upgrade before revenue passes its cap).
- [ ] **FFmpeg LGPL compliance** (§4.1): notice, source offer and replaceable
      shared libraries in every package. For the App Store, confirm this is
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
- [ ] **Crash reporting:** decide none vs. opt-in (e.g. Sentry). Anything
      opt-in must appear in the privacy policy.

### 8.2 Cross-platform release setup
Build once (after Phase 7), reused for every platform.

- **Single version number:** CMake `project(VERSION)` is the source of truth;
  generate `anomp_version()` from it instead of the hard-coded string, and
  keep `Cargo.toml`, `tauri.conf.json` and `package.json` in sync with a
  script checked in CI. MusicBrainz `User-Agent` includes this version and a
  contact address.
- **Release workflow** (GitHub Actions, triggered by a version tag): build and
  test on every platform, sign, package, attach the artifacts plus SHA-256
  checksums to a draft GitHub Release, and generate release notes from
  `CHANGELOG.md`.
- **Signing secrets** (Apple certificates and notarization key, Windows
  signing, updater key) stored as CI secrets, with an offline backup. Losing
  the updater key strands existing installs.
- **Auto-updates** for direct-download builds: Tauri updater plugin with a
  signed update manifest hosted with the releases. Store builds (App Store,
  Microsoft Store, Flathub) update through the store instead.
- **Third-party notices:** a generated `THIRD_PARTY_NOTICES` file shipped in
  every package and shown in the app's About screen. It covers JUCE; FFmpeg
  (LGPL text, exact version and configure flags, link to the matching
  source tarball); TagLib; Rust crates (`cargo-about`); and npm packages. CI
  fails if a dependency's license is unknown.
- **App icon and metadata:** icon set for every platform (`cargo tauri icon`),
  app description, screenshots.

### 8.3 macOS
- Apple Developer Program membership and a **Developer ID Application**
  certificate (direct download); Mac App Store needs its own certificates.
- Universal binary (arm64 + x86_64), including the core and FFmpeg.
- Bundle the FFmpeg dylibs in `Contents/Frameworks`, set install names to
  `@rpath`, and sign all nested code with the hardened runtime.
- Entitlements: network client; for the sandbox, user-selected read access
  and app-scoped bookmarks.
- Notarize with `notarytool`, staple the ticket, ship as a DMG.
- Verify on a clean Mac: Gatekeeper accepts it (`spctl --assess`), first
  launch, library import, update from the previous version.

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
    start with an empty library).

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
2. Bump the version; update `CHANGELOG.md`.
3. CI green on every platform, including the format decode tests.
4. Regenerate third-party notices; check FFmpeg source link matches the
   pinned version.
5. Tag; the release workflow builds, signs, notarizes and packages.
6. Smoke test each artifact on a clean machine: install/upgrade, play MP3,
   FLAC and AAC, seek, gapless album, media keys, MusicBrainz lookup.
7. Publish the GitHub Release and the updater manifest; submit store builds
   (TestFlight → App Store review).
8. After release: watch crash reports (if enabled) and issue tracker; keep
   the previous version's artifacts available for rollback.
