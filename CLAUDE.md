# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

ano-mp is a music player for desktop (macOS, later Linux/Windows) and iPhone/iPad. It
plays MP3, FLAC and other common formats, shows file metadata enriched from services
such as MusicBrainz, and has an admin screen for configuring displayed fields, enabled
services, library sort/grouping rules and visualization preferences.

`PLAN.md` is the authoritative roadmap: its status line, §2 (where each component
lives, and the test counts), the phases, §4 decisions (JUCE commercial license, FFmpeg,
TagLib, Svelte 5, minimum OS targets), risks and release gates. Read it before
starting anything non-trivial, and update it when a step finishes or a decision
is made (§2's test counts too: `check-docs.py --counts` compares them).
Phases 0–6c are built; Phase 7 (hardening, macOS) is under way and follows its
"Order of work"; the signed release waits on the owner's §8.1 decisions.

Finished work's design notes, steps as built and known limits go in
`docs/design/` (`phase-<n>-<name>.md`, linked from the phase in `PLAN.md`, which
keeps status, decisions and open items). Move them there word for word when they
are finished: a whole phase, or one of Phase 7's steps or H items
(`docs/design/phase-7-hardening.md`). `docs/` also holds the Step 4 checklist,
the release decisions' briefs, the release smoke test and how to run a bundle
check safely.

## Build & test

```sh
scripts/build-ffmpeg.sh   # once, and after changing its pin/flags (~1.5 min)
cmake --preset debug && cmake --build --preset debug && ctest --preset debug
scripts/format-cpp.py     # clang-format core/ after editing it (--check: report only)
scripts/format-python.py  # ruff format + ruff check scripts/*.py after editing them (--check: diff only)
scripts/format-frontend.py # Prettier over app/ after editing the frontend (--check: report only)
scripts/check-all.py      # every check, as CI runs it (--quick: formatters, repo checks, script tests, gitleaks)
git config core.hooksPath scripts/hooks  # once per clone: the pre-commit hook (gitleaks, check-all --quick)

cmake --workflow --preset asan  # configure, build and ctest under ASan + UBSan (build/asan)
cmake --workflow --preset tsan  # the same under TSan (build/tsan; ~4.5 min)
scripts/run-fuzzers.py          # build and run each libFuzzer target 60 s (--seconds N, --target decoder|tags)
scripts/run-fuzzers.py --target tags build/fuzz/crashes/tags-crash-…  # reproduce one input
```

The fuzz targets (`core/fuzz/`, PLAN.md H5) need Homebrew's `llvm@22` (Apple's
clang has no libFuzzer runtime; LLVM 21's ASan hangs on macOS 26). The `fuzz`
preset links an instrumented static FFmpeg (`build-ffmpeg.sh --fuzz`, which
`run-fuzzers.py` runs). On CI, `ANOMP_FUZZ_DEVELOPER_DIR` points the fuzz build
alone at the runner's Xcode 26.3 (`run-fuzzers.py` passes it as
`DEVELOPER_DIR`), since Xcode 16's ld can't read clang 22's objects and
Homebrew's lld breaks C++ exceptions; every other build keeps the default Xcode.
A crash found becomes a regression test with a committed fixture in
`core/tests/fixtures/fuzz/` ("Inputs the fuzzer found stay harmless" in
`TagReaderTests.cpp`); a new target goes in `core/fuzz/CMakeLists.txt` and
`run-fuzzers.py`'s `TARGETS`. TagLib is built with only the formats FFmpeg
plays (`cmake/TagLib.cmake`'s `WITH_*`): turn one on only with its FFmpeg
demuxer.

Releases (PLAN.md §8.2, §8.3, §8.7):

```sh
scripts/version.py 0.2.0         # set the version everywhere (--check: compare only)
scripts/make-notices.py          # regenerate THIRD_PARTY_NOTICES (--check in check-all)
CI=true scripts/build-app.py     # universal .app and DMG (--native: this Mac only; ad-hoc
                                 #   unless --identity/APPLE_SIGNING_IDENTITY, then hardened;
                                 #   CI=true skips the DMG's Finder layout, which waits
                                 #   on the Automation permission)
scripts/check-bundle.py PATH.app # slices, FFmpeg install names, signatures, entitlements
scripts/notarize.py PATH.dmg     # notarytool + staple; prints the plan without NOTARY_* keys
scripts/release.py BUNDLE_DIR    # check-bundle, then dist/: DMG, app zip, SHA256SUMS, notes
scripts/check-signing.py         # signing certificates' expiry (monthly once they exist)
scripts/self-test-bundle.py      # build a sandboxed bundle with the self-test and run it (H14;
                                 #   CI only: --local runs it here, in the real container)
scripts/bench.py                 # benchmarks against H18's budgets and the baseline (--update,
                                 #   --only rust|core); a release step, not in check-all
```

- **Benchmarks** (H18): `bench.py` runs the ignored Rust benchmarks
  (`library/bench.rs`) and the core's hidden `[.][bench]` tests
  (`core/tests/BenchTests.cpp`, `bench` preset), and checks each against
  `scripts/bench-baseline.json`'s budgets (and, on the machine it records, its
  results within 25%). A new benchmark prints `bench <key> <value> <unit>` and
  gets a budget in the JSON; `--update` rewrites results, never budgets. Run it
  after scanner, browse, search or DB changes.
