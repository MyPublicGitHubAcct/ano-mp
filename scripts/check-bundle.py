#!/usr/bin/env python3
"""Checks a built macOS app bundle before it is released (PLAN.md §8.3).

- Architectures: the executable and every FFmpeg dylib hold arm64 and
  x86_64 (--native: this Mac's, at least).
- FFmpeg: Contents/Frameworks holds exactly tauri.conf.json's frameworks;
  each has an @rpath install name; the executable finds them through an
  @executable_path/../Frameworks rpath; nothing links a library by a path
  outside the bundle and the OS (the build tree, Homebrew).
- Signatures: `codesign --verify --deep --strict` passes; the executable
  and each dylib are signed, ad-hoc or (--signed) by one Developer ID team
  with the hardened runtime on.
- Entitlements: the signature's are exactly Entitlements.plist's.
- Contents: THIRD_PARTY_NOTICES is the committed one; the version is
  version.py's; the minimum macOS is tauri.conf.json's.

Lists every problem it finds and exits non-zero if there are any. Changes
nothing. release.py runs it on the bundle it releases.

Usage: scripts/check-bundle.py PATH/ano-mp.app [--native] [--signed]
"""

import argparse
import importlib.util
import json
import pathlib
import platform
import plistlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
CRATE = REPO_ROOT / "app" / "src-tauri"
UNIVERSAL = {"arm64", "x86_64"}
# Where a bundled binary may link a library from, besides @rpath.
SYSTEM_PREFIXES = ("/usr/lib/", "/System/Library/")
RPATH = "@executable_path/../Frameworks"
RUNTIME_FLAG = 0x10000


def run(command):
    """A tool's output (stdout and stderr together); a thin wrapper the tests
    replace. Raises CalledProcessError if it fails."""
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    return result.stdout + result.stderr


def parse_archs(output):
    return set(output.split())


def parse_install_name(output):
    """`otool -D`: the path line(s) after the file name; the first one."""
    lines = [line.strip() for line in output.splitlines()[1:] if line.strip()]
    return lines[0] if lines else None


def parse_linked(output):
    """`otool -L`: the libraries linked, for every architecture."""
    linked = set()
    for line in output.splitlines():
        if line.startswith("\t"):
            linked.add(line.strip().split(" (compatibility")[0])
    return linked


def parse_rpaths(output):
    """`otool -l`: LC_RPATH paths."""
    return re.findall(r"cmd LC_RPATH\n\s+cmdsize \d+\n\s+path (.+?) \(offset \d+\)", output)


def parse_signature(output):
    """`codesign -d -vvv`: whether ad-hoc, the hardened runtime flag, the
    team (None if none)."""
    flags = re.search(r"flags=0x([0-9a-f]+)", output)
    team = re.search(r"^TeamIdentifier=(.+)$", output, re.MULTILINE)
    team = team.group(1).strip() if team else None
    return {
        "adhoc": "Signature=adhoc" in output,
        "runtime": bool(flags and int(flags.group(1), 16) & RUNTIME_FLAG),
        "team": None if team in (None, "not set") else team,
    }


def parse_entitlements(output):
    """`codesign -d --entitlements - --xml`: the entitlements dictionary."""
    start = output.find("<?xml")
    end = output.find("</plist>", start)
    if start < 0 or end < 0:
        return {}
    return plistlib.loads(output[start : end + len("</plist>")].encode())


def expected_versions(root=REPO_ROOT):
    """(app version, minimum macOS, framework file names) from the repo."""
    spec = importlib.util.spec_from_file_location("version", root / "scripts" / "version.py")
    version = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(version)
    config = json.loads((root / "app" / "src-tauri" / "tauri.conf.json").read_text())
    macos = config["bundle"]["macOS"]
    frameworks = sorted(pathlib.Path(path).name for path in macos["frameworks"])
    return version.versions(root)["CMakeLists.txt"], macos["minimumSystemVersion"], frameworks


def gather(app, run=run):
    """What the checks need from the bundle, through the tools."""
    contents = app / "Contents"
    info = plistlib.loads((contents / "Info.plist").read_bytes())
    executable = contents / "MacOS" / info["CFBundleExecutable"]
    frameworks = sorted((contents / "Frameworks").glob("*.dylib"))
    binaries = [executable, *frameworks]
    facts = {
        "info": info,
        "frameworks": [path.name for path in frameworks],
        "archs": {path.name: parse_archs(run(["lipo", "-archs", str(path)])) for path in binaries},
        "install_names": {
            path.name: parse_install_name(run(["otool", "-D", str(path)])) for path in frameworks
        },
        "linked": {path.name: parse_linked(run(["otool", "-L", str(path)])) for path in binaries},
        "rpaths": parse_rpaths(run(["otool", "-l", str(executable)])),
        "signatures": {
            path.name: parse_signature(run(["codesign", "-d", "-vvv", str(path)]))
            for path in binaries
        },
        "entitlements": parse_entitlements(
            run(["codesign", "-d", "--entitlements", "-", "--xml", str(app)])
        ),
        "executable": executable.name,
    }
    try:
        run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)])
        facts["verify"] = None
    except subprocess.CalledProcessError as error:
        facts["verify"] = (error.stdout or "") + (error.stderr or "")
    notices = contents / "Resources" / "THIRD_PARTY_NOTICES"
    facts["notices"] = notices.read_text(encoding="utf-8") if notices.is_file() else None
    return facts


