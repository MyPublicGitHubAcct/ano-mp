#!/usr/bin/env python3
"""Builds the macOS app bundle and DMG with Tauri (PLAN.md §8.2, §8.3).

Universal (arm64 and x86_64) by default, as releases ship; --native builds
for this Mac only, which is quicker. The bundle lands in
app/src-tauri/target/<target>/release/bundle/.

Signing follows one rule: with a signing identity (--identity, or
APPLE_SIGNING_IDENTITY as Tauri reads it), the app is signed with it and
the hardened runtime is on, as notarization needs; without one it is
ad-hoc signed with the hardened runtime off, as tauri.conf.json says for
local builds. A Developer ID build is tauri.conf.json plus that change
only, so a local build and a release differ in nothing else. With
APPLE_CERTIFICATE (a base64 .p12) and APPLE_CERTIFICATE_PASSWORD set,
Tauri imports the certificate into a temporary keychain first (CI).

Usage: scripts/build-app.py [--native] [--identity NAME] [--bundles app,dmg]
"""

import argparse
import json
import os
import pathlib
import platform
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"
TARGET_DIR = APP / "src-tauri" / "target"

UNIVERSAL = "universal-apple-darwin"
NATIVE = {"arm64": "aarch64-apple-darwin", "x86_64": "x86_64-apple-darwin"}


def identity_from(argument, environment):
    """The signing identity to use, or None for an ad-hoc build ("-" is
    Tauri's name for ad-hoc)."""
    identity = argument if argument is not None else environment.get("APPLE_SIGNING_IDENTITY")
    if identity in (None, "", "-"):
        return None
    return identity


def tauri_command(target, bundles, identity):
    """The `tauri build` command line."""
    command = ["npm", "run", "tauri", "build", "--", "--target", target, "--bundles", bundles]
    if identity is not None:
        overlay = {"bundle": {"macOS": {"signingIdentity": identity, "hardenedRuntime": True}}}
        command += ["--config", json.dumps(overlay, separators=(",", ":"))]
    return command


def bundle_dir(target):
    return TARGET_DIR / target / "release" / "bundle"


def main():
    parser = argparse.ArgumentParser(description="Build the macOS app bundle and DMG.")
    parser.add_argument("--native", action="store_true", help="this Mac's architecture only")
    parser.add_argument("--identity", help="signing identity (default: APPLE_SIGNING_IDENTITY)")
    parser.add_argument("--bundles", default="app,dmg", help="Tauri bundle types (default app,dmg)")
    args = parser.parse_args()

    if sys.platform != "darwin":
        sys.exit("build-app: macOS only for now (PLAN.md Phases 9 and 10 add the others)")
    target = NATIVE[platform.machine()] if args.native else UNIVERSAL
    identity = identity_from(args.identity, os.environ)
    environment = dict(os.environ)
    if identity is None:
        # Tauri would sign with this even when the overlay isn't given.
        environment.pop("APPLE_SIGNING_IDENTITY", None)
    print(
        f"build-app: {target}, " + (f"signed by {identity!r}, hardened" if identity else "ad-hoc"),
        flush=True,
    )
    command = tauri_command(target, args.bundles, identity)
    result = subprocess.run(command, cwd=APP, env=environment, check=False)
    if result.returncode != 0:
        return result.returncode
    print(f"build-app: {bundle_dir(target).relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