- **Bundle self-test** (H14): `src-tauri/src/self_test.rs`, compiled only with
  the `self-test` feature; `ano-mp --self-test` scans, resolves a bookmark,
  reads covers, decodes and plays a gapless hand-off on the embedded fixtures
  inside the sandbox, with none of the app's files (`cargo run --features
  self-test -- --self-test` runs it unsandboxed). A new sandbox-only behaviour
  gets a stage there; a new fixture in `core/tests/fixtures/` goes in its
  embedded list (`every_audio_fixture_is_embedded`).
- **`THIRD_PARTY_NOTICES`** (repo root) is generated and committed: regenerate
  it whenever `Cargo.lock`, `package-lock.json`, a native pin or `BUILD_INFO`
  changes (Dependabot's updates included), or `check-all` fails. A licence
  outside `deny.toml`'s `allow` list fails it, as does a library JUCE vendors
  (`JUCE.spdx.json`) that it doesn't list; a package without a licence text gets
  a standard one from `scripts/licenses/`.
- **The release workflow** (`release.yml`, on a `v*` tag or by hand for a
  trial) calls `ci.yml`, then `build-app.py`, `notarize.py` and `release.py`,
  and drafts a GitHub Release. Checks go in the scripts, never in the workflow.
  Steps that need the Developer ID secrets are named "[signed releases]" and
  skipped without them. Release notes are the version's section of
  `CHANGELOG.md` (Keep a Changelog): add to `## [Unreleased]` as you go, rename
  it when tagging. The hardened runtime is on exactly when a signing identity is
  given (`tauri.conf.json` keeps the ad-hoc local settings); the bundle
  identifier is the owner's (`docs/release-decisions.md`).

Tools beyond the build: `brew install uv gitleaks llvm@22`, and `cargo install
cargo-deny --version 0.20.2 --locked` (the version CI installs). Rust and
Node are pinned by `rust-toolchain.toml` and `.nvmrc`; `app/.npmrc` makes
npm refuse a Node outside `package.json`'s `engines`.

