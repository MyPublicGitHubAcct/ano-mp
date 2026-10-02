#!/usr/bin/env python3
"""Builds the core's libFuzzer targets and runs each one for a while (PLAN.md H5).

Builds FFmpeg's instrumented static build (scripts/build-ffmpeg.sh --fuzz,
skipped when up to date) and the CMake `fuzz` preset, then runs each target
for --seconds (60 by default, as check-all.py runs it) over its corpus in
build/fuzz/corpus/<target>, seeded from core/tests/fixtures/. A target
that crashes, leaks, times out or trips ASan or UBSan leaves the input in
build/fuzz/crashes/ and fails the run; it becomes a regression test with
a committed fixture.

Needs Homebrew's llvm@22 (`brew install llvm@22`): Apple's clang has no
libFuzzer runtime, and LLVM 21's ASan hangs at start-up on macOS 26.

Usage: scripts/run-fuzzers.py [--seconds N] [--target NAME] [--no-build]
       scripts/run-fuzzers.py --target NAME CRASH_FILE  (reproduce one input)
"""

import argparse
import os
import pathlib
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
BUILD = REPO_ROOT / "build" / "fuzz"
FIXTURES = REPO_ROOT / "core" / "tests" / "fixtures"
# The fuzz preset's compiler (CMakePresets.json, build-ffmpeg.sh).
LLVM = pathlib.Path("/opt/homebrew/opt/llvm@22")

# Short name -> the executable core/fuzz/CMakeLists.txt builds.
TARGETS = {
    "decoder": "anomp_decoderfuzzer",
    "tags": "anomp_tagreaderfuzzer",
}

SANITIZER_ENV = {
    "ASAN_OPTIONS": "abort_on_error=0:halt_on_error=1:detect_stack_use_after_return=1",
    "UBSAN_OPTIONS": "print_stacktrace=1:halt_on_error=1",
}


def fuzz_command(target, seconds, build=BUILD, fixtures=FIXTURES):
    """The libFuzzer command for one target: new inputs go to the first
    directory (its corpus), the fixtures only seed it."""
    return [
        str(build / "core" / "fuzz" / TARGETS[target]),
        f"-max_total_time={seconds}",
        # One input taking longer than this is a hang, and fails the run.
        "-timeout=30",
        "-rss_limit_mb=4096",
        "-print_final_stats=1",
        f"-artifact_prefix={build / 'crashes'}/{target}-",
        str(build / "corpus" / target),
        str(fixtures),
    ]


def run(command, env=None):
    print("$ " + " ".join(command), flush=True)
    return subprocess.run(command, cwd=REPO_ROOT, env=env, check=False).returncode == 0


def build():
    if not (LLVM / "bin" / "clang").exists():
        print(f"run-fuzzers: {LLVM}/bin/clang not found: brew install llvm@22")
        return False
    return (
        run([str(REPO_ROOT / "scripts" / "build-ffmpeg.sh"), "--fuzz"])
        and run(["cmake", "--preset", "fuzz"])
        and run(["cmake", "--build", "--preset", "fuzz"])
    )


def main():
    parser = argparse.ArgumentParser(description="Build and run the core's fuzz targets.")
    parser.add_argument("--seconds", type=int, default=60, help="how long each target runs")
    parser.add_argument("--target", choices=sorted(TARGETS), help="run only this target")
    parser.add_argument("--no-build", action="store_true", help="use the existing build")
    parser.add_argument("inputs", nargs="*", help="run these inputs once instead of fuzzing")
    args = parser.parse_args()

    if args.inputs and not args.target:
        parser.error("reproducing inputs needs --target")
    if not args.no_build and not build():
        return 1

    env = {**os.environ, **SANITIZER_ENV}
    targets = [args.target] if args.target else sorted(TARGETS)

    if args.inputs:
        command = fuzz_command(args.target, args.seconds)[:1] + args.inputs
        return 0 if run(command, env) else 1

    (BUILD / "crashes").mkdir(parents=True, exist_ok=True)
    failed = []
    for target in targets:
        (BUILD / "corpus" / target).mkdir(parents=True, exist_ok=True)
        if not run(fuzz_command(target, args.seconds), env):
            failed.append(target)

    if failed:
        print(f"run-fuzzers: failed: {', '.join(failed)} (inputs in {BUILD / 'crashes'})")
        return 1
    print(f"run-fuzzers: {len(targets)} targets ran {args.seconds}s each without a failure")
    return 0


if __name__ == "__main__":
    sys.exit(main())
