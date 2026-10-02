#!/usr/bin/env python3
"""Formats the frontend (app/: Svelte, TypeScript, JavaScript, CSS, JSON) with Prettier.

Uses app/.prettierrc.json and app/.prettierignore. Run it after editing the
frontend; it rewrites files in place. With --check it changes nothing: it
lists each unformatted file and exits non-zero when there are any (for CI
or a pre-commit hook).

Prettier's output changes between versions, so it runs the version pinned
in app/package.json (installed by `npm ci` in app/), never a global one.

Usage: scripts/format-frontend.py [--check]
"""

import argparse
import pathlib
import shutil
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"


def prettier_command(npx, check):
    """The command that runs app/'s own Prettier over app/."""
    # --no: never download a Prettier that isn't installed.
    return [npx, "--no", "--", "prettier", "--check" if check else "--write", "."]


def main():
    parser = argparse.ArgumentParser(description="Format the frontend with Prettier.")
    parser.add_argument("--check", action="store_true", help="report only, change nothing")
    args = parser.parse_args()

    npx = shutil.which("npx")
    if npx is None:
        sys.exit("error: npx not found; install Node.js (see .nvmrc)")
    if not (APP / "node_modules" / "prettier").is_dir():
        sys.exit("error: Prettier isn't installed; run `npm ci` in app/")
    return subprocess.run(prettier_command(npx, args.check), cwd=APP, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
