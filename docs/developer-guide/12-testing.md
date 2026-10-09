# 12. Testing

What each suite covers, how its fixtures are made, the fakes that keep
tests off the hardware and the network, the benchmarks and the fuzzers.
`PLAN.md` §2 states each suite's size, and `check-docs.py --counts`
compares those numbers with the suites, so update them when you add or
remove tests.

No test touches a sound device, the network or the user's library. The
engine is rendered offline, the services are recorded responses, and
folders are temporary directories.

## The suites

| Suite | Run | Where | Covers |
|---|---|---|---|
| The core's Catch2 tests | `ctest --preset debug` | `core/tests/` | decoding, tags, the player, analysis, recording, the C API, platform code |
| The effects' Catch2 tests | the same `ctest` | `effects/tests/` | each effect, the chain, the FFT, the freeze |
| The same, sanitized | `cmake --workflow --preset asan` / `tsan` | `build/asan`, `build/tsan` | memory errors, undefined behaviour, data races |
| Rust's tests | `cargo test` (in `app/src-tauri`) | a `tests` module in each file | every backend module, the C API wrappers against the real core, the generated files |
| Rust's ignored tests | `cargo test -- --ignored` | `library/bench.rs`, `live_*` | the 50,000-track benchmarks; one live check per online service |
| The frontend's tests | `npm test` (in `app/`) | `app/tests/*.test.mjs` | the pure modules |
| The scripts' tests | `scripts/test-python.py` | `scripts/tests/` | every script, each repo check once against the real tree |

### The core (`core/tests/`)

One file per area; each `TEST_CASE` is its own CTest test
(`catch_discover_tests`), tagged by area (`[player]`, `[tags]`,
`[formats]`, `[c-api]`…):