CI (`.github/workflows/ci.yml`, macOS) runs `scripts/check-all.py` and nothing
else, so a new check goes in that script, not in the workflow. It runs by hand
and on version tags, not on pushes (README.md, "When GitHub Actions run"), so
run `check-all.py` locally before pushing, and CI by hand on a branch whose
result matters (Dependabot's). The repo checks are read-only scripts:
`check-c-api.py` (each `anomp.h` function bound in `anomp.rs` with the same
parameter count, unless in its `NOT_BOUND` list), `check-sources.py`,
`check-migrations.py`, `check-docs.py` (every path and link in the docs exists)
and `version.py --check`. Scripts use the standard library only; their tests
are in `scripts/tests/` (pytest, `scripts/test-python.py`), each repo check
with one test against the real tree. Actions are pinned by commit SHA (look a
new one up with `git ls-remote`, never guess it); Dependabot proposes updates
monthly.

The format scripts are Python so they run on every platform (`py` on Windows),
running pinned formatters through `uvx`. `.clang-format` is JUCE style;
`core/include/.clang-format` keeps `anomp.h` in C style. `format-frontend.py`
runs the Prettier pinned in `app/package.json`. Reformat in a commit of its own.

FFmpeg is built by `scripts/build-ffmpeg.sh` (pinned version, LGPL, audio-only,
shared) into `third_party/ffmpeg/<platform>/` (git-ignored); CMake refuses to
configure without it. `cmake/FFmpeg.cmake` exposes `FFmpeg::avformat`,
`FFmpeg::avcodec`, `FFmpeg::swresample`, `FFmpeg::avutil`. To add a format, add
its demuxer/decoder/parser to the script's lists (the configure flags stay
minimal on purpose) and extend `core/tests/FFmpegBuildTests.cpp`.

