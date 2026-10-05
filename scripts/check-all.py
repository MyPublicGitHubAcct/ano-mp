#!/usr/bin/env python3
"""Runs every check the repo has: the one entry point for CI and for a local run.

First doctor.py, which checks the tools are installed (--quick: those the
quick checks need). Then the quick checks: each formatter in --check mode, the repo checks
(check-c-api.py, check-sources.py, check-migrations.py, version.py
--check, check-docs.py, check-user-guide.py, check-developer-guide.py,
class-dictionary.py --check), the scripts' tests and gitleaks over the history. Then, unless --quick, the builds and
test suites: the core (CMake debug preset, clang-tidy through lint-cpp.py
and ctest, then ctest again under ASan and UBSan, and under TSan, then each fuzz target for a
minute), the frontend (svelte-check,
ESLint, npm test and the build), THIRD_PARTY_NOTICES (make-notices.py
--check), the Rust crate (clippy, cargo test,
and cargo deny over its dependencies), and on CI the sandboxed bundle's
self-test (self-test-bundle.py). CI runs this and nothing else, so
a local run matches it; --quick suits a pre-commit hook.

Every step runs even after one fails, and a summary at the end lists the
failures. Exits non-zero if any step failed.

Needs `npm ci` (or `npm install`) run in app/, also for --quick
(Prettier), and gitleaks. The full run also needs FFmpeg built
(scripts/build-ffmpeg.sh), Homebrew's llvm@22 for the fuzzers, and cargo-deny (`cargo install cargo-deny
--locked`, at the version in .github/workflows/ci.yml).

Usage: scripts/check-all.py [--quick] [--list]
"""

import argparse
import dataclasses
import pathlib
import subprocess
import sys
import time

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"


@dataclasses.dataclass(frozen=True)
class Step:
    name: str
    command: list
    cwd: pathlib.Path = REPO_ROOT
    quick: bool = True
    # The command for a full run, when it differs from the quick one.
    full_command: list = None


def script(name, *args):
    """A command running scripts/<name> with this Python, as on Windows."""
    return [sys.executable, str(REPO_ROOT / "scripts" / name), *args]


STEPS = [
    # Missing or outdated tools first, before they fail a step obscurely.
    Step("tools", script("doctor.py", "--quick"), full_command=script("doctor.py")),
    Step("format C++", script("format-cpp.py", "--check")),
    Step("format Python", script("format-python.py", "--check")),
    Step("format Rust", script("format-rust.py", "--check")),
    Step("format frontend", script("format-frontend.py", "--check")),
    Step("C API bindings", script("check-c-api.py")),
    Step("core source lists", script("check-sources.py")),
    Step("DB migrations", script("check-migrations.py")),
    Step("version number", script("version.py", "--check")),
    Step("docs' paths and links", script("check-docs.py")),
    Step("user guide coverage", script("check-user-guide.py")),
    Step("developer guide's map", script("check-developer-guide.py")),
    Step("class dictionary", script("class-dictionary.py", "--check")),
    Step("script tests", script("test-python.py", "-q")),
    # Every commit's changes; scripts/hooks/pre-commit checks the staged ones.
    Step("secrets", ["gitleaks", "git", "--redact", "--no-banner", "--log-level", "warn"]),
    Step("core configure", ["cmake", "--preset", "debug"], quick=False),
    Step("core build", ["cmake", "--build", "--preset", "debug"], quick=False),
    # clang-tidy over core/src (PLAN.md H21), from the configure's compile database.
    Step("core lint", script("lint-cpp.py"), quick=False),
    Step("core tests", ["ctest", "--preset", "debug", "--output-on-failure"], quick=False),
    # The core's tests again under the sanitizers (PLAN.md H6): configure,
    # build and test each preset (CMakePresets.json's workflow presets).
    Step("core tests (ASan, UBSan)", ["cmake", "--workflow", "--preset", "asan"], quick=False),
    Step("core tests (TSan)", ["cmake", "--workflow", "--preset", "tsan"], quick=False),
    # Each libFuzzer target for 60 s (PLAN.md H5), built with llvm@22.
    Step("core fuzzing", script("run-fuzzers.py"), quick=False),
    Step("frontend check", ["npm", "run", "check"], cwd=APP, quick=False),
    Step("frontend lint", ["npm", "run", "lint"], cwd=APP, quick=False),
    Step("frontend tests", ["npm", "test"], cwd=APP, quick=False),
    # Before the Rust steps: Tauri embeds app/build when the crate compiles.
    Step("frontend build", ["npm", "run", "build"], cwd=APP, quick=False),
    # After the core's configure (JUCE, TagLib, …) and the frontend build
    # (the npm packages it bundled): THIRD_PARTY_NOTICES is up to date.
    Step("third-party notices", script("make-notices.py", "--check"), quick=False),
    Step(
        "Rust lint",
        ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        cwd=APP / "src-tauri",
        quick=False,
    ),
    Step("Rust tests", ["cargo", "test"], cwd=APP / "src-tauri", quick=False),
    # After the suites have built: PLAN.md §2's test counts are current.
    Step("docs' test counts", script("check-docs.py", "--counts"), quick=False),
    # The sandboxed bundle's self-test (H14); on CI only, unless run by
    # hand with --local, since it runs in the app's real container.
    Step("bundle self-test", script("self-test-bundle.py"), quick=False),
    # Advisories (fetched from RustSec), licences, bans and sources (deny.toml).
    Step(
        "Rust dependencies",
        ["cargo", "deny", "check", "--hide-inclusion-graph"],
        cwd=APP / "src-tauri",
        quick=False,
    ),
]


def select(steps, quick):
    """The steps a run makes, each with its command for that run."""
    return [
        step
        if quick or step.full_command is None
        else dataclasses.replace(step, command=step.full_command)
        for step in steps
        if step.quick or not quick
    ]


def run_step(step):
    """Runs one step, streaming its output; returns whether it passed."""
    try:
        return subprocess.run(step.command, cwd=step.cwd, check=False).returncode == 0
    except FileNotFoundError as error:
        print(f"check-all: {step.name}: {error}")
        return False


def run(steps, run_step=run_step, out=print):
    """Runs every step in order; returns the names of those that failed."""
    failed = []
    for step in steps:
        out(f"\n==> {step.name}", flush=True)
        started = time.monotonic()
        passed = run_step(step)
        elapsed = time.monotonic() - started
        out(f"<== {step.name}: {'ok' if passed else 'FAILED'} ({elapsed:.1f}s)", flush=True)
        if not passed:
            failed.append(step.name)
    return failed


def main():
    parser = argparse.ArgumentParser(description="Run every check (CI's entry point).")
    parser.add_argument("--quick", action="store_true", help="skip the builds and test suites")
    parser.add_argument("--list", action="store_true", help="list the steps without running")
    args = parser.parse_args()

    steps = select(STEPS, args.quick)
    if args.list:
        for step in steps:
            print(f"{step.name}: {' '.join(str(part) for part in step.command)}")
        return 0

    failed = run(steps)
    print()
    if failed:
        print(f"check-all: {len(failed)} of {len(steps)} steps failed: {', '.join(failed)}")
        return 1
    print(f"check-all: all {len(steps)} steps passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
