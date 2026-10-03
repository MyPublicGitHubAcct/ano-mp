#!/usr/bin/env python3
"""Checks the dependencies for published vulnerabilities (PLAN.md §9.1, §9.2 M5).

Three checks, each run even after one fails:
- `cargo deny check` in app/src-tauri (RustSec advisories, and deny.toml's
  licences, bans and sources; H7);
- `npm audit` in app/ (the npm registry's advisories, from
  package-lock.json), less those NPM_IGNORED lists with their reasons, as
  deny.toml's `ignore` does for RustSec's;
- the FFmpeg pin (scripts/build-ffmpeg.sh) against ffmpeg.org's security
  page: it fails when the page lists vulnerabilities fixed by a newer
  release of the pinned branch, and notes those fixed only in a newer
  branch or not yet released.
TagLib's and JUCE's release notes have no feed to check; read them with
check-pins.py's report (§9.1).

The weekly audit workflow (.github/workflows/audit.yml) runs it; advisories
are published without any change here, so a quiet week can still fail.
Exits non-zero if any check failed.

Usage: scripts/audit-deps.py [--only cargo|npm|ffmpeg]
"""

import argparse
import html
import json
import pathlib
import re
import shutil
import subprocess
import sys
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"
SECURITY_PAGE = "https://ffmpeg.org/security.html"

# npm advisories allowed, each with its reason (npm audit has no ignore list).
NPM_IGNORED = {
    # cookie < 0.7.0 through @sveltejs/kit 2 (low). Kit's cookie handling
    # runs in a SvelteKit server; adapter-static prerenders the app, and the
    # webview never runs it. Fixed in Kit 3, a major update (§9.1's npm row).
    "GHSA-pxg6-pf52-xh8x": "SvelteKit 2's server-side cookie parsing, never run",
}


def fetch(url):
    """The body at `url`, as text (the tests replace it)."""
    request = urllib.request.Request(url, headers={"User-Agent": "ano-mp audit-deps"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8", "replace")


def run(command, cwd):
    """Runs `command`, streaming its output; returns whether it passed."""
    if shutil.which(command[0]) is None:
        print(f"audit-deps: {command[0]} not found (scripts/doctor.py)")
        return False
    return subprocess.run(command, cwd=cwd, check=False).returncode == 0


def npm_advisories(audit_json):
    """{advisory id: "package (severity): title"} from `npm audit --json`."""
    found = {}
    for package, entry in json.loads(audit_json).get("vulnerabilities", {}).items():
        for via in entry.get("via", []):
            if isinstance(via, dict):  # a string names the package it comes through
                advisory = via.get("url", "").rsplit("/", 1)[-1] or str(via.get("source"))
                found[advisory] = f"{package} ({via.get('severity')}): {via.get('title')}"
    return found


def check_npm(run_json=None, out=print):
    """npm audit, failing on any advisory NPM_IGNORED doesn't list."""
    if run_json is None:
        if shutil.which("npm") is None:
            out("audit-deps: npm not found (scripts/doctor.py)")
            return False
        result = subprocess.run(
            ["npm", "audit", "--json"], cwd=APP, capture_output=True, text=True, check=False
        )
        run_json = result.stdout
    try:
        advisories = npm_advisories(run_json)
    except json.JSONDecodeError:
        out("audit-deps: npm audit gave no report")
        return False
    failed = False
    for advisory, summary in sorted(advisories.items()):
        if advisory in NPM_IGNORED:
            out(f"allowed  {advisory} {summary}: {NPM_IGNORED[advisory]}")
        else:
            out(f"FAIL     {advisory} {summary}")
            failed = True
    for advisory in sorted(set(NPM_IGNORED) - set(advisories)):
        out(f"note     {advisory} no longer reported: remove it from NPM_IGNORED")
    if not advisories:
        out("ok       npm audit: no advisories")
    return not failed


def key(version):
    return tuple(int(part) for part in version.split("."))


def ffmpeg_pin(root=REPO_ROOT):
    text = (root / "scripts" / "build-ffmpeg.sh").read_text()
    return re.search(r'FFMPEG_VERSION="([\d.]+)"', text).group(1)


def security_fixes(page):
    """{release: [CVE ids]} from the security page, which heads each
    release's fixes with an <h3>, under an <h2> per branch ("FFmpeg 9.0")
    and one for git master (key "master")."""
    fixes = {}
    current = None
    for line in page.splitlines():
        heading = re.search(r"<h([23])[^>]*>(.*?)</h\1>", line)
        if heading:
            title = html.unescape(re.sub(r"<[^>]+>", "", heading.group(2))).strip()
            if heading.group(1) == "2":
                current = "master" if "master" in title.lower() else None
            elif re.fullmatch(r"\d+(\.\d+)+", title):
                current = title
            else:
                current = None
            if current is not None:
                fixes.setdefault(current, [])
            continue
        if current is not None:
            for cve in re.findall(r"CVE-\d{4}-\d+", line):
                if cve not in fixes[current]:
                    fixes[current].append(cve)
    return fixes


def ffmpeg_findings(pin, fixes):
    """(failures, notes) for `pin` against the page's fixes."""
    branch = key(pin)[:2]
    failures, notes = [], []
    for release, cves in sorted(fixes.items(), key=lambda item: (item[0] != "master", item[0])):
        if not cves:
            continue
        if release == "master":
            notes.append(f"{len(cves)} fixed in git master, not yet in a release")
            continue
        version = key(release)
        if version[:2] == branch and version > key(pin):
            failures.append(f"FFmpeg {release} fixes {', '.join(cves)}; the pin is {pin}")
        elif version[:2] > branch:
            notes.append(f"FFmpeg {release} (a newer branch) fixes {len(cves)}")
    return failures, notes


def check_ffmpeg(fetch=fetch, root=REPO_ROOT, out=print):
    pin = ffmpeg_pin(root)
    try:
        fixes = security_fixes(fetch(SECURITY_PAGE))
    except OSError as error:
        out(f"audit-deps: couldn't read {SECURITY_PAGE}: {error}")
        return False
    if not any(release != "master" for release in fixes):
        out(f"audit-deps: no releases found on {SECURITY_PAGE}; has the page changed?")
        return False
    failures, notes = ffmpeg_findings(pin, fixes)
    for line in failures:
        out(f"FAIL  {line}")
    for line in notes:
        out(f"note  {line}")
    if not failures:
        out(f"ok    FFmpeg {pin}: no newer {'.'.join(pin.split('.')[:2])}.x release lists fixes")
    return not failures


CHECKS = {
    "cargo": lambda: run(["cargo", "deny", "check", "--hide-inclusion-graph"], APP / "src-tauri"),
    "npm": lambda: check_npm(),
    "ffmpeg": lambda: check_ffmpeg(),
}


def main():
    parser = argparse.ArgumentParser(description="Check the dependencies for vulnerabilities.")
    parser.add_argument("--only", choices=sorted(CHECKS), help="run one check")
    args = parser.parse_args()

    failed = []
    for name, check in CHECKS.items():
        if args.only and name != args.only:
            continue
        print(f"\n==> {name}", flush=True)
        if not check():
            failed.append(name)
    print()
    if failed:
        print(f"audit-deps: failed: {', '.join(failed)}")
        return 1
    print("audit-deps: nothing found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
