# 2. Getting set up

From a fresh clone to a running app, the tests and the full check. The
step-by-step commands for a first build are in the
[README](../../README.md#first-time-setup); this chapter says what each
step does and what to do when one fails. macOS is the only platform that
builds today (Linux, Windows and iOS are later phases of `PLAN.md`).

## Tools

`scripts/doctor.py` is the authority: it checks each tool against the
repo's pins and minimums and prints what to install. Run it first, and
again whenever something odd happens with a build.

| Tool | Why | Pinned or checked by |
|---|---|---|
| Command Line Tools (Apple clang, the macOS SDK) | the core, Rust's linker | `doctor.py` |
| CMake ≥ 3.25 and Ninja | the core's build (and Cargo's copy of it) | `CMakeLists.txt` |
| nasm, pkg-config | FFmpeg's build | `build-ffmpeg.sh` |
| Rust | the backend | `rust-toolchain.toml` (rustup installs it) |
| Node and npm | the frontend | `.nvmrc`; `app/.npmrc` makes npm refuse another |
| uv | runs the pinned formatters and pytest | `doctor.py` |
| gitleaks | the secret scan (pre-commit hook, `check-all.py`) | `doctor.py` |
| cargo-deny | licences and advisories | the version CI installs |
| Homebrew's `llvm@22` | the fuzzers only (Apple's clang has no libFuzzer) | `doctor.py` |

## The first build

1. **FFmpeg**: `scripts/build-ffmpeg.sh` downloads the pinned release,
   checks it against its pinned SHA-256, and builds LGPL, audio-only, shared
   libraries into `third_party/ffmpeg/<platform>/` (about 1.5 minutes).
   CMake refuses to configure without it (`cmake/FFmpeg.cmake`), and the
   script does nothing on a rerun unless its pin or flags changed
   (`BUILD_INFO` records them).
2. **The core**: `cmake --preset debug && cmake --build --preset debug &&
   ctest --preset debug`. The first configure downloads JUCE, TagLib,
   Catch2 and Signalsmith Stretch into `build/_downloads/`, each checked
   against its SHA-256 (`cmake/Fetch.cmake`), which every preset and
   Cargo's build share afterwards.
3. **The frontend's packages**: `cd app && npm install`.
4. **The hook**: `git config core.hooksPath scripts/hooks` runs gitleaks
   and `check-all.py --quick` before each commit.
5. **The app**: `npm run tauri dev` from `app/`. Cargo builds its own copy
   of the core into `app/src-tauri/target/` (`build.rs`, through the
   `cmake` crate with Ninja and the tests off), separate from
   `build/debug`, so the first run takes several minutes; later runs
   rebuild the core only when `core/`, `effects/`, `cmake/` or the
   top-level `CMakeLists.txt` changes.

`tauri dev` runs the app unsandboxed, from Vite's dev server, with debug
logging copied to the terminal and the /dev page available. The data it
keeps is the unsandboxed app's (chapter 9), separate from a bundle's.

## Everyday commands

The full list, with one-line explanations, is `CLAUDE.md`'s "Build &
test". The ones you will use most:

```sh
# Core and effects (from the repo root)
cmake --build --preset debug && ctest --preset debug
ctest --preset debug -R "FormatRegistry round-trips"   # one test by name
./build/debug/core/tests/anomp_core_tests "[formats]"   # by tag

# Backend (from app/src-tauri)
cargo test                               # unit tests, C API wrappers, generated files
cargo test scanner::                     # one module's tests
cargo clippy --all-targets -- -D warnings

# Frontend (from app/)
npm run check && npm run lint && npm test

# Everything CI runs
scripts/check-all.py           # --quick for the fast checks only
```

After editing code, run its formatter: `scripts/format-cpp.py`,
`scripts/format-rust.py`, `scripts/format-frontend.py` or
`scripts/format-python.py` (each with `--check` to report only). Commit
a reformat on its own.

## The presets

`CMakePresets.json` defines one build folder per purpose, all under
`build/`:

| Preset | Folder | For |
|---|---|---|
| `debug` | `build/debug` | everyday building and `ctest`; the compile database clang-tidy and editors read |
| `release` | `build/release` | an optimised core without tests |
| `asan` | `build/asan` | the tests under AddressSanitizer and UndefinedBehaviorSanitizer: `cmake --workflow --preset asan` |
| `tsan` | `build/tsan` | the tests under ThreadSanitizer (about 4.5 minutes): `cmake --workflow --preset tsan` |
| `bench` | `build/bench` | the core's benchmarks, run by `scripts/bench.py` |
| `fuzz` | `build/fuzz` | the libFuzzer targets, built by `scripts/run-fuzzers.py` with Homebrew's clang and an instrumented static FFmpeg |

## When the first build fails

| Symptom | Cause and fix |
|---|---|
| CMake: "FFmpeg not found in …third_party/ffmpeg/…" or "lib… missing" | Run `scripts/build-ffmpeg.sh`, then configure again. |
| `doctor.py`: "ffmpeg … built from another pin or flags" | The pin or flags changed since your build: rerun `build-ffmpeg.sh`. |
| CMake: "…: SHA-256 …, expected …" | A download doesn't match its pin: a broken download (delete it from `build/_downloads/` and retry) or a pin changed by hand (use `scripts/bump-pin.py`). |
| The first configure or `tauri dev` seems stuck | JUCE is downloading, once for CMake and once for Cargo's build; it takes minutes. |
| npm: "Unsupported engine" | Your Node isn't `.nvmrc`'s; use a version manager. |
| `cargo test`: "… is out of date: run `ANOMP_WRITE_BINDINGS=1 cargo test bindings`" | A Rust type or command changed: `ANOMP_WRITE_BINDINGS=1 cargo test bindings`, then commit the generated files. |
| `cargo test --release` can't load `libavcodec…dylib` | Release binaries have no rpath into `third_party/`; run it from `app/src-tauri`, where `.cargo/config.toml` sets the library path. |
| Clippy: "unsafe block missing a safety comment" | Every `unsafe` block needs a `// SAFETY:` comment; see `CLAUDE.md`. |
| The fuzz build fails to link, or ASan hangs | The fuzzers need Homebrew's `llvm@22` exactly (LLVM 21's ASan hangs on macOS 26); see `CLAUDE.md`. |
| A sandboxed bundle skips every track ("isn't in the correct format") | An ad-hoc signed bundle was rebuilt: add each folder again, or sign with a stable certificate ([README](../../README.md#the-app-bundle)). |

## Before you push

CI runs on demand, not on every push (README, "When GitHub Actions
run"), so run `scripts/check-all.py` locally first. It runs `doctor.py`,
the formatters' checks, every repo check, the scripts' tests and
gitleaks (the `--quick` steps), then the core's build, clang-tidy and
tests, the sanitizers, a minute of each fuzzer, the frontend's checks,
tests and build, the third-party notices, clippy and the Rust tests, the
test counts in `PLAN.md`, the bundle self-test (on CI only) and
cargo-deny. `--list` shows
the steps; a failing step prints its command, so you can rerun it alone.
