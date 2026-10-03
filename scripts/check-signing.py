#!/usr/bin/env python3
"""Lists the code-signing certificates in the keychain and when each
expires (PLAN.md §9.1, monthly once §8.3 is set up).

Warns about a certificate expiring within --days (60 by default) and fails
on one already expired. No certificates is not a failure: there are none
until the Developer ID certificate exists (§8.3). The notarization key and
the updater key aren't in the keychain; check their offline backups by
hand.

Usage: scripts/check-signing.py [--days N]
"""

import argparse
import datetime
import re
import subprocess
import sys

IDENTITY = re.compile(r'^\s*\d+\)\s+([0-9A-F]{40})\s+"(.+)"', re.MULTILINE)


def run(command):
    """A tool's stdout (a thin wrapper the tests replace)."""
    return subprocess.run(command, check=True, capture_output=True, text=True).stdout


def identities(output):
    """`security find-identity -v -p codesigning`: [(SHA-1, name)]."""
    return IDENTITY.findall(output)


def certificate_pem(output, sha1):
    """The PEM after "SHA-1 hash: <sha1>" in `security find-certificate -a
    -Z -p` output."""
    marker = output.find(f"SHA-1 hash: {sha1}")
    if marker < 0:
        return None
    start = output.find("-----BEGIN CERTIFICATE-----", marker)
    end = output.find("-----END CERTIFICATE-----", start)
    if start < 0 or end < 0:
        return None
    return output[start : end + len("-----END CERTIFICATE-----")] + "\n"


def not_after(output):
    """`openssl x509 -noout -enddate`: the expiry, in UTC."""
    match = re.search(r"notAfter=(.+)", output)
    if match is None:
        return None
    text = re.sub(r"\s+", " ", match.group(1).strip()).replace(" GMT", " +0000")
    return datetime.datetime.strptime(text, "%b %d %H:%M:%S %Y %z")


def status(expires, now, days):
    if expires <= now:
        return "expired"
    if expires - now <= datetime.timedelta(days=days):
        return "expires soon"
    return "ok"


def main():
    parser = argparse.ArgumentParser(description="List signing certificates and their expiry.")
    parser.add_argument("--days", type=int, default=60, help="warn this many days ahead")
    args = parser.parse_args()
    if sys.platform != "darwin":
        print("check-signing: macOS only for now")
        return 0
    found = identities(run(["security", "find-identity", "-v", "-p", "codesigning"]))
    if not found:
        print("check-signing: no code-signing identities in the keychain (none until §8.3)")
        return 0
    certificates = run(["security", "find-certificate", "-a", "-Z", "-p"])
    now = datetime.datetime.now(datetime.UTC)
    failed = False
    for sha1, name in found:
        pem = certificate_pem(certificates, sha1)
        if pem is None:
            print(f"check-signing: {name}: certificate not found")
            failed = True
            continue
        enddate = subprocess.run(
            ["openssl", "x509", "-noout", "-enddate"],
            input=pem,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        expires = not_after(enddate)
        state = status(expires, now, args.days)
        print(f"check-signing: {name}: expires {expires:%Y-%m-%d} ({state})")
        failed |= state == "expired"
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
