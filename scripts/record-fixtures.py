#!/usr/bin/env python3
"""Re-records the online services' responses the Rust tests use (PLAN.md §9.1, §9.2 M3).

The fixtures are in app/src-tauri/src/metadata/fixtures/, and its
manifest.json says how each was made: a URL and the rules that trim the
response, or `made_by_hand` with why it can't be fetched. It fetches each
URL at one request a second with the app's User-Agent (the version from
CMakeLists.txt, the contact from metadata/http.rs), trims it and writes
it as JSON indented by two spaces. With --check it writes nothing and
shows how each file would change instead, exiting 1 if any would: a
changed field means a parser may need work. Run `cargo test` after
writing.

A rule has up to three parts, applied in this order:
- "select": {"path": {"by": [field, ...], "items": [[value, ...], ...]}}
  keeps, in the order listed, the items of the list at `path` whose
  fields (dotted paths, like "url.id") have the values listed; an item no
  longer upstream is reported, not invented.
- "keep": a nested object of the keys to keep (true for a whole value),
  applied to every item of a list on the way.
- "set": {"path": value} replaces a value, for counts that must match a
  selection and for text the tests mustn't copy (Wikipedia's is CC BY-SA).
Paths are keys joined by dots; a list on the way applies the rest to each
item.

Usage: scripts/record-fixtures.py [--check] [--only FILE ...]
"""

import argparse
import copy
import difflib
import json
import pathlib
import re
import sys
import time
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = REPO_ROOT / "app" / "src-tauri" / "src" / "metadata" / "fixtures"
MANIFEST = FIXTURES / "manifest.json"
HTTP_RS = pathlib.Path("app", "src-tauri", "src", "metadata", "http.rs")

# MusicBrainz allows one request a second; the others allow more.
INTERVAL = 1.0


class RecordError(Exception):
    pass


def user_agent(root=REPO_ROOT):
    """The app's User-Agent, as metadata/http.rs's user_agent() builds it."""
    version = re.search(
        r"project\s*\(\s*\S+\s+VERSION\s+([\d.]+)", (root / "CMakeLists.txt").read_text()
    )
    contact = re.search(r'const CONTACT: &str = "([^"]+)";', (root / HTTP_RS).read_text())
    if version is None or contact is None:
        raise RecordError("can't build the User-Agent from CMakeLists.txt and http.rs")
    return f"ano-mp/{version.group(1)} ( {contact.group(1)} )"


# Trimming.


def lookup(value, path):
    """The value at dotted `path` in `value`, or None."""
    for key in path.split("."):
        if not isinstance(value, dict) or key not in value:
            return None
        value = value[key]
    return value


def at_path(value, path, change):
    """Calls `change(container, key)` where `path` ends, through every list
    on the way."""
    if isinstance(value, list):
        for item in value:
            at_path(item, path, change)
        return
    if not isinstance(value, dict):
        return
    key, _, rest = path.partition(".")
    if key not in value:
        return
    if rest:
        at_path(value[key], rest, change)
    else:
        change(value, key)


def select(value, path, rule, missing):
    """Keeps the listed items of the list at `path`, in the listed order;
    appends to `missing` those upstream no longer has."""
    by, wanted = rule["by"], [tuple(item) for item in rule["items"]]

    def change(container, key):
        items = container[key]
        if not isinstance(items, list):
            return
        found = {}
        for item in items:
            found.setdefault(tuple(lookup(item, field) for field in by), item)
        container[key] = [found[item] for item in wanted if item in found]
        missing.extend(f"{path} {list(item)}" for item in wanted if item not in found)

    at_path(value, path, change)


def keep(value, shape):
    """`value` with only the keys in `shape`."""
    if shape is True:
        return value
    if isinstance(value, list):
        return [keep(item, shape) for item in value]
    if not isinstance(value, dict):
        return value
    return {key: keep(item, shape[key]) for key, item in value.items() if key in shape}


def set_value(value, path, new):
    at_path(value, path, lambda container, key: container.__setitem__(key, copy.deepcopy(new)))


