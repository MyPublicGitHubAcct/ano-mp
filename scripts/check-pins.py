#!/usr/bin/env python3
"""Reports which pinned dependencies have newer releases (PLAN.md §9.1, §9.2 M5).

Reads each pin from its file (the native libraries' in CMake and
build-ffmpeg.sh; the formatters', linters' and pytest's in their scripts)
and asks upstream for the latest release: the repository's tags (with `git
ls-remote`, which needs no GitHub token), ffmpeg.org's release listing, or
PyPI. Then summarizes `cargo update --dry-run` (app/src-tauri) and `npm
outdated` (app/). Report only: it changes nothing, and exits 0 unless a pin
can't be read from its file. Move a pin with scripts/bump-pin.py; the crates
and npm packages move with Dependabot's monthly pull requests (§9.1).

The weekly audit workflow (.github/workflows/audit.yml) runs it with
--summary "$GITHUB_STEP_SUMMARY", which appends the report as Markdown.

Usage: scripts/check-pins.py [--summary FILE] [--skip-packages]
"""

import argparse
import dataclasses
import json
import pathlib
import re
import shutil
import subprocess
import sys
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"

# A plain release number; pre-releases (rc, beta, dev) don't match.
RELEASE = re.compile(r"^\d+(\.\d+)*$")


@dataclasses.dataclass(frozen=True)
class Pin:
    """A pinned dependency: where its version is and where releases come from.

    `pattern` finds the pinned version (its `version` group) in `file`.
    `upstream` is "github:OWNER/REPO" (release tags, after `tag_prefix`),
    "pypi:NAME" or "ffmpeg". bump-pin.py uses the rest: `fetch_id` names
    the `anomp_fetch_declare` to rewrite, `url` is the tarball's URL
    template ({version}, or {commit} for a commit's archive), and `label`
    is how the docs name a version ("JUCE 9.0.2").
    """

    name: str
    file: str
    pattern: str
    upstream: str
    tag_prefix: str = ""
    fetch_id: str = ""
    url: str = ""
    label: str = ""


PINS = [
    Pin(
        "juce",
        "CMakeLists.txt",
        r"# JUCE (?P<version>[\d.]+)\n",
        "github:juce-framework/JUCE",
        fetch_id="JUCE",
        url="https://github.com/juce-framework/JUCE/archive/{commit}.tar.gz",
        label="JUCE {version}",
    ),
    Pin(
        "catch2",
        "CMakeLists.txt",
        r"# Catch2 v(?P<version>[\d.]+)\n",
        "github:catchorg/Catch2",
        tag_prefix="v",
        fetch_id="Catch2",
        url="https://github.com/catchorg/Catch2/archive/{commit}.tar.gz",
        label="Catch2 v{version}",
    ),
    Pin(
        "taglib",
        "cmake/TagLib.cmake",
        r"releases/download/v(?P<version>[\d.]+)/",
        "github:taglib/taglib",
        tag_prefix="v",
        fetch_id="taglib",
        url="https://github.com/taglib/taglib/releases/download/v{version}/taglib-{version}.tar.gz",
        label="TagLib {version}",
    ),
    Pin(
        "signalsmith-stretch",
        "cmake/Signalsmith.cmake",
        r"signalsmith-stretch/archive/refs/tags/(?P<version>[\d.]+)\.tar\.gz",
        "github:Signalsmith-Audio/signalsmith-stretch",
        fetch_id="signalsmith_stretch",
        url="https://github.com/Signalsmith-Audio/signalsmith-stretch/archive/refs/tags/{version}.tar.gz",
        label="Signalsmith Stretch {version}",
    ),
    Pin(
        "signalsmith-linear",
        "cmake/Signalsmith.cmake",
        r"linear/archive/refs/tags/(?P<version>[\d.]+)\.tar\.gz",
        "github:Signalsmith-Audio/linear",
        fetch_id="signalsmith_linear",
        url="https://github.com/Signalsmith-Audio/linear/archive/refs/tags/{version}.tar.gz",
    ),
    Pin(
        "ffmpeg",
        "scripts/build-ffmpeg.sh",
        r'FFMPEG_VERSION="(?P<version>[\d.]+)"',
        "ffmpeg",
        url="https://ffmpeg.org/releases/ffmpeg-{version}.tar.xz",
        label="FFmpeg {version}",
    ),
    Pin(
        "clang-format",
        "scripts/format-cpp.py",
        r'CLANG_FORMAT_VERSION = "(?P<version>[\d.]+)"',
        "pypi:clang-format",
    ),
    Pin(
        "clang-tidy",
        "scripts/lint-cpp.py",
        r'CLANG_TIDY_VERSION = "(?P<version>[\d.]+)"',
        "pypi:clang-tidy",
    ),
    Pin("ruff", "scripts/format-python.py", r'RUFF_VERSION = "(?P<version>[\d.]+)"', "pypi:ruff"),
    Pin(
        "pytest",
        "scripts/test-python.py",
        r'PYTEST_VERSION = "(?P<version>[\d.]+)"',
        "pypi:pytest",
    ),
]


class PinError(Exception):
    pass


def by_name(name, pins=PINS):
    for pin in pins:
        if pin.name == name:
            return pin
    raise PinError(f"no pin named {name!r} (pins: {', '.join(pin.name for pin in pins)})")


def key(version):
    """A release number as a tuple of ints, for comparing."""
    return tuple(int(part) for part in version.split("."))


def pinned(pin, root=REPO_ROOT):
    """The version `pin` is at, read from its file."""
    match = re.search(pin.pattern, (root / pin.file).read_text())
    if match is None:
        raise PinError(f"{pin.name}: no version found in {pin.file}")
    return match.group("version")


