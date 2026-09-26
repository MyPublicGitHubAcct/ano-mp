#!/usr/bin/env python3
"""Formats the Python code in the repo (scripts/) with ruff, using ruff.toml.

Run it after editing Python; it rewrites files in place. With --check it
changes nothing: it prints the diff and exits non-zero when any file is not
formatted (for CI or a pre-commit hook).

The ruff version is pinned and run through uvx (https://docs.astral.sh/uv/),
so everyone formats the same way whatever ruff is on PATH.

Usage: scripts/format-python.py [--check]
"""

import argparse
import pathlib
import shutil
import subprocess
import sys

RUFF_VERSION = "0.16.9"

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description="Format the Python code with ruff.")
    parser.add_argument("--check", action="store_true", help="report only, change nothing")
    args = parser.parse_args()

    uvx = shutil.which("uvx")
    if uvx is None:
        sys.exit("error: uvx not found; install uv: https://docs.astral.sh/uv/")

    # ruff skips .gitignore'd paths (build/, node_modules/, third_party/ffmpeg/).
    command = [uvx, f"ruff@{RUFF_VERSION}", "format"]
    if args.check:
        command += ["--check", "--diff"]
    return subprocess.run([*command, "."], cwd=REPO_ROOT, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
