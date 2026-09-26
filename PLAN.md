# ano-mp — Implementation Plan

Status as of 2026-09-25: repository skeleton in place, C++ core builds and its
Catch2 suite passes on macOS. The Phase 0–7 toolchain (§3) is installed.
Phase 0 complete: the Tauri app links the core, JUCE plays a test tone inside
the Tauri process, and device-change events reach the UI. Phase 1 complete:
FFmpeg decodes every format through `FFmpegAudioFormat`, and `PlayerEngine`
plays, pauses, seeks and hands off gaplessly to a queued next track, checked
by offline tests and by ear in the app. Phase 2 (metadata and library) is in
progress: the core reads tags and embedded art with TagLib, the Rust library
database and incremental folder scanner are in place, and the library is
browsed a page at a time under configurable sort/grouping rules
(2026-09-26). Security-scoped bookmarks are the last Phase 2 item.

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
| `anomp_core` static lib; `FormatRegistry` registers `FFmpegAudioFormat` only | `core/src` |
| `FFmpegAudioFormat`: FFmpeg-backed JUCE reader (float output, gapless trimming, exact seeks and lengths) | `core/src/FFmpegAudioFormat.*` |
| 21 committed audio fixtures (750 KB) of one deterministic chirp (two of them tagged, with cover art), and their generator | `core/tests/fixtures/`, `scripts/make-test-fixtures.py` |
| C API: `anomp_version`, `anomp_can_decode_extension`, `anomp_read_tags`, `anomp_engine_*` (device, player, events) | `core/include/anomp/anomp.h` |
| TagLib 2.3.2 (MPL, static, from the pinned release tarball) and `TagReader`: tags, MusicBrainz IDs, embedded art | `cmake/TagLib.cmake`, `core/src/TagReader.*` |
| `PlayerEngine`: load/play/pause/stop/seek/volume, gapless next track, resampling to the device rate | `core/src/PlayerEngine.*` |
| 35 passing Catch2 tests (~1870 assertions) | `core/tests` |
| Tauri 2 app (SvelteKit + `adapter-static`, Svelte 5, TS) showing `anomp_version()` via the `core_version` command | `app/` |
| `build.rs` builds `anomp_core` with the `cmake` crate and links it plus the Apple frameworks | `app/src-tauri/build.rs` |
| Safe Rust wrappers over the C API | `app/src-tauri/src/anomp.rs` |
| Library: SQLite schema and migrations, folders, incremental parallel scanner, sort/grouping rules, paged browsing, `library_*` commands | `app/src-tauri/src/library/` |
| 49 passing `cargo test` tests (C API wrappers, schema, folders, scanner, sort keys, genres, rules, browsing) | `app/src-tauri/src` |
| `AudioEngine` + `anomp_engine_*` C API: default output device, test tone, device-change event | `core/src/AudioEngine.*` |
| Pinned LGPL audio-only FFmpeg 9.0.2 (universal dylibs) and `FFmpeg::*` CMake targets | `scripts/build-ffmpeg.sh`, `cmake/FFmpeg.cmake` |
| Main-thread engine host; `audio_device_name`, test-tone and `player_*` commands; `player-*` events | `app/src-tauri/src/audio.rs` |
| Dev UI: device name, test tone, player panel (file picker or typed paths, transport, seek, volume, next track), library panel (add/scan/remove folders, browse by sort rule) | `app/src/routes/+page.svelte` |
| Tauri dialog plugin (`dialog:allow-open`) for the dev UI's file picker | `app/src-tauri/src/lib.rs`, `app/src-tauri/capabilities/default.json` |

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
  and freetype on Linux). Done in Phase 1: the core links only
  `juce_audio_formats` and `juce_audio_devices` (plus their dependencies).

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
- [x] **Spike: JUCE inside a Tauri process.** Tauri (tao) owns the main thread and
  run loop. Verify that JUCE's `AudioDeviceManager` plays audio when
  initialised via `ScopedJuceInitialiser_GUI` without a `JUCEApplication`, and
  that device-change notifications still arrive.
  Findings (2026-09-25):
  - Works with no JUCE changes. Without a `JUCEApplication`, JUCE leaves
    `NSApp`'s delegate alone and posts its messages to a CFRunLoop source on
    the *main* run loop, which tao's `[NSApp run]` services. The one rule:
    create, use and destroy the engine on the main thread.
  - `AudioEngine` (core) owns the `ScopedJuceInitialiser_GUI`, the
    `AudioDeviceManager` and a test-tone source; the C API is
    `anomp_engine_*` plus one event callback (`ANOMP_EVENT_DEVICE_CHANGED` for
    now; Phase 1 adds playback events to the same callback).
  - Rust keeps the engine in a main-thread `thread_local`, reaches it from
    commands directly or through `run_on_main_thread`, and drops it on
    `RunEvent::Exit` so JUCE shuts down before the process exits.
  - Device changes checked by creating and removing a public CoreAudio
    aggregate device while the app runs: each change reached the Svelte UI
    as an `audio-device-changed` event.
