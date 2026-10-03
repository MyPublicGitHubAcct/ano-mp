#!/usr/bin/env python3
"""Collects a release's files: the checked bundle, checksums and notes
(PLAN.md §8.2, §8.7).

From the bundle directory build-app.py reports, into --out (dist/):
- checks the .app with check-bundle.py (--signed when it should be
  Developer ID-signed), and refuses to go on if it fails;
- copies the DMG and zips the .app (`ditto`, as Apple's tools expect),
  named ano-mp_<version>_macos-universal.{dmg,app.zip};
- writes SHA256SUMS over them (`shasum -a 256 -c SHA256SUMS` checks it);
- cuts the version's section of CHANGELOG.md into RELEASE_NOTES.md, and
  fails if it has none or it's empty (--unreleased: the Unreleased
  section, for a trial run before the version's notes are written);
- with --tag, first fails unless the tag is v<version> (version.py).

The Tauri updater's manifest is added here once the updater is (§8.2,
after the distribution decision in §8.1).

Usage: scripts/release.py BUNDLE_DIR [--out DIR] [--tag vX.Y.Z] [--unreleased]
                          [--signed] [--native]
       scripts/release.py --notes VERSION   (print the notes only)
"""

import argparse
import hashlib
import importlib.util
import pathlib
import re
import shutil
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
CHANGELOG = REPO_ROOT / "CHANGELOG.md"
SCRIPTS = REPO_ROOT / "scripts"


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def notes(changelog, version):
    """The body of CHANGELOG.md's `## [version]` section, or None if it has
    none or it's empty."""
    heading = re.compile(rf"^## \[{re.escape(version)}\][^\n]*\n", re.MULTILINE)
    match = heading.search(changelog)
    if match is None:
        return None
    following = re.search(r"^## ", changelog[match.end() :], re.MULTILINE)
    end = match.end() + following.start() if following else len(changelog)
    body = changelog[match.end() : end].strip()
    return body + "\n" if body else None


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def checksums(paths):
    """SHA256SUMS's text: `<hex>  <name>` lines, sorted by name."""
    return "".join(f"{sha256(path)}  {path.name}\n" for path in sorted(paths, key=lambda p: p.name))


def find_one(directory, pattern):
    found = sorted(directory.glob(pattern))
    if len(found) != 1:
        raise SystemExit(f"release: expected one {pattern} in {directory}, found {len(found)}")
    return found[0]


def main():
    parser = argparse.ArgumentParser(description="Collect a release's files.")
    parser.add_argument("bundle_dir", nargs="?", type=pathlib.Path, help="Tauri's bundle dir")
    parser.add_argument("--out", type=pathlib.Path, default=REPO_ROOT / "dist")
    parser.add_argument("--signed", action="store_true", help="expect a Developer ID signature")
    parser.add_argument("--native", action="store_true", help="this Mac's architecture only")
    parser.add_argument("--notes", metavar="VERSION", help="print VERSION's notes and stop")
    parser.add_argument("--tag", help="the release tag, which must name the version")
    parser.add_argument("--unreleased", action="store_true", help="notes from [Unreleased]")
    args = parser.parse_args()

    changelog = CHANGELOG.read_text(encoding="utf-8")
    if args.notes:
        text = notes(changelog, args.notes)
        if text is None:
            print(f"release: CHANGELOG.md has no notes for {args.notes}")
            return 1
        print(text, end="")
        return 0
    if args.bundle_dir is None:
        parser.error("BUNDLE_DIR is needed")

    versions = load("version")
    found = versions.versions(REPO_ROOT)
    problems = versions.problems(found, args.tag)
    for problem in problems:
        print(f"release: {problem}")
    if problems:
        return 1
    version = found["CMakeLists.txt"]
    section = "Unreleased" if args.unreleased else version
    text = notes(changelog, section)
    if text is None:
        print(f"release: CHANGELOG.md has no `## [{section}]` section with notes")
        return 1

    app = find_one(args.bundle_dir / "macos", "*.app")
    problems = load("check-bundle").check(app, native=args.native, signed=args.signed)
    for problem in problems:
        print(f"release: check-bundle: {problem}")
    if problems:
        return 1

    arch = "native" if args.native else "universal"
    stem = f"ano-mp_{version}_macos-{arch}"
    args.out.mkdir(parents=True, exist_ok=True)
    dmg = args.out / f"{stem}.dmg"
    shutil.copy2(find_one(args.bundle_dir / "dmg", "*.dmg"), dmg)
    archive = args.out / f"{stem}.app.zip"
    archive.unlink(missing_ok=True)
    zip_command = ["ditto", "-c", "-k", "--keepParent", str(app), str(archive)]
    subprocess.run(zip_command, check=True)
    (args.out / "SHA256SUMS").write_text(checksums([dmg, archive]), encoding="utf-8")
    (args.out / "RELEASE_NOTES.md").write_text(text, encoding="utf-8")
    print(f"release: {version} in {args.out}: {dmg.name}, {archive.name}, SHA256SUMS, notes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
