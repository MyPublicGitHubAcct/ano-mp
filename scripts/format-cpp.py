#!/usr/bin/env python3
"""Formats the C++ and Objective-C++ code in core/ with clang-format.

Uses .clang-format (JUCE style) and core/include/.clang-format (the C API).
Run it after editing core code; it rewrites files in place. With --check it
changes nothing: it reports each unformatted line and exits non-zero when
there are any (for CI or a pre-commit hook).

clang-format's output changes between versions, so the version is pinned and
run through uvx (https://docs.astral.sh/uv/), not whatever is on PATH.

Usage: scripts/format-cpp.py [--check]
"""

import argparse
import pathlib
import shutil
import subprocess
import sys

CLANG_FORMAT_VERSION = "23.1.1"

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
CORE = REPO_ROOT / "core"
EXTENSIONS = {".h", ".cpp", ".mm"}

# Files per clang-format run, keeping the command line well inside Windows'
# 32K-character limit however many files there are.
BATCH_SIZE = 100


def main():
    parser = argparse.ArgumentParser(description="Format the core's C++ code with clang-format.")
    parser.add_argument("--check", action="store_true", help="report only, change nothing")
    args = parser.parse_args()

    uvx = shutil.which("uvx")
    if uvx is None:
        sys.exit("error: uvx not found; install uv: https://docs.astral.sh/uv/")

    files = sorted(
        str(path.relative_to(REPO_ROOT))
        for path in CORE.rglob("*")
        if path.suffix in EXTENSIONS and path.is_file()
    )
    options = ["--dry-run", "--Werror"] if args.check else ["-i"]
    command = [uvx, f"clang-format@{CLANG_FORMAT_VERSION}", "--style=file", *options]

    status = 0
    for start in range(0, len(files), BATCH_SIZE):
        batch = files[start : start + BATCH_SIZE]
        status = subprocess.run([*command, *batch], cwd=REPO_ROOT, check=False).returncode or status
    return status


if __name__ == "__main__":
    sys.exit(main())
