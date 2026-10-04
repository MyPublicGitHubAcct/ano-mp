#!/usr/bin/env python3
"""Checks that the C++ libraries' CMake source lists match the files on disk.

The core's and the effects library's CMakeLists.txt files list their sources
by hand (no globbing), so a new file that isn't listed is silently left out
of the build. Every .cpp and .mm under core/src, core/tests, core/fuzz,
effects/src and effects/tests must be listed in that directory's owning
CMakeLists.txt, and every file listed must exist. Lists
every problem it finds and exits non-zero if there are any. Changes nothing.

Usage: scripts/check-sources.py
"""

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE_EXTENSIONS = {".cpp", ".mm"}

# (CMakeLists.txt, the directory of sources it lists), relative to the repo.
SOURCE_LISTS = [
    ("core/CMakeLists.txt", "core/src"),
    ("core/tests/CMakeLists.txt", "core/tests"),
    ("core/fuzz/CMakeLists.txt", "core/fuzz"),
    ("effects/CMakeLists.txt", "effects/src"),
    ("effects/tests/CMakeLists.txt", "effects/tests"),
]


def listed_sources(cmake_text):
    """Every .cpp/.mm path named in a CMake file, relative to that file."""
    text = re.sub(r"#[^\n]*", " ", cmake_text)
    pattern = r"[\w./-]+(?:" + "|".join(re.escape(e) for e in SOURCE_EXTENSIONS) + r")\b"
    return set(re.findall(pattern, text))


def sources_on_disk(directory):
    """Every .cpp/.mm under `directory`, relative to it, '/'-separated."""
    return {
        path.relative_to(directory).as_posix()
        for path in directory.rglob("*")
        if path.suffix in SOURCE_EXTENSIONS and path.is_file()
    }


def compare(cmake_path, source_dir, root=REPO_ROOT):
    """Problems for one CMakeLists.txt and the source directory it lists."""
    cmake_file = root / cmake_path
    cmake_dir = cmake_file.parent
    listed = {
        (cmake_dir / path).resolve().relative_to(root.resolve()).as_posix()
        for path in listed_sources(cmake_file.read_text(encoding="utf-8"))
    }
    on_disk = {f"{source_dir}/{path}" for path in sources_on_disk(root / source_dir)}

    problems = []
    for path in sorted(on_disk - listed):
        problems.append(f"{path}: not listed in {cmake_path}")
    for path in sorted(listed - on_disk):
        problems.append(f"{path}: listed in {cmake_path} but doesn't exist")
    return problems


def main():
    parser = argparse.ArgumentParser(description="Check the C++ libraries' CMake source lists.")
    parser.parse_args()

    problems = []
    for cmake_path, source_dir in SOURCE_LISTS:
        problems += compare(cmake_path, source_dir)
    for problem in problems:
        print(f"check-sources: {problem}")
    if not problems:
        print("check-sources: every C++ source is listed")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