# Upstream. The network is reached only through these two, which the tests
# replace.


def fetch(url):
    """The body at `url`, as text."""
    request = urllib.request.Request(url, headers={"User-Agent": "ano-mp check-pins"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8", "replace")


def run(command, cwd=REPO_ROOT):
    """Runs `command`; returns (exit status, stdout, stderr), or None if it
    isn't installed."""
    if shutil.which(command[0]) is None:
        return None
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    return result.returncode, result.stdout, result.stderr


def tags(ls_remote_output, prefix):
    """The release versions among `git ls-remote --tags` output's tags."""
    found = set()
    for line in ls_remote_output.splitlines():
        ref = line.split("\t")[-1].removesuffix("^{}")
        name = ref.removeprefix("refs/tags/")
        if name.startswith(prefix) and RELEASE.match(name[len(prefix) :]):
            found.add(name[len(prefix) :])
    return found


def ffmpeg_releases(listing):
    """The versions in ffmpeg.org/releases/'s listing of tarballs."""
    return set(re.findall(r'href="ffmpeg-(\d+(?:\.\d+)+)\.tar\.xz"', listing))


def latest(pin, fetch=fetch, run=run):
    """The newest release upstream has for `pin`."""
    kind, _, where = pin.upstream.partition(":")
    if kind == "github":
        result = run(["git", "ls-remote", "--tags", f"https://github.com/{where}"])
        if result is None or result[0] != 0:
            raise PinError(f"git ls-remote failed for {where}")
        versions = tags(result[1], pin.tag_prefix)
    elif kind == "pypi":
        versions = {json.loads(fetch(f"https://pypi.org/pypi/{where}/json"))["info"]["version"]}
    elif kind == "ffmpeg":
        versions = ffmpeg_releases(fetch("https://ffmpeg.org/releases/"))
    else:
        raise PinError(f"{pin.name}: unknown upstream {pin.upstream}")
    versions = {version for version in versions if RELEASE.match(version)}
    if not versions:
        raise PinError(f"{pin.name}: no releases found upstream")
    return max(versions, key=key)


@dataclasses.dataclass(frozen=True)
class Row:
    name: str
    pinned: str
    latest: str
    status: str


def pin_rows(pins=PINS, root=REPO_ROOT, fetch=fetch, run=run):
    rows = []
    for pin in pins:
        current = pinned(pin, root)
        try:
            newest = latest(pin, fetch, run)
        except (PinError, OSError, ValueError, KeyError) as error:  # reported, not fatal
            rows.append(Row(pin.name, current, "?", f"lookup failed: {error}"))
            continue
        if key(newest) > key(current):
            status = "update available"
        else:
            status = "up to date"
        rows.append(Row(pin.name, current, newest, status))
    return rows


def cargo_updates(stderr):
    """`cargo update --dry-run`'s "Updating a v1 -> v2" lines, as text."""
    return [
        line.strip().removeprefix("Updating ").strip()
        for line in stderr.splitlines()
        if re.match(r"^\s*Updating \S+ v\S+ -> v\S+", line)
    ]


def npm_outdated(stdout):
    """`npm outdated --json`'s packages, as "name current -> latest"."""
    if not stdout.strip():
        return []
    packages = json.loads(stdout)
    return [
        f"{name} {info.get('current', 'missing')} -> {info.get('latest', '?')}"
        for name, info in sorted(packages.items())
    ]


def package_lines(run=run):
    """The crates and npm packages with newer versions, by tool."""
    sections = {}
    cargo = run(["cargo", "update", "--dry-run"], cwd=APP / "src-tauri")
    sections["cargo update --dry-run"] = (
        cargo_updates(cargo[2])
        if cargo and cargo[0] == 0
        else ["(cargo update failed or cargo missing)"]
    )
    npm = run(["npm", "outdated", "--json"], cwd=APP)
    # npm outdated exits 1 when anything is outdated.
    try:
        sections["npm outdated"] = npm_outdated(npm[1]) if npm else ["(npm missing)"]
    except json.JSONDecodeError:
        sections["npm outdated"] = ["(npm outdated failed)"]
    return sections


def text_report(rows, sections):
    width = max(len(row.name) for row in rows)
    lines = [f"{row.name:<{width}}  {row.pinned:<10} {row.latest:<10} {row.status}" for row in rows]
    for title, entries in sections.items():
        lines.append(f"\n{title}: {len(entries) if entries else 'nothing'} to update")
        lines.extend(f"  {entry}" for entry in entries)
    return "\n".join(lines)


def markdown_report(rows, sections):
    lines = ["## Pins", "", "| Pin | Pinned | Latest | Status |", "|---|---|---|---|"]
    lines += [f"| {row.name} | {row.pinned} | {row.latest} | {row.status} |" for row in rows]
    for title, entries in sections.items():
        lines += ["", f"### `{title}`", ""]
        lines += [f"- {entry}" for entry in entries] or ["Nothing to update."]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description="Report pins with newer releases upstream.")
    parser.add_argument("--summary", type=pathlib.Path, help="append the report as Markdown here")
    parser.add_argument(
        "--skip-packages", action="store_true", help="leave out cargo update and npm outdated"
    )
    args = parser.parse_args()

    try:
        rows = pin_rows()
    except PinError as error:
        sys.exit(f"check-pins: {error}")
    sections = {} if args.skip_packages else package_lines()
    print(text_report(rows, sections))
    if args.summary:
        with args.summary.open("a") as summary:
            summary.write(markdown_report(rows, sections))
    return 0


if __name__ == "__main__":
    sys.exit(main())
