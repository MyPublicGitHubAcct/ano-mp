# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

ano-mp is a music player for desktop (macOS, later Linux/Windows) and iPhone/iPad. It
plays MP3, FLAC and other common formats, shows file metadata enriched from services
such as MusicBrainz, and has an admin screen for configuring displayed fields, enabled
services, library sort/grouping rules and visualization preferences.

`PLAN.md` is the authoritative roadmap: phased plan, §4 decisions (JUCE commercial
license, FFmpeg, TagLib, Svelte 5, minimum OS targets), risks and release gates. Read it before starting
anything non-trivial, and update it when a phase completes or a decision is made.
A finished phase's design notes, steps as built and known limits are in
`docs/design/phase-<n>-<name>.md` (PLAN.md H20), linked from its heading in
`PLAN.md`, which keeps its status, decisions and open steps. Move a phase's
design there, word for word, when it is finished.
Current state: Phases 0–3 are complete: the player UI (queue, browser,
search, cover art, now-playing bar) and OS media integration (macOS Now
Playing and media keys: `MediaControls` in the core, hosted by
`app/src-tauri/src/media.rs`, which keeps it in step with the queue). The C++ core
plays any supported file with gapless hand-off to a pre-opened next track;
the Rust queue (`app/src-tauri/src/queue/`) keeps it armed across the whole
queue. The Svelte UI is in `app/src/lib/` (`api.ts` wraps the commands; their
payload types and wrappers are generated into `generated/`) and
`app/src/routes/`; the old dev panels are at `/dev`, in debug builds only.
The core reads tags and art (`anomp_read_tags`, TagLib), the Rust library
(`app/src-tauri/src/library/`: SQLite DB and incremental folder scanner) fills
from it, and `library_browse` pages through it under configurable sort/grouping
rules (stored in `settings`). Each folder keeps a security-scoped bookmark
(`library/access.rs`, over the core's `FolderAccess`), and the bundled app is
sandboxed (`Entitlements.plist`). Phase 4 (online metadata) is in progress:
`app/src-tauri/src/metadata/` has the source settings (which sources supply
each kind of data, in what order), the rate-limited HTTP client, folder-image
art, MusicBrainz matching, Cover Art Archive covers downloaded into an
on-disk image cache (`metadata/images.rs`), and the metadata worker: a
thread (`metadata/worker.rs`) running `metadata/jobs.rs`, which matches
albums and artists and fetches covers and biographies by priority, pauses
while a service is unreachable, and emits `metadata-changed` and
`metadata-progress`. Artist pages (`library/artists.rs`,
`ArtistPage.svelte`) show MusicBrainz facts (`metadata/artists.rs`) and a
Wikipedia biography (`metadata/wikipedia.rs`, CC BY-SA: always credit the
article and licence where it's shown); the album header shows a Wikipedia
description of the album the same way, found through its MusicBrainz
release group. An artist page links to the artist's releases the library
lacks (`metadata/discography.rs`, `DiscographyPage.svelte`, from
MusicBrainz's release-group browse). The album details
(`library/albums.rs`), the "Find details", "Choose cover" and "Find artist"
dialogs (commands in `metadata/commands.rs`) and the Online sources panel
(`ServicesPanel.svelte`) are done; anything a command needs from a service
runs on the worker through `worker::call`. 4.8 settled which sources ship
(the sources table in `PLAN.md` Phase 4 has each decision): Discogs is an
opt-in second album-details source (`metadata/discogs.rs`) behind
`albums::ReleaseSource`, which MusicBrainz implements too. What remains of
Phase 4 is checking its exit in the app.
Phase 5 (visualization) is built: `PlayerEngine` writes what it plays to a
`SignalTap`, `AnalysisThread` runs `SpectrumAnalyser` over it while a
callback is set (`anomp_engine_set_analysis_callback`, called on that
thread, never the main one), `app/src-tauri/src/visualizer.rs` streams
binary frames over a Tauri `Channel` while the UI subscribes, and the
renderers in `app/src/lib/visualizer/` draw them (the cover wall's albums
come from `library/covers.rs`). What remains is checking them by eye with
music playing.
Phase 6 (settings) is built: `settings.rs` holds the typed `AppSettings`
(display, playback/ReplayGain, output device, visualizer) under `app` in
`settings`, applied by `settings_save`; the UI is `SettingsPage.svelte` with
its sections in `components/settings/`, and reads the settings through
`state/settings.svelte.ts` (loaded in `routes/+layout.ts` before any page
renders). What remains is checking device switching and ReplayGain by ear.
Phase 6b (the optional features O1–O19, `PLAN.md` §4.6) is built: each
has a switch in Settings › Features (`FeatureSettings` in `settings.rs`,
`FeaturesOptions.svelte`), and code that honours it. The core analyses
files (`FileAnalyser`, `anomp_analyse_file`: loudness, peaks, silences,
spectrum cutoff, waveform), plays parts of files with an optional skip
region (`anomp_track_options`), loops A–B, stretches tempo and pitch
(Signalsmith Stretch, `cmake/Signalsmith.cmake`), crossfeeds
(`Crossfeed`) and reports the signal path. Rust has the analysis worker
(`library/analysis.rs`), how each track is played (`library/playback.rs`:
parts, trims, gain offsets, computed ReplayGain, silence skips), cue
sheets (`library/cue.rs`), discovery, health, lyrics and preferences
(`library/{discover,health,lyrics,prefs}.rs`), the listening history and
ListenBrainz (`history/`), library radio (`queue/radio.rs`) and the LAN
remote (`remote/`). The UI adds the Home, History and Library health
views. What remains is checking them in the app (Phase 6b's exit).
The expected features F1–F21 (`PLAN.md` §4.7, Phase 6c) are built:
playlists and smart playlists (`library/playlists.rs`, `library/smart.rs`),
favourites and ratings (`library/marks.rs`), moved files keeping their ids
(the scanner), credited artists and compilations, substring search
(migrations 007–009), folder watching and rescans at launch
(`library/watch.rs`), Get Info (`library/info.rs`), files opened from outside
the library (`library/external.rs`), data export and import
(`library/transfer.rs`), crossfade and the `Equaliser` in the core, and the
app outside its page in `app/src-tauri/src/shell/` (menus, the Dock menu,
the menu-bar item, the mini player, opened files, notifications). What
remains is checking them in the app (Phase 6c's exit).
Phase 7 (hardening) follows the "Order of work" in `PLAN.md`: Steps 1–3
are done (CI baseline, quick security fixes, and H10, H22 and H9: the
library DB's copies and launch check, missing folders, and logs). Step 4,
the exit checks in a sandboxed bundle, has its checklist in
`docs/step4-checklist.md`: the probe checks are done, the owner's (by
hand, by ear, by eye) wait. Step 5 is done: H5 (fuzzing, `core/fuzz/`),
H11 (files opened off the main thread) and H12 (cloud placeholders); its
owner's checks are § 5 of the checklist. Step 7, the release setup that
needs no certificate (§8.2 and part of §8.3), is done: one version number,
the third-party notices, the release workflow (unsigned until the
Developer ID secrets exist), and the macOS build, bundle and notarization
scripts. Step 6, the owner's decisions (§8.1), has its briefs in
`docs/release-decisions.md`; the signed release waits on them.
Step 8, the signed release, has begun with what needs no decision: the
clean-Mac smoke test (`docs/release-smoke-test.md`) and H14, the
sandboxed bundle's self-test (`self_test.rs`, `self-test-bundle.py`);
then Part 5: CI fixed (downloads through `cmake/Fetch.cmake`, the fuzz
build on CI's newer Xcode), H18's budgets (`bench.py`), H20 (finished
phases' design in `docs/design/`, `check-docs.py`), H15 (generated
command bindings), H16 (the queue as rows and numbered edits) and H17
(cover thumbnails).
`docs/` holds the Step 4 checklist, the release decisions' briefs, the
release smoke test and, in `docs/design/`, finished phases' design notes.

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

The fuzz targets (PLAN.md H5) need Homebrew's `llvm@22` (`brew install
llvm@22`): Apple's clang has no libFuzzer runtime, and LLVM 21's ASan hangs
at start-up on macOS 26. The `fuzz` preset (build/fuzz) compiles everything
with it under ASan and UBSan and links FFmpeg's instrumented static build
(`scripts/build-ffmpeg.sh --fuzz`, which `run-fuzzers.py` runs).
Xcode 16's ld, the macos-15 runners' default, can't read clang 22's
objects ("invalid r_symbolnum"), and Homebrew's lld breaks C++ exceptions
in the fuzz build, so CI sets `ANOMP_FUZZ_DEVELOPER_DIR` to the runner's
Xcode 26.3, which `run-fuzzers.py` passes to its own builds as
`DEVELOPER_DIR`; every other build keeps the default Xcode. A crash
found becomes a regression test with a committed fixture in
`core/tests/fixtures/fuzz/` ("Inputs the fuzzer found stay harmless" in
`TagReaderTests.cpp`, which the asan preset runs with UBSan fatal); a new
target goes in `core/fuzz/CMakeLists.txt` and `run-fuzzers.py`'s `TARGETS`.
TagLib is built with only the formats FFmpeg plays (`cmake/TagLib.cmake`'s
`WITH_*`): turn one on only with its FFmpeg demuxer.
`.github/workflows/fuzz.yml` runs each target for 30 min weekly.

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

`scripts/bench.py` (PLAN.md H18, §9.2 M4) runs the ignored Rust
benchmarks (`library/bench.rs`, `cargo test --release`) and the core's
hidden `[.][bench]` Catch2 tests (`core/tests/BenchTests.cpp`, built by
the Release `bench` preset), reads their `bench <key> <value> <unit>`
lines, and checks each against `scripts/bench-baseline.json`: its
budgets on any machine, and its results (25% margin) only on the machine
it records. A new benchmark prints such a line, and a budget for it goes
in the JSON; `--update` rewrites the results and machine, never the
budgets. Run it after scanner, browse, search or DB changes. Launch
time, memory and whole-app CPU are the owner's checks (§ 6 of
`docs/release-smoke-test.md`).

The bundle self-test (PLAN.md H14) is `src-tauri/src/self_test.rs`,
compiled in only with the `self-test` Cargo feature: `ano-mp --self-test`
runs instead of the app, with no window and none of the app's files. It
writes the core's fixtures (embedded; `every_audio_fixture_is_embedded`
fails when one is added to `core/tests/fixtures/` but not to its list)
into the temporary folder, then scans, resolves the bookmark, reads covers,
decodes and plays a gapless hand-off at volume 0 (skipped without an output
device), one line per stage. `check-all.py` runs `self-test-bundle.py`,
which builds into `target/self-test` and does nothing outside CI unless
given `--local`. `cargo run --features self-test -- --self-test` runs it
unsandboxed. A new sandbox-only behaviour gets a stage there.

`THIRD_PARTY_NOTICES` (repo root) is generated and committed: regenerate it
whenever `Cargo.lock`, `package-lock.json`, a native pin or `BUILD_INFO`
changes, or `check-all` fails (Dependabot's updates included). It lists
the crates the app links (normal dependencies, both macOS targets), the
npm packages in the built frontend (the list `app/vite.config.js`'s
`bundledPackages` plugin writes), JUCE and what it vendors into the
modules the core links (from `JUCE.spdx.json`: a new vendored library
fails the script until it is listed), FFmpeg, TagLib, Signalsmith and
Catch2. A licence outside `deny.toml`'s `allow` list fails it; a package
without a licence text gets a standard one from `scripts/licenses/`. The
bundle carries it (`bundle.resources`) and Settings › About shows it
(`diagnostics_notices`).

The release workflow (`.github/workflows/release.yml`, on a `v*` tag, or
by hand for a trial) calls `ci.yml`, then runs `build-app.py`,
`notarize.py` and `release.py`, and attaches dist/ to a draft GitHub
Release. Checks go in the scripts, never in the workflow. Steps that
need the Developer ID secrets are named "[signed releases]" and skipped
without them, and the run warns that the build is unsigned. Release notes
are the version's section of `CHANGELOG.md` (Keep a Changelog headings,
`## [X.Y.Z] - date`): add to `## [Unreleased]` as you go, rename it when
tagging. The hardened runtime is on exactly when a signing identity is
given; `tauri.conf.json` keeps the ad-hoc local settings, and the bundle
identifier is the owner's (`docs/release-decisions.md`).

Tools beyond the build: `brew install uv gitleaks llvm@22`, and `cargo install
cargo-deny --version 0.20.2 --locked` (the version CI installs). Rust and
Node are pinned by `rust-toolchain.toml` and `.nvmrc`; `app/.npmrc` makes
npm refuse a Node outside `package.json`'s `engines`.

CI (`.github/workflows/ci.yml`, macOS) runs `scripts/check-all.py` and nothing
else, so a new check goes in that script, not in the workflow. It runs by
hand (Actions › CI › Run workflow) and through `release.yml` on a version
tag, not on pushes or pull requests (README.md, "When GitHub Actions run"),
so run `check-all.py` locally before pushing, and CI by hand on a branch
whose result matters (Dependabot's). The repo
checks it runs are read-only scripts: `check-c-api.py` (each `anomp.h`
function declared in `anomp.rs` with the same parameter count, apart from its
`NOT_BOUND` list), `check-sources.py` (the core's CMake source lists) and
`check-migrations.py`, and `version.py --check` (every copy of the version
agrees); the full run adds `make-notices.py --check` (after the core's
configure and the frontend build). Scripts use the standard library only; their tests are
in `scripts/tests/` (pytest, run by `scripts/test-python.py`), each repo check
with one test against the real tree. The full run also runs clippy, `cargo
deny`, ESLint and the sanitizer presets; `--quick` is what the pre-commit
hook runs. Actions in the workflow are pinned by commit SHA (look a new one
up with `git ls-remote`, never guess it), and Dependabot
(`.github/dependabot.yml`) proposes updates monthly.

The format scripts are Python so they run on every platform (on Windows, run them
with `py`). The C++ and Python ones run a pinned formatter through `uvx` (`brew
install uv`); `.clang-format` is JUCE style, `core/include/.clang-format` keeps
`anomp.h` in C style. `format-frontend.py` runs the Prettier pinned in
`app/package.json` (`app/.prettierrc.json`); reformat in a commit of its own.

FFmpeg is built by `scripts/build-ffmpeg.sh` (pinned version, LGPL, audio-only,
shared) into `third_party/ffmpeg/<platform>/` (git-ignored). CMake refuses to
configure without it. `cmake/FFmpeg.cmake` exposes it as `FFmpeg::avformat`,
`FFmpeg::avcodec`, `FFmpeg::swresample`, `FFmpeg::avutil`. To add a format, add its
demuxer/decoder/parser to the script's lists (the configure flags stay minimal on
purpose) and extend `core/tests/FFmpegBuildTests.cpp`.

Decoder tests use committed fixtures in `core/tests/fixtures/`, all encoding one
deterministic chirp that the tests regenerate (`core/tests/TestSignal.h`, which must
match `scripts/make-test-fixtures.py`). Regenerating needs `brew install ffmpeg
vorbis-tools` — Homebrew's FFmpeg is only a fixture encoder, never linked. Lengths and
lags in the tests' fixture table are properties of the encoded files, so update them if
you regenerate. The script rewrites every fixture, but only the Vorbis ones change
(oggenc picks random stream serials): `git checkout` them unless you meant to change
them. The two `tagged-*` fixtures carry the tags `TagReaderTests.cpp` expects.

JUCE 9.0.2 and Catch2 v3.16.0 are pinned by commit in the top-level `CMakeLists.txt`
(GitHub's archive of the release's commit + SHA-256), TagLib
2.3.2 (tarball + SHA-256) in `cmake/TagLib.cmake`, Signalsmith Stretch 1.4.0 and its
FFT library (MIT, header-only, tarballs + SHA-256) in `cmake/Signalsmith.cmake`, all
declared with `anomp_fetch_declare` (`cmake/Fetch.cmake`): each tarball is
downloaded once, with retries, into `build/_downloads/<sha256>-<name>` and
checked, then FetchContent extracts it into `build/<preset>/_deps` (Cargo's
build shares the downloads too) — the first configure takes several minutes.
Declare a new pinned tarball the same way, never with `FetchContent_Declare`
(`test_fetch_cmake.py` checks). TagLib is a
separate static lib (`libtag.a`), so `build.rs` links it next to `anomp_core`.
The `release` preset sets `ANOMP_BUILD_TESTS=OFF`, so tests only run in `debug`.
Tauri's crates and plugins are pinned exactly in `app/src-tauri/Cargo.toml`, and
their npm packages exactly in `app/package.json`; `tauri build` refuses mismatched
major.minor versions, so update each crate and its npm package together.

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
into Cargo's `target/` dir, separate from `build/<preset>`, so the first Cargo build
fetches JUCE again. `build.rs` reruns when `core/`, `cmake/` or the top-level `CMakeLists.txt`
changes. FFI declarations and their safe wrappers live only in
`app/src-tauri/src/anomp.rs`; add a wrapper there for each new C API function.

The library DB schema changes only by appending a numbered SQL file to
`app/src-tauri/src/library/migrations/` and listing it in `MIGRATIONS`
(`library/db.rs`); never edit a migration that has shipped. Tracks store paths
relative to their folder, '/'-separated (`library::track_path` joins them).
`db::configure` registers SQL functions (`anomp_sort_key`, `anomp_genres`,
`anomp_has_genre`) on every connection. Open connections only through `db`, and
never use these functions in the schema, an index or a migration, since other
SQLite clients don't have them. Browse queries (`library/browse.rs`) are built
from fixed SQL fragments; bind every value, never format it in.

Before applying migrations to an existing database, `db::open` writes a copy,
`library.sqlite3.pre-<n>` (n: the first migration applied), next to it and
keeps the newest two (PLAN.md H10); a copy that can't be written stops the
migration. A launch check (`library/recovery.rs`, `PRAGMA quick_check`)
offers to restore that copy or to rebuild; the choice is carried out at the
next launch, before anything opens the database, and the damaged file is
kept as `library.sqlite3.damaged-<secs>`. Test a migration against a file
database too (`db` tests' `file_at`), so the copy is exercised.

Under the macOS sandbox a library file can be opened only while its folder's
bookmark is resolved, so anything that opens library files goes through
`library::access::open_folder`/`open_folder_of` first and holds the result
until the file is open (as `scan_folder`, `player_load` and `player_set_next`
do). `tauri dev` runs unsandboxed, so a missing call only fails in a
sandboxed bundle. A security-scoped bookmark resolves only in the build
that made it while the app is ad-hoc signed (the identity changes each
build; the error is "isn't in the correct format"): `open_folder` replaces
it when the folder is still readable, as in dev, but a rebuilt sandboxed
bundle needs its folders picked again (`add_folder` on a library folder
replaces its bookmark) (`npm run tauri build -- --bundles app`: ad-hoc signed,
hardened runtime off, FFmpeg embedded in `Contents/Frameworks`). The
`bundle.macOS.frameworks` list in `tauri.conf.json` names FFmpeg's major
versions, so update it when the FFmpeg pin changes. Only debug builds have an
rpath into `third_party/`; `cargo test --release` gets it from
`app/src-tauri/.cargo/config.toml`.

Running a bundle check safely. The bundle is sandboxed, so it uses the
real container (`~/Library/Containers/dev.anomp.player`), which holds the
owner's library, settings and bookmarks: ask before running one. macOS
won't let another process move the container or copy all of it, so with
the app quit, copy its `Data/Library` somewhere safe, then move the app's
own files out of it (`Application Support/dev.anomp.player`,
`Preferences/dev.anomp.player.plist`, `Caches/dev.anomp.player`,
`Caches/WebKit`, `WebKit`, `Logs/dev.anomp.player`) and `killall
cfprefsd`. Afterwards delete the scratch files, move the real ones back
and diff them against the copy. Never change the bundle identifier to
avoid this (an owner decision, PLAN.md §8.1). Run the executable directly
(`…/ano-mp.app/Contents/MacOS/ano-mp`): it is still sandboxed and gets
your environment, so a temporary probe started from `setup` can read its
stage from a variable. The sandbox can always read its own container, so
a scratch library written there (the fixtures, embedded with
`include_bytes!`) needs no open panel; folders elsewhere need the owner.
The log is `Data/Library/Logs/dev.anomp.player/ano-mp.log` in the
container; a crash leaves a report in `~/Library/Logs/DiagnosticReports`
(build with `CARGO_PROFILE_RELEASE_STRIP=none
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` to get symbols in it).
Remove every probe afterwards.

A folder that can't be read is never treated as empty (PLAN.md H22): its
state (`access::FolderState`, kept by `library::availability`) says why, and
its tracks stay. A scan that would remove all of a folder's tracks, or most
of a large one, keeps them and fails the folder (`scanner::holds`); only
`library_remove_missing` removes them. Code that opens files must treat a
`folderUnavailable` error (`FolderState::of_error`) as "not now", never as a
broken file: don't store it as a failure. Lists show an unreadable
folder's tracks dimmed (each track carries `folderId`; `lib/folders.ts`
works out which folders from `library.folders`), and anything that picks
tracks to play (radio, smart playlists' play, Home's suggestions) leaves
them out with `availability::unreadable` (not `mostlyGone` folders),
binding the ids through `json_each`. Under the sandbox a deleted folder's
bookmark fails with "isn't in the correct format", as another build's
does: `access::unresolved` goes by the stored path first. `open_folder` doesn't follow a
folder into the Trash. Folder-state tests use `access::testing::FakeBookmarks`
over temp dirs, not real bookmarks.

`PlayerEngine` (`core/src/PlayerEngine.*`) is a plain `juce::AudioSource` with no
device; `AudioEngine` owns the device and feeds it. Tests render it offline by calling
`getNextAudioBlock` directly (`core/tests/PlayerEngineTests.cpp`);
`docs/design/phase-1-playback-engine.md` records its design (why not
`AudioTransportSource`, the host-owned queue, threading).

**The engine is main-thread only.** JUCE's message loop rides on the main run loop
that Tauri runs, so `anomp_engine_*` calls must happen on the main thread and event
callbacks arrive there. `app/src-tauri/src/audio.rs` enforces this: the engine lives
in a main-thread `thread_local`, commands go through `with_engine` (which hops to
the main thread if needed), and the engine is dropped on `RunEvent::Exit`. The queue
lives on the main thread too (`queue::run`); do database work before hopping there,
and never hold the library connection while waiting for the main thread. Queue logic
goes in `queue/model.rs` behind the `Player` trait, tested against a fake engine.
The media controls (`anomp_media_controls_*`) are main-thread only too, and
`media.rs` decides what to publish in `NowPlaying`, behind a `Publisher` trait.
`run_on_main_thread` called on the main thread runs the closure at once; it
doesn't defer it.

The queue opens files off the main thread (PLAN.md H11): `Player::load`
and `set_next` may return `Opening::Pending`, and the host reports the
outcome with `Queue::load_finished` (`queue/opening.rs`: the row and the
bookmark on a blocking thread, then `Engine::load_track_async` on the
main thread, holding the folder open until the engine's
`Event::LoadFinished`). The core opens and prefills the track on a thread
of its own and hands it over in `dispatchEvents`; every request is
reported once, in request order; a later load supersedes earlier
requests. Never open a library file synchronously on the main thread for
playback; the synchronous `load_track` stays for the core's tests and the
dev page. A request still opening after `LOAD_TIMEOUT` fails with
`openTimedOut` (a cloud placeholder gets `DOWNLOAD_TIMEOUT`). Engine event callbacks may call the engine (the queue arms
the next track from `TrackEnded`); `anomp.h` states the rule.

The settings' TypeScript types (`app/src/lib/generated/settings.ts`) are
generated from the Rust types by ts-rs (a dev-dependency: derive it with
`#[cfg_attr(test, derive(ts_rs::TS))]` and list the type in
`settings::bindings`); `cargo test` fails while the file is stale, so
regenerate and commit it after changing them. Never edit it by hand.
The commands' side is generated the same way (PLAN.md H15,
`src-tauri/src/bindings.rs`): `generated/ipc.ts` holds every payload and
event type (derive `TS` on a new one and add it, or a type containing
it, to `declare_types`; a name the frontend already uses differently
gets `#[cfg_attr(test, ts(rename = "…"))]`), and `generated/commands.ts`
a typed wrapper per command in `generate_handler!`, read from its
signature with syn. `api.ts` calls `commands.*`, never `invoke` with a
string, so a renamed command, argument or field fails `npm run check`.
`ANOMP_WRITE_BINDINGS=1 cargo test bindings` rewrites all three files. A new
setting goes in `AppSettings` with a default and a `validate` rule; stored
values are read leniently, so no migration is needed. Track gains
(ReplayGain) are computed in Rust (`PlaybackSettings::gain`) and passed to
the engine with each track.

Cloud placeholders (PLAN.md H12): with "Optimize Mac Storage" a file can
be dataless, downloaded by any read. Never read a library file's tags or
audio in the background (scans, workers, art) without checking
`anomp::file_is_dataless` first (`stat()`, which doesn't download): the
scanner records a placeholder unread and marks it `tracks.dataless`, the
analysis worker marks and leaves one, and every scan checks marked ones
again. Reads the user asks for (playing, Get Info) may download.

A track is a part of a file: `tracks` is unique on (folder, path,
`range_start`), with `range_end` NULL for the end of the file (migration
005), so a cue sheet's tracks or a file's chapters are rows of one file.
Anything that opens a track for playback goes through
`library::playback::track_play`, which turns the row, the user's
preferences, the analysis and the feature settings into the engine's
`TrackOptions`; refresh a gain with `set_track_gain_at` (by file and
start), never by file alone. Each optional feature checks its switch in
`FeatureSettings` where it acts (commands refuse, workers idle, the UI
hides); a new one gets a switch there, off by default if it costs a lot,
changes what is heard, goes online or listens on the network. The LAN
remote answers local addresses only and keeps only hashes of tokens; any
change to `remote/` needs the security review in `PLAN.md` §8.1.

The saved queue is rows of `queue_items` (PLAN.md H16), kept in step by
`queue/store.rs` applying the same `model::Edit`s the frontend gets; the
rest of it (current index, position, repeat, `shuffled`, volume) is the
`player.queue` setting. A model change to the list must log an edit
(`edited`) or a reset (`reset_list`), or neither the UI nor the rows
follow it. The UI applies edits by `listVersion` (`lib/queueEdits.ts`) and
asks for the whole state when it misses one.

Search uses FTS5 tables kept in step by triggers: word indexes (recreated by
migration 008 over `IFNULL(artist_credit, …)`) and trigram indexes (009). A
schema change to `tracks`, `artists` or `albums` columns they index must update
both sets of triggers in a new migration. Run the search benchmark
(`library/bench.rs`) after changing the search SQL.

Log through the `log` macros, never `eprintln!` (PLAN.md H9; `logging.rs`).
The file is `ano-mp.log` in the app's log directory (`~/Library/Logs/
dev.anomp.player`, inside the container when sandboxed); release builds
write info and above, debug builds debug too and also stderr. The target is
the module; don't prefix messages with it. At info and above, write ids and
counts only: titles, artists, file names and paths go at debug (the
formatter scrubs absolute paths and URL paths at info and above anyway, and
redacts keys, `Authorization` values and `token=`-like parameters at every
level). A new key or token the app holds goes through `metadata::keys`, which
registers it with `logging::keep_secret`. The core logs through
`anomp_set_log_callback` (`core/src/Log.h`: `anomp::log::warn` and so on);
JUCE's `Logger` and failed assertions (`JUCE_LOG_ASSERTIONS=1`) arrive there.
Anything in the core that outlives JUCE's runtime at exit (a static, a
base class of `AudioEngine`) must let go of JUCE first: a failed assertion
during shutdown is logged, and logging into a destroyed object aborts the
quit.
"Copy diagnostics" (`diagnostics.rs`) holds no paths or titles; adding to
it, or to what logs may contain, is an owner decision.

Every string the UI shows comes from `app/src/lib/i18n/en.json` through `t`
(typed keys; a plural message is an object of `Intl.PluralRules` forms), never
a literal in a component. Rust errors the UI shows are made with
`crate::coded` (`{code, params, message}`), each code with an `error.<code>`
message in `en.json` (`tests/i18n.test.mjs` checks), and the UI shows a
command's error through `errorText`, never `String(error)`. A coded error with
a `reason` param may have `error.<code>.<reason>` messages, which `errorText`
prefers (`folderUnavailable`); `errorCode` reads a code to branch on.

The /dev page and its commands (`src/dev.rs`) exist in debug builds only
(`PLAN.md` H2): the commands are registered under `#[cfg(debug_assertions)]`
in `generate_handler!`, and the page is loaded only when the Vite constant
`__DEV_TOOLS__` is true (`vite dev`, or a debug build from the Tauri CLI).
Put anything that bypasses the queue or takes a raw path there.

Covers go to the UI as thumbnails (PLAN.md H17, `library/thumbs.rs`):
`artUrl(…, "list" | "header")`, or `"full"` only where a picture is shown
full size. Thumbnails live in the image cache, named by the SHA-256 of
their picture, and `art_thumbs` remembers which picture each album shows.
A new album-art source sets `Art::origin`, or its thumbnails aren't
indexed. Code that changes which picture an album shows calls
`thumbs::forget` (one album) or `thumbs::forget_all`, next to
`art.remove`/`art.clear`.

The webview runs under a strict CSP (`tauri.conf.json`; what it allows and
why is in `PLAN.md` H1): no remote scripts, styles, images or connections,
and images only from the app, `anomp-art:`, `data:` and `blob:`. The URL
opener allows `https` only: open a web link with `openLink`/`openWebLink`
(`lib/openLink.ts`), and pass any URL from service data through `webLink`
(`lib/links.ts`) before it becomes an `href`.

Each window gets only the commands it needs: `build.rs` reads the handlers in
`lib.rs`'s `generate_handler!` and writes the main window's permission set, so
a new command needs no step for the main window; add it to
`permissions/mini-window.toml` if the mini player calls it.

Metadata services (`app/src-tauri/src/metadata/`) go through `http::Client`,
which rate-limits per host and backs off when offline; never call a service
another way. Tests never touch the network: they use the fake `Transport` and
`Clock` (`http::testing`) with recorded responses in `metadata/fixtures/`, and
live checks are `#[ignore]`d (`cargo test live_ -- --ignored`). A link row
with `chosen_by = 'user'` or an `album_art` row is the user's pick, and
automatic matching must never replace it. The `anomp-art` handler serves
only local and already-downloaded pictures; it never goes online.

Discogs' terms shape its code: store only the match (`album_links` with
`details` NULL; `SourceInfo::stores_details` is false), fetch its data
through `Client::get_json_fresh` (memory only, at most `discogs::MAX_AGE`,
no offline copy), never fetch or show its images, and show
`discogs::CREDIT` linked to the release page next to its data. Keys
(the Discogs token) live in the OS keychain through `metadata::keys`,
never in the settings JSON, which records only `hasKey`; tests use its
in-memory store.

## Architecture

Three layers, described in full in `PLAN.md` §1:

1. `core/` — `anomp_core`, a C++20 JUCE static library: decoding, playback, queue and
   gapless, tag reading, FFT/levels for the visualizer, OS media integration.
2. `app/src-tauri` — Rust: SQLite library DB, settings,
   MusicBrainz/Cover Art Archive clients, file scanning; bridges UI to core.
3. `app/src` — Svelte 5 + TypeScript frontend (SvelteKit with `adapter-static`, from
   the Tauri template).

Constraints that shape the code and must not be broken casually:

- **The core is statically linked into the Tauri process, never a sidecar.** iOS forbids
  spawning helper processes, so an in-process static library is the only design that
  works on all targets. Rust's `build.rs` builds it with the `cmake` crate.
- **`core/include/anomp/anomp.h` is the only public surface.** Plain C only: no C++
  types and no exceptions across the boundary. Everything in `core/src` is private
  (the C API translates, e.g. `bool` → `int`, null pointers → a safe default). Async
  events (position, track-ended, spectrum frames) will travel through registered C
  callbacks that Rust forwards as Tauri events/channels.
- **Rust owns all non-audio services** (DB, HTTP, settings), keeping the core small
  and testable.
- **FFmpeg will decode every format on every platform** (decided; see `PLAN.md` §4.3),
  wrapped as a single JUCE `AudioFormat` (`core/src/FFmpegAudioFormat.*`) so nothing
  else in the core knows FFmpeg exists; only that `.cpp` includes FFmpeg headers.
  `docs/design/phase-1-playback-engine.md` records its design and the container
  quirks behind it (why rewinding reopens the demuxer, the seek margins, the
  length rule) — read that before changing the reader.
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
  compiled from it (`ANOMP_VERSION`, `core/CMakeLists.txt`), and the copies the tools
  need (`Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, `package.json`,
  `package-lock.json`) are set together by `scripts/version.py X.Y.Z` and checked by
  `version.py --check`. Never edit one by hand, and don't add another copy.