def problems(facts, expected, archs, signed):
    """Every problem with the bundle. `expected` holds the repo's version,
    minimum macOS, framework names, entitlements and notices."""
    found = []
    info = facts["info"]
    if info.get("CFBundleShortVersionString") != expected["version"]:
        found.append(
            f"version {info.get('CFBundleShortVersionString')}, expected {expected['version']}"
        )
    if info.get("LSMinimumSystemVersion") != expected["minimum_os"]:
        found.append(
            f"minimum macOS {info.get('LSMinimumSystemVersion')}, expected {expected['minimum_os']}"
        )
    if facts["frameworks"] != expected["frameworks"]:
        found.append(
            f"Contents/Frameworks holds {facts['frameworks']}, expected {expected['frameworks']}"
        )
    for name, has in sorted(facts["archs"].items()):
        if not archs <= has:
            found.append(f"{name}: architectures {sorted(has)}, expected {sorted(archs)}")
    for name, install_name in sorted(facts["install_names"].items()):
        if install_name != f"@rpath/{name}":
            found.append(f"{name}: install name {install_name}, expected @rpath/{name}")
    bundled = set(facts["frameworks"])
    for name, linked in sorted(facts["linked"].items()):
        for library in sorted(linked):
            if library.startswith("@rpath/"):
                if library.removeprefix("@rpath/") not in bundled:
                    found.append(f"{name}: links {library}, which isn't bundled")
            elif not library.startswith(SYSTEM_PREFIXES):
                found.append(f"{name}: links {library} from outside the bundle and the OS")
    if RPATH not in facts["rpaths"]:
        found.append(f"{facts['executable']}: no {RPATH} rpath (has {facts['rpaths']})")
    if facts["verify"] is not None:
        found.append(f"codesign --verify --deep --strict failed: {facts['verify'].strip()}")
    teams = set()
    for name, signature in sorted(facts["signatures"].items()):
        if signed:
            if signature["adhoc"] or signature["team"] is None:
                found.append(f"{name}: not signed with a Developer ID")
            if not signature["runtime"]:
                found.append(f"{name}: hardened runtime off")
            teams.add(signature["team"])
        elif not signature["adhoc"] and signature["team"] is None:
            found.append(f"{name}: not signed")
    if signed and len(teams) > 1:
        found.append(f"signed by more than one team: {sorted(str(team) for team in teams)}")
    if facts["entitlements"] != expected["entitlements"]:
        found.append(
            f"entitlements {sorted(facts['entitlements'])}, "
            f"expected Entitlements.plist's {sorted(expected['entitlements'])}"
        )
    if facts["notices"] is None:
        found.append("Contents/Resources/THIRD_PARTY_NOTICES missing")
    elif facts["notices"] != expected["notices"]:
        found.append("Contents/Resources/THIRD_PARTY_NOTICES isn't the committed one")
    return found


def expected_facts(root=REPO_ROOT):
    version, minimum_os, frameworks = expected_versions(root)
    entitlements = plistlib.loads((root / "app" / "src-tauri" / "Entitlements.plist").read_bytes())
    notices = (root / "THIRD_PARTY_NOTICES").read_text(encoding="utf-8")
    return {
        "version": version,
        "minimum_os": minimum_os,
        "frameworks": frameworks,
        "entitlements": entitlements,
        "notices": notices,
    }


def check(app, native=False, signed=False):
    """The problems with the bundle at `app`."""
    archs = {platform.machine()} if native else UNIVERSAL
    return problems(gather(app), expected_facts(), archs, signed)


def main():
    parser = argparse.ArgumentParser(description="Check a built macOS app bundle.")
    parser.add_argument("app", type=pathlib.Path, help="the .app bundle")
    parser.add_argument("--native", action="store_true", help="expect this Mac's arch only")
    parser.add_argument("--signed", action="store_true", help="expect a Developer ID signature")
    args = parser.parse_args()
    if not (args.app / "Contents" / "Info.plist").is_file():
        print(f"check-bundle: {args.app}: not an app bundle")
        return 1
    found = check(args.app, args.native, args.signed)
    for problem in found:
        print(f"check-bundle: {problem}")
    if found:
        return 1
    kind = "Developer ID" if args.signed else "ad-hoc"
    print(
        f"check-bundle: {args.app.name} passed ({kind}, {'native' if args.native else 'universal'})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
