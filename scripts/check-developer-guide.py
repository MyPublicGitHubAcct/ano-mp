#!/usr/bin/env python3
"""Checks that the developer guide (docs/developer-guide/, PLAN.md Phase 7c
D2) describes the tree as it is.

- The index (README.md) links every page of the guide.
- The repository map (03-repository-map.md) names every top-level folder
  and every module of the four source trees, and nothing that has gone:
  - "## Top-level folders": each tracked top-level folder, as `name/`;
  - a section whose heading names `core/src/` or `effects/src/`: each C++
    unit by its stem (`PlayerEngine` for PlayerEngine.h and .cpp; a
    platform file's suffix, `_apple`, `_none` or `_unsandboxed`, folds into
    its unit's: `FolderAccess`);
  - a section naming `app/src-tauri/src/`: each Rust file, by its path in
    that folder (`library/scanner.rs`);
  - a section naming `app/src/`: each file, by its path in that folder,
    except the generated ones, which are named once as `lib/generated/`.
  Within a section, a `###` heading naming a folder (`lib/components/`)
  makes the names under it relative to that folder.
  An entry is a list item whose names lead it, in backquotes, separated by
  commas or "and": "- `Log`: …" or "- `media.rs`, `shell/dock.rs`: …".

What each entry says is for the reader; paths elsewhere in the guide are
checked by check-docs.py.

Lists every problem it finds and exits non-zero if there are any. Changes
nothing.

Usage: scripts/check-developer-guide.py
"""

import argparse
import pathlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
GUIDE = REPO_ROOT / "docs" / "developer-guide"
INDEX = "README.md"
MAP = "03-repository-map.md"
TOP_LEVEL = "Top-level folders"

# The source trees the map covers, and how a file there becomes a name.
CPP_ROOTS = ("core/src/", "effects/src/")
RUST_ROOT = "app/src-tauri/src/"
FRONTEND_ROOT = "app/src/"
ROOTS = (*CPP_ROOTS, RUST_ROOT, FRONTEND_ROOT)
CPP_EXTENSIONS = (".h", ".cpp", ".mm")
PLATFORM_SUFFIXES = ("_apple", "_none", "_unsandboxed")
FRONTEND_EXTENSIONS = (".ts", ".svelte", ".js", ".json", ".html")
GENERATED = "lib/generated/"

CODE = re.compile(r"`([^`\n]+)`")
# A list item's leading names: code spans joined by commas or "and".
LEADING_NAMES = re.compile(r"^\s*[-*] ((?:`[^`\n]+`(?:,\s*|\s+and\s+|,\s+and\s+)?)+)")


def tracked_files(root=REPO_ROOT):
    """Every file git tracks or would (not ignored), '/'-separated."""
    output = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return sorted(path for path in output.split("\0") if path)


def cpp_unit(name):
    """A C++ file's unit: its stem without a platform suffix."""
    stem = name.rsplit(".", 1)[0]
    for suffix in PLATFORM_SUFFIXES:
        if stem.endswith(suffix):
            return stem[: -len(suffix)]
    return stem


def expected(files):
    """{section: the names the map must give} from the repo's files."""
    names = {TOP_LEVEL: set()}
    for root in ROOTS:
        names[root] = set()
    for path in files:
        if "/" in path:
            names[TOP_LEVEL].add(path.split("/", 1)[0] + "/")
        for root in ROOTS:
            if not path.startswith(root):
                continue
            inner = path[len(root) :]
            if root in CPP_ROOTS:
                if "/" not in inner and inner.endswith(CPP_EXTENSIONS):
                    names[root].add(cpp_unit(inner))
            elif root == RUST_ROOT:
                if inner.endswith(".rs"):
                    names[root].add(inner)
            elif inner.startswith(GENERATED):
                names[root].add(GENERATED)
            elif inner.endswith(FRONTEND_EXTENSIONS):
                names[root].add(inner)
    return names


def heading_code(line):
    """The first code span of a heading, or None."""
    match = CODE.search(line)
    return match.group(1) if match else None


def joined_items(lines):
    """The lines with each list item's continuation lines joined to it."""
    joined = []
    for line in lines:
        continues = line.startswith("  ") and line.strip() and joined
        if continues and re.match(r"^\s*[-*] ", joined[-1]):
            joined[-1] += " " + line.strip()
        else:
            joined.append(line)
    return joined


def listed(map_text):
    """{section: the names the map gives}, sections keyed as `expected`'s."""
    names = {}
    section = None
    prefix = ""
    for line in joined_items(map_text.split("\n")):
        if line.startswith("## "):
            title = line[3:].strip()
            code = heading_code(title)
            if title == TOP_LEVEL:
                section = TOP_LEVEL
            elif code in ROOTS:
                section = code
            else:
                section = None
            prefix = ""
            if section is not None:
                names.setdefault(section, set())
            continue
        if line.startswith("### "):
            code = heading_code(line)
            prefix = code if code and code.endswith("/") else ""
            continue
        if section is None:
            continue
        match = LEADING_NAMES.match(line)
        if match:
            for name in CODE.findall(match.group(1)):
                names[section].add(name if section == TOP_LEVEL else prefix + name)
    return names


def map_problems(want, have):
    problems = []
    for section, names in want.items():
        where = "the top-level folders" if section == TOP_LEVEL else f"`{section}`"
        if section not in have:
            problems.append(f"{MAP} has no section for {where}")
            continue
        problems += [
            f"{MAP} doesn't name {name} in {where}" for name in sorted(names - have[section])
        ]
        problems += [
            f"{MAP} names {name} in {where}, which isn't in the tree"
            for name in sorted(have[section] - names)
        ]
    return problems


def index_problems(pages, index_text):
    linked = set(re.findall(r"\]\(([^)#]+\.md)(?:#[^)]*)?\)", index_text))
    return [f"{INDEX} doesn't link {page}" for page in sorted(pages) if page not in linked]


def problems_in(read, files):
    """Every problem, reading each page with `read(path)`."""
    pages = {path.name: read(path) for path in sorted(GUIDE.glob("*.md"))}
    if INDEX not in pages:
        return [f"docs/developer-guide/{INDEX} is missing"]
    problems = index_problems(set(pages) - {INDEX}, pages[INDEX])
    if MAP not in pages:
        return [*problems, f"docs/developer-guide/{MAP} is missing"]
    return problems + map_problems(expected(files), listed(pages[MAP]))


def main():
    parser = argparse.ArgumentParser(description="Check the developer guide maps the tree.")
    parser.parse_args()
    files = tracked_files()
    problems = problems_in(lambda path: path.read_text("utf-8"), files)
    for problem in problems:
        print(f"check-developer-guide: {problem}")
    if not problems:
        counts = expected(files)
        modules = sum(len(counts[root]) for root in ROOTS)
        print(f"check-developer-guide: the map names {modules} modules and every top-level folder")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
