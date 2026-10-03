#!/usr/bin/env python3
"""Checks or sets the app's version number (PLAN.md §8.2).

CMake's `project(VERSION)` in CMakeLists.txt is the source of truth: the
core's `anomp_version()` is compiled from it, and the Rust crate's tests
check that `Cargo.toml` agrees. The other copies are the ones the tools
need:

- app/src-tauri/Cargo.toml (`[package] version`) and its entry in
  Cargo.lock;
- app/src-tauri/tauri.conf.json (`version`, the bundle's version);
- app/package.json (`version`) and the two copies in package-lock.json.

`--check` fails unless every copy agrees; with `--tag`, also unless the
tag (vX.Y.Z) names that version, as the release workflow checks. Given a
version, rewrites each copy in place, changing nothing else in the files.
Versions are MAJOR.MINOR.PATCH, numbers only (macOS bundle versions must
be).

Usage: scripts/version.py --check [--tag vX.Y.Z]
       scripts/version.py X.Y.Z
"""

import argparse
import json
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")

CMAKE = re.compile(r"(\bproject\s*\(\s*ano_mp\s+VERSION\s+)([^\s)]+)")
# `version` in Cargo.toml's [package] table, before any other table.
CARGO_TOML = re.compile(
    r'(\A(?:(?!^\[).*\n)*?^\[package\]\n(?:(?!^\[).*\n)*?^version = ")([^"]*)', re.MULTILINE
)
CARGO_LOCK = re.compile(r'(^\[\[package\]\]\nname = "ano-mp"\nversion = ")([^"]*)', re.MULTILINE)
# The first `"version": "…"` of a JSON file: the top-level one, as these
# files put it before any nested object.
JSON_VERSION = re.compile(r'("version"\s*:\s*")([^"]*)')


def _json_read(text):
    return [json.loads(text).get("version")]


def _lock_read(text):
    data = json.loads(text)
    return [data.get("version"), data.get("packages", {}).get("", {}).get("version")]


# (path relative to the repo, pattern, how many matches, and for JSON files
# a reader of the versions by key, so a nested "version" isn't taken for
# the top-level one).
FILES = [
    ("CMakeLists.txt", CMAKE, 1, None),
    ("app/src-tauri/Cargo.toml", CARGO_TOML, 1, None),
    ("app/src-tauri/Cargo.lock", CARGO_LOCK, 1, None),
    ("app/src-tauri/tauri.conf.json", JSON_VERSION, 1, _json_read),
    ("app/package.json", JSON_VERSION, 1, _json_read),
    ("app/package-lock.json", JSON_VERSION, 2, _lock_read),
]


def read_version(text, pattern, count, reader=None):
    """The version a file holds, None if it can't be found, or its
    differing copies joined with " / "."""
    matches = list(pattern.finditer(text))[:count]
    if len(matches) < count:
        return None
    found = [match.group(2) for match in matches]
    if reader is not None:
        found += reader(text)
    found = list(dict.fromkeys(str(value) for value in found))
    return " / ".join(found)


def set_version(text, pattern, count, version, reader=None, path=""):
    """The file's text with its version replaced; raises ValueError if the
    version isn't where it should be."""
    new, replaced = pattern.subn(lambda match: match.group(1) + version, text, count=count)
    if replaced != count:
        raise ValueError(f"{path}: version not found")
    if read_version(new, pattern, count, reader) != version:
        raise ValueError(f"{path}: the rewrite didn't set the version")
    return new


def versions(root):
    """Each file's version (None where it can't be found)."""
    result = {}
    for name, pattern, count, reader in FILES:
        text = (root / name).read_text(encoding="utf-8")
        result[name] = read_version(text, pattern, count, reader)
    return result


def problems(found, tag=None):
    """What's wrong with the versions found: every copy must equal
    CMakeLists.txt's, and the tag, if given, must name it."""
    expected = found.get("CMakeLists.txt")
    if expected is None or VERSION.match(expected) is None:
        return [f"CMakeLists.txt: no MAJOR.MINOR.PATCH version in project() (found {expected})"]
    listed = []
    for name, version in found.items():
        if version is None:
            listed.append(f"{name}: no version found")
        elif version != expected:
            listed.append(f"{name}: {version}, but CMakeLists.txt has {expected}")
    if tag is not None and tag != f"v{expected}":
        listed.append(f"tag {tag} doesn't name version {expected} (expected v{expected})")
    return listed


def write(root, version):
    """Sets every copy to `version`. Reads and rewrites all of them in
    memory first, so a file that can't be rewritten leaves all unchanged."""
    if VERSION.match(version) is None:
        raise ValueError(f"{version}: not MAJOR.MINOR.PATCH")
    rewritten = []
    for name, pattern, count, reader in FILES:
        path = root / name
        text = path.read_text(encoding="utf-8")
        rewritten.append((path, set_version(text, pattern, count, version, reader, name)))
    for path, text in rewritten:
        path.write_text(text, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description="Check or set the app's version number.")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true", help="fail unless every copy agrees")
    group.add_argument("version", nargs="?", help="the new version, MAJOR.MINOR.PATCH")
    parser.add_argument("--tag", help="with --check: a release tag that must be vVERSION")
    args = parser.parse_args()

    if args.check:
        listed = problems(versions(REPO_ROOT), args.tag)
        for problem in listed:
            print(f"version: {problem}")
        if listed:
            return 1
        print(f"version: {versions(REPO_ROOT)['CMakeLists.txt']} everywhere")
        return 0
    if args.tag:
        parser.error("--tag goes with --check")
    try:
        write(REPO_ROOT, args.version)
    except ValueError as error:
        print(f"version: {error}")
        return 1
    print(f"version: set {args.version} in {len(FILES)} files; update CHANGELOG.md too")
    return 0


if __name__ == "__main__":
    sys.exit(main())