Decoder tests use committed fixtures in `core/tests/fixtures/`, all encoding one
deterministic chirp (`core/tests/TestSignal.h`, which must match
`scripts/make-test-fixtures.py`). Regenerating needs `brew install ffmpeg
vorbis-tools` (Homebrew's FFmpeg only encodes fixtures; it is never linked).
Lengths and lags in the tests' fixture table are properties of the encoded
files, so update them if you regenerate. Only the Vorbis fixtures change on a
rerun (random stream serials): `git checkout` them unless you meant to change
them. The two `tagged-*` fixtures carry the tags `TagReaderTests.cpp` expects.

Pins: JUCE 9.0.2 and Catch2 v3.16.0 (GitHub's archive of the release's commit +
SHA-256, top-level `CMakeLists.txt`), TagLib 2.3.2 (`cmake/TagLib.cmake`),
Signalsmith Stretch 1.4.0 and its FFT library (`cmake/Signalsmith.cmake`). Each
is declared with `anomp_fetch_declare` (`cmake/Fetch.cmake`), never
`FetchContent_Declare` (`test_fetch_cmake.py` checks): the tarball is downloaded
once into `build/_downloads/` and checked, shared by every preset and Cargo's
build. The first configure takes several minutes. TagLib is a separate static
lib (`libtag.a`), so `build.rs` links it next to `anomp_core`. The `release`
preset sets `ANOMP_BUILD_TESTS=OFF`. Tauri's crates and plugins are pinned
exactly in `app/src-tauri/Cargo.toml`, and their npm packages exactly in
`app/package.json`; `tauri build` refuses mismatched major.minor versions, so
update each crate and its npm package together.

Each Catch2 `TEST_CASE` is registered with CTest individually via `catch_discover_tests`:

```sh
ctest --preset debug -N                              # list test names
ctest --preset debug -R "FormatRegistry round-trips" # run one by name (regex)
./build/debug/core/tests/anomp_core_tests "[formats]" # or run the binary by tag
```

The app (run from `app/`; `npm install` once):

```sh
npm run tauri dev            # run the desktop app
npm run check                # svelte-check / TypeScript
npm run lint                 # ESLint (eslint.config.js)
npm test                     # frontend unit tests (node --test tests/, plain .mjs)
cd src-tauri && cargo test   # Rust tests, including the C API wrappers
cargo clippy --all-targets -- -D warnings  # Rust lint, as check-all runs it
cargo deny check             # advisories, licences, bans, sources (deny.toml)
ANOMP_WRITE_BINDINGS=1 cargo test bindings  # regenerate src/lib/generated/ (settings, ipc, commands)
../scripts/format-rust.py    # rustfmt the Rust code after editing it (--check: diff only)
```

Clippy runs with `-D warnings` and the `[lints]` in `Cargo.toml`: every
`unsafe` block has a `// SAFETY:` comment, and an unsafe fn's body still
wraps its unsafe operations in blocks. Fix a new warning, or allow it at
the site with the reason. A crate with a licence outside `deny.toml`'s list
is an owner decision (`PLAN.md` §8.1).

`app/src-tauri/build.rs` builds `anomp_core` with the `cmake` crate (Ninja, tests off)
into Cargo's `target/` dir, separate from `build/<preset>`. It reruns when `core/`,
`cmake/` or the top-level `CMakeLists.txt` changes. FFI declarations and their safe
wrappers live only in `app/src-tauri/src/anomp.rs`; add a wrapper there for each new
C API function.

## Rules by area

**Library DB.** The schema changes only by appending a numbered SQL file to
`app/src-tauri/src/library/migrations/` and listing it in `MIGRATIONS`
(`library/db.rs`); never edit a migration that has shipped. Test a migration
against a file database too (`db` tests' `file_at`), so the copy `db::open`
writes first (`library.sqlite3.pre-<n>`, H10; a copy that can't be written
stops the migration) is exercised. The launch check (`library/recovery.rs`)
carries out a restore or rebuild at the next launch, before anything opens the
database, and keeps the damaged file. Tracks store paths
relative to their folder, '/'-separated (`library::track_path` joins them).
`db::configure` registers SQL functions (`anomp_sort_key`, `anomp_genres`,
`anomp_has_genre`) on every connection: open connections only through `db`, and
never use these functions in the schema, an index or a migration, since other
SQLite clients don't have them. Browse queries (`library/browse.rs`) are built
from fixed SQL fragments; bind every value, never format it in. Search uses FTS5
word indexes (migration 008) and trigram indexes (009) kept in step by triggers:
a schema change to `tracks`, `artists` or `albums` columns they index must update
both sets of triggers in a new migration. Run the search benchmark after
changing the search SQL.

**Tracks are parts of files.** `tracks` is unique on (folder, path,
`range_start`), `range_end` NULL for the end of the file, so a cue sheet's
tracks or a file's chapters are rows of one file. Anything that opens a track
for playback goes through `library::playback::track_play`, which turns the row,
the user's preferences, the analysis and the feature settings into the
engine's `TrackOptions`; refresh a gain with `set_track_gain_at` (by file and
start), never by file alone. Track gains (ReplayGain) are computed in Rust
(`PlaybackSettings::gain`) and passed to the engine with each track.

**Sandbox and folder access.** Under the macOS sandbox a library file can be
opened only while its folder's bookmark is resolved, so anything that opens
library files goes through `library::access::open_folder`/`open_folder_of` first
and holds the result until the file is open. `tauri dev` runs unsandboxed, so a
missing call only fails in a sandboxed bundle (`npm run tauri build -- --bundles
app`: ad-hoc signed, FFmpeg embedded). While ad-hoc signed, a bookmark resolves
only in the build that made it ("isn't in the correct format"): `open_folder`
replaces it when the folder is still readable, but a rebuilt bundle's folders
must be picked again (`add_folder` replaces the bookmark). A bundle check uses
the real container (`~/Library/Containers/dev.anomp.player`), which holds the
owner's library: **ask before running one**, and follow `docs/bundle-checks.md`.
`tauri.conf.json`'s `bundle.macOS.frameworks` names FFmpeg's major versions:
update it when the FFmpeg pin changes. Only debug builds have an rpath into
`third_party/`; `cargo test --release` gets it from `app/src-tauri/.cargo/config.toml`.

**Unreadable folders are never empty** (H22). A folder's state
(`access::FolderState`, kept by `library::availability`) says why it can't be
read, and its tracks stay. A scan that would remove all of a folder's tracks, or
most of a large one, keeps them and fails the folder (`scanner::holds`); only
`library_remove_missing` removes them. Code that opens files treats a
`folderUnavailable` error (`FolderState::of_error`) as "not now", never as a
broken file, and doesn't store it as a failure. Lists show an unreadable
folder's tracks dimmed (`lib/folders.ts`); anything that picks tracks to play
(radio, smart playlists' play, Home's suggestions) leaves them out with
`availability::unreadable` (not `mostlyGone` folders), binding the ids through
`json_each`. A deleted folder's bookmark fails like another build's, so
`access::unresolved` goes by the stored path first; `open_folder` doesn't follow
a folder into the Trash. Folder-state tests use `access::testing::FakeBookmarks`
over temp dirs, not real bookmarks.

**Cloud placeholders** (H12). A file can be dataless ("Optimize Mac
Storage"), downloaded by any read. Never read a library file's tags or audio in
the background (scans, workers, art) without checking `anomp::file_is_dataless`
first (`stat()`, which doesn't download): the scanner records a placeholder
unread and marks it `tracks.dataless`, the analysis worker leaves it, and every
scan checks marked ones again. Reads the user asks for (playing, Get Info) may
download.

**The engine is main-thread only.** JUCE's message loop rides on the main run
loop that Tauri runs, so `anomp_engine_*` calls happen on the main thread and
event callbacks arrive there. `app/src-tauri/src/audio.rs` enforces this: the
engine lives in a main-thread `thread_local`, commands go through `with_engine`
(which hops to the main thread if needed), and the engine is dropped on
`RunEvent::Exit`. `run_on_main_thread` called on the main thread runs the
closure at once. The queue lives on the main thread too (`queue::run`): do
database work before hopping there, and never hold the library connection while
waiting for the main thread. Queue logic goes in `queue/model.rs` behind the
`Player` trait, tested against a fake engine. The media controls
(`anomp_media_controls_*`) are main-thread only; `media.rs` decides what to
publish in `NowPlaying`, behind a `Publisher` trait. The visualizer's analysis
callback (`anomp_engine_set_analysis_callback`) is called on its own thread,
never the main one. Engine event callbacks may call the engine (the queue arms
the next track from `TrackEnded`); `anomp.h` states the rule. `PlayerEngine`
is a plain `juce::AudioSource` with no device (`AudioEngine` owns the device);
tests render it offline through `getNextAudioBlock`
(`core/tests/PlayerEngineTests.cpp`), and its design is in
`docs/design/phase-1-playback-engine.md`.

**Files open off the main thread** (H11). `Player::load` and `set_next` may
return `Opening::Pending`, and the host reports the outcome with
`Queue::load_finished` (`queue/opening.rs`: the row and the bookmark on a
blocking thread, then `Engine::load_track_async` on the main thread, holding the
folder open until the engine's `Event::LoadFinished`). Every request is
reported once, in request order; a later load supersedes earlier ones. Never
open a library file synchronously on the main thread for playback; the
synchronous `load_track` stays for the core's tests and the dev page. A request
still opening after `LOAD_TIMEOUT` fails with `openTimedOut` (a placeholder gets
`DOWNLOAD_TIMEOUT`).

**The saved queue** (H16) is rows of `queue_items`, kept in step by
`queue/store.rs` applying the same `model::Edit`s the frontend gets; the rest
(current index, position, repeat, `shuffled`, volume) is the `player.queue`
setting. A model change to the list must log an edit (`edited`) or a reset
(`reset_list`), or neither the UI nor the rows follow it. The UI applies edits
by `listVersion` (`lib/queueEdits.ts`) and asks for the whole state when it
misses one.

**Generated types** (H15). The settings' TypeScript types
(`app/src/lib/generated/settings.ts`) are generated from the Rust types by ts-rs
(derive it with `#[cfg_attr(test, derive(ts_rs::TS))]` and list the type in
`settings::bindings`). `src-tauri/src/bindings.rs` generates `generated/ipc.ts`
(every payload and event type: derive `TS` on a new one and add it, or a type
containing it, to `declare_types`; a name the frontend already uses differently
gets `#[cfg_attr(test, ts(rename = "…"))]`) and `generated/commands.ts` (a typed
wrapper per command in `generate_handler!`, read from its signature with syn).
`cargo test` fails while any is stale: regenerate (`ANOMP_WRITE_BINDINGS=1 cargo
test bindings`) and commit; never edit them by hand. `api.ts` calls
`commands.*`, never `invoke` with a string. A new setting goes in `AppSettings`
with a default and a `validate` rule; stored values are read leniently, so no
migration is needed. The UI reads settings through `state/settings.svelte.ts`,
loaded in `routes/+layout.ts` before any page renders.

**Optional features.** Each checks its switch in `FeatureSettings` where it acts
(commands refuse, workers idle, the UI hides); a new one gets a switch there, off
by default if it costs a lot, changes what is heard, goes online or listens on
the network. The LAN remote answers local addresses only and keeps only hashes
of tokens; any change to `remote/` needs the security review in `PLAN.md` §8.1.

**Logging** (H9). Log through the `log` macros, never `eprintln!`
(`logging.rs`). The target is the module; don't prefix messages with it. At info
and above, write ids and counts only: titles, artists, file names and paths go
at debug (the formatter also scrubs absolute paths and URL paths at info and
above, and redacts keys, `Authorization` values and `token=`-like parameters at
every level). A new key or token the app holds goes through `metadata::keys`,
which registers it with `logging::keep_secret`. The core logs through
`anomp_set_log_callback` (`core/src/Log.h`: `anomp::log::warn` and so on);
JUCE's `Logger` and failed assertions arrive there. Anything in the core that
outlives JUCE's runtime at exit (a static, a base class of `AudioEngine`) must
let go of JUCE first, or the quit aborts. "Copy diagnostics" (`diagnostics.rs`)
holds no paths or titles; adding to it, or to what logs may contain, is an owner
decision.

**UI text.** Every string the UI shows comes from `app/src/lib/i18n/en.json`
through `t` (typed keys; a plural message is an object of `Intl.PluralRules`
forms), never a literal in a component. Rust errors the UI shows are made with
`crate::coded` (`{code, params, message}`), each code with an `error.<code>`
message in `en.json` (`tests/i18n.test.mjs` checks); the UI shows a command's
error through `errorText`, never `String(error)`. A coded error with a `reason`
param may have `error.<code>.<reason>` messages, which `errorText` prefers;
`errorCode` reads a code to branch on.

**Developer surface** (H2). The /dev page and its commands (`src/dev.rs`) exist
in debug builds only: the commands are registered under
`#[cfg(debug_assertions)]` in `generate_handler!`, and the page loads only when
the Vite constant `__DEV_TOOLS__` is true. Put anything that bypasses the queue
or takes a raw path there.

**Covers** go to the UI as thumbnails (H17, `library/thumbs.rs`): `artUrl(…,
"list" | "header")`, or `"full"` only where a picture is shown full size.
Thumbnails live in the image cache, named by the SHA-256 of their picture, and
`art_thumbs` remembers which picture each album shows. A new album-art source
sets `Art::origin`, or its thumbnails aren't indexed. Code that changes which
picture an album shows calls `thumbs::forget` (one album) or
`thumbs::forget_all`, next to `art.remove`/`art.clear`.

**Webview security** (H1, H3). The webview runs under a strict CSP
(`tauri.conf.json`): no remote scripts, styles, images or connections, and
images only from the app, `anomp-art:`, `data:` and `blob:`. The URL opener
allows `https` only: open a web link with `openLink`/`openWebLink`
(`lib/openLink.ts`), and pass any URL from service data through `webLink`
(`lib/links.ts`) before it becomes an `href`. Each window gets only the commands
it needs: `build.rs` writes the main window's permission set from `lib.rs`'s
`generate_handler!`, so a new command needs no step for the main window; add it
to `permissions/mini-window.toml` if the mini player calls it.

**Metadata services** (`app/src-tauri/src/metadata/`) go through `http::Client`,
which rate-limits per host and backs off when offline; never call a service
another way. Anything a command needs from a service runs on the metadata
worker through `worker::call`. Tests never touch the network: they use the fake
`Transport` and `Clock` (`http::testing`) with recorded responses in
`metadata/fixtures/`, and live checks are `#[ignore]`d (`cargo test live_ --
--ignored`). A link row with `chosen_by = 'user'` or an `album_art` row is the
user's pick, and automatic matching must never replace it. The `anomp-art`
handler serves only local and already-downloaded pictures; it never goes
online. Wikipedia text is CC BY-SA: always credit the article and licence where
it's shown. The sources table in `docs/design/phase-4-online-metadata.md` records which
sources ship.

**Discogs' terms shape its code**: store only the match (`album_links` with
`details` NULL; `SourceInfo::stores_details` is false), fetch its data through
`Client::get_json_fresh` (memory only, at most `discogs::MAX_AGE`, no offline
copy), never fetch or show its images, and show `discogs::CREDIT` linked to the
release page next to its data. Keys (the Discogs token) live in the OS keychain
through `metadata::keys`, never in the settings JSON, which records only
`hasKey`; tests use its in-memory store.

## Architecture

Three layers, described in full in `PLAN.md` §1:

1. `core/` — `anomp_core`, a C++20 JUCE static library: decoding, playback, queue and
   gapless, tag reading, FFT/levels for the visualizer, OS media integration.
2. `app/src-tauri` — Rust: SQLite library DB, settings, online metadata,
   file scanning; bridges UI to core.
3. `app/src` — Svelte 5 + TypeScript frontend (SvelteKit with `adapter-static`).

Constraints that shape the code and must not be broken casually:

- **The core is statically linked into the Tauri process, never a sidecar.** iOS forbids
  spawning helper processes. Rust's `build.rs` builds it with the `cmake` crate.
- **`core/include/anomp/anomp.h` is the only public surface.** Plain C only: no C++
  types and no exceptions across the boundary. Everything in `core/src` is private
  (the C API translates, e.g. `bool` → `int`, null pointers → a safe default). Async
  events travel through registered C callbacks that Rust forwards as Tauri
  events/channels.
- **Rust owns all non-audio services** (DB, HTTP, settings), keeping the core small
  and testable.
- **FFmpeg decodes every format on every platform** (`PLAN.md` §4.3), wrapped as a
  single JUCE `AudioFormat` (`core/src/FFmpegAudioFormat.*`); only that `.cpp`
  includes FFmpeg headers. Read `docs/design/phase-1-playback-engine.md` (why
  rewinding reopens the demuxer, the seek margins, the length rule) before
  changing the reader.
- **Keep the core platform-neutral.** Platform code (media controls, file access, audio
  session) lives behind small interfaces with one implementation per OS; no AppKit or
  CoreAudio calls elsewhere. UTF-8 across the C API, no assumed `/` separators or
  case-sensitive paths, and folder access stored as security-scoped bookmarks rather
  than raw paths.

## Conventions

- C++ code follows JUCE style: `anomp` namespace, `juce::` qualified everywhere, a
  space before the argument list in declarations and calls (`canDecodeExtension (ext)`),
  Allman braces. `juce_recommended_warning_flags` is on — keep it warning-clean.
- New core source files must be added to the `add_library` list in `core/CMakeLists.txt`
  (and tests to `core/tests/CMakeLists.txt`); there is no globbing.
- The version is CMake's `project(VERSION)` (PLAN.md §8.2): `anomp_version()` is
  compiled from it, and the copies the tools need (`Cargo.toml`, `Cargo.lock`,
  `tauri.conf.json`, `package.json`, `package-lock.json`) are set together by
  `scripts/version.py X.Y.Z` and checked by `version.py --check`. Never edit one by
  hand, and don't add another copy.
