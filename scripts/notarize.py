#!/usr/bin/env python3
"""Notarizes a Developer ID-signed .dmg or .app and staples the ticket
(PLAN.md §8.3), with Apple's notarytool.

Credentials are an App Store Connect API key: NOTARY_KEY_PATH (the .p8
file), NOTARY_KEY_ID and NOTARY_ISSUER. Without all three, or with
--dry-run, it prints what it would run and changes nothing, exiting 0, so
an unsigned build passes through. With them it refuses an ad-hoc signed
file, submits it and waits, prints Apple's log if it isn't accepted, then
staples and checks the ticket and asks Gatekeeper (`spctl`).

A .dmg is submitted as it is (the ticket covers the app inside); an .app
is zipped with `ditto` first.

Usage: scripts/notarize.py PATH [--dry-run]
"""

import argparse
import json
import os
import pathlib
import shlex
import subprocess
import sys
import tempfile

CREDENTIALS = ("NOTARY_KEY_PATH", "NOTARY_KEY_ID", "NOTARY_ISSUER")


def credentials(environment):
    """The key's (path, id, issuer), or None unless all three are set."""
    values = tuple(environment.get(name, "") for name in CREDENTIALS)
    return values if all(values) else None


def key_arguments(creds):
    path, key_id, issuer = creds
    return ["--key", path, "--key-id", key_id, "--issuer", issuer]


def plan(path, creds, work):
    """The commands, in order: (what, command). `work` is a scratch dir."""
    keys = key_arguments(creds or tuple(f"${name}" for name in CREDENTIALS))
    steps = []
    upload = path
    if path.suffix == ".app":
        upload = work / f"{path.stem}.zip"
        steps.append(("zip", ["ditto", "-c", "-k", "--keepParent", str(path), str(upload)]))
    submit = ["xcrun", "notarytool", "submit", str(upload), *keys]
    steps.append(("submit", [*submit, "--wait", "--output-format", "json"]))
    steps.append(("staple", ["xcrun", "stapler", "staple", str(path)]))
    steps.append(("validate", ["xcrun", "stapler", "validate", str(path)]))
    if path.suffix == ".dmg":
        assess = ["spctl", "--assess", "--type", "open", "--context", "context:primary-signature"]
    else:
        assess = ["spctl", "--assess", "--type", "execute"]
    steps.append(("assess", [*assess, "--verbose=2", str(path)]))
    return steps


def submission(output):
    """notarytool's JSON result: (id, status)."""
    result = json.loads(output)
    return result.get("id"), result.get("status")


def is_adhoc(path):
    output = subprocess.run(
        ["codesign", "-d", "-vv", str(path)], capture_output=True, text=True, check=False
    )
    return "Signature=adhoc" in output.stderr + output.stdout or output.returncode != 0


def main():
    parser = argparse.ArgumentParser(description="Notarize and staple a .dmg or .app.")
    parser.add_argument("path", type=pathlib.Path)
    parser.add_argument("--dry-run", action="store_true", help="print the commands only")
    args = parser.parse_args()
    if args.path.suffix not in (".dmg", ".app") or not args.path.exists():
        print(f"notarize: {args.path}: not a .dmg or .app")
        return 1
    creds = credentials(os.environ)
    with tempfile.TemporaryDirectory() as scratch:
        steps = plan(args.path, creds, pathlib.Path(scratch))
        if creds is None or args.dry_run:
            why = "--dry-run" if args.dry_run else f"no credentials ({', '.join(CREDENTIALS)})"
            print(f"notarize: {why}: would run")
            for _, command in steps:
                print("  " + shlex.join(command))
            return 0
        if is_adhoc(args.path):
            print(f"notarize: {args.path.name} is ad-hoc signed: notarization needs a Developer ID")
            return 1
        for what, command in steps:
            print(f"notarize: {what}", flush=True)
            result = subprocess.run(command, capture_output=True, text=True, check=False)
            if what == "submit":
                ident, status = submission(result.stdout)
                print(f"notarize: submission {ident}: {status}")
                if status != "Accepted":
                    log = ["xcrun", "notarytool", "log", str(ident), *key_arguments(creds)]
                    subprocess.run(log, check=False)
                    return 1
            elif result.returncode != 0:
                print(result.stdout + result.stderr)
                return 1
    print(f"notarize: {args.path.name} notarized and stapled")
    return 0


if __name__ == "__main__":
    sys.exit(main())
