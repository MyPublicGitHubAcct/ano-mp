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
| Visualizer analysis: `SignalTap` (lock-free tap on the player's output), `SpectrumAnalyser` (bands, chroma, levels, triggered waveform, beats), `AnalysisThread`, `anomp_engine_set_analysis_callback` | `core/src/SignalTap.h`, `core/src/SpectrumAnalyser.*`, `core/src/AnalysisThread.*` |
| 63 passing Catch2 tests (~2675 assertions) | `core/tests` |
| Tauri 2 app (SvelteKit + `adapter-static`, Svelte 5, TS) showing `anomp_version()` via the `core_version` command | `app/` |
| `build.rs` builds `anomp_core` with the `cmake` crate and links it plus the Apple frameworks | `app/src-tauri/build.rs` |
| Safe Rust wrappers over the C API | `app/src-tauri/src/anomp.rs` |
| Library: SQLite schema and migrations, folders, incremental parallel scanner, sort/grouping rules, paged browsing, FTS5 search, cover art (`anomp-art` URI scheme), `library_*` commands | `app/src-tauri/src/library/` |
| Play queue: order, shuffle, repeat, gapless hand-off across it, persistence, `queue_*` commands and `queue-changed` event | `app/src-tauri/src/queue/` |
| OS media integration host: Now Playing kept in step with the queue and player, remote commands routed to the queue, artwork | `app/src-tauri/src/media.rs` |
| Metadata sources (Phase 4, in progress): source settings and order, HTTP client with rate limits, backoff and response cache, folder-image art, MusicBrainz search/lookup and album matching, Cover Art Archive covers and listings, the on-disk image cache, the metadata worker (job queue, priorities, background enrichment, offline pause, `metadata-changed` and `metadata-progress` events, calls for the dialogs), release/cover/artist candidates and the user's picks, Wikipedia artist biographies and album descriptions, Discogs as an opt-in second album-details source (only matches stored, the token in the keychain) | `app/src-tauri/src/metadata/` |
| Visualizer stream: frames encoded and sent over a Tauri `Channel` while subscribed (`visualizer_*` commands); the cover wall's albums (`library_cover_wall`) | `app/src-tauri/src/visualizer.rs`, `app/src-tauri/src/library/covers.rs` |
| Visualizer UI: eight canvas visualizations, picker, full screen, colours from the cover | `app/src/lib/visualizer/`, `app/src/lib/components/Visualizer*.svelte` |
| 5 frontend tests (`npm test`: frame decoding, key estimation) | `app/tests/` |
| 232 passing `cargo test` tests (C API wrappers, schema, folders, scanner, sort keys, genres, rules, browsing, search, art sources and candidates, album details, queue, Now Playing sync, metadata settings and keys, HTTP client, MusicBrainz parsing and matching, Cover Art Archive, image cache, metadata worker, candidates and choices, Wikipedia, discographies, Discogs, visualizer frames and subscribers, cover walls), plus 3 ignored 50,000-track benchmarks and 6 ignored live tests (MusicBrainz, Cover Art Archive, biographies, descriptions, discographies, Discogs) | `app/src-tauri/src` |
| `AudioEngine` + `anomp_engine_*` C API: default output device, test tone, device-change event | `core/src/AudioEngine.*` |
| Pinned LGPL audio-only FFmpeg 9.0.2 (universal dylibs) and `FFmpeg::*` CMake targets | `scripts/build-ffmpeg.sh`, `cmake/FFmpeg.cmake` |
| Main-thread engine host; `audio_device_name`, test-tone and `player_*` commands; `player-*` events | `app/src-tauri/src/audio.rs` |
| Player UI: sidebar (views, folders, scanning, online sources), browser with album details, search, queue panel, now-playing bar, artist pages, the metadata dialogs and the Online sources panel; responsive down to 360 px, light and dark | `app/src/routes/+page.svelte`, `app/src/lib/` |
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
- [x] 4.4 Cover Art Archive provider and the image cache.

  Done 2026-09-26 (`metadata/coverartarchive.rs`, `images.rs`,
  `library/art.rs`):
  - `fetch_album_art(client, conn, cache, album_id)`, for the 4.5 worker:
    the archive picture the user chose (an `album_art` reference on
    `coverartarchive.org`), else for an album with a *matched*
    MusicBrainz link the release's `front-500` (skipped when MusicBrainz
    says the release has no front cover), else the release group's. It
    does nothing for an album that isn't linked or only awaits review, or
    whose cover is cached; a 404 moves on to the next URL. Offline it
    fails with `Error::Offline` and writes nothing. It returns what it did
    (`NotLinked`, `Cached`, `Downloaded`, `NotFound`); `fetch_image` does
    one URL, for the "Choose cover" dialog. Whether the source is enabled
    is the caller's check. Requests go through `http::Client` with
    `IMAGE_LIMIT`; ureq follows the 307 to archive.org.
  - 500 px, of the archive's 250/500/1200: sharp at the largest size the
    player shows art, at around 100 KB.
  - `release_images` lists a release's pictures (`/release/{mbid}/`,
    cached 7 days, a 404 meaning none) with types, the front flag,
    comment, and thumbnails by width. Older pictures only have `small`
    and `large`, read as 250 and 500; ids are sometimes numbers, sometimes
    strings; `http://` URLs are made `https://`. For the 4.6 dialog (now
    `cover_candidates`).
  - `images::ImageCache`: `<app cache dir>/images` (inside the container
    under the sandbox), one file per URL named by the **SHA-256** of the
    URL in hex, from `ring`, which rustls already links in, so no new
    code; SHA-256 is fixed by its standard, unlike `DefaultHasher`.
    Written to a `.part` file, synced, then renamed into place; `.part`
    files over an hour old are left by a crash and removed. Only data
    starting like a JPEG, PNG, GIF or WebP is stored or served, so an
    error page served with a 200 is refused. Budget 500 MB (about 5,000
    covers at 500 px): beyond it the least recently used files go, down
    to 90% so eviction doesn't run on every download. Recency is the
    file's modification time, moved on by a read at most once a day so
    browsing doesn't write; the total is counted by one directory scan at
    the first store and kept in memory after that.
  - `art::lookup` serves the `CoverArtArchive` source from the cache
    only: the user's picked URL, or the first cached of the album's
    cover URLs, which are derived from its matched link, so no schema
    change. It never goes online.
  - Tests: recorded listings for two "In Rainbows" releases and an 8×8
    JPEG in `metadata/fixtures/coverartarchive/`. `live_cover_and_listing`
    (ignored) fetches a real cover through the redirect and a listing.
  - Known limits, for 4.5: an album whose cover the archive doesn't have
    costs up to two requests every time it's fetched, since no "not
    found" is recorded (an `album_links` row for `cover-art-archive`
    could hold it); after a download the worker must drop that album from
    `ArtCache`, which may remember it as having no art or a local
    picture; the cache isn't cleared when the MusicBrainz match changes,
    so the old release's cover stays until evicted; with the online
    switch off, `sources_for` leaves the archive out, so downloaded covers
    aren't shown either.