- [x] **Exit:** a tone plays from a Tauri app on macOS. Confirmed by ear
  2026-09-25. **Phase 0 complete.**

### Phase 1 — Playback engine (C++ core)
- [x] `scripts/build-ffmpeg.sh`: fetch a pinned FFmpeg release, configure it
  LGPL/audio-only/shared (see §4.3), and install into
  `third_party/ffmpeg/<platform>-<arch>/`. It covers macOS arm64 and x86_64
  (merged with `lipo`) now; Phase 8–10 add iOS, Linux and Windows. CMake finds
  the result via an imported target; CI caches it by version and flags.
  Done 2026-09-25:
  - Pinned FFmpeg **9.0.2**, SHA-256 checked (pin verified once against the
    release GPG signature). Output `third_party/ffmpeg/macos-universal/`:
    4 dylibs (avformat, avcodec, swresample, avutil), 4.6 MB universal,
    `@rpath` install names, macOS 14 minimum, NASM asm on x86_64.
  - `BUILD_INFO` records version, checksum and configure flags; the script
    skips the build when it matches (`--force` rebuilds), and it is the
    CI cache key and the source for the §8.2 FFmpeg notice.
  - `cmake/FFmpeg.cmake` defines `FFmpeg::avformat` etc. (`ANOMP_FFMPEG_DIR`
    overrides the location); configure fails with instructions if the
    script hasn't run.
  - `core/tests/FFmpegBuildTests.cpp` checks LGPL, every planned demuxer and
    decoder, and no encoders.
- [x] `FFmpegAudioFormat` / `FFmpegAudioFormatReader`: a JUCE `AudioFormat` backed
  by libavformat + libavcodec, converting to float with libswresample.
  Requirements:
  - sample-accurate seeking;
  - encoder delay/padding trimming for gapless MP3/AAC/Opus;
  - accurate duration;
  - opens from a path (UTF-8) or a JUCE `InputStream` via a custom `AVIOContext`.

  Done 2026-09-25. How it works, and what the probes of each container found:
  - Every read goes through an `AVIOContext` over the JUCE `InputStream`
    (files, bookmarks and memory alike). Output is planar float at the
    file's own rate; JUCE resamples to the device.
  - Positions count from the first decoded sample, anchored on its timestamp.
    Libavcodec already trims encoder delay (LAME header, M4A edit list, Opus
    pre-skip), so trimming falls out of that.
  - **Length** = min(header duration, measured end of decoded output). The
    header over-reports for Opus (includes pre-skip) and FFmpeg misses the
    end trim for some Vorbis files; neither under-reports. The end is
    measured by decoding the last ~2 s.
  - **Seeking**: timestamp seek to `target − preroll` (16384 samples for
    lossy codecs; VBR MP3 and Opus need more than 4096 to match a straight
    decode), then decode and discard. Targets within 1 s of either end
    rewind or seek earlier instead, because timestamps there are unreliable
    (a seek into the first or last Ogg page comes back mislabelled by up to
    a block). Rewinding reopens the demuxer: seeking to 0 is *not* the same
    for AAC/Vorbis (need the previous packet) or MP3/Opus (start trimming).
  - MP3 and raw ADTS files get a demux-only scan on open that builds an
    exact seek index (the Xing TOC is approximate on VBR files; FFmpeg's
    `usetoc` is off). Cost on a 5-minute VBR MP3: 7 ms to open, <1 ms per
    seek.
  - Known limits, inherent in the files: ADTS AAC and header-less MP3 carry
    no gapless info, so their encoder priming (1024 / 1105 samples) stays
    in; WMA (ASF) timestamps are in milliseconds, so WMA seeks decode from
    the start; AAC noise substitution (PNS) is random, so AAC output after
    a seek differs slightly (not in position) from a straight decode.
