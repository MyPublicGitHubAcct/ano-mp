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
| The state modules' tests | `npm run test:state` (in `app/`) | `app/tests/state/*.spec.ts` | the rune stores (`lib/state/`), against a fake backend |
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

### The state modules (`app/tests/state/`)

The rune stores (`lib/state/*.svelte.ts`) need Svelte's compiler, which
`node --test` doesn't run, so they have a suite of their own: Vitest
(`npm run test:state`, configured by `tests/state/vitest.config.ts`)
under jsdom, with SvelteKit's plugin compiling the runes and resolving
`$lib`, and Svelte's browser build, whose `$effect`s run. The files are
`*.spec.ts` because Node's test runner takes any `*.test.ts` under
`tests/` for its own. There is a file per store (`status.spec.ts` holds
the updates' and the metadata worker's; `player.spec.ts` covers
`position.svelte.ts` too).

`api.ts` runs as it does in the app. `backend.ts`'s `FakeBackend`
installs Tauri's IPC mocks (`@tauri-apps/api/mocks`) and answers each
command by its wrapper's name in `generated/commands.ts`, its arguments
and reply typed from the wrapper:

```ts
const backend = fakeBackend()
  .answer("settingsGet", settingsFixtures.get) // a reply
  .answer("settingsSave", ({ settings }) => settings); // or one made from the arguments
backend.fail("queueNext", coded("trackGone")); // what a Rust Err(String) rejects with
const { player } = await import("$lib/state/player.svelte");
player.connect();
await settle(); // replies arrive, listeners are registered
await backend.emit("queue-changed", queueFixtures.removed);
expect(backend.argsOf("queueSeek")).toEqual([{ seconds: 60 }]);
```

- **Import the store inside the test**, after `fakeBackend()`. The
  stores are singletons, so `setup.ts` resets the module registry before
  each test (and empties `localStorage`); a static import would keep one
  store for the whole file.
- **Answer every command the store calls.** An unanswered one rejects,
  and `setup.ts` fails the test, naming it, so a store that starts
  calling something new shows up. Replies and payloads are copied, as
  the real IPC deserializes them.
- **Effects**: `reactive.svelte.ts`'s `watch(read)` runs `read` in an
  `$effect` under `$effect.root` and counts its runs, as a component
  would; `flush()` is `flushSync`. Import it inside the test as well:
  the registry's reset gives each test a fresh copy of Svelte's runtime,
  and a static import would drive another copy than the store's.
- **Payloads** come from `fixtures.ts`, the JSON the Rust tests record
  (below), typed with the generated types. A small record no backend
  logic builds (a `PlayerStatus`, a `Playlist`) is a literal typed from
  `generated/ipc.ts`, so svelte-check catches a field that changes; a
  coded error no fixture records is `coded(code, params)`.
- **Test behaviour**: the commands sent and their arguments, the state
  after a reply or an event, errors shown through `errorText`. Compare
  text with `t(key, params)`, never with English.
- **Timers**: `vi.useFakeTimers({ toFake: ["setInterval",
  "clearInterval"] })` for a store that polls; `settle` needs the real
  `setTimeout`.
- The fake answers the app's commands only, so the system's dialogs
  (`@tauri-apps/plugin-dialog`) and the window's API (full screen) stay
  untested; jsdom has no layout, so `drag.spec.ts` supplies
  `elementFromPoint`, and `appearance.spec.ts` supplies `matchMedia`.

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
- **The state tests' payloads** (`app/tests/state/fixtures/`): JSON
  written by a `ui_fixtures` test in each module that builds them
  (`settings.rs`; `queue/model.rs`, with the queue's fake player;
  `servers/offline.rs`, over a synced test source;
  `library/availability.rs`), through `ui_fixtures.rs`'s `record`. Like
  the bindings, `cargo test` fails while one is stale, and
  `ANOMP_WRITE_BINDINGS=1 cargo test ui_fixtures` rewrites them. Times,
  temporary paths and the system's words are pinned, so a rerun changes
  nothing. Only what the state tests use is recorded.
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
| `FakeBackend` | the backend, for the state tests: commands answered by their wrappers' names, events through Tauri's mocks | `app/tests/state/backend.ts` |

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