- [x] 4.5 The metadata worker: job queue, priorities, background
  enrichment after a scan (if enabled), progress and `metadata-changed`
  events, offline backoff.

  Done 2026-09-26 (`metadata/jobs.rs`, `worker.rs`, `commands.rs`):
  - `jobs.rs` holds the logic, tested without Tauri or a thread: the
    queue, the skip rules, and `Worker::step`, which runs one job and
    reports through a `Host` trait (like `media.rs`'s `Publisher`).
    `worker.rs` runs it on a "metadata" thread that owns the
    `http::Client` (ureq, system clock) and a connection from `db::open`;
    other threads only queue requests, so nothing waits on the network
    while holding the shared connection, and nothing runs on the main
    thread. Started at setup; on `RunEvent::Exit` it's told to stop and
    not joined, since a request can take its 30 s timeout. Queued replies
    are dropped then, so a waiting caller hears at once.
  - Jobs: `Match(album)` matches the album if it needs it, then queues
    its cover at the front of its priority; `Cover(album)` fetches the
    cover. Priorities: user requests, then the album playing (queued from
    `queue::publish` when the current album changes), then background
    enrichment; first come first served within each. A job is queued once;
    a higher request moves it forward and adds its reply. Whether a job
    is needed and its source usable (`sources_for`) is decided when it
    runs, from the settings and the database then. Automatic jobs (the
    album playing, background) need "match automatically"; a user request
    doesn't, skips the 30-day waits, and gets an error if the source is
    off.
  - Background enrichment is queued after each scan, after the settings
    are saved, and at launch (for a run cut short by quitting, and albums
    due for a retry); the worker checks `auto_match` when it gets to it.
    Skip rules (`needs_match`, `needs_cover`): an album the user chose a
    match for is never re-matched, but its cover is still fetched if
    missing, since that follows their choice; an accepted match whose
    cover is downloaded is left alone; 'none' **and 'review'** are retried
    30 days after the check. 'review' is retried because MusicBrainz grows
    and a better release may appear. Meanwhile its candidate waits for the
    4.6 dialog and gets no cover. No cover is fetched when the user picked
    a picture from another source.
  - "No cover found" is recorded as the album's `album_links` row for
    `cover-art-archive` ('matched' or 'none', `external_id` = the release
    asked about), written by `fetch_album_art`; so no migration. A record
    for a release the album is no longer matched to doesn't count.
  - A changed match: the worker compares the album's cover URLs before
    and after matching. They're derived from the link, so the old
    release's cover is never picked again. When they differ, the album is
    dropped from `ArtCache` (new `ArtCache::remove`) and named in
    `metadata-changed`. The old file stays in the image cache until
    evicted; that's harmless.
  - Offline: while `http::Client` backs off from a host, automatic jobs
    for it stay queued and jobs for other hosts go on (covers keep coming
    while MusicBrainz is down). With only waiting jobs left, `step`
    returns how long until the first host is tried again (`Client::retry_at`)
    and the thread sleeps that long or until a request. A background job
    that fails offline goes back to the front of the queue, not counted
    as failed. A user request fails at once with `Error::Offline`. If it had
    also been queued automatically, it stays queued for that.
    `metadata_retry_now` clears the backoff.
  - Events: `metadata-changed` (`{albums, artists}`; no artist ids until
    4.7) is batched at most once a second, sent at once after a user
    request (before its answer) and when the worker goes idle or pauses.
    `metadata-progress` (`{done, total, current, paused, unreachable}`,
    counted in albums since the worker was last idle; zeros when idle) is
    sent at most four times a second, and at once on pausing and going
    idle. Changed art is dropped from `ArtCache` at once, and Now Playing
    looks up its artwork again if it shows that album
    (`media::art_changed`).
  - Reloading art in the webview: the UI keeps a count per album of the
    `metadata-changed` events that named it, and adds it to the art URL
    next to the scan count (`artUrl(key, generation, version)`), so
    `max-age=86400` stays.
  - Downloaded covers stay visible with the online switch off
    (`ServiceSettings::sources_shown`, used by `art::lookup`): the switch
    stops the app contacting services, and the Phase 4 exit asks for the
    app to behave the same with the network off. Turning the Cover Art
    Archive (or MusicBrainz, which it needs) off hides them.
  - Commands: `metadata_status`, `metadata_retry_now`, and
    `metadata_update_album` (a user request that waits for the match and
    cover); wrappers and payload types in `api.ts`. The dialogs and the
    Services panel are 4.6.
  - The online modules' `#[allow(dead_code)]` is gone. It's kept only on
    `choose_release`, `clear_link` and the image listing (for the 4.6
    dialogs), and on `cache::prune`.
  - Checked in the app (2026-09-26) on a 16-album library: with requests
    failing as unreachable (`ALL_PROXY` pointed at a closed port) the
    worker paused at once, nothing was written, and the app worked on
    local data. Online, 13 albums were matched and got covers in about 80
    s; one was 'none', and two searches failed with MusicBrainz 503s
    after three tries. The next launch matched those two and requested
    nothing else. Not checked by eye: covers appearing in an album list
    during a run (the window was on a view without art, and the check
    didn't drive the UI), and adding a folder through the picker and
    scanning it.
  - Known limits: a job that fails with an HTTP error (e.g. a 503 after
    three tries) is logged and dropped until the next enrichment; nothing
    prunes `mb_cache` yet; a rescan that changes an album's tracks
    doesn't make an accepted match be looked at again; covers are
    downloaded even when a local picture comes first in the order (the
    DB doesn't record which albums have embedded art), so a library of
    more than about 5,000 albums can churn the 500 MB image cache; 4.6's
    "Use automatic" (`clear_link`) should drop the album from `ArtCache`
    itself, since the worker only does that once it has re-matched; on
    Windows (WebView2's HTTP cache persists across launches) the scan
    count and per-album count restart at 0 each launch, so an art URL may
    repeat one cached in an earlier run (Phase 10).
- [x] 4.6 Commands and UI: album details with source labels, "Find
  details" and "Choose cover" dialogs with candidates per source, and a
  Services panel (master switch, enable, order, keys, status) that Phase 6
  folds into the admin screen.

  Plan (2026-09-26):
  - **Worker calls.** The dialogs' searches, lookups and listings go
    online, so they run on the metadata worker (the only owner of
    `http::Client`): `Shared::call` queues a closure that the worker runs
    with its client, connection and image cache ahead of every job, and
    the command waits for its answer on a blocking thread. Offline, the
    client refuses at once, so a dialog shows "offline" rather than
    hanging. Changes a call makes are reported like a job's.
  - **Album details** (`metadata_album`): the tag values (title, album
    artist, year, tracks, length, genres, the tagged release MBID), the
    MusicBrainz link (status, chosen by the user or not, score, when
    checked) with the release's fields while MusicBrainz is shown, and
    where the cover shown comes from (`Art` records its source) and
    whether the user chose it. The UI labels each value with its source.
  - **"Find details"**: `metadata_release_candidates` lists, per
    album-details source in the configured order, the candidates: the
    search hits (the default query is the automatic one, so it's usually
    cached), the best three looked up and scored with track lengths, the
    rest scored on the search result alone and flagged so, plus the
    current match or review candidate. The title and artist searched for
    can be edited, and a MusicBrainz release URL or MBID is looked up
    directly. Actions: pick a release (`choose_release`, then its cover
    is fetched), "None of these" (a 'none' row chosen by the user, which
    automatic matching leaves alone), and "Use automatic" (clears the
    row, drops the album's art, and matches again at once).
  - **"Choose cover"**: `metadata_cover_candidates` lists the album-art
    sources in the configured order: the embedded picture, every image
    in the album's folders (`folder_art::images`), and the archive's
    listing for the matched release plus its release group's front. The
    archive's 250 px thumbnails are downloaded on demand through the
    worker (`metadata_fetch_image`, archive URLs only) into the image
    cache, so the webview never contacts a service. Previews come from
    the art handler: `anomp-art://localhost/album-<id>/<source>?ref=…`
    serves one candidate through the same `from_source`, never online; a
    folder reference must be a relative path without `..`. Picking one
    stores `album_art` (a folder picture only if it's one of the album's
    images; an archive picture by its 500 px URL, downloaded next);
    "Use automatic" deletes it. Both drop the album's art and send
    `metadata-changed`.
  - **Artists**: the same for the 'review' artists of 4.7: candidates from
    the artist search (name editable, MBID or URL looked up), pick, "None
    of these", "Use automatic".
  - **Services panel**: a main view opened from the sidebar, whose entry
    shows what the worker is doing. The online switch, "match
    automatically", each source (on/off, what it supplies, what it needs,
    a key field if it takes one, its status from `metadata-progress`: in
    use, off, can't be reached with "Try now"), each kind's order (move up
    and down), the worker's progress, and "Reset to defaults".
    `SourceInfo` gains the hosts each source contacts, so status maps to
    sources.
  - **UI**: a modal `Dialog` (native `<dialog>`) and the three dialogs,
    opened through `ui.dialog`. The browser shows an album header when the
    node is an album: cover (click to choose), the release facts with
    their source, the match status, "Find details…", "Choose cover…", and
    a details table (field, value, source). Album menus in the browser,
    search and artist page get "Find details…" and "Choose cover…"; the
    artist page gets "Choose…" for a 'review' match and "Wrong artist?"
    for a match.
  - **Tests**: the worker's calls; candidates, choosing, rejecting and
    clearing with the fake transport and recorded responses; folder image
    listing; cover choices' validation; the handler's candidate paths.
    Then `npm run check`, and the app checked by eye.

  Done 2026-09-26, as planned (`metadata/commands.rs`, `library/albums.rs`,
  `library/art.rs`, `AlbumInfo.svelte`, the `*Dialog.svelte` components,
  `ServicesPanel.svelte`). Beyond the plan:
  - `Release` gains each medium's format and the release's
    disambiguation (both defaulted, so details stored before still parse):
    without them the dialog's releases of one album look the same.
  - A call's answer is sent after the changes it made are reported
    (`Call` returns an `Answer`), as jobs do, so the UI reloads before the
    dialog closes. Calls run before any queued job, but wait for the job
    that's running.
  - The "Find details" dialog shows the selected release's tracks next to
    the album's with their lengths, marking those within 3 s, and scores
    from a search result alone as "~".
  - "Use automatic" for a cover queues the archive's cover, since a user's
    pick from another source had stopped it being fetched.
  - The art handler's candidate route answers with `no-store`; the
    webview's `convertFileSrc` encodes slashes, so the UI adds
    `/<source>` after it.
  - Checked in the app (2026-09-26) on the 16-album library, driven by a
    temporary dev-only script (scripted clicks need an accessibility
    permission the session didn't have): the album header and its details
    table; "Find details" listing the match with its tracks compared, and
    other releases told apart by country, format and label; choosing a
    CD release (its details and cover replaced the automatic ones);
    "Choose cover" with archive previews fetched through the worker, a
    pick marked "Your choice", then "Use automatic" for the cover and the
    release, which put back the original match and cover; the Online
    sources panel, where turning MusicBrainz off showed the Cover Art
    Archive and Wikipedia as needing it and struck them from the order;
    "Find artist" from an artist page. Two layout bugs found and fixed
    (the sidebar's section heading growing, the dialog body collapsing).
    Not checked by eye: the dialogs offline, dark mode, narrow windows,
    and a 'review' album or artist (none in that library).
  - Known limits: album details come from MusicBrainz only, but the
    commands and dialog are per source, ready for Discogs (4.7); "Choose
    cover" offers only the matched release's archive pictures and its
    release group's front, not other releases'; only the first embedded
    picture found in an album's first files is offered; archive previews
    go into the image cache and count toward its budget; keys, when a
    source takes one, are stored in the settings JSON (4.8 decides on the
    keychain); "Use automatic" while offline leaves the album unmatched
    until the worker can reach MusicBrainz again.
- [x] 4.7 More sources: Wikidata/Wikipedia descriptions, then Discogs
  (user token). fanart.tv, TheAudioDB, iTunes and Deezer after their terms
  are checked.

  Done 2026-09-26: artist biographies and album descriptions from
  Wikipedia. Discogs moved to 4.8 (below), with fanart.tv and the rest:
  its API terms, as far as they could be checked, rule out its covers in a
  commercial app and any offline copy of its data, so whether and how it
  ships is 4.8's decision, not an implementation step. The provider traits
  planned in 4.1 wait for it too, since no second source of the same kind
  exists yet.

  Artist pages with Wikipedia biographies done 2026-09-26
  (`metadata/artists.rs`, `wikipedia.rs`, `library/artists.rs`,
  `ArtistPage.svelte`), then album descriptions:
  - Settings: source `wikipedia` (requires MusicBrainz) and kind
    `artistInfo` (an artist's biography). Stored settings from before get
    its default order. Artists are matched on MusicBrainz whenever
    MusicBrainz is usable; the match gives the facts and the link to a
    biography.
  - Artist matching (`artists::match_artist`), first that works: the
    artist MBID in the tags; the artist credited alone, under the same
    name, on the releases the artist's albums are matched to (the most
    common, score 0.95); a search by name and alias. Names are shared
    ("Nirvana" is several bands), so a hit is accepted only at a
    MusicBrainz score of 90 or more with a lead of 15 over the next of
    the same name, else 'review'; MusicBrainz ranks the best-known well
    ahead (100 vs 75 for the two Nirvanas). "Various Artists" is never
    looked up. The lookup (`url-rels+genres`, cached 30 days) keeps type,
    area, begin/end area and dates, disambiguation, genres, and the
    Wikidata item, a direct English Wikipedia link (older entries) and the
    homepage. Stored in `artist_links` like albums; no migration.
  - Biography: Wikidata `wbgetentities` (sitelinks, `enwiki`; a merged
    item comes back under its new id) → the article's lead section as
    plain text (`prop=extracts&exintro&explaintext`, redirects followed),
    each cached 30 days. Stored as the artist's `wikipedia` row with the
    MusicBrainz artist it was fetched for as `external_id`, so a changed
    match never shows the old biography; "none" (no article, or no link)
    is retried after 30 days like the other sources. CC BY-SA 4.0: the
    page credits the article by name with a link, and the licence.
  - Worker: `Job::Artist` (match, then its biography) and
    `Job::Biography`; jobs are keyed by `Subject` (album or artist), a
    `Biography` job waits while either Wikimedia host is backing off, and
    progress counts albums and artists (`current` is tagged `album` or
    `artist`). New priority `Viewing` (the artist page shown) between the
    user's requests and the album playing: automatic, but its changes are
    sent at once. Background enrichment queues album artists after the
    albums (their matches help), only while a biography source is usable;
    the page looks an artist up when shown anyway. `metadata-changed` now
    names artists.
  - Commands: `library_artist` (the page: albums the artist is album
    artist of, oldest first, with the release type from their matched
    release; other artists' albums they appear on; the metadata; queues
    the artist at `Viewing`) and `metadata_update_artist` (a user request,
    skipping the retry waits). `TrackSummary` and queue items carry
    `artistId`.
  - UI: the artist view (`ui.showArtist`, with a back stack) shows type,
    area, disambiguation, formed/born and ended, genres, the biography
    (two paragraphs, then "Read more"), "Look up again" when there is
    none, links to MusicBrainz and the homepage, and albums grouped as
    Albums, EPs, Singles, Live albums, Compilations, Soundtracks and
    Other, then "Appears on". Opened from a search's artist, the "Artist"
    button inside an artist in the browser, "Go to artist" on tracks,
    queue items and artist rows, and the artist on the now-playing view.
    Links open in the browser through `tauri-plugin-opener`, allowed only
    `http(s)` URLs.
  - Tests: recorded MusicBrainz artist and search responses, a recorded
    Wikidata response, and an extract whose text is a stand-in (no CC
    BY-SA text committed). `live_biography` (ignored) checks the real
    chain. Checked in the app (2026-09-26): at launch the five album
    artists of a 16-album library were matched through their releases and
    got biographies; "Various Artists" made no request; the page for
    Swans showed its facts, biography, credit and five albums.
  - Album descriptions (done 2026-09-26; `wikipedia::fetch_description`,
    `musicbrainz::lookup_release_group`): kind `albumInfo`, supplied by
    Wikipedia (settings stored before get its default order). The album's
    matched release (not a 'review' candidate) gives its release group,
    looked up with `url-rels` (cached 30 days) for its Wikidata item or a
    direct English Wikipedia link, then the same sitelink and extract
    requests as biographies. A separate release group lookup rather than
    more `inc` on the release lookup, so albums matched before get
    descriptions without their releases being fetched again. Stored as
    the album's `wikipedia` row in `album_links` with the release group
    as `external_id`: another release of the same album keeps the
    description, and a match to another album hides it; 'none' is retried
    after 30 days. `AlbumLink` keeps the stored JSON (`details`, not sent
    to the UI) and parses `release` only for MusicBrainz;
    `albums::store_source_link` stores another source's row. `Biography`
    became `wikipedia::Article`, and `SourcedBiography` `SourcedArticle`
    (both in the UI too), used for both.
  - Worker: `Job::Description`, following an album's `Match` (after its
    `Cover`, if one is wanted) and `Cover`, so the user's "update" and a
    chosen release get one too; background enrichment queues it for
    albums needing nothing else. It pauses while MusicBrainz or either
    Wikimedia host is backing off. Skip rules in `needs_description`,
    like `needs_biography`'s.
  - UI: `AlbumDetails.description`; the album header shows its first
    paragraph with "Read more", credited like the biography ("From the
    Wikipedia article “…”, under CC BY-SA 4.0", both linked), and the
    details table names it. The Online sources panel lists "Album
    descriptions" with its order.
  - Tests: a recorded MusicBrainz release group (trimmed, CC0), a recorded
    Wikidata response, and a stand-in extract, as for artists; fetching,
    'none', the release group rule, the worker's chain and skip rules,
    and the album details. `live_description` (ignored) passed against the
    real services, as did `live_biography`. Checked in the app
    (2026-09-26): at launch the 15 matched albums of the 16-album library
    were looked up; 11 got the right article (disambiguated ones such as
    "Decay (Godflesh album)" and "Godflesh (EP)" included) and 4 have none.
    Not checked by eye: the album header showing the description.
  - Known limits: English Wikipedia only; a biography, a description and
    the artist's MusicBrainz details aren't refreshed once found (an
    accepted match is kept, like albums'); an album gets a description
    only once matched, not while it awaits review; a 'review' candidate is confirmed in the "Find
    artist" dialog (4.6); an artist found only as a track artist is looked up only
    when their page is opened; several artists in one tag ("A; B") are one
    name and rarely match.
  - Releases not in the library (done 2026-09-26;
    `metadata/discography.rs`, `DiscographyPage.svelte`): an artist page
    matched on MusicBrainz links to a page listing the artist's release
    groups that the library lacks, grouped like the artist page (the
    grouping moved to `releases.ts`), oldest first, each linked to its
    MusicBrainz page, with its credit when it isn't the artist alone.
    - MusicBrainz's browse (`release-group?artist=…`) with
      `release-group-status=website-default`, as its own artist page
      lists them: no groups with only bootleg or promotional releases (106
      of Radiohead's 585). 100 a page, at most 10 pages (1,000 groups,
      10 s), each page cached 7 days like searches; "Check again" skips
      the cache. Nothing else is stored: the list is worked out from the
      cached pages and the albums' matches each time, so no migration.
    - In the library: a release group any album is matched to (not a
      'review' candidate), whoever its album artist; otherwise, for the
      artist's albums without a match, a group whose title is alike
      (`title_similarity` ≥ 0.9, so "OK Computer (Collector's Edition)"
      counts), so an album isn't listed as missing only because matching
      hasn't reached it.
    - `metadata_artist_discography` runs on the worker while MusicBrainz
      is usable; while it is shown but online services are off, only the
      cached pages are read (else an error saying so); with MusicBrainz
      off it fails as turned off.
    - Tests: a recorded browse page (trimmed to 14 of Radiohead's groups,
      CC0), paging and its limit, pages that overlap or fall short, the
      cache-only path, an unmatched artist. `live_discography` (ignored)
      passed against the real service. `npm run check` passes; not checked
      by eye in the app.
    - Known limits: no covers (each would be a Cover Art Archive request);
      a title match can hide more than one group of that title (an EP and
      a single both called "Creep"); an unmatched album filed under
      another artist isn't counted; a match changing while the page is
      open shows when it's opened again.
- [x] 4.8 Select and configure the alternative sources. Go through the
  sources table above and decide which ones ship, recording each decision
  and its reason in the table. Start with Discogs (moved from 4.7): read
  its API terms first-hand, and decide whether a details-only source that
  stores only the match (the Discogs release id) and fetches details when
  shown, cached at most 6 hours, with no pictures and with its
  attribution, is worth shipping. The provider traits (4.1) come with the
  first second source of a kind. For each source that ships:
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
  - Add recorded-response fixtures (with `record-fixtures.py`, §9.2 M3)
    and an ignored live test.
  - Make sure the Services panel (4.6) lists it, with a key field if it
    needs one.

  Done 2026-09-26. Decisions (the sources table has each one's terms and
  reason): Discogs ships, off by default; fanart.tv waits for its written
  consent; TheAudioDB isn't shipped; the iTunes Search API and Deezer are
  excluded; AcoustID stays deferred. Discogs (`metadata/discogs.rs`):
  - **Terms, first-hand**: the API Terms of Use (updated 2025-05-27) were
    read in full through the Help Center's article API. Its release data is
    CC0, but the terms still forbid showing it more than 6 hours behind
    discogs.com or keeping it longer than needed; images are Restricted
    Data, not for commercial use. The fee clause (charging for the part of
    an app that uses the API needs their permission) became a release gate
    (§8.1), since the owner chose to ship it opt-in now.
  - **Settings**: `SourceId::Discogs` (`discogs`), supplying `release`,
    needing a key, off by default (`enabled_by_default`), second in the
    album-details order. Settings stored before get it off, at the end.
    `SourceInfo` gained `keyName`/`keyUrl`, `storesDetails`, `credit` and
    `notice` for the UI.
  - **Keys** (`metadata/keys.rs`): in the OS keychain, not the settings
    JSON, since a personal access token can act on the user's Discogs
    account and the database is a plain file. The settings keep only
    `hasKey`, which saving the settings can't change; `metadata_set_key`
    writes the keychain (and turns the source on), and a reset keeps keys.
    macOS uses `keyring-core` with `apple-native-keyring-store`'s
    `keychain` store (security-framework was already linked in);
    elsewhere saving a key fails until that platform's phase (iOS needs
    the `protected` store and a provisioning profile). Reads are cached
    for the process; tests use a map in memory.
  - **Only the match is stored.** `album_links` gets the Discogs release
    id, status, score and `chosen_by`, with `details` NULL. Responses go
    through `Client::get_json_fresh`: in memory only, at most 5 hours
    (`discogs::MAX_AGE`, under the 6 the terms allow), no stale copy
    offline, the token in an `Authorization` header so it's never in a
    URL, an error or a cache key. `metadata_release_details` fetches a
    linked release through the worker when the album is shown.
  - **Matching** (`albums::ReleaseSource`, now also MusicBrainz's): the
    matcher, "Find details" and the user's picks run the same for both
    sources. Discogs' known release is the one the album's accepted
    MusicBrainz match links to (`release?inc=url-rels`, cached 30 days,
    a separate lookup so matched albums aren't fetched again), else a
    search (`/database/search`) whose best three are looked up and scored
    with durations; a search hit has no track count, which the matcher now
    leaves out rather than scoring 0. Masters play the part of release
    groups in `decide`. A changed MusicBrainz match drops an automatic
    Discogs match (it may have come from it), never the user's.
  - **Worker**: `Job::Discogs`, after an album's match, cover and
    description (so the MusicBrainz match can name the release), pausing
    while MusicBrainz or Discogs is backing off; background enrichment
    queues it, and runs with MusicBrainz off if Discogs is on. Discogs is
    1 request/s (`request_interval`), its limit being 60 a minute.
  - **Attribution**: "Data provided by Discogs", linked to the release's
    page, next to its data in the album header's summary and every row of
    the details table, and in "Find details" (linked to the Discogs search
    for the section, to the release for the one selected). The
    non-affiliation notice is under Discogs in Online sources. No Discogs
    image is fetched or shown anywhere, not even thumbnails.
  - **UI**: the Online sources panel has a token field (a password field,
    "Get one" linking to discogs.com's developer settings, "Remove"), notes
    that Discogs data is "fetched when shown, never stored", and says
    "Needs a personal access token" until there is one. The album header
    shows Discogs' summary, genres and styles when it's the first matched
    source, and "Styles" and "Credits" rows; without a connection it says
    the details aren't available. "Find details" lists each source with
    its own "Use automatic" and "None of these", and takes MusicBrainz or
    Discogs release links.
  - **Tests** (12 new, 222 in all): the memory cache, key checks, Discogs
    parsing (release, search, dates, positions, durations, formats,
    names), ids in links, the token header and a refused token, matching
    through MusicBrainz's link and by search, the user's pick surviving a
    new MusicBrainz match, the worker's chain with and without MusicBrainz,
    the "needs a token" message, and album details' credit and page. The
    three Discogs releases are recorded (fetched without a token) and
    trimmed to CC0 fields; the search response couldn't be recorded
    without a token, so it was put together in the documented format from
    them. `live_discogs` (ignored) needs `DISCOGS_TOKEN` and hasn't been
    run. `npm run check` passes. The keychain calls were checked in a
    scratch program (write, replace, read, delete) outside the sandbox.
  - **Not done or not checked**: nothing checked by eye in the app, with or
    without a real token; the keychain inside the sandboxed bundle;
    `record-fixtures.py` (§9.2 M3), which waits for M1's test harness, so
    the new fixtures were trimmed by hand.
  - **Known limits**: Discogs details need a connection each time an album
    is shown after 5 hours; a Discogs release id in the tags (as Picard
    plugins and beets write) isn't read, since the core reads MusicBrainz
    ids only; Discogs artist profiles (CC0) could be a biography source
    later; matching a large library on Discogs takes 1 to 4 requests an
    album at 1 a second; `live_discogs` and Discogs' own paging aren't
    exercised (10 results are asked for).
- **Exit:** a library of tagged and untagged albums gets details and covers
  from MusicBrainz and the Cover Art Archive; the user can reorder or turn
  off sources, pick another source's cover or match for an album, and the
  app behaves the same with the network off, showing what it cached.

### Phase 5 — Visualization
- [x] Core: lock-free FIFO tap on the output; FFT (`juce::dsp::FFT`) → log-spaced
  bins plus peak/RMS levels, published at ~30–60 Hz via callback.
- [x] Rust forwards the frames over a Tauri `Channel` (binary/compact payload,
  not per-frame JSON if it proves costly).
- [x] Frontend: canvas/WebGL renderers (spectrum bars, oscilloscope, VU) that
  users pick in preferences.
- [x] A cover wall (covers of albums from the current track's year or
  by its artist), and unusual visualizations: ridgelines, the circle of
  fifths with a key estimate, a vectorscope, a kaleidoscope of the cover.

  Built 2026-09-26. Design:
  - **The tap** (`SignalTap`) is a ring of relaxed atomic floats (32768
    samples a channel) that `PlayerEngine` writes each block while
    playing, before the volume (so turning it down doesn't shrink the
    visuals) and not while paused or stopped. Readers copy the latest
    window without taking it; a race costs at worst a glitched frame.
  - **Analysis** (`SpectrumAnalyser`) takes the latest 8192 samples: bands
    from a 2048-point FFT (about 43 ms, so the bars keep up), each band's
    loudest bin on a 70 dB scale tilted +3 dB an octave around 1 kHz;
    chroma (12 pitch classes, 80 Hz–5 kHz) from an 8192-point FFT for
    semitone resolution; peak and RMS over 2048 samples; a waveform
    starting at the latest rising zero crossing (a triggered scope); and
    beats, a bass (below 250 Hz) flux 1.5 deviations above its last
    1.5 s, at most 4 a second.
  - **`AnalysisThread`** runs only while a callback is set, at a steady
    rate (60 fps from Rust), and only when the tap has new samples. After
    150 ms without any it sends one silent frame and then nothing, so a
    paused player costs no IPC. The callback is on that thread, never the
    main one; the C API says it mustn't call the engine.
  - **Rust** (`visualizer.rs`) starts the analysis for the first
    subscriber and stops it after the last, on the main thread, never
    holding the subscriber lock while it waits for the analysis thread.
    Frames are 2.2 KB of binary (`encode` documents the layout: bands
    and chroma as bytes, the waveform as 16-bit), which go through
    Tauri's fetch path (over 1 KB). A page load drops all subscribers,
    since a reloaded page's channels accept frames that nobody reads.
  - **The cover wall** asks `library_cover_wall` for albums from the
    track's album year, widening by up to ±5 years until there are 16,
    or by the track's performer (their albums and those they appear on).
    The UI loads each cover as a 256 px thumbnail, leaves out albums
    without one, maps tiles to bands by distance from the centre (bass in
    the middle) and flips tiles on beats.
  - Every visualization takes its colours from the current cover
    (`palette.ts`); the art scheme now sends
    `Access-Control-Allow-Origin: *` so a canvas may read a cover's
    pixels. The VU meters put 0 VU at -12 dBFS RMS, not the studio's -18,
    which mastered music would pin.
  - The choice of visualization and the wall's year/artist switch are per
    viewer (`localStorage`) until Phase 6 moves them into the settings.
    Full screen (F, or double-click) is the window's, and shows the
    visualizer alone; V cycles the visualizations.
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
- **Known limits**: the bands' lowest octave has less than a bin each
  at 2048 points and is interpolated; the key estimate needs about ten
  seconds of music and trusts tonal music; the analysis sees what the
  player renders, a few blocks ahead of the speakers (output latency),
  and the bands lag about 20 ms behind the latest sample, which roughly
  cancel.

### Phase 6 — Admin / settings screen
- Settings are persisted in SQLite (or a Tauri store), with a typed schema in
  Rust and shared TS types (generated with `specta`/`ts-rs`).
- Sections: displayed fields and columns, enabled services, library folders
  and rescan, sort/grouping rules, visualization choice and parameters, audio
  output device and buffer size (desktop), and replay-gain on/off. The
  visualizer's choice and cover-wall basis move here from `localStorage`.

### Phase 7 — Hardening (macOS)
- CI (GitHub Actions, macOS runner): CMake build + ctest, `cargo test`,
  frontend lint/type-check/tests, all through `scripts/check-all.py`
  (§9.2 M4), plus a weekly job for outdated pins and advisories (M5).
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
| FFmpeg parser vulnerabilities (large attack surface) | Minimal configure (only needed demuxers/decoders); a weekly CI job checks the pin against FFmpeg's security releases, and `bump-pin.py` makes the bump quick (§9) |
| MusicBrainz rate limits and bans | Strict limiter, caching, User-Agent with contact info |
| Decoder behaviour differing across platforms | Same FFmpeg version and flags everywhere; the Phase 1 format tests run on every CI OS |
| WebKitGTK (Linux) and WebView2 (Windows) behave differently from WKWebView | Keep the frontend to standard web APIs; run frontend smoke tests on each OS in CI |
| Bandcamp refuses, limits or withdraws access (no API for fans; scraping forbidden) | Phase 11 starts only with written permission; the Bandcamp code is one Rust module behind the library-source kind, so it can be dropped without touching local playback; fallback 11.7 (purchases as local files) |
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

### 8.2 Cross-platform release setup
Build once (after Phase 7), reused for every platform.

- **Single version number:** CMake `project(VERSION)` is the source of truth;
  generate `anomp_version()` from it instead of the hard-coded string, and
  keep `Cargo.toml`, `tauri.conf.json` and `package.json` in sync with
  `scripts/version.py`, checked in CI (§9.2 M6). MusicBrainz `User-Agent` includes this version and a
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
  source tarball); TagLib; Rust crates (`cargo-about`); and npm packages,
  generated by `scripts/make-notices.py` (§9.2 M6). CI fails if a
  dependency's license is unknown.
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
2. Bump the version (`scripts/version.py`); update `CHANGELOG.md`.
3. CI green on every platform, including the format decode tests.
4. Regenerate third-party notices (`scripts/make-notices.py`); check the
   FFmpeg source link matches the pinned version. Run `scripts/bench.py`
   and `scripts/check-signing.py`, and do the §9.1 rows due "each release".
5. Tag; the release workflow builds, signs, notarizes and packages.
6. Smoke test each artifact on a clean machine: install/upgrade, play MP3,
   FLAC and AAC, seek, gapless album, media keys, MusicBrainz lookup.
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
| Native dependency pins: JUCE and Catch2 (`CMakeLists.txt`), TagLib (tarball + SHA-256, `cmake/TagLib.cmake`), FFmpeg (version + SHA-256, checked against its GPG signature, in `build-ffmpeg.sh`) | Monthly check; at once for a security release | Find the new release, download it, hash it, rewrite the pin, rebuild, run every test suite. For FFmpeg, also re-check the configure output and `BUILD_INFO` | `check-pins.py`, `bump-pin.py` |
| FFmpeg dylib names in `tauri.conf.json` (`bundle.macOS.frameworks`, later the Linux and Windows lists) | Each FFmpeg bump that changes a major version | The names must match what the build produced, or the bundle step fails | `sync-ffmpeg-frameworks.py` |
| Formatter pins: clang-format (`format-cpp.py`), ruff (`format-python.py`) | A few times a year | Bump, reformat the tree in one separate commit | `check-pins.py`, `bump-pin.py` |
| Rust crates and npm packages | Monthly | `cargo update` / `npm update`; read the changelogs of Tauri, Svelte, rusqlite (its bundled SQLite version), ureq/rustls; run all tests | `check-pins.py` (reports; updating stays manual) |
| Security advisories | Weekly (scheduled CI job); FFmpeg security releases as announced | `cargo audit` (RustSec), `npm audit`, the pinned FFmpeg against ffmpeg.org's security page, TagLib and JUCE release notes | `audit-deps.py` |
| Toolchains: Rust, Node (Node 26 becomes LTS in October 2026, §3), CMake, Ninja, nasm, uv; Xcode and SDKs from Phase 8 | Each stable/LTS release; Xcode each year | Update §3's versions, check the build still passes, raise documented minimums | `doctor.py` |
| Recorded service responses (`app/src-tauri/src/metadata/fixtures/`) | Quarterly, whenever a live test fails, and for each new source (4.8) | Re-fetch the same URLs with the app's `User-Agent` at the services' rate limits, trim them the same way, and diff with the committed copies: a changed field means a parser needs work | `record-fixtures.py` |
| Audio fixtures (`core/tests/fixtures/`) | When a format is added or the test signal changes | Regenerate, update the lengths and lags in the tests' fixture table | `make-test-fixtures.py` (extend) |
| Library DB migrations | Every schema change | Numbered with no gaps, listed in `MIGRATIONS`, shipped ones unchanged, FTS triggers updated when an indexed column changes | `check-migrations.py` |
| C API surface | Every change to `anomp.h` | Each function has an FFI declaration and a safe wrapper in `anomp.rs` | `check-c-api.py` |
| Core source lists | Every new core file | Listed in `core/CMakeLists.txt` or `core/tests/CMakeLists.txt` (no globbing) | `check-sources.py` |
| Docs drift: `PLAN.md` status and §2, `CLAUDE.md`, `README.md` | Each finished step | Test counts match the suites; every repo path the docs mention exists | `check-docs.py` |
| Performance baselines (the ignored 50,000-track benchmarks) | Each release; after scanner, browse or DB changes | Run them in release mode, compare with committed numbers, flag regressions | `bench.py` |
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
- [ ] M1 Test harness. `scripts/test-python.py` runs a pinned pytest
  through uvx (`uvx --from pytest==<version> pytest scripts/tests`), and
  `format-python.py` gains `ruff check` (lint) next to `ruff format`. Add
  tests for the existing scripts' pure logic (`format-cpp.py`'s file
  selection and batching; `make-test-fixtures.py`'s signal matching
  `TestSignal.h`'s constants).
- [ ] M2 Repo checks, each a read-only script that lists every problem it
  finds:
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
    builds, so not in the quick check).
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
- [ ] M4 CI entry point (with Phase 7's CI):
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
    set margin; `--update` rewrites the baseline.
- [ ] M5 Dependency tools (with Phase 7):
  - `check-pins.py`: reads every pin (JUCE, Catch2, TagLib, FFmpeg,
    clang-format, ruff) from its file and asks upstream for the latest
    release (GitHub releases, ffmpeg.org, PyPI); also summarizes
    `cargo update --dry-run` and `npm outdated`. Report only.
  - `bump-pin.py NAME VERSION`: downloads the release, computes its
    SHA-256 (and for FFmpeg verifies the GPG signature against the key
    recorded in `build-ffmpeg.sh`), rewrites the pin in place and prints
    the rebuild and test commands. Tests rewrite copies of the real files.
  - `audit-deps.py`: runs `cargo audit` and `npm audit`, and checks the
    FFmpeg pin against ffmpeg.org's security page; a scheduled weekly CI
    job runs it with `check-pins.py`.
- [ ] M6 Release tools (with §8.2):
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
- **Exit:** `check-all.py` runs in CI on every push, the scheduled job
  reports outdated pins and advisories, and each §9.1 row either has its
  script or is marked manual.