def trim(response, rule):
    """`response` (parsed JSON) trimmed by `rule`; returns it and the
    selected items upstream no longer has."""
    value = copy.deepcopy(response)
    missing = []
    for path, selection in rule.get("select", {}).items():
        select(value, path, selection, missing)
    if "keep" in rule:
        value = keep(value, rule["keep"])
    for path, new in rule.get("set", {}).items():
        set_value(value, path, new)
    return value, missing


def dumps(value):
    return json.dumps(value, indent=2, ensure_ascii=False) + "\n"


# The manifest.


def load_manifest(path=MANIFEST):
    return json.loads(path.read_text())["fixtures"]


def coverage(manifest, folder=FIXTURES):
    """(files with no manifest entry, entries with no file)."""
    files = {
        str(path.relative_to(folder))
        for path in folder.rglob("*")
        if path.is_file() and path.name != "manifest.json" and not path.name.startswith(".")
    }
    return sorted(files - set(manifest)), sorted(set(manifest) - files)


def problems(manifest):
    """Entries that are neither fetched nor made by hand, or both."""
    found = []
    for name, entry in manifest.items():
        if ("url" in entry) == ("made_by_hand" in entry):
            found.append(f"{name}: needs either a url or made_by_hand")
        if "url" in entry and not entry["url"].startswith("https://"):
            found.append(f"{name}: not an https URL")
        unknown = set(entry) - {"url", "select", "keep", "set", "made_by_hand"}
        if unknown:
            found.append(f"{name}: unknown rule {', '.join(sorted(unknown))}")
    return found


# The network, reached only through these.


class Throttle:
    """At most one request per `interval` seconds."""

    def __init__(self, interval=INTERVAL, clock=time.monotonic, sleep=time.sleep):
        self.interval, self.clock, self.sleep = interval, clock, sleep
        self.last = None

    def wait(self):
        if self.last is not None:
            remaining = self.last + self.interval - self.clock()
            if remaining > 0:
                self.sleep(remaining)
        self.last = self.clock()


def fetch(url, agent):
    request = urllib.request.Request(
        url, headers={"User-Agent": agent, "Accept": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8")


def record(manifest, names, agent, fetch=fetch, throttle=None, out=print):
    """{file: new text} for each fetched entry in `names`."""
    throttle = throttle or Throttle()
    recorded = {}
    for name in names:
        entry = manifest[name]
        if "made_by_hand" in entry:
            out(f"{name}: made by hand ({entry['made_by_hand']})")
            continue
        throttle.wait()
        value, missing = trim(json.loads(fetch(entry["url"], agent)), entry)
        for item in missing:
            out(f"{name}: no longer upstream: {item}")
        recorded[name] = dumps(value)
    return recorded


def main():
    parser = argparse.ArgumentParser(description="Re-record the metadata services' fixtures.")
    parser.add_argument("--check", action="store_true", help="show the changes, write nothing")
    parser.add_argument("--only", action="append", default=[], metavar="FILE")
    args = parser.parse_args()

    manifest = load_manifest()
    uncovered, absent = coverage(manifest)
    errors = problems(manifest)
    errors += [f"{name}: not in manifest.json" for name in uncovered]
    errors += [f"{name}: in manifest.json, but no such file" for name in absent]
    errors += [f"{name}: not in manifest.json" for name in args.only if name not in manifest]
    if errors:
        sys.exit("record-fixtures: " + "\n  ".join(errors))

    names = args.only or sorted(manifest)
    try:
        recorded = record(manifest, names, user_agent())
    except (OSError, ValueError, RecordError) as error:
        sys.exit(f"record-fixtures: {error}")

    changed = 0
    for name, text in recorded.items():
        path = FIXTURES / name
        old = path.read_text()
        if old == text:
            continue
        changed += 1
        if args.check:
            sys.stdout.writelines(
                difflib.unified_diff(
                    old.splitlines(True), text.splitlines(True), f"a/{name}", f"b/{name}"
                )
            )
        else:
            path.write_text(text)
            print(f"{name}: recorded")
    print(f"record-fixtures: {changed} of {len(recorded)} recorded files changed")
    return 1 if args.check and changed else 0


if __name__ == "__main__":
    sys.exit(main())
