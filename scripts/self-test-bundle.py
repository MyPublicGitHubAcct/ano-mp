#!/usr/bin/env python3
"""Builds a sandboxed app bundle with the self-test in it and runs it
(PLAN.md H14).

Sandbox mistakes show only in a bundle. This builds one for this Mac's
architecture, ad-hoc signed with the app's entitlements as a local build
is, but with the `self-test` Cargo feature, into
app/src-tauri/target/self-test/ (apart from the bundles build-app.py makes).
Then it runs the bundle's executable with `--self-test --require-sandbox`:
the app, sandboxed, writes the core's audio fixtures into its temporary
folder, adds them to a scratch library, scans them, resolves the folder's
bookmark, reads the covers, decodes every file and plays a gapless
hand-off at volume 0 (skipped without an output device, as on CI's
runners). It prints one line per stage and exits non-zero if any failed.

A sandboxed bundle runs in the app's real container
(~/Library/Containers/<identifier>), which on your Mac holds your library.
The self-test only writes (and removes) a folder in the container's
temporary directory and never opens the app's own files, but it still
runs only on CI (CI is set) unless --local is given; check-all.py runs it,
so a local check-all skips it. See "Running a bundle check safely" in
CLAUDE.md.

Usage: scripts/self-test-bundle.py [--local] [--no-build] [--require-audio]
"""

import argparse
import json
import os
import pathlib
import platform
import plistlib
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"
TARGET_DIR = APP / "src-tauri" / "target" / "self-test"
NATIVE = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}
# The fixtures are a few seconds long; the run includes JUCE's start-up.
TIMEOUT_SECONDS = 180


def should_run(environment, local):
    """Whether to run: on CI, or when asked to locally."""
    return local or environment.get("CI", "") not in ("", "0", "false")


def build_command(target):
    return [
        "npm",
        "run",
        "tauri",
        "build",
        "--",
        "--target",
        target,
        "--bundles",
        "app",
        "--features",
        "self-test",
    ]


def build_environment(environment):
    """Ad-hoc signed into the self-test's own target dir."""
    result = dict(environment)
    result["CARGO_TARGET_DIR"] = str(TARGET_DIR)
    # Tauri would sign with this even without the overlay (build-app.py).
    result.pop("APPLE_SIGNING_IDENTITY", None)
    return result


def bundle_path(target, product_name):
    return TARGET_DIR / target / "release" / "bundle" / "macos" / f"{product_name}.app"


def executable(bundle):
    info = plistlib.loads((bundle / "Contents" / "Info.plist").read_bytes())
    return bundle / "Contents" / "MacOS" / info["CFBundleExecutable"]


def run_command(executable_path, require_audio):
    command = [str(executable_path), "--self-test", "--require-sandbox"]
    if require_audio:
        command.append("--require-audio")
    return command


def verdict(output, returncode):
    """The problems with a run's output, or [] if it passed."""
    lines = [line for line in output.splitlines() if line.startswith("self-test: ")]
    problems = [line for line in lines if ": FAILED" in line]
    if not any(line.startswith("self-test: sandbox: ok") for line in lines):
        problems.append("the app didn't report running in the sandbox")
    if "self-test: passed" not in lines:
        problems.append("the self-test didn't finish")
    if returncode != 0:
        problems.append(f"exit status {returncode}")
    return problems


def main():
    parser = argparse.ArgumentParser(description="Build the self-test bundle and run it.")
    parser.add_argument(
        "--local", action="store_true", help="run outside CI (uses the real container)"
    )
    parser.add_argument("--no-build", action="store_true", help="run the bundle already built")
    parser.add_argument(
        "--require-audio", action="store_true", help="fail without an output device"
    )
    args = parser.parse_args()

    if sys.platform != "darwin":
        print("self-test-bundle: macOS only for now")
        return 0
    if not should_run(os.environ, args.local):
        print(
            "self-test-bundle: skipped outside CI: the sandboxed bundle runs in the app's "
            "real container; run with --local to run it here (CLAUDE.md)"
        )
        return 0

    target = NATIVE[platform.machine()]
    config = json.loads((APP / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    bundle = bundle_path(target, config["productName"])
    if not args.no_build:
        print(f"self-test-bundle: building {target} with the self-test", flush=True)
        built = subprocess.run(
            build_command(target), cwd=APP, env=build_environment(os.environ), check=False
        )
        if built.returncode != 0:
            return built.returncode
    if not bundle.is_dir():
        sys.exit(f"self-test-bundle: no bundle at {bundle.relative_to(REPO_ROOT)}")

    command = run_command(executable(bundle), args.require_audio)
    print(f"self-test-bundle: running {bundle.relative_to(REPO_ROOT)}", flush=True)
    try:
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired:
        sys.exit(f"self-test-bundle: no result within {TIMEOUT_SECONDS} s")
    output = result.stdout + result.stderr
    print(output, end="" if output.endswith("\n") else "\n")
    problems = verdict(output, result.returncode)
    for problem in problems:
        print(f"self-test-bundle: {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