- [x] Replace `registerBasicFormats()` in `FormatRegistry` with
  `FFmpegAudioFormat`. Keep JUCE's WAV writer only for generating test
  fixtures. JUCE's own MP3/FLAC/Ogg decoders are now compiled out.
- [x] `PlayerEngine`: `AudioDeviceManager` → `AudioSourcePlayer` →
  `PlayerEngine`, each track an `AudioFormatReaderSource` behind a
  `BufferingAudioSource` on a shared read-ahead thread.
- [x] Commands: load, play, pause, stop, seek, volume. State machine with
  thread-safe status reads.
- [x] Queue with gapless transition: pre-open the next reader and hand it off at
  end of stream.
- [x] C API: engine create/destroy, commands, `anomp_engine_set_event_callback`
  (state/position/track-ended).

  Done 2026-09-25. Design:
  - **No `AudioTransportSource`** (the original plan). It wraps one source
    with its own resampler, so two tracks can't be joined sample-exactly.
    `PlayerEngine` is itself the `AudioSource`: it joins the current and
    next track at the file rate, then resamples the joined stream once to
    the device rate. A 44.1 kHz album on a 48 kHz device is still gapless.
    Consecutive tracks at different rates switch at a chunk boundary (a few
    ms of silence).
  - **Resampling** uses JUCE's `WindowedSincInterpolator` (200 taps), not
    `ResamplingAudioSource` (linear interpolation, audible aliasing). It is
    bypassed when the rates match, so output is then bit-exact. The sinc
    doesn't lower its cutoff when downsampling (e.g. 96 kHz files on a
    48 kHz device), so content above the device's Nyquist can alias.
    Replace it if that proves audible.
  - **The host owns the queue.** The core holds the current track and one
    pre-opened next track; on `TRACK_ENDED` with `advanced` the host sets
    the following one. Order, shuffle, repeat and persistence belong with
    the library in Rust (Phase 2/3).
  - **Threading:** commands take a lock that the audio callback also
    holds, and they never open or free a file inside it. Tracks are opened
    and freed on the message thread. A track retired by a hand-off on the
    audio thread is freed at the next event dispatch. Events are polled
    every 50 ms by a timer and never fire from inside a command.
  - Play and pause fade over one audio block; volume changes ramp the same
    way. Stop, seek and load cut.
  - Known limits: the resampler's ~100-sample latency and its last few
    input samples are dropped when the last track ends (about 2 ms). Files
    with more than two channels play their first two. Opens are
    synchronous on the main thread (7 ms for a 5-minute VBR MP3).
