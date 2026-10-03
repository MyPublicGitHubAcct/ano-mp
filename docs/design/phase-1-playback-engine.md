# Phase 1 — Playback engine (C++ core)

Moved from `PLAN.md` (H20) on 2026-10-03, unchanged but for this
heading. Section numbers (§) refer to `PLAN.md`, whose Phase 1
keeps the phase's status, decisions and open steps.

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
    synchronous on the main thread (7 ms for a 5-minute VBR MP3). H11
    (Phase 7) moved the queue's opens to threads of their own.
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
