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
started (2026-09-26): steps 4.1–4.3 (source settings, HTTP client, folder
art, MusicBrainz matching) are done.

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
| C API: `anomp_version`, `anomp_can_decode_extension`, `anomp_read_tags`, `anomp_engine_*` (device, player, events, advance count), `anomp_media_controls_*` | `core/include/anomp/anomp.h` |
| TagLib 2.3.2 (MPL, static, from the pinned release tarball) and `TagReader`: tags, MusicBrainz IDs, embedded art | `cmake/TagLib.cmake`, `core/src/TagReader.*` |
| `PlayerEngine`: load/play/pause/stop/seek/volume, gapless next track, resampling to the device rate | `core/src/PlayerEngine.*` |
| `MediaControls`: OS Now Playing info and remote commands (Apple: `MPNowPlayingInfoCenter`/`MPRemoteCommandCenter`; no-op fallback elsewhere) | `core/src/MediaControls*` |
| 52 passing Catch2 tests (~2090 assertions) | `core/tests` |
| Tauri 2 app (SvelteKit + `adapter-static`, Svelte 5, TS) showing `anomp_version()` via the `core_version` command | `app/` |
| `build.rs` builds `anomp_core` with the `cmake` crate and links it plus the Apple frameworks | `app/src-tauri/build.rs` |
| Safe Rust wrappers over the C API | `app/src-tauri/src/anomp.rs` |
| Library: SQLite schema and migrations, folders, incremental parallel scanner, sort/grouping rules, paged browsing, FTS5 search, cover art (`anomp-art` URI scheme), `library_*` commands | `app/src-tauri/src/library/` |
| Play queue: order, shuffle, repeat, gapless hand-off across it, persistence, `queue_*` commands and `queue-changed` event | `app/src-tauri/src/queue/` |
| OS media integration host: Now Playing kept in step with the queue and player, remote commands routed to the queue, artwork | `app/src-tauri/src/media.rs` |
| Metadata sources (Phase 4, in progress): source settings and order, HTTP client with rate limits, backoff and response cache, folder-image art, MusicBrainz search/lookup and album matching | `app/src-tauri/src/metadata/` |
| 133 passing `cargo test` tests (C API wrappers, schema, folders, scanner, sort keys, genres, rules, browsing, search, art sources, queue, Now Playing sync, metadata settings, HTTP client, MusicBrainz parsing and matching), plus 3 ignored 50,000-track benchmarks and 1 ignored live MusicBrainz test | `app/src-tauri/src` |
| `AudioEngine` + `anomp_engine_*` C API: default output device, test tone, device-change event | `core/src/AudioEngine.*` |
| Pinned LGPL audio-only FFmpeg 9.0.2 (universal dylibs) and `FFmpeg::*` CMake targets | `scripts/build-ffmpeg.sh`, `cmake/FFmpeg.cmake` |
| Main-thread engine host; `audio_device_name`, test-tone and `player_*` commands; `player-*` events | `app/src-tauri/src/audio.rs` |
| Player UI: sidebar (views, folders, scanning), browser, search, queue panel, now-playing bar; responsive down to 360 px, light and dark | `app/src/routes/+page.svelte`, `app/src/lib/` |
| Developer page: device name, test tone, loading typed paths straight into the engine, event log | `app/src/routes/dev/+page.svelte` |
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
- [x] macOS sandbox: user-selected folders plus security-scoped bookmarks. Store
  bookmarks, not raw paths, so the same model works on iOS later.

  Done 2026-09-26 (`core/src/FolderAccess*`, `library/access.rs`,
  `Entitlements.plist`):
  - **Core:** `FolderAccess` is the platform interface: create a bookmark
    for a folder, resolve one and hold access while the object lives. The
    Apple implementation (`FolderAccess_apple.mm`, ARC) makes read-only
    security-scoped bookmarks on macOS (plain ones on iOS, where the scope is
    implicit; not yet built) and resolves them without UI or mounting. Other
    platforms (`FolderAccess_unsandboxed.cpp`) store the UTF-8 path. The C
    API is `anomp_bookmark_create`/`_free` and
    `anomp_folder_access_start`/`_path`/`_is_stale`/`_stop`, callable from
    any thread. NSURL spells resolved names decomposed (NFD), so the path
    goes through `realpath` to match what `add_folder` stores.
  - **Rust:** `add_folder` saves the bookmark in `folders.bookmark` (no
    schema change). `access::open_folder` resolves it before each scan and
    before each `player_load`/`player_set_next` of a file under a library
    folder, and holds access until the scan ends or the file is open (the
    reader keeps its stream, so rewinds don't reopen). If the folder moved,
    its stored path follows (after the overlap check), and so do its tracks,
    which are stored relative to it. A stale bookmark is replaced. A folder
    saved before this change gets a bookmark the next time it can be read.
    An unresolvable bookmark reports "Folder not available" and changes
    nothing.
  - **Sandbox:** `Entitlements.plist` has the app sandbox, user-selected
    read-only files, app-scoped bookmarks and network client (for Phase 4;
    §8.3 lists it). `tauri.conf.json` signs the bundle ad hoc (`"-"`) so
    local builds carry the entitlements, with the hardened runtime off: it
    loads only libraries signed by the app's team, and ad-hoc signatures
    have none, so no separately signed dylib could load. Release signing
    (§8.3) sets a Developer ID and turns the hardened runtime back on.
    `tauri dev` is unsigned and so unsandboxed. The sandboxed app keeps its
    database in `~/Library/Containers/dev.anomp.player/`, separate from the
    unsandboxed one, so folders must be added again there.
  - **Release builds and FFmpeg:** `build.rs` now links clang's runtime
    (`libclang_rt.osx.a`, found with `xcrun`): JUCE's `@available` checks
    need `___isPlatformVersionAtLeast`, which debug builds took from Rust's
    std but release LTO dropped, so no release build had linked. The bundle
    embeds FFmpeg (`bundle.macOS.frameworks`, pulled forward from §8.3):
    Tauri copies the four dylibs under their install names into
    `Contents/Frameworks`, signs them, and adds the
    `@executable_path/../Frameworks` rpath. Only debug builds keep an rpath
    into `third_party/` (for `tauri dev` and `cargo test`); a release app
    never searches there, and a path under `~/Desktop` would make macOS ask
    for access. `cargo test --release` finds FFmpeg through
    `DYLD_LIBRARY_PATH` from `app/src-tauri/.cargo/config.toml`. The
    framework list names the libraries' major versions, so it changes with
    the FFmpeg pin (a stale name fails the bundle step).
  - Tests: `FolderAccessTests.cpp` (round trip, a moved folder, errors,
    null handles) and `cargo test` (`anomp.rs`; `access.rs`: saved on add,
    backfill, a moved folder, an unavailable folder; `scanner.rs`: a moved
    folder rescans with the same track ids, a deleted one keeps its tracks).
    These run unsandboxed. The bundle from `npm run tauri build --
    --bundles app` launches in the sandbox as built: FFmpeg loads from
    `Contents/Frameworks`, the database is in the container, and no
    denials are logged.
  - Checked by hand 2026-09-26 in the sandboxed bundle: add a folder, quit,
    relaunch, then scan it and play a track from it.
  - Known limits: symlinks leading out of a library folder aren't readable
    in the sandbox and are reported as scan failures.
- **Tests:** Catch2 tests for tag reading over fixture files (done, above);
  `cargo test` for schema, scanner (done: `library/db.rs`, `library/mod.rs`
  and `library/scanner.rs`, over temp folders of fixture copies) and sort
  rules (done: `sort_key.rs`, `genres.rs`, `rules.rs` and `browse.rs`, over
  in-memory databases of synthetic rows: disc/track order, missing values,
  accents and case, articles, natural numbers, the folder tree, multiple
  genres, album years, paging, and saving, resetting and falling back from
  bad stored settings). **Phase 2 complete.**

### Phase 3 — Frontend: core player UI
- [x] Play queue in Rust (PLAN.md Phase 1: the host owns the queue).

  Done 2026-09-26 (`app/src-tauri/src/queue/`, `core/src/PlayerEngine.*`):
  - **Model and host are separate.** `queue/model.rs` is the logic, driving
    the engine through a small `Player` trait, so its tests run against a
    fake with no audio device. `queue/mod.rs` hosts it in a main-thread
    thread-local next to the engine: every operation runs on the main
    thread (`run`, via `audio::on_main`), so the queue never waits for the
    engine from another thread while holding a lock. Commands that need the
    database first (track ids → titles, a browse node → its tracks) do that
    on a blocking thread, then hop over. `TrackEnded` is handled inside
    the engine's event callback, which the C API allows: the engine has
    finished with what it reports before it calls back. (This note first
    said it was deferred to the next turn of the main loop; it never was,
    since `run_on_main_thread` runs at once on the main thread. Corrected
    2026-09-26, when `anomp.h` came to state the rule and a Catch2 test to
    cover it.)
  - **State:** the items (a uid per item, so a track can be queued twice and
    survives reorders) in play order, the current index, repeat
    (off/all/one), and with shuffle on, the order before shuffling.
    Shuffle reorders only the items after the current one (Fisher–Yates on
    a seeded xorshift); turning it off restores the order before, keeping
    the current item; tracks added meanwhile keep their place in both
    orders. The order stays put while playing and across launches. With
    repeat all, each pass gets a new order: when the last item starts, it
    moves to the top and the rest are shuffled after it (the track that
    played just before it doesn't come straight back), and the new first
    item is armed as its gapless next.
  - **Gapless across the queue:** the queue records which item it armed as
    the engine's next track. On `TrackEnded` with `advanced` it moves to
    that item and arms the one after. After *every* change (reorder,
    remove, add, shuffle, repeat, jump) it recomputes what should follow
    the current item and calls `set_next` only if that differs, so edits
    mid-track keep the hand-off gapless. Repeat one arms the current track
    again; repeat all wraps.
  - **Core change: `anomp_engine_advance_count`.** The engine hands off on
    the audio thread but reports it up to 50 ms later, so a queue edit in
    that window would re-arm against the track that just finished, and a
    `load` (Next pressed) left a stale `advanced` event behind that moved
    the queue one item too far. The count is the number of hand-offs so
    far, updated as they happen; every queue operation first applies any it
    hasn't seen (`reconcile`), which makes the event a harmless duplicate.
    `load` now also drops an end-of-track event not yet reported, which
    belonged to the track it replaced. Catch2: `PlayerEngineTests` (counted
    before reported, a load keeps a pending hand-off, a load drops a
    pending end) and `CApiTests`.
  - **Commands:** `queue_state`, `queue_play` (ids from an index),
    `queue_play_node` (a browse node's tracks from a track), `queue_add`
    and `queue_add_node` (after the current item or at the end),
    `queue_remove`, `queue_move`, `queue_clear`, `queue_jump`, `queue_next`,
    `queue_previous`, `queue_toggle`, `queue_seek`, `queue_set_shuffle`,
    `queue_set_repeat`. Next and previous keep the play state (paused stays
    paused). Previous restarts the track after 3 s, else goes back (at the
    first item, restarts it). Removing the current item starts the one
    after it (or, if it was last, selects the one before without playing).
    `queue::next`/`previous`/`toggle`/`seek` are plain main-thread
    functions, ready for the media controls item below.
  - **Node tracks:** `browse::node_track_ids` lists every track under a
    node in exactly the order browsing shows them (a test walks `browse`
    depth first and compares), so "play this artist" plays album by album.
    A folder node lists its own files, or with `recursive` its whole tree
    with subfolders first (a stable sort in Rust over SQL's track order).
    Under a genre level a track is listed once, ordered by its whole genre
    tag. A node may name a stored rule or carry a whole rule (`RuleSpec`),
    so search can play an album whatever rules exist.
  - **Unreadable tracks** (missing file, unavailable folder, a file the
    decoder refuses) are skipped when starting or arming, marked
    unavailable, and reported in the next `queue-changed`; they stay in the
    queue, and jumping to one tries it again. If nothing opens, nothing
    plays. Every open goes through `open_folder_of`, held until the file is
    open.
  - **Event:** `queue-changed` carries a `QueueState` (current index and
    item, shuffle, repeat, next/previous availability, unavailable items,
    skipped tracks). The whole list is included only when it changed, so
    moving to the next track doesn't resend it.
  - **Persistence:** `player.queue` in `settings` (no migration): track
    ids, the pre-shuffle order, the current index, the position in the
    track, repeat and the volume. Saved after every change, on pause and at
    exit. At launch the queue is restored *not loaded*: nothing opens and
    nothing plays until asked; the first play opens the current track at
    the saved position (seeking before that moves where it will start).
    Tracks no longer in the library are dropped.
  - The dev page's `player_load`/`player_set_next` detach the queue until
    it next starts a track itself.
  - Speed (release, 50,000 synthetic tracks): the ids of every track under
    the album-artist rule's top node, in order, 190 ms; a library folder's
    whole tree 113 ms; one artist's tracks 0.1 ms; their queue metadata
    60 ms; the list as JSON 6.6 MB, 8 ms to serialize.
  - Known limits: queued items keep the tags they had when queued until a
    scan refreshes them. The saved queue is one JSON value rewritten on
    every change (about 350 KB for 50,000 tracks). A 50,000-track queue
    sends its whole list (6.6 MB) whenever the list changes. With shuffle
    and repeat all, drawing a new pass drops the previous pass's order, so
    previous from the top of a pass goes to the end of the new one rather
    than to the track that played before. Next at the end of the queue with
    repeat off does nothing.
  - Tests (`cargo test`, `queue::model`): advancing with the next track
    armed, starting mid-list, repeat modes, shuffle order stability (and
    unshuffling, and after a save and restore), a new order each pass with
    repeat all, additions while shuffled,
    previous restarting, paused next, edits while playing re-arming, a
    hand-off not yet reported, a late edit after the last track,
    unreadable tracks, restoring paused at the saved position, detaching.
- [x] Search.

  Done 2026-09-26 (`library/search.rs`, `migrations/002_search.sql`):
  - **FTS5, not a LIKE scan.** rusqlite's bundled SQLite is built with
    `SQLITE_ENABLE_FTS5`. Three contentless tables (`contentless_delete=1`,
    rowid = the indexed row's id): `artists_search(name)`,
    `albums_search(title, artist)` and `tracks_search(title, artist, album,
    album_artist)`. The `unicode61 remove_diacritics 2` tokenizer folds
    case and accents itself, so no `anomp_*` function is involved (they
    must stay out of the schema). Triggers keep the indexes in step and
    fire only when an indexed value changes; migration 002 indexes existing
    rows. A track without a title is indexed by its file name.
  - **Query:** each whitespace-separated word becomes a quoted prefix
    phrase (`"ac/dc"*`), so FTS5 syntax typed in is plain text and every
    word must match the start of a word in some field. The expression is a
    bound parameter. Results are ranked by bm25, then by sort key.
    `library_search(query, kinds?, offset, limit)` returns artists (with
    their track counts), albums (with album artist and year) and tracks,
    each a page plus a total.
  - Speed (release, 50,000 synthetic tracks with an 8,000-word vocabulary,
    count + 50 of each kind): a word in 3% of tracks ("love") 4.2 ms, a rarer
    word 2 ms, two words 0.5 ms, no match 0.1 ms; the worst case, a one- or
    two-letter prefix matching 28% of tracks, 31 ms. For comparison a LIKE
    scan over pre-folded copies of the same fields takes 5–10 ms just to
    count, before sorting, and would need a column filled from Rust (a
    migration can't call the folding function). Cost to scanning: 50,000
    track inserts take 2.5 s with the triggers instead of 1.0 s; rescans
    touch only changed files.
  - Known limits: words match from their start ("tles" doesn't find
    "Beatles"). unicode61 folds without compatibility decomposition, so
    ligatures like "ﬁ" differ from browse's folding; like browse, "ø", "ł"
    and "ß" aren't folded. unicode61 doesn't segment CJK text, so a run of
    CJK characters is one word, matched only from its start.
- [x] Cover art.

  Done 2026-09-26 (`library/art.rs`):
  - Served to the webview on the `anomp-art` URI scheme
    (`anomp-art://localhost/album-<id>`, or `/track-<id>` for tracks
    without an album), so an `<img loading="lazy">` shows it with no IPC
    round trip. Requests are answered on a blocking thread.
  - `art::lookup` is the only way in; its one source is the embedded
    picture (the album's first three tracks, in disc/track order, are
    tried), read through `open_folder_of`. Phase 4 adds the Cover Art
    Archive as a second source there.
  - An in-memory cache (64 MB, least recently used out, "no art" remembered
    too) means scrolling an album list doesn't re-read files. It is cleared
    after each scan, and the UI adds a scan counter to the URL so the
    webview's cache (`max-age` one day) doesn't keep old art.
  - Known limits: images are served at full size (no thumbnails), and the
    cache is memory only, so art is re-read after a relaunch.
- [x] Library browser (artists / albums / tracks / folders), search, queue
  view, now-playing bar with transport and seek.
- [x] Desktop window layout. Build it responsive (no fixed widths) so the
  iPhone/iPad layouts in Phase 8 are adjustments, not rewrites.

  Done 2026-09-26 (`app/src/lib/`, `app/src/routes/`):
  - **Plain CSS**, no UI framework: colours are custom properties on
    `:root`, redefined under `prefers-color-scheme: dark`; icons are inline
    SVG paths.
  - **Layout:** a CSS grid of sidebar | main | queue, with the now-playing
    bar along the bottom. Below 900 px the queue becomes an overlay; below
    640 px the sidebar becomes a drawer (☰), the bar stacks into two rows
    and the volume slider hides. Widths are `minmax`/`clamp` ranges, and
    track rows use a container query. The window can shrink to 360 × 480.
  - **Components:** `Sidebar` (sort rules as views; folders with add,
    rescan, remove and scan progress), `Header` (breadcrumbs, search box),
    `BrowsePane`, `SearchResults`, `QueuePanel`, `NowPlayingBar` with
    `SeekBar`, `ContextMenu`, `Toasts`, `Art`, and a `VirtualList` that
    renders only the rows in view and asks for pages as they come into
    view. Payload types and command wrappers are in `lib/api.ts`.
  - **State:** Svelte 5 runes in `lib/state/`. The position (every 50 ms) is
    a store of its own that only `SeekBar` reads, so it never re-renders a
    list. The queue mirror applies `queue-changed` and keeps the list
    unless the event carries a new one.
  - **Browsing:** pages of 200 are fetched as they scroll into view (sparse,
    so jumping to the end of a 50,000-track node fetches only that page).
    Click or Enter opens a group; double-click or Enter on a track plays
    the node's tracks from it; ▶ plays a group; right-click or ⋯ offers
    play next and add to queue. Album rows show art, artist and year.
    `library_browse` now runs on a blocking thread, off the main thread.
  - **Album order:** a rule now has `albumOrder`, `title` (the default, and
    what rules saved before it get) or `year` (each album's earliest year,
    oldest first, undated last, ties by title). Wherever a node lists
    albums, a Sort menu in its header switches it and saves it with the
    rule; playing a node (`node_track_ids`) follows the same order.
  - **Queue view:** the sidebar's Queue item (or the panel's expand button)
    shows the queue in the main area, where the side panel then hides;
    choosing a library view goes back.
  - **Search** runs 150 ms after typing stops and shows artists, albums
    and tracks with "More" for each. An artist or album opens in the
    browser under the first rule that starts with album artist (→ album)
    or artist, else plays; a track plays its album from that track.
  - **Queue view:** the current item highlighted and kept in view; click
    to jump, ✕ or Delete to remove, drag the handle to move, Clear. Drags
    use pointer events rather than HTML drag and drop, so they work on
    touch screens and aren't intercepted by the window's file drop.
  - **Keyboard:** space plays/pauses, ←/→ seek 5 s, ⌘←/→ previous/next,
    ⌘F focuses search (Escape clears it); lists take ↑/↓, Page Up/Down,
    Home/End and Enter.
  - The Phase 0–2 dev panels (test tone, typed paths, event log) moved to
    `/dev`; folder management and scan progress moved into the sidebar.
    Removing a folder asks first (the dialog plugin's `ask`; WKWebView
    shows no `confirm()`).
  - Known limits: no multi-select; space on a focused button plays/pauses
    rather than pressing it; the context menu has no arrow-key navigation.
- [x] **Check by hand** in `npm run tauri dev` and in the sandboxed bundle
  (`npm run tauri build -- --bundles app`): browse each built-in rule,
  search, play an album and hear gapless transitions across the queue,
  shuffle/repeat, reorder the queue while playing, quit and relaunch with
  the queue restored, and resize the window down to phone width.
  Done 2026-09-26. The checks led to three changes, recorded above: a new
  shuffle order each pass with repeat all, the queue view in the main area,
  and the album order. One relaunch came back with an empty queue; the
  saved state was correct and restores in tests from a copy of that
  database, so the cause wasn't found, and it was set aside.
- [x] OS media integration: macOS Now Playing info and remote commands
  (`MPNowPlayingInfoCenter`, `MPRemoteCommandCenter`) implemented in Obj-C++
  in the core. This covers media keys and Control Center; the same APIs serve
  the iOS lock screen in Phase 8. The remote commands call
  `queue::next`/`previous`/`toggle`/`seek` on the main thread; the Now
  Playing info comes from the `QueueState`.

  Done 2026-09-26 (`core/src/MediaControls*`, `app/src-tauri/src/media.rs`):
  - **Core:** `MediaControls` is platform-neutral. It keeps what is
    published (title, artist, album, playing or paused, elapsed time and
    duration, artwork bytes, whether next and previous are enabled), checks
    the values (non-finite ones refused, the position clamped to the
    track), and gates the commands that come back: next and previous are
    dropped while disabled, and a seek must be finite and is clamped. The
    platform part is a `Backend` with one implementation per OS, like
    `FolderAccess`. `MediaControls_apple.mm` (ARC) is the only code that
    touches MediaPlayer, AppKit or UIKit. It rebuilds the `nowPlayingInfo`
    dictionary on each change, with rate 1 or 0 so the system moves the
    progress bar on by itself. It sets `playbackState`, which macOS uses to
    pick the app that gets the media keys; a cleared queue sets it to
    stopped. Artwork is decoded into an `NSImage` (a `UIImage` on iOS),
    and bytes that don't decode are refused. It registers play, pause,
    toggle, next, previous and change-playback-position. The handlers hold
    their target weakly, so a late one does nothing after destroy; one
    that arrives off the main thread is re-sent to the main queue. The base
    `Backend` does nothing and is the fallback (`MediaControls_none.cpp`
    until Phases 9–10). `build.rs` and the core's CMake link MediaPlayer.
  - **C API:** `anomp_media_controls_create(callback, user_data)`,
    `_destroy`, `_set_track`, `_set_playback` (an `ANOMP_STATE_*` value,
    elapsed, duration), `_set_artwork` (encoded bytes; empty clears),
    `_set_navigation`, `_clear`, `_supported`, and `_perform`, which
    delivers a command as if the OS had sent it (for tests). Main thread
    only, null handles safe. **Commands use a callback of their own, not
    new engine event types:** engine events are polled every 50 ms, but
    commands arrive the moment the OS sends them. Commands also carry
    their own payload (a seek position), and the controls don't depend on
    the audio engine: separate lifetime, and they are tested without it.
  - **Rust:** `anomp::MediaControls` wraps the C API. `media.rs` hosts it
    in a main-thread thread-local. `NowPlaying` decides what to publish,
    driven through a `Publisher` trait so `cargo test` runs it against a
    fake. It compares what the queue and player are doing with what it
    last published, and sends only the differences:
    - After every queue change (`queue::publish`) it gets the `QueueState`
      plus the engine's state, position and duration. Not loaded, or no
      current item: clear. A new current item (uid or tags differ): track
      and playback. Changed `has_next`/`has_previous`: navigation.
    - The engine's `StateChanged` republishes playback. `Position` (every
      50 ms while playing) is compared, not forwarded. Playback is
      republished when the duration changes (the exact one arrives on
      load), when paused and the position moved, or when playing and the
      position is more than 0.25 s from where the system has moved it on
      to. That is how seeks from anywhere (UI, keyboard, scrubbing in
      Control Center, the dev page) reach the progress bar without the
      position being pushed every 50 ms.
    - Until the engine has a duration, the tags' one is shown.
  - **Nothing is published until the queue has a track loaded,** so a
    launch (which restores the queue unloaded) doesn't take the media keys
    from another player. The dev page's direct loads detach the queue,
    which clears Now Playing.
  - **Commands** go straight to `queue::play`, `pause` (new, like `toggle`
    it saves the position), `toggle`, `next`, `previous` and `seek`,
    inside the callback. The design first deferred them with
    `run_on_main_thread`, but on the main thread Tauri runs that closure
    at once, so it would not defer anything. Calling directly is safe: the
    core calls the handler only from the OS's handler, never inside a call
    into the core, so no queue, engine or media borrow is held then. The
    Rust wrapper keeps the handler behind a raw pointer, so running it
    never overlaps a `&mut` borrow of the controls.
  - **Artwork:** the key is the current item's album (`ArtKey::Album`), or
    the track without one, the same as the UI uses. When it changes, the
    old art is cleared and `art::lookup` runs on a blocking thread (the
    library connection is used only there). The result comes back through
    `run_on_main_thread` and is shown only if that key is still current.
    Tracks from the same album keep the art, with no lookup and no flicker.
  - Tests: Catch2 `MediaControlsTests.cpp` (a recording backend: what is
    published and what changed, value checks, artwork refusal, command
    gating and clamping, the fallback; the C API: null handles, argument
    checks, commands reaching the callback through `_perform`, and PNG
    artwork decoding). No test publishes a track, which would show on the
    machine's real Now Playing. `cargo test`: the command and state
    mapping, and `NowPlaying` (nothing before a track is loaded, clearing,
    art kept per album, late art ignored, when positions are republished,
    the tags' duration, a rescan's new tags).
  - Known limits: a seek of less than 0.25 s while playing isn't
    republished. For up to 50 ms after a gapless hand-off, the progress bar
    can show the new track's position under the old title, until the queue
    moves on. Artwork is sent at full size. Nothing is shown after a
    relaunch until playback starts, so the media keys can't resume the
    restored queue until then. The OS's command status is not the
    queue's: a command the queue can't act on (play with an empty queue)
    still reports success.
  - Checked by hand 2026-09-26 in `tauri dev` (unbundled, which Now
    Playing accepts) and in the sandboxed bundle: the media keys play,
    pause, skip and go back; Control Center and the menu-bar widget show
    title, artist, album, artwork and a progress bar that follows playback
    and seeks; scrubbing there seeks the app; next and previous gray out
    at the ends of the queue with repeat off; clearing the queue clears
    the info.

  **Phase 3 complete.**

### Phase 4 — Online metadata services
Goal: enrich the library with album details, artist information and cover
art from online sources and local files, with **more than one source for
each kind of data and the user choosing between them**: an ordered list of
sources per kind (the first with a result wins), and a per-album choice
that pins a specific source's match or picture. Every service can be turned
off, and the app works offline on what it has already fetched.

**Sources considered** (checked 2026-09-26; "terms" is about a
closed-source commercial app, §4.1, and every online source is re-checked
before release, §8.1):

| Source | Provides | Access and limits | Terms | Plan |
|---|---|---|---|---|
| Embedded art (tags) | Album/track art | Local, already read by the core | — | On (exists) |
| Folder images | Album art (`cover`, `folder`, `front`, `album`, `albumart*` .jpg/.png/.webp next to the tracks) | Local; readable through the folder's bookmark | — | On |
| MusicBrainz | Release, recording and artist metadata (dates, label, catalogue number, country, release type, genres), and links to Wikidata, Discogs etc. | No key; a `User-Agent` with contact details; **~1 request/s per IP**, 503 when exceeded | Core data CC0; MetaBrainz asks commercial users to become supporters | On, the primary source |
| Cover Art Archive | Album art by release or release-group MBID; 250/500/1200 px thumbnails | No key; no limits today; images redirect (307) to archive.org | Images belong to their owners; showing them in a player is the norm | On |
| Wikidata + Wikipedia | Artist and album descriptions, reached through MusicBrainz URL relationships | No key; `User-Agent` | Text CC BY-SA: show attribution and a link with it | On |
| Discogs | Release metadata (credits, labels, catalogue numbers, styles) and images | 60 requests/min authenticated, 25 unauthenticated; images need authentication. A personal access token supplied by the user, since a secret shipped in a desktop app isn't secret | API terms require attribution; check caching rules | Off until a token is entered |
| fanart.tv | Artist images, logos, backgrounds; album covers; keyed by MBIDs | Project API key, optional personal key | Check | Off (later) |
| TheAudioDB | Artist bios and images, album descriptions | Free key "123" limited to 30 requests/min and one result per search; $8/month premium | Not stated; check | Off (later), user-supplied key |
| iTunes Search API | Large album art, release dates | No key; low rate limit | Tied to Apple's affiliate/promotion terms | Off; only after a terms check |
| Deezer | Album art up to 1000 px, search | No key | Check | Off; only after a terms check |
| AcoustID + Chromaprint | Identifies untagged files by audio fingerprint | API key; 3 requests/s; Chromaprint is LGPL and a new native dependency | **Free for non-commercial use only**; commercial use is a paid plan | Deferred (after Phase 4) |
| Last.fm | Artist bios, tags, similar artists | API key | **Non-commercial only** without written permission, 100 MB storage cap, mandatory branding | Excluded |
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
  so adding a source is one module.
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

**MusicBrainz matching** (album → release):
1. A release MBID from the tags: look it up directly (score 1).
2. Otherwise search releases by album title and album artist (Lucene
   special characters escaped, "Various Artists" handled), then fetch the
   top few candidates with their tracklists.
3. Score each on title and artist similarity (folded like `sort_key`),
   track count, per-track durations (aligned by disc and track number;
   ±3 s counts as a match), and year. Accept automatically at a score of
   0.85 or more that also leads the runner-up by 0.05; otherwise keep the
   candidates for the "Find details" dialog and mark the album "needs
   review". The scorer is a pure function, tested on its own.
4. The release's release group gives the Cover Art Archive a fallback when
   the release has no front cover.

**Steps:**
- [x] 4.1 Foundation: migration 003, `metadata::settings`, the HTTP client
  (`Transport`, limiter, `User-Agent`, backoff), the response cache. Tests
  with the fake transport.

  Done 2026-09-26 (`app/src-tauri/src/metadata/`):
  - `settings.rs`: `SourceId` (embedded, folder, musicbrainz,
    cover-art-archive; the serialized id is what the `source` columns
    store) and `Kind` (release, albumArt). `ServiceSettings` holds the
    online switch, auto-match, per-source enabled flag and key, and an
    order per kind. Stored settings are completed on read: unknown
    sources dropped, missing ones appended in default order, each kind's
    order listing exactly its sources. `sources_for(kind)` gives the usable
    ones in order: enabled, online only with the switch on, keyed if they
    need a key, and with what they rely on usable (the Cover Art Archive
    needs MusicBrainz).
  - `http.rs`: `Client` over a `Transport` (ureq in the app, a scripted
    fake in tests) and a `Clock` (the fake one advances on sleep, so the
    tests run the limiter and backoff without waiting). Per-host minimum
    interval (MusicBrainz 1 s, others 250 ms); 503/429 retried up to 3
    tries, honouring `Retry-After` up to 30 s; an unreachable host is
    refused at once for 1 min, doubling to 30 min, until it answers or
    `retry_now`. `get_json` reads through the cache and falls back to a
    stale copy when offline. The `User-Agent` is `ano-mp/<core version> (
    <repo URL> )`, the version from `anomp_version()` so no new copy of it.
  - `cache.rs`: `mb_cache` as a URL → body cache for every service.
  - Commands: `metadata_settings` (with each source's name, kinds and
    dependencies for the UI), `metadata_save_settings`,
    `metadata_reset_settings`; typed wrappers in `api.ts`.
  - Provider traits are left until a second source of the same kind
    exists (Discogs, 4.7); MusicBrainz is called directly until then.
  - The online modules have no caller in the app until the worker (4.5),
    so they carry `#[allow(dead_code)]` until then.
- [x] 4.2 Local art: folder images as an `AlbumArt` source; `art::lookup`
  walks the user's choice, then local sources in the configured order.

  Done 2026-09-26 (`metadata/folder_art.rs`, `library/art.rs`):
  - An image directly in a track's folder named `cover`, `folder`,
    `front`, `album` or `albumart` (that order, any case; .jpg, .jpeg,
    .png, .webp; at most 32 MB), then Windows Media Player's
    `AlbumArt*Large`/`AlbumArtSmall`. For a track in a disc folder ("CD1",
    "Disc 2") the folder above is tried too, never above the library
    folder.
  - `art::lookup` tries the user's pick in `album_art` (falling back to the
    order if that picture has gone or its source is unknown), then
    `sources_for(AlbumArt)`. It opens each library folder by id through
    `access::open_folder`, and skips one that can't be opened rather than
    failing the request. Saving the settings clears the art cache.
- [x] 4.3 MusicBrainz provider and the matcher; album links and details
  stored.

  Done 2026-09-26 (`metadata/musicbrainz.rs`, `matcher.rs`, `albums.rs`):
  - Search (`release:(…) AND artist:(…) tracks:N`, Lucene-escaped terms
    rather than phrases, 10 hits, cached 7 days) and lookup (recordings,
    artist credits, labels, release group, genres; cached 30 days). Tag
    MBIDs are checked to be UUIDs before going into a URL. `Release`
    keeps title, credit, date, country, status, barcode, labels and
    catalogue numbers, release group and its types and first date, genres
    (the release's, else its group's, most voted first), track list with
    lengths, and whether the Cover Art Archive has a front cover.
  - Scoring as designed above, plus title and artist as **gates**: the
    weighted score is multiplied by a factor that is 1 at a similarity of
    0.7 and 0 at 0.3, so another album by the same artist, or the same
    title by another artist, can't get through on track count and year.
    Titles compare folded (`sort_key::fold`), punctuation-blind, "&" as
    "and", with a bracketed suffix ignored at a small cost.
  - `match_album`: a user's choice is left alone; a tagged release MBID
    is looked up (1 request) and trusted, falling back to a search if
    MusicBrainz no longer has it; otherwise one search plus lookups of
    the best 3 hits (about 4 s at the rate limit). The result is stored
    as matched, review (best candidate kept) or none. `choose_release`
    stores the user's pick; `clear_link` returns an album to automatic.
  - Tests: recorded responses for "In Rainbows" (a search and four
    releases, trimmed; CC0) in `metadata/fixtures/musicbrainz/`, served by
    the fake transport. `live_search_and_lookup` (ignored; `cargo test
    live_ -- --ignored`) checks the real service, TLS and `User-Agent`.
  - Artist links (`artist_links`) are filled with artist info in 4.7.
- [ ] 4.4 Cover Art Archive provider and the image cache.
- [ ] 4.5 The metadata worker: job queue, priorities, background
  enrichment after a scan (if enabled), progress and `metadata-changed`
  events, offline backoff.
- [ ] 4.6 Commands and UI: album details with source labels, "Find
  details" and "Choose cover" dialogs with candidates per source, and a
  Services panel (master switch, enable, order, keys, status) that Phase 6
  folds into the admin screen.
- [ ] 4.7 More sources: Wikidata/Wikipedia descriptions, then Discogs
  (user token). fanart.tv, TheAudioDB, iTunes and Deezer after their terms
  are checked.
- [ ] 4.8 Select and configure the alternative sources. Go through the
  sources table above and decide which ones ship, recording each decision
  and its reason in the table. For each source that ships:
  - Settle its terms (commercial use, attribution, caching, image display)
    and record the outcome; §8.1 still re-checks them before release.
  - Add its `SourceId`, the kinds it supplies, what it relies on, and
    whether it needs a key.
  - Set its defaults: enabled or not, and its place in each kind's order.
  - Decide how it gets a key: supplied by the user, or a project key
    shipped with the app if its terms allow that. Decide where keys are
    stored (the settings JSON, or the OS keychain).
  - Set its request interval in `http::request_interval` from its
    published limits.
  - Show its attribution wherever its data appears, if its terms require
    one.
  - Add recorded-response fixtures and an ignored live test.
  - Make sure the Services panel (4.6) lists it, with a key field if it
    needs one.
- **Exit:** a library of tagged and untagged albums gets details and covers
  from MusicBrainz and the Cover Art Archive; the user can reorder or turn
  off sources, pick another source's cover or match for an album, and the
  app behaves the same with the network off, showing what it cached.

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
- [ ] **Metadata service terms** (Phase 4): re-read the terms of every
      source that is on by default or selectable, for commercial use,
      attribution, caching and image display. Decide on a MetaBrainz
      supporter plan. Set the `User-Agent` contact to a real address.
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
  `@rpath`, and sign all nested code with the hardened runtime. Done except
  the hardened runtime (Phase 2): the dylibs are embedded and signed, and
  `tauri.conf.json` turns the hardened runtime off for ad-hoc local builds.
  Turn it back on with the Developer ID identity.
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
