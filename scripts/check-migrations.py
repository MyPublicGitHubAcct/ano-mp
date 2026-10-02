#!/usr/bin/env python3
"""Checks the library DB migrations (app/src-tauri/src/library/migrations/).

- The files are numbered from 001 with no gaps or repeats.
- `MIGRATIONS` in library/db.rs lists each file once, in order.
- No migration that existed at the latest release tag has changed since
  (skipped while there are no tags, before the first release).
- A warning, which doesn't fail the check, for a migration after
  FTS_REVIEWED_UP_TO that alters `tracks`, `artists` or `albums` without
  touching the search triggers. Columns the search indexes don't cover need
  no trigger change: say so with a comment starting `-- fts:` in the file.

Lists every problem it finds and exits non-zero if there are any. Changes
nothing.

Usage: scripts/check-migrations.py
"""

import argparse
import pathlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
LIBRARY = REPO_ROOT / "app" / "src-tauri" / "src" / "library"
MIGRATIONS_DIR = LIBRARY / "migrations"
DB_RS = LIBRARY / "db.rs"

# Migrations up to this number were checked against the search triggers by
# hand when this script was written.
FTS_REVIEWED_UP_TO = 9

FILE_NAME = re.compile(r"^(\d{3})_[a-z0-9_]+\.sql$")
INDEXED_TABLES = ("tracks", "artists", "albums")


def numbering_problems(names):
    """Problems with the numbering of the migration file names."""
    problems = []
    numbers = []
    for name in sorted(names):
        match = FILE_NAME.match(name)
        if match is None:
            problems.append(f"{name}: not named NNN_lower_case.sql")
            continue
        numbers.append(int(match.group(1)))
    for expected, number in enumerate(sorted(numbers), start=1):
        if number != expected:
            problems.append(f"migration {expected:03} missing or repeated (found {number:03})")
            break
    return problems


def listed_migrations(db_rs_text):
    """The file names in `MIGRATIONS`, in order."""
    match = re.search(r"const MIGRATIONS[^=]*=\s*&\[(.*?)\];", db_rs_text, re.DOTALL)
    if match is None:
        return None
    return re.findall(r'include_str!\("migrations/([^"]+)"\)', match.group(1))


def listing_problems(names, listed):
    if listed is None:
        return ["db.rs: MIGRATIONS not found"]
    problems = []
    for name in sorted(set(listed)):
        if listed.count(name) > 1:
            problems.append(f"{name}: listed more than once in MIGRATIONS")
    for name in sorted(set(names) - set(listed)):
        problems.append(f"{name}: not listed in MIGRATIONS")
    for name in sorted(set(listed) - set(names)):
        problems.append(f"{name}: listed in MIGRATIONS but doesn't exist")
    if not problems and listed != sorted(listed):
        problems.append("MIGRATIONS is not in numeric order")
    return problems


def touches_indexed_table(sql):
    tables = "|".join(INDEXED_TABLES)
    return re.search(
        rf"\bALTER\s+TABLE\s+({tables})\b|\bCREATE\s+TABLE\s+({tables})_new\b",
        sql,
        re.IGNORECASE,
    )


def fts_warnings(migrations, reviewed_up_to=FTS_REVIEWED_UP_TO):
    """Warnings for {name: sql} migrations that may need trigger changes."""
    warnings = []
    for name, sql in sorted(migrations.items()):
        match = FILE_NAME.match(name)
        if match is None or int(match.group(1)) <= reviewed_up_to:
            continue
        if not touches_indexed_table(sql):
            continue
        if re.search(r"\bTRIGGER\b", sql, re.IGNORECASE) or re.search(
            r"^\s*--\s*fts:", sql, re.MULTILINE
        ):
            continue
        warnings.append(
            f"{name}: alters {'/'.join(INDEXED_TABLES)} without touching the search "
            "triggers; update both sets (word and trigram) or add a '-- fts:' comment"
        )
    return warnings


def git(*args):
    result = subprocess.run(
        ["git", *args], cwd=REPO_ROOT, capture_output=True, text=True, check=False
    )
    return result.stdout if result.returncode == 0 else None


def latest_release_tag():
    """The newest tag reachable from HEAD, or None before the first release."""
    output = git("describe", "--tags", "--abbrev=0")
    return output.strip() if output else None


def shipped_changes(tag):
    """Migrations that existed at `tag` and differ from it now."""
    relative = MIGRATIONS_DIR.relative_to(REPO_ROOT).as_posix()
    output = git("diff", "--name-only", "--diff-filter=MDR", tag, "--", relative)
    if output is None:
        return [f"git diff against {tag} failed"]
    return [
        f"{pathlib.PurePosixPath(path).name}: changed since {tag}, where it had shipped"
        for path in output.split()
    ]


def main():
    parser = argparse.ArgumentParser(description="Check the library DB migrations.")
    parser.parse_args()

    files = {path.name: path for path in MIGRATIONS_DIR.glob("*.sql")}
    problems = numbering_problems(files)
    problems += listing_problems(list(files), listed_migrations(DB_RS.read_text("utf-8")))
    tag = latest_release_tag()
    if tag is not None:
        problems += shipped_changes(tag)
    warnings = fts_warnings({name: path.read_text("utf-8") for name, path in files.items()})

    for warning in warnings:
        print(f"check-migrations: warning: {warning}")
    for problem in problems:
        print(f"check-migrations: {problem}")
    if not problems:
        shipped = f"none changed since {tag}" if tag else "no release tag yet"
        print(f"check-migrations: {len(files)} migrations in order ({shipped})")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
