# 10. Troubleshooting

Where to look when something goes wrong: the logs, the diagnostics, the
developer page, running one test, a fuzzer's crash, the sandboxed
bundle, and a table from symptom to code. For what a user can try
themselves, the [user guide's troubleshooting chapter](../user-guide/14-troubleshooting.md)
is the place.

## The logs

Every layer writes to one log file, `ano-mp.log`:

- unsandboxed (`npm run tauri dev`):
  `~/Library/Logs/dev.anomp.player/ano-mp.log`, and every line is also
  printed in the terminal;
- a bundle: `~/Library/Containers/dev.anomp.player/Data/Library/Logs/dev.anomp.player/ano-mp.log`;
- Settings › About › **Show logs** opens the folder in the Finder.

A line reads `2026-10-02T09:15:42.123Z WARN library::scanner: message`:
time (UTC), level, **target**, message. The target says where it came
from: the Rust module path (`queue`, `library::scanner`,
`metadata::jobs`), `core` for the C++ core (including JUCE's messages and
failed assertions), `webview` for the page's uncaught errors, and
`panic` for a panic's message and backtrace.

**Levels.** Release builds write info and above; debug builds also write
debug. There is no switch at run time: to see debug lines (titles, file
names, paths), run a debug build (`npm run tauri dev`). At info and
above, absolute paths become `<path>` and URLs keep only their host, and
at every level keys and tokens are redacted (`logging.rs`), so a log
attached to a bug report holds ids and counts, not the user's music.

The file starts again past 2 MB, keeping two older ones (`ano-mp.log`
plus rotated copies). A crash of a bundle also leaves a report in
`~/Library/Logs/DiagnosticReports`.

## "Copy diagnostics"

Settings › About › **Copy diagnostics** (`diagnostics.rs`) copies a
short report for a bug, with no paths, titles or artists:

| Section | What it tells you |
|---|---|
| Versions | the app, the core, Tauri, the webview, the OS, a debug or release build, and whether it runs sandboxed |
| Output | the open device, its sample rate and buffer size, and how many devices there are |
| Library | row counts (folders, tracks, albums, artists, playlists, plays…) |
| Folders | how many folders are in each state (`available`, `missing`, `empty`, `mostlyGone`, `inTrash`, `noPermission`) |
| Features on, Online sources on | the feature switches that are on, and the online sources in use (or "online off") |
| Database | the schema version, the file's size, how many pre-migration copies exist, the launch check's result |
| Log (last lines) | the log's last lines at info and above |

## The developer page

In a debug build, the sidebar ends with **Developer tools**, which opens
`/dev` (`lib/components/dev/DevPage.svelte`, `dev.rs`): the core's
version and the output device, a test tone (is the device working at
all?), and loading any file by path straight into the engine, bypassing
the library and the queue (does the core play this file?). Loading a
file there detaches the queue until it next starts a track. None of this
exists in a release build (H2): the commands are compiled out and the
route redirects to the player.

## Running one test

```sh
# Core (from the repo root): by name, by tag, or the binary directly
ctest --preset debug -R "PlayerEngine state transitions"
./build/debug/core/tests/anomp_core_tests "[player]"
./build/debug/effects/tests/anomp_effects_tests "[chain]"
ctest --preset debug -N        # list every test's name

# Under a sanitizer (after `cmake --workflow --preset asan` once)
./build/asan/core/tests/anomp_core_tests "[player]"

# Rust (from app/src-tauri): a module, or one test, with its output
cargo test queue::model
cargo test a_file_moved_to_another_folder_keeps_its_track -- --nocapture
cargo test live_ -- --ignored  # the live checks against the real services

# Frontend (from app/)
node --test tests/queueEdits.test.mjs

# Scripts (from the repo root)
scripts/test-python.py scripts/tests/test_check_docs.py
```

List the tests with `ctest --preset debug -N` or `cargo test -- --list`.

## Reproducing a fuzzer crash

The fuzz targets (`core/fuzz/`) feed random input to the decoder and the
tag reader. A crash leaves its input in `build/fuzz/crashes/`, named
after its target:

```sh
scripts/run-fuzzers.py --target tags build/fuzz/crashes/tags-crash-…   # rerun that input
scripts/run-fuzzers.py --target decoder --seconds 600                 # fuzz longer
```

Fix the bug, then make the input a regression test: commit it in
`core/tests/fixtures/fuzz/` and add its name to the test that checks
such inputs stay harmless ("Inputs the fuzzer found stay harmless" in
`TagReaderTests.cpp`; a decoder crash gets a test of its own in
`FFmpegAudioFormatTests.cpp` the same way). CI's weekly fuzz
workflow uploads any failing input as an artifact.

## The sandboxed bundle

`tauri dev` runs unsandboxed, so a missing `open_folder` call, a
bookmark problem or an entitlement only shows in a sandboxed bundle.

- **The self-test** (`self_test.rs`, H14) checks what only a sandboxed
  bundle shows: it builds a bundle with the `self-test` feature and runs
  `ano-mp --self-test`, which scans the embedded fixtures in a scratch
  folder, resolves a bookmark, reads covers, decodes, and plays a
  gapless hand-off, then exits with a status. `scripts/self-test-bundle.py`
  runs it; `check-all.py` does so on CI only, because locally it runs in
  the real container (`--local` to do it anyway). `cargo run --features
  self-test -- --self-test` runs the same stages unsandboxed.
- **A full bundle check** uses the real container, which holds the
  owner's library, settings and bookmarks. Ask before running one, and
  follow `docs/bundle-checks.md` to set the real files aside and restore
  them.
- An ad-hoc signed bundle's bookmarks resolve only in the build that
  made them: after a rebuild, its folders show as unavailable ("isn't in
  the correct format") until they are added again. That is expected;
  see the [README](../../README.md#the-app-bundle).

## Symptoms

| Symptom | Where to look | Log lines |
|---|---|---|
| **No sound** | Is a device open? Settings › Playback shows it (`audio_output_status`); the /dev page's test tone checks the device alone. `audio.rs` (`open_output`, `reopen_chosen`), `AudioEngine::openDevice`. Is the volume or a sleep timer's fade at zero (`player.queue`'s volume)? Is the track loaded (`QueueState.loading`)? | `audio`: "device changed", "cannot reopen …", "cannot open the default device either"; `queue`: "skipped track …" |
| **A track won't play, or is skipped** | `queue/opening.rs` (`resolve`: the row, the folder), `library::playback::track_play`, the core's `FFmpegAudioFormat`. Load the file on the /dev page to tell the core from the library. A placeholder downloading gets `DOWNLOAD_TIMEOUT`. | `queue`: "skipped track <id>: <error>", "downloading a cloud placeholder to play it"; `core`: the decoder's error |
| **"Folder not available"**, tracks dimmed | `library/access.rs` (`open_folder`, `FolderState`), `library/availability.rs`. Which state? `missing` (unmounted drive, deleted), `inTrash`, `noPermission` (often another build's bookmark), `empty` or `mostlyGone` (a scan held its tracks: `scanner::holds`). | `library::availability`: "folder <id> is now <state>"; `library::scanner`: "kept the … tracks a scan of folder … didn't find (…)"; `library::watch`: "<n> folders, <m> unavailable" |
| **Covers missing** | `library/art.rs` (`lookup`: the user's choice, then the sources in `metadata.services` order), `library/thumbs.rs`, `metadata/folder_art.rs`, `metadata/coverartarchive.rs`. Is the album matched (`album_links`)? Is Cover Art Archive on? Is the folder readable (folder images need it)? `art_thumbs` rows with a stale `stamp` are remade. | `library::art`: "cannot serve a picture: …"; `library::thumbs`: "cannot keep a thumbnail"; `metadata::jobs`: "Cover(<id>): …" |
| **No online details** | Settings › Online sources: "match automatically", each source's switch. `metadata/jobs.rs` (`attempt`, `needs_match`), `metadata/http.rs` (back-off). An album marked `review` waits for the user ("Find details…"). | `metadata::jobs`: "paused: musicbrainz.org can't be reached", "resumed", "Match(<id>): …" |
| **A scan never ends**, or repeats | `library/scanner.rs` (`read_all` waits on every file: a hung network share stalls it), `library/watch.rs` (events keep arriving: something writes into the folder, so it rescans after each quiet period). Cloud placeholders are checked by every scan but never read. | `library::watch`: "background scan: …"; `library::scanner`: "folder <id>: <n> cloud placeholders left unread" |
| **Search misses something** | `library/search.rs`; the FTS triggers in the migrations (a column the triggers don't index isn't searchable). Words under three characters match only word starts. | (none at info) |
| **The database repair dialog appears** | `library/recovery.rs`: the launch check failed. The copies are `library.sqlite3.pre-<n>`; the damaged file is kept as `…damaged-<time>`. | `library::commands`: "the database check failed: [...]" |
| **The UI shows an old queue, or a wrong order** | `queue/model.rs`: a list change that didn't log an `Edit` (`edited`) or `reset_list`; `lib/queueEdits.ts` applies edits by `listVersion`. | (none; compare `queue_state` with the UI) |
| **High CPU** | The loudness analysis (Settings › Features; `library/analysis.rs`), a scan, the visualizer (its frame rate), effects at a high sample rate (`scripts/bench.py` has their budgets). | `library::analysis` progress; the Activity Monitor's threads are named (`analysis`, `metadata`, `anomp read-ahead`…) |
| **The app won't start** | `lib.rs`'s `setup` logs each part's failure and carries on; a panic is in the log with its backtrace. | `panic`: the message and backtrace; any `ERROR` line from `setup` |
| **`cargo test`: "… is out of date"** | A Rust type, command or setting changed: `ANOMP_WRITE_BINDINGS=1 cargo test bindings` and commit the generated files (chapter 6). | — |
| **Clippy fails on `unsafe`** | Every `unsafe` block needs a `// SAFETY:` comment saying why it is sound, and an unsafe function's body still wraps its unsafe operations in blocks (`Cargo.toml`'s `[lints]`). | — |
| **`check-all.py` fails a repo check** | Its message names the file. `check-c-api.py`: a function in `anomp.h` not bound in `anomp.rs`. `check-sources.py`: a C++ file not listed in its `CMakeLists.txt`. `check-docs.py`: a path in a doc that doesn't exist. `check-user-guide.py`: a UI name or error the user guide lacks. `check-developer-guide.py`: a module missing from (or gone from) the repository map. | — |
