#!/usr/bin/env python3
"""Runs every check the repo has: the one entry point for CI and for a local run.

First the quick checks: each formatter in --check mode, the repo checks
(check-c-api.py, check-sources.py, check-migrations.py) and the scripts'
tests. Then, unless --quick, the builds and test suites: the core (CMake
debug preset and ctest), the frontend (svelte-check, npm test and the
build) and the Rust crate (cargo test). CI runs this and nothing else, so
a local run matches it; --quick suits a pre-commit hook.

Every step runs even after one fails, and a summary at the end lists the
failures. Exits non-zero if any step failed.

Needs FFmpeg built (scripts/build-ffmpeg.sh) and `npm ci` (or `npm install`)
run in app/ for the full run.

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


def script(name, *args):
    """A command running scripts/<name> with this Python, as on Windows."""
    return [sys.executable, str(REPO_ROOT / "scripts" / name), *args]


STEPS = [
    Step("format C++", script("format-cpp.py", "--check")),
    Step("format Python", script("format-python.py", "--check")),
    Step("format Rust", script("format-rust.py", "--check")),
    Step("C API bindings", script("check-c-api.py")),
    Step("core source lists", script("check-sources.py")),
    Step("DB migrations", script("check-migrations.py")),
    Step("script tests", script("test-python.py", "-q")),
    Step("core configure", ["cmake", "--preset", "debug"], quick=False),
    Step("core build", ["cmake", "--build", "--preset", "debug"], quick=False),
    Step("core tests", ["ctest", "--preset", "debug", "--output-on-failure"], quick=False),
    Step("frontend check", ["npm", "run", "check"], cwd=APP, quick=False),
    Step("frontend tests", ["npm", "test"], cwd=APP, quick=False),
    # Before the Rust tests: Tauri embeds app/build when the crate compiles.
    Step("frontend build", ["npm", "run", "build"], cwd=APP, quick=False),
    Step("Rust tests", ["cargo", "test"], cwd=APP / "src-tauri", quick=False),
]


def select(steps, quick):
    return [step for step in steps if step.quick or not quick]


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
