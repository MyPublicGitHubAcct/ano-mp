# ano-mp

A music player for macOS, with iPhone/iPad, Linux and Windows to follow. It plays
MP3, FLAC, AAC/M4A, ALAC, Ogg Vorbis, Opus, WAV, AIFF and WMA, shows file metadata
enriched from services such as MusicBrainz, and has an admin screen for choosing
displayed fields, library sorting and visualization.

**Status:** in development, macOS only so far, and not yet released. Working:
gapless playback of a play queue, a library of your music folders with browsing,
search, playlists, favourites and cover art, macOS Now Playing, media keys and
menus, online details (MusicBrainz, Cover Art Archive, Wikipedia, optionally
Discogs), visualizations, a Settings screen, and optional features you can turn on
in Settings › Features. Current work is hardening for a first signed release. See
[PLAN.md](PLAN.md) for the roadmap and current phase.

## How it's built

| Layer | Location | Technology |
|---|---|---|
| Audio core: decoding, playback, tags, device output | `core/` | C++20, [JUCE](https://juce.com), [FFmpeg](https://ffmpeg.org), [TagLib](https://taglib.org) |
| Backend: library database, settings, online services | `app/src-tauri/` | Rust, [Tauri 2](https://tauri.app), SQLite |
| Frontend | `app/src/` | Svelte 5, TypeScript |

The core is a static library with a plain C API (`core/include/anomp/anomp.h`) that
the Rust backend links directly; it does not run as a separate process.
[PLAN.md](PLAN.md) §1 explains why.

## Requirements (macOS)

- macOS 14 (Sonoma) or later, Apple silicon or Intel
- Xcode Command Line Tools (`xcode-select --install`). The full Xcode app is only
  needed to create a signing certificate (see [The app bundle](#the-app-bundle))
  and, later, for iPhone/iPad builds.
- [Homebrew](https://brew.sh) packages:
  ```sh
  brew install cmake ninja nasm pkg-config node uv gitleaks
  ```
  CMake 3.25 or later is required. `uv` runs the pinned code formatters and
  `gitleaks` the secret scan in the pre-commit hook. Node must be the version in
  `.nvmrc` (npm refuses another); use a version manager such as `fnm` or `nvm` if
  Homebrew's differs.
- Rust via [rustup](https://rustup.rs), which installs the version pinned in
  `rust-toolchain.toml` by itself:
  ```sh
  curl https://sh.rustup.rs -sSf | sh
  ```
- Only for the fuzzers and the full check (`scripts/check-all.py`):
  `brew install llvm@22` and `cargo install cargo-deny --version 0.20.2 --locked`.

## First-time setup

Run these from the repository root. You need an internet connection for the first
build.

1. **Build FFmpeg** (about 1.5 minutes). This downloads a pinned FFmpeg release,
   checks it and builds audio-only libraries into `third_party/ffmpeg/`:
   ```sh
   scripts/build-ffmpeg.sh
   ```
   The CMake build refuses to configure until this has run. Rerunning it does
   nothing unless the pinned version or its settings change.

2. **Build and test the audio core.** The first configure downloads JUCE, TagLib
   and Catch2 and takes several minutes:
   ```sh
   cmake --preset debug && cmake --build --preset debug && ctest --preset debug
   ```

3. **Install the frontend dependencies, and the pre-commit hook:**
   ```sh
   cd app && npm install
   git config core.hooksPath scripts/hooks   # from the repository root
   ```

4. **Run the app** (from `app/`):
   ```sh
   npm run tauri dev
   ```
   The first run takes a few minutes: Cargo builds its own copy of the audio core,
   which downloads JUCE again. Later runs are fast. Add a music folder from the
   sidebar to start.

## The app bundle

To build a standalone `ano-mp.app` (from `app/`):

```sh
npm run tauri build -- --bundles app
```

It lands in `app/src-tauri/target/release/bundle/macos/`. Unlike `tauri dev`, the
bundle runs in the macOS App Sandbox, keeps its data in
`~/Library/Containers/dev.anomp.player/` (separate from the dev app's library) and
has FFmpeg embedded. Rebuild it after pulling changes; it doesn't update itself.

**Signing and your music folders.** The sandbox lets the app read a folder you
picked only through a saved bookmark, and macOS honours a bookmark only for the
app identity that saved it. The bundle is ad-hoc signed by default
(`signingIdentity: "-"` in `tauri.conf.json`), which gives every build a new
identity, so after each rebuild its tracks are skipped until you add each music
folder again ("Add a folder…" and pick the same folder: this renews its access and
keeps everything else). The dev app repairs this by itself.

To stop that, sign with a free Apple Development certificate, which stays the same
across builds:

1. Install Xcode, add your Apple ID under **Settings → Accounts**, then
   **Manage Certificates… → + → Apple Development**.
2. Find its name with `security find-identity -v -p codesigning`, for example
   `Apple Development: you@example.com (TEAMID1234)`.
3. Build with it:
   ```sh
   APPLE_SIGNING_IDENTITY="Apple Development: you@example.com (TEAMID1234)" \
     npm run tauri build -- --bundles app
   ```
4. Add your music folders again once. Later builds keep them, until the
   certificate is renewed (yearly).

A bundle signed this way runs only on your own Macs; distribution needs Developer
ID signing and notarization ([PLAN.md](PLAN.md) §8).

## Everyday commands

| Task | Command | Run from |
|---|---|---|
| Run the app with live reload | `npm run tauri dev` | `app/` |
| Build the app bundle | `npm run tauri build -- --bundles app` | `app/` |
| Core tests (C++) | `cmake --build --preset debug && ctest --preset debug` | repo root |
| One core test by name | `ctest --preset debug -R "<name regex>"` | repo root |
| Backend tests (Rust) | `cargo test` | `app/src-tauri/` |
| Live checks against the online services | `cargo test live_ -- --ignored` | `app/src-tauri/` |
| Frontend tests | `npm test` | `app/` |
| Frontend type check and lint | `npm run check`, `npm run lint` | `app/` |
| Format after editing (add `--check` to only report) | `scripts/format-cpp.py`, `scripts/format-rust.py`, `scripts/format-python.py`, `scripts/format-frontend.py` | repo root |
| Every check, as CI runs it (`--quick`: what the pre-commit hook runs) | `scripts/check-all.py` | repo root |
| Rebuild FFmpeg from scratch | `scripts/build-ffmpeg.sh --force` | repo root |
| Regenerate decoder test fixtures (rarely; needs `brew install ffmpeg vorbis-tools`) | `scripts/make-test-fixtures.py` | repo root |

## When GitHub Actions run

CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs
`scripts/check-all.py`, the same command you can run locally. It takes a long
time on a macOS runner (it builds FFmpeg, JUCE and every sanitizer preset), so
it doesn't run on every push. It runs:

- **On a version tag.** Pushing a `v*` tag starts
  [release.yml](.github/workflows/release.yml), which runs all of CI first and
  builds the release only if CI passes:
  ```sh
  scripts/version.py 0.2.0      # set the version everywhere, then commit
  git tag v0.2.0 && git push origin v0.2.0
  ```
- **By hand,** on any branch (see [Running CI by hand](#running-ci-by-hand)).

The weekly fuzzing run ([fuzz.yml](.github/workflows/fuzz.yml)) runs on its
own schedule, Mondays at 06:00 UTC. Between releases, the pre-commit hook
(`git config core.hooksPath scripts/hooks`) runs `scripts/check-all.py --quick`
on every commit, and `scripts/check-all.py` runs the full set locally.

Dependabot's monthly pull requests don't get CI automatically. Run CI by hand
on their branch, or run `scripts/check-all.py` locally, before you merge them.

### Running CI by hand

CI runs on what is pushed to GitHub, so push the branch first. Run it on
`main` after pushing work that matters, and on a Dependabot branch before
merging its pull request.

On GitHub:

1. Open the repository's **Actions** tab and choose **CI** in the list of
   workflows on the left.
2. Click **Run workflow** (top right of the list of runs), pick the branch
   under **Use workflow from**, and click the green **Run workflow** button.
3. The run appears at the top of the list after a few seconds. Open it to
   follow the log; the **Check** step is `scripts/check-all.py`, and its last
   lines name any step that failed.

With the [GitHub CLI](https://cli.github.com) (`brew install gh`, then
`gh auth login` once):

```sh
gh workflow run ci.yml --ref main   # start CI on a branch (main here)
gh run list --workflow ci.yml       # recent runs and their results
gh run watch                        # follow a run until it finishes (pick one)
gh run view --log-failed            # the log of a failed run's failed step
gh pr list                          # open pull requests, with their branches
gh workflow run ci.yml --ref dependabot/cargo/app/src-tauri/cargo-…  # a Dependabot branch
```

A run takes about 25 minutes, longer when it has to build FFmpeg and JUCE
from scratch (the first run, or after a pin or the toolchain changes). Starting a second run on the same branch cancels
the first.

### Other ways to set the triggers

To change when CI runs, edit the `on:` block at the top of `ci.yml`. Keep
`workflow_call`, because `release.yml` uses it.

| Option | `on:` in `ci.yml` | Trade-off |
|---|---|---|
| **Version tag and by hand** (current) | `workflow_call:` and `workflow_dispatch:` | The fewest runs. A broken commit is found only at release time, or when you run a check yourself. |
| **When the version changes on `main`** | add `push:` with `branches: [main]` and `paths:` listing the files `scripts/version.py` writes: `CMakeLists.txt`, `app/package.json`, `app/package-lock.json`, `app/src-tauri/Cargo.toml`, `app/src-tauri/Cargo.lock`, `app/src-tauri/tauri.conf.json` | Checks a version bump before you tag it. Dependency updates also change `Cargo.lock` and `package-lock.json`, so they trigger it too. |
| **Pull requests only** | add `pull_request:` | Every pull request is checked before it is merged, Dependabot's included. Direct pushes to `main` aren't checked. |
| **Pushes to `main` and pull requests** | add `push:` with `branches: [main]`, and `pull_request:` | `main` is always checked. Work-in-progress branches aren't. |
| **Every push and pull request** (the old setting) | add `push:` with `branches: ["**"]`, and `pull_request:` | Every commit is checked. Uses the most runner time. |

To stop the weekly fuzzing too, delete the `schedule:` block in `fuzz.yml`
and keep `workflow_dispatch:`, so it can still be run by hand.

## Troubleshooting

- **`FFmpeg not found in …third_party/ffmpeg/macos-universal`**: run
  `scripts/build-ffmpeg.sh` from the repository root, then configure again.
- **The first build seems stuck:** JUCE is being downloaded (once for the CMake
  build, once for the Cargo build), which can take several minutes.
- **Tracks are skipped with "Folder not available … Cannot resolve the bookmark:
  The file couldn't be opened because it isn't in the correct format":** the app
  bundle was rebuilt with a different signature. Add the folder again, or sign
  with a certificate (see [The app bundle](#the-app-bundle)).
- **"Folder not available" for a folder on an external or network drive:** the
  drive isn't mounted. Mount it and try again; the library keeps its tracks
  meanwhile.
- **No sound:** check that the device in Settings › Playback is the one you are
  listening on, that it isn't muted, and that the volume slider in the player bar
  isn't at zero.

## Repository layout

```
core/          C++ audio core (static library), its C API and Catch2 tests
app/           Tauri app: Svelte frontend (src/) and Rust backend (src-tauri/)
scripts/       build-ffmpeg.sh, the checks, format, release and test tooling
cmake/         CMake helpers (pinned downloads, FFmpeg, TagLib, Signalsmith)
third_party/   locally built FFmpeg (git-ignored)
docs/          checklists, release decisions, and finished phases' design (design/)
PLAN.md        roadmap, status, decisions, risks and release plan
CLAUDE.md      conventions and rules for AI-assisted development
```

## License

Proprietary; all rights reserved. Third-party components keep their own licenses:
JUCE is used under a commercial license, FFmpeg under the LGPL 2.1 or later as
shared libraries, and TagLib under the MPL 1.1. Every component and its license
is listed in [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES) (generated by
`scripts/make-notices.py`). See [PLAN.md](PLAN.md) §4.