- [x] **Tests:** decode every format in §4.3 from small fixture files (checking
  sample rate, channels, length and a checksum of the decoded samples), plus
  state transitions, seek accuracy, and the gapless handoff, run
  against generated WAV/FLAC fixtures with a null/offline device
  (`AudioProcessorGraph` or direct `getNextAudioBlock` calls, so no hardware
  is needed in CI).
  Decoding part done: `FFmpegAudioFormatTests.cpp` checks every fixture
  against the source signal (lossless bit-exact; lossy aligned to the exact
  sample by cross-correlation, since lossy float output isn't bit-stable
  across CPUs), exact lengths, seeks against a straight decode, memory
  streams and bad input. `PlayerEngineTests.cpp` renders `PlayerEngine`
  offline, with and without read-ahead. It checks state transitions,
  bit-exact playback and seeks, fades, volume, and gapless hand-offs across
  formats against the decoded files joined end to end. A 44.1 kHz pair on a
  48 kHz device matches the joined files resampled as one stream, and a
  next track at a different rate is also covered. `CApiTests.cpp` covers
  the player C API, including a null engine.
- [x] **Exit:** a library of MP3/FLAC files plays through the app with
  working seek and volume, and a gapless album plays without clicks at track
  boundaries. Confirmed by ear 2026-09-26 (MP3 album with continuous tracks,
  queued one at a time through the dev UI). **Phase 1 complete.**

