#!/usr/bin/env python3
"""Runs clang-tidy over the core's and the effects library's sources (PLAN.md H21).

The checks are .clang-tidy's (bugprone-*, performance-* and concurrency-*,
less those it turns off with their reasons), over every core/src and
effects/src file in the debug preset's compile database, so run `cmake --preset debug` first. A
platform's files the database doesn't compile (DockMenu_none.cpp on macOS)
are linted on their own platform. Warnings in core/, effects/ (src and
include) are reported, each once however many files include its header; JUCE's, TagLib's
and FFmpeg's are not. Exits non-zero if there are any. Silence a finding
that isn't a bug at its line with `// NOLINT(check-name): reason`.

clang-tidy's checks change between versions, so the version is pinned and
run through uvx (https://docs.astral.sh/uv/), not whatever is on PATH. On
macOS the SDK comes from `xcrun --show-sdk-path`, since the compile commands
leave it to Apple's compiler driver.

Usage: scripts/lint-cpp.py [--build-dir DIR] [--jobs N] [FILE ...]
"""

import argparse
import concurrent.futures
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys

# PyPI's clang-tidy wheels lag clang-format's by a release.
CLANG_TIDY_VERSION = "22.1.8"

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE_DIRS = [REPO_ROOT / "core" / "src", REPO_ROOT / "effects" / "src"]
BUILD_DIR = REPO_ROOT / "build" / "debug"

# The first line of a diagnostic: "path:line:col: warning: text [check]".
DIAGNOSTIC = re.compile(r"^\S.*:\d+:\d+: (warning|error): ")


def units(database, sources):
    """The files in `database` (compile_commands.json's entries) under
    `sources`, sorted and without repeats."""
    found = set()
    for entry in database:
        path = pathlib.Path(entry["directory"], entry["file"]).resolve()
        if path.is_relative_to(sources):
            found.add(path)
    return sorted(found)


def sdk_args(platform=sys.platform, run=subprocess.run):
    """Extra compiler arguments clang-tidy needs on this platform."""
    if platform != "darwin":
        return []
    sdk = run(["xcrun", "--show-sdk-path"], capture_output=True, text=True, check=True)
    return [f"--extra-arg=-isysroot{sdk.stdout.strip()}"]


def diagnostics(output):
    """clang-tidy's output split into diagnostics, each its first line and
    the notes and source excerpts under it."""
    found = []
    for line in output.splitlines():
        if DIAGNOSTIC.match(line):
            found.append([line])
        elif found:
            found[-1].append(line)
    return ["\n".join(lines) for lines in found]


def unique(reports):
    """Each diagnostic once, in order, by its first line: a header's warning
    comes once from every file that includes it."""
    seen = set()
    kept = []
    for report in reports:
        for diagnostic in report:
            first = diagnostic.split("\n", 1)[0]
            if first not in seen:
                seen.add(first)
                kept.append(diagnostic)
    return kept


def tidy(command, path):
    """Runs clang-tidy on one file; returns its output."""
    result = subprocess.run(
        [*command, str(path)], cwd=REPO_ROOT, capture_output=True, text=True, check=False
    )
    return result.stdout + result.stderr


def main():
    parser = argparse.ArgumentParser(description="Run clang-tidy over the C++ libraries' sources.")
    parser.add_argument("--build-dir", type=pathlib.Path, default=BUILD_DIR)
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    parser.add_argument("files", nargs="*", type=pathlib.Path, help="only these files")
    args = parser.parse_args()

    uvx = shutil.which("uvx")
    if uvx is None:
        sys.exit("error: uvx not found; install uv: https://docs.astral.sh/uv/")
    database_path = args.build_dir / "compile_commands.json"
    if not database_path.is_file():
        sys.exit(f"error: no {database_path}; run `cmake --preset debug` first")

    database = json.loads(database_path.read_text())
    files = sorted({path for sources in SOURCE_DIRS for path in units(database, sources)})
    if args.files:
        wanted = {path.resolve() for path in args.files}
        files = [path for path in files if path in wanted]
    if not files:
        sys.exit("error: no core/src or effects/src files to lint in the compile database")

    command = [
        uvx,
        f"clang-tidy@{CLANG_TIDY_VERSION}",
        "-p",
        str(args.build_dir),
        "--quiet",
        *sdk_args(),
    ]
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        outputs = list(pool.map(lambda path: tidy(command, path), files))

    found = unique(diagnostics(output) for output in outputs)
    for diagnostic in found:
        print(diagnostic)
    print(f"lint-cpp: {len(found)} findings in {len(files)} files")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