- `FFmpegAudioFormatTests.cpp`: every fixture decodes to the source
  signal (lossless bit-exact; lossy aligned by cross-correlation, since
  lossy float output isn't bit-stable across CPUs), exact lengths, seeks
  against a straight decode, memory streams, bad input.
- `FFmpegBuildTests.cpp`: the FFmpeg build is LGPL and has exactly the
  demuxers, decoders, encoders and muxers the scripts list.
- `FormatRegistryTests.cpp`: extensions and the registry.
- `PlayerEngineTests.cpp`: the player rendered offline through
  `getNextAudioBlock`, with and without read-ahead: state transitions,
  bit-exact playback and seeks, fades, volume, gapless hand-offs across
  formats and rates, crossfades, parts of files, loops, tempo,
  asynchronous loads, the effects, equaliser and crossfeed.
- `TagReaderTests.cpp`: the two `tagged-*` fixtures' tags, pictures,
  chapters, lyrics, ratings; streams in memory; the inputs the fuzzer
  found.
- `AnalysisTests.cpp` and `FileAnalysisTests.cpp`: the visualizer's
  analysis and the loudness analysis on synthetic signals with known
  answers.
- `RecorderTests.cpp`: the FIFO, track marks, a rate change's new file,
  write failures, with a fake encoder and the real one.
- `FolderAccessTests.cpp`, `MediaControlsTests.cpp`,
  `VolumeWatcherTests.mm`: the platform interfaces' logic and their C
  API, including null handles.
- `CApiTests.cpp`: the player's C API, including a null engine.
- `BenchTests.cpp`: hidden benchmarks (`[.][bench]`), run by `bench.py`.

### The effects (`effects/tests/`)

`ChainTests.cpp` (the catalogue, bit-identical when off, clamping, fades
without clicks, tails ringing out, the freeze's hold, settings from
another thread), `EffectTests.cpp` (each effect at every sample rate),
`FftTests.cpp`, `FreezeTests.cpp`, with test signals in `Signals.h`.

### Rust

Each module tests itself in a `#[cfg(test)] mod tests`, against an
in-memory database (`db::open_in_memory`, or `open_in_memory_at(n)` to
start from an older schema) or a temporary folder. Some areas worth
knowing:

- `queue/model.rs`'s tests drive the queue's logic against a fake
  player, step by step, including pending loads and timeouts.
- `library/scanner.rs`'s tests scan temporary folders of copied fixture
  files: moves keep ids, folders aren't emptied, placeholders aren't
  read.
- `library/db.rs`'s tests run every migration on a file database and
  check the copy written first.
- `metadata/jobs.rs`'s tests run the worker a step at a time with the
  fake transport and clock.
- `bindings.rs` and `guide.rs` are tests that compare the generated files
  with what they would write.
- `self_test.rs` runs its stages unsandboxed.

### The frontend (`app/tests/`)

Plain JavaScript (`.mjs`) run by `node --test`, importing the pure
TypeScript modules as Node runs them (svelte-check leaves `.mjs` alone,
which would otherwise need Node's types): `queueEdits`, `selection`,
`folders`, `links`, `similar` (every recommendation reason has a
message), `find` (finding a feature: the matching, the ranking, where a
result opens, and an entry for every sidebar item, Settings section,
feature switch and page menu item, with its control in its section),
`workbench`, `i18n` (the catalogue is well formed and every Rust error code
has a message), `theme` and `contrast` (every built-in theme meets WCAG
AA), `effects`, `equaliser`, `guide` (the user guide parses and its links
resolve), `visualizer` (decoding the backend's frames, the key
estimate), `visualizations` (the musical analysis) and `safety` (the
flash guard). Components have no unit tests; `npm run check` type-checks
them.

### The scripts (`scripts/tests/`)

pytest, pinned and run through `uvx` by `scripts/test-python.py`. Each
script has a test file; each repo check has at least one test that runs
it against the real tree, so a check that would fail on the repo as it
is fails here first. `conftest.py`'s `script` fixture imports a script
by name.

## Fixtures

- **Audio** (`core/tests/fixtures/`): every file encodes the same
  deterministic chirp (`core/tests/TestSignal.h`, which must match
  `scripts/make-test-fixtures.py`), in each format and container the app
  plays, including long ones for seeking, a header-less MP3 and ADTS
  AAC. The decoder tests' table holds each file's length and encoder lag
  (properties of the encoded files: update them if you regenerate).
  Regenerating needs Homebrew's `ffmpeg` and `vorbis-tools` (used only to
  encode fixtures, never linked); `--only NAME` remakes one, and a rerun
  changes nothing unless the signal or an encoder changed. The two
  `tagged-*` files carry the tags `TagReaderTests.cpp` expects.
  `self_test.rs` embeds every audio fixture.
- **Fuzzer finds** (`core/tests/fixtures/fuzz/`): inputs that once
  crashed a fuzz target, kept as regression tests.
- **Service responses** (`app/src-tauri/src/metadata/fixtures/`): real
  responses, trimmed, each with its `manifest.json` entry saying how it
  was made; `scripts/record-fixtures.py` re-records them and `--check`
  shows the drift from the live services.
- **A synthetic library** (`library/test_library.rs`): an in-memory
  library of made-up tracks, for browse, search and recommendation
  tests.

## Fakes

| Fake | Stands in for | Where |
|---|---|---|
| `access::testing::FakeBookmarks` | security-scoped bookmarks: folders that move, bookmarks that fail | `library/access.rs` |
| `scanner::testing::DATALESS` | which files are cloud placeholders | `library/scanner.rs` |
| the queue's `Fake` player | the engine behind the `Player` trait, with pending loads | `queue/model.rs` tests |
| `http::testing::FakeTransport` and `FakeClock` | the network and time: recorded responses, failures, rate limits, back-off | `metadata/http.rs` |
| `FakeHost` | the worker's events | `metadata/jobs.rs` tests |
| `metadata::keys`' in-memory store | the keychain | `metadata/keys.rs` |
| `images::testing` | the image cache's pictures | `metadata/images.rs` |
| `media.rs`'s fake `Publisher` | the OS's Now Playing | `media.rs` tests |
| `visualizer.rs`'s `FakeSink` | the page's channel | `visualizer.rs` tests |
| `LibraryState::for_tests` | the app's library state over a test connection | `library/commands.rs` |
| a fake encoder | FFmpeg's encoder, failing on demand | `core/tests/RecorderTests.cpp` |

## Benchmarks

`scripts/bench.py` runs the Rust benchmarks (`library/bench.rs`:
scanning, browsing, search and recommendations on a synthetic 50,000-track
library, in release) and the core's hidden `[.][bench]` tests
(`core/tests/BenchTests.cpp`, `bench` preset: playback through the
engine, with and without effects; the visualizer's analysis; every
effect at once and the recording's tap at 192 kHz).
Each prints `bench <key> <value> <unit>`; `scripts/bench-baseline.json`
holds a **budget** per key (H18's limits, which fail the run) and the
**results** last recorded on one machine (compared within 25% only on
that machine). `--only rust|core` runs one half; `--update` rewrites the
results, never the budgets. A new benchmark prints its line and gets a
budget. Run it after changing the scanner, browsing, search, the
database or the effects; it is a release step, not part of `check-all`.

## Fuzzing

`core/fuzz/` holds two libFuzzer targets: `DecoderFuzzer.cpp` (any bytes
as an audio file, through `FFmpegAudioFormat`) and `TagReaderFuzzer.cpp`
(through TagLib from memory). They build with the `fuzz` preset, which
needs Homebrew's `llvm@22` and an instrumented static FFmpeg
(`build-ffmpeg.sh --fuzz`, without LAME); `scripts/run-fuzzers.py` does
both and runs each target for 60 seconds (`--seconds N`, `--target
decoder|tags`). `check-all.py` runs it on every full check, and a weekly
workflow (`.github/workflows/fuzz.yml`) runs each for 30 minutes, keeping
the corpus between runs. [Chapter 10](10-troubleshooting.md#reproducing-a-fuzzer-crash)
says how to reproduce and keep a crash. A new target goes in
`core/fuzz/CMakeLists.txt` and `run-fuzzers.py`'s `TARGETS`.