### Phase 2 — Metadata and library
- [x] Core: add TagLib (FetchContent) with `anomp_read_tags(path) → struct` for
  title, artist, album, album artist, track/disc, year, genre, duration,
  MusicBrainz IDs (if tagged), and embedded art.

  Done 2026-09-26:
  - TagLib **2.3.2** from the release tarball (SHA-256 pinned; it bundles
    utfcpp, which is forced over any installed copy), static, no zlib
    (compressed ID3v2 frames are rare and are skipped without it). TagLib's
    target only exports include paths for installs, so `cmake/TagLib.cmake`
    adds them. `build.rs` links `libtag.a` next to the core.
  - `anomp_read_tags (path, flags, error)` returns an `anomp_tags*` freed
    with `anomp_tags_free`; strings are UTF-8 and never null. Unlike the
    engine it may be called from any thread, for the scanner. The embedded
    picture is copied only with `ANOMP_TAGS_PICTURE`, so a scan doesn't
    copy art for every track.
  - Fields come from TagLib's unified `PropertyMap`, so every format maps the
    same way: several values join with "; "; track/disc accept "3/12" or a
    separate TRACKTOTAL/DISCTOTAL; the year is the first four digits of
    DATE. MusicBrainz IDs are named after their entity
    (`musicbrainz_recording_id` is Picard's "track id").
  - The picture is the one typed "Front Cover", else the first (MP4 cover
    atoms have no type). An empty MIME type is sniffed from JPEG/PNG magic.
  - Files are opened through a read-only `FileStream`.
  - **Duration comes from TagLib's headers**, which is cheap enough for
    scanning but approximate for lossy files: MP3 lengths include encoder
    delay and padding (~40 ms). The player's `anomp_engine_duration` is the
    exact one. If TagLib gives no duration, the FFmpeg reader supplies the
    audio properties; a file neither can read is an error.
  - Known limits (of the formats): ASF/WMA holds one artist string, and
    ID3v2 keeps the recording ID in a UFID frame.
  - Tests: `TagReaderTests.cpp` reads an ID3v2.3 MP3 and a FLAC tagged by
    FFmpeg (non-ASCII text, MusicBrainz IDs, PNG cover), checks untagged
    fixtures' properties, round-trips tags and two pictures written by TagLib
    through all ten formats, and checks errors and that files are untouched.
- [x] Rust: SQLite schema (tracks, albums, artists, folders, settings, mb_cache),
  migrations, and an incremental folder scanner (mtime/size change detection).

  Done 2026-09-26 (`app/src-tauri/src/library/`):
  - `rusqlite` with its bundled SQLite (the same version on every platform,
    iOS included). The database is `library.sqlite3` in the app data folder,
    in WAL mode with foreign keys on. Migrations are numbered SQL files in
    `library/migrations/`, applied in order and counted in `PRAGMA
    user_version`. A database from a newer app version is refused, not
    touched. Tables are `STRICT`.
  - **Tracks store their path relative to their folder** ('/'-separated), so
    a folder that moves keeps its tracks. On iOS the app container path
    changes between installs, and bookmarks resolve to the new path.
    `folders.bookmark` (a BLOB, unused so far) is ready for the sandbox item
    below. Adding a folder canonicalizes its path and refuses one that is
    inside, or contains, a folder already in the library.
  - Artists have one row per name (`COLLATE NOCASE`, ASCII only). An album is
    (album artist, title), and the album artist falls back to the track
    artist. A unique index on `IFNULL(artist_id, 0)` covers albums without
    one. Tracks point at their artist, album and effective album artist.
    Release MBIDs go on albums, artist MBIDs on artists, recording and
    release-track MBIDs on tracks. Albums and artists that no track uses are
    deleted after each scan and folder removal. `settings` (key → JSON) and
    `mb_cache` (request → response) are schema only, for Phases 4 and 6.
  - **Scanner:** walks the folder with `walkdir`, following symlinks (loops
    are detected and reported) and skipping dotfiles and dot-folders (which
    also skips macOS's `._` AppleDouble files). It keeps the files whose
    extension the core decodes. A file is re-read when its size or
    modification time (ns) differs. Tags are read without pictures, on one
    thread per core, and written 256 files per transaction, with a progress
    callback after each batch. Updates are upserts, so track IDs stay stable.
    Unreadable files are reported and left out (a track that becomes
    unreadable is removed). If the folder itself is missing (e.g. an
    unmounted drive), the scan fails and changes nothing. Tracks under a
    subfolder that can't be read are kept.
  - Tauri: `library_folders`, `library_add_folder`, `library_remove_folder`,
    `library_tracks` and `library_scan` (one folder or all; one scan at a
    time; runs on a blocking thread with its own connection and emits
    `library-scan-progress`). The dev UI has a library panel; double-clicking
    a track plays it.
  - Speed (release build on the dev Mac, warm cache): 5,000 small fixture
    files take 0.46 s to scan the first time and 27 ms to rescan. Phase 7
    measures real libraries.
  - Known limits: compilations tagged without an album artist split into one
    album per track artist. Several artists in one tag ("A; B") count as one
    artist. Folder overlap checks compare paths case-sensitively.
    `library_tracks` returned every row unsorted; `library_browse` (below)
    replaced it. Scans run only when asked, with no rescan at launch and no
    file watching yet.
- [x] Logical sort/grouping rules: by album artist → album → disc/track, by folder,
  by genre, by year. Rules are configurable (feeds the admin screen).

  Done 2026-09-26 (`library/rules.rs`, `browse.rs`, `sort_key.rs`,
  `genres.rs`):
  - **Rules are data.** A rule is an id, a name, a list of grouping levels
    (album artist, artist, album, genre, year, or the folder tree on its
    own) and a track order (album artist, artist, album, year, disc, track
    number, title, path). The built-ins are album artist → album, genre →
    album artist → album (both with tracks by disc, number, title, path),
    year → album, and folder. The rules and the ignored leading articles
    (default "The" and "A") are one JSON value under `library.sort` in
    `settings`; there is no schema change. Reading it keeps what is
    usable: a rule that doesn't parse or validate (e.g. from a newer
    version) is dropped, and anything missing or invalid falls back to the
    built-ins.
  - **Browsing is one node at a time.** `library_browse(ruleId, path,
    offset, limit)` returns a page of the node's groups (key, display name,
    track count; albums also carry album artist and year) or tracks, and
    the node's total. The path holds one group key per level (an id, a
    year, a genre; null for "Unknown …"). Under the folder rule it is a
    library folder id and then folder names, and a node lists subfolders,
    then tracks, paged as one list. Paging is limit (at most 1000) + offset
    over an order that always ends in a unique key, so pages don't repeat
    or skip rows. SQL is assembled only from fixed fragments chosen by the
    rule; every value is a bound parameter.
  - **Sort order** comes from `anomp_sort_key(text, articles)`, an SQL
    function registered on every connection in `db::configure`. It returns
    a BLOB that sorts bytewise: text folded with ICU (NFKD, marks removed,
    lowercase, so "élodie" sorts with "Elodie"), digit runs by value
    ("Track 2" before "Track 10"), '/' first (a folder's files before its
    neighbours'), and an optional leading article skipped ("The Beatles"
    under B; display names are unchanged). This replaces the planned custom
    collation: a key is computed once per row and compared with memcmp,
    whereas a collation folds both strings on every comparison, and the
    articles arrive as a bound parameter rather than per-connection state.
    Ties fall back to the original text, then the id. Missing values sort
    last. A missing disc number counts as disc 1, since single-disc albums
    usually have none. Genres that differ only in case or accents are one
    group. `icu_normalizer`/`icu_properties` were already linked (via
    Tauri's `url`), so folding adds no new code.
  - **Genres:** a track is under each of its genres. `anomp_genres(tag)`
    splits a tag at ';' (the core joins values with "; ") into a JSON
    array for SQLite's `json_each`, and `anomp_has_genre(tag, genre)`
    filters. Splitting at query time avoids a `track_genres` table and a
    scanner change. A recursive CTE did the same, but at 4× the cost.
  - **Year** is the album's year (the earliest among its tracks, from an
    `album_years` CTE joined only when the rule groups by year), else the
    track's own.
  - Speed (release build, 50,000 synthetic tracks in an in-memory database,
    count + page of 200): album artists 28 ms, one album's tracks 0.2 ms,
    genres 71 ms, one genre's artists 63 ms, years 62 ms, one year's albums
    63 ms, every track by title 34 ms (85 ms at offset 40,000), a folder of
    2,000 subfolders 54 ms. Group levels scan every track, so they grow
    linearly. A keyset cursor or cached keys are the next steps if Phase 7
    finds this too slow.
  - Tauri: `library_browse`, `library_sort_settings`,
    `library_save_sort_rule` (adds, or replaces by id),
    `library_remove_sort_rule` (not the last), `library_set_ignored_articles`
    and `library_reset_sort_settings`. `TrackSummary` gained genre and year.
    The dev UI has a rule picker, breadcrumbs, a click-through list with
    "Show more", and double-click to play.
  - Known limits: compilations tagged without an album artist still split
    into one album per track artist (left for now). Folding doesn't
    decompose letters like "ø", "ł" or "ß", and CJK sorts by code point
    rather than by reading. Articles are skipped only before a space ("The
    Beatles", not "L'Amour"). Genres split only at ';', not '/' or ','
    ("Hip-Hop/Rap" is one genre). Two artists whose names differ only in
    non-ASCII case (the schema's `NOCASE`) are separate, adjacent groups.
    The genre group's display name is the spelling that sorts first
    bytewise, not the most common one. Pages come from separate queries, so
    a scan between them can shift rows.
- macOS sandbox: user-selected folders plus security-scoped bookmarks. Store
  bookmarks, not raw paths, so the same model works on iOS later.
- **Tests:** Catch2 tests for tag reading over fixture files (done, above);
  `cargo test` for schema, scanner (done: `library/db.rs`, `library/mod.rs`
  and `library/scanner.rs`, over temp folders of fixture copies) and sort
  rules (done: `sort_key.rs`, `genres.rs`, `rules.rs` and `browse.rs`, over
  in-memory databases of synthetic rows: disc/track order, missing values,
  accents and case, articles, natural numbers, the folder tree, multiple
  genres, album years, paging, and saving, resetting and falling back from
  bad stored settings).

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
| JUCE message loop vs. Tauri's main-thread ownership | **Resolved on macOS (Phase 0):** JUCE's messages ride tao's main run loop; keep the engine on the main thread. Re-check on iOS (Phase 8), Linux and Windows |
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
