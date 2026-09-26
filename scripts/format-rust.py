#!/usr/bin/env python3
"""Formats the Rust code in app/src-tauri with rustfmt (via cargo fmt).

Run it after editing Rust; it rewrites files in place. With --check it
changes nothing: it prints the diff and exits non-zero when any file is not
formatted (for CI or a pre-commit hook).

Usage: scripts/format-rust.py [--check]
"""

import argparse
import pathlib
import shutil
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = REPO_ROOT / "app" / "src-tauri" / "Cargo.toml"


def has_rustfmt(cargo):
    result = subprocess.run([cargo, "fmt", "--version"], capture_output=True, check=False)
    return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(description="Format the Rust code with rustfmt.")
    parser.add_argument("--check", action="store_true", help="report only, change nothing")
    args = parser.parse_args()

    cargo = shutil.which("cargo")
    if cargo is None or not has_rustfmt(cargo):
        sys.exit("error: cargo fmt not found; install it with: rustup component add rustfmt")

    command = [cargo, "fmt", "--manifest-path", str(MANIFEST), "--all"]
    if args.check:
        command.append("--check")
    return subprocess.run(command, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
