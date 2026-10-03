#!/usr/bin/env python3
"""Moves a pinned dependency to another release (PLAN.md §9.1, §9.2 M5).

NAME is one of check-pins.py's pins (`scripts/check-pins.py` lists them and
what is newer). For the native libraries it downloads the release, computes
its SHA-256 and rewrites the pin in place: JUCE and Catch2 as GitHub's
archive of the commit the release tag names (looked up with `git
ls-remote`), TagLib and Signalsmith as their release tarballs, FFmpeg as
its tarball after checking its GPG signature was made by the key whose
fingerprint build-ffmpeg.sh records (gpg, in a throwaway keyring). The
formatters', linters' and pytest's pins are versions uvx fetches from PyPI,
so only the version is checked and rewritten. Then it prints the rebuild
and test commands, and where the docs name the old version.

With --check it downloads and verifies the same way, prints the change as
a diff and writes nothing; it exits 1 when the pin would change.

Usage: scripts/bump-pin.py NAME VERSION [--check]
"""

import argparse
import difflib
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import urllib.request

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
SCRIPTS = REPO_ROOT / "scripts"

FFMPEG_KEY_URL = "https://ffmpeg.org/ffmpeg-devel.asc"

# The docs that name versions ("JUCE 9.0.2"), searched after a bump.
DOCS = ["CLAUDE.md", "PLAN.md", "README.md", "docs"]


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check_pins = load("check-pins")


class BumpError(Exception):
    pass


# The network and gpg are reached only through these, which the tests replace.


def download(url, destination):
    """Saves `url` to `destination`; returns the file's SHA-256."""
    request = urllib.request.Request(url, headers={"User-Agent": "ano-mp bump-pin"})
    digest = hashlib.sha256()
    with urllib.request.urlopen(request, timeout=300) as response, destination.open("wb") as out:
        while chunk := response.read(1 << 20):
            digest.update(chunk)
            out.write(chunk)
    return digest.hexdigest()


def run(command, env=None):
    result = subprocess.run(command, capture_output=True, text=True, check=False, env=env)
    return result.returncode, result.stdout, result.stderr


# Rewriting a pin's file.


def replace_version(text, pin, version):
    """`text` with the version `pin.pattern` finds replaced by `version`."""
    match = re.search(pin.pattern, text)
    if match is None:
        raise BumpError(f"{pin.name}: no version found in {pin.file}")
    start, end = match.span("version")
    return text[:start] + version + text[end:]


def rewrite_fetch(text, pin, version, url, sha256):
    """`text` with `pin`'s anomp_fetch_declare moved to `url` and `sha256`."""
    block = re.search(rf"anomp_fetch_declare\({re.escape(pin.fetch_id)}\s.*?\)", text, re.DOTALL)
    if block is None:
        raise BumpError(f"{pin.name}: no anomp_fetch_declare({pin.fetch_id} in {pin.file}")
    new_block = re.sub(r"(\bURL\s+)\S+", lambda m: m.group(1) + url, block.group(0), count=1)
    new_block = re.sub(
        r"(URL_HASH\s+SHA256=)[0-9a-f]{64}", lambda m: m.group(1) + sha256, new_block, count=1
    )
    text = text[: block.start()] + new_block + text[block.end() :]
    return replace_version(text, pin, version)


def rewrite_ffmpeg(text, pin, version, sha256):
    text = replace_version(text, pin, version)
    text, count = re.subn(r'(FFMPEG_SHA256=")[0-9a-f]{64}(")', rf"\g<1>{sha256}\g<2>", text)
    if count != 1:
        raise BumpError("build-ffmpeg.sh: no FFMPEG_SHA256")
    return text


def ffmpeg_fingerprint(script_text):
    """The signing key's fingerprint build-ffmpeg.sh's comment records."""
    match = re.search(r"signing key\s*\n#\s*([0-9A-F ]+)\)", script_text)
    fingerprint = match.group(1).replace(" ", "") if match else ""
    if len(fingerprint) != 40:
        raise BumpError("build-ffmpeg.sh records no signing key fingerprint")
    return fingerprint


def signed_by(status_output):
    """The fingerprints gpg --status-fd's VALIDSIG lines name: the signing
    key's and its primary key's."""
    found = set()
    for line in status_output.splitlines():
        parts = line.split()
        if parts[:2] == ["[GNUPG:]", "VALIDSIG"]:
            found.add(parts[2])
            if len(parts) > 11:
                found.add(parts[11])
    return found


def verify_signature(tarball, signature, key, fingerprint, run=run):
    """Checks `signature` over `tarball` was made by `fingerprint`'s key,
    with only `key` (the public key file) in a throwaway keyring."""
    with tempfile.TemporaryDirectory() as home:
        env = {**os.environ, "GNUPGHOME": home}
        try:
            imported = run(["gpg", "--batch", "--import", str(key)], env)
        except FileNotFoundError:
            raise BumpError("gpg not found (brew install gnupg)") from None
        if imported[0] != 0:
            raise BumpError(f"gpg could not import {key.name}: {imported[2].strip()}")
        verified = run(
            ["gpg", "--batch", "--status-fd", "1", "--verify", str(signature), str(tarball)], env
        )
    if verified[0] != 0 or fingerprint not in signed_by(verified[1]):
        raise BumpError(f"{tarball.name}'s signature isn't from key {fingerprint}")


def tag_commit(pin, version, run=run):
    """The commit `pin`'s release tag names (the tag's own if annotated)."""
    tag = f"refs/tags/{pin.tag_prefix}{version}"
    repository = "https://github.com/" + pin.upstream.removeprefix("github:")
    result = run(["git", "ls-remote", repository, tag, f"{tag}^{{}}"])
    refs = dict(reversed(line.split("\t")) for line in result[1].splitlines() if "\t" in line)
    commit = refs.get(f"{tag}^{{}}") or refs.get(tag)
    if result[0] != 0 or commit is None:
        raise BumpError(f"{pin.name}: no tag {pin.tag_prefix}{version} in {repository}")
    return commit


def pypi_has(pin, version, fetch):
    name = pin.upstream.removeprefix("pypi:")
    releases = json.loads(fetch(f"https://pypi.org/pypi/{name}/json"))["releases"]
    if version not in releases:
        raise BumpError(f"{pin.name}: PyPI has no {name} {version}")


def bumped(pin, version, text, workdir, download=download, run=run, fetch=check_pins.fetch):
    """`pin`'s file text moved to `version`."""
    kind = pin.upstream.partition(":")[0]
    if kind == "pypi":
        pypi_has(pin, version, fetch)
        return replace_version(text, pin, version)
    if pin.name == "ffmpeg":
        url = pin.url.format(version=version)
        tarball = workdir / url.rsplit("/", 1)[1]
        sha256 = download(url, tarball)
        signature = workdir / (tarball.name + ".asc")
        download(url + ".asc", signature)
        key = workdir / "ffmpeg-devel.asc"
        download(FFMPEG_KEY_URL, key)
        verify_signature(tarball, signature, key, ffmpeg_fingerprint(text), run)
        return rewrite_ffmpeg(text, pin, version, sha256)
    if "{commit}" in pin.url:
        url = pin.url.format(commit=tag_commit(pin, version, run))
    else:
        url = pin.url.format(version=version)
    sha256 = download(url, workdir / "release.tar.gz")
    return rewrite_fetch(text, pin, version, url, sha256)


def mentions(pin, version, root=REPO_ROOT):
    """file:line for each place the docs name `pin` at `version`."""
    if not pin.label:
        return []
    needle = pin.label.format(version=version)
    found = []
    for entry in DOCS:
        path = root / entry
        for doc in sorted(path.rglob("*.md")) if path.is_dir() else [path]:
            if not doc.is_file():
                continue
            for number, line in enumerate(doc.read_text().splitlines(), 1):
                if needle in line:
                    found.append(f"{doc.relative_to(root)}:{number}")
    return found


def next_steps(pin):
    """The commands to run after moving `pin`."""
    kind = pin.upstream.partition(":")[0]
    if pin.name == "ffmpeg":
        return [
            "scripts/build-ffmpeg.sh && scripts/build-ffmpeg.sh --fuzz",
            "a new major version of a library: update tauri.conf.json's bundle.macOS.frameworks",
            "scripts/make-notices.py",
            "scripts/check-all.py",
        ]
    if pin.name == "clang-format":
        return ["scripts/format-cpp.py, committed on its own", "scripts/check-all.py --quick"]
    if pin.name == "ruff":
        return ["scripts/format-python.py, committed on its own", "scripts/check-all.py --quick"]
    if kind == "pypi":
        return [f"scripts/{pin.file.removeprefix('scripts/')}", "scripts/check-all.py"]
    return [
        "cmake --preset debug && cmake --build --preset debug && ctest --preset debug",
        "scripts/make-notices.py",
        "scripts/check-all.py",
    ]


def main():
    parser = argparse.ArgumentParser(description="Move a pinned dependency to another release.")
    parser.add_argument("name", help="the pin, as check-pins.py names it")
    parser.add_argument("version", help="the release to move to, e.g. 9.0.3")
    parser.add_argument("--check", action="store_true", help="show the change, write nothing")
    args = parser.parse_args()

    try:
        pin = check_pins.by_name(args.name)
        if not check_pins.RELEASE.match(args.version):
            raise BumpError(f"{args.version!r} isn't a release number")
        path = REPO_ROOT / pin.file
        text = path.read_text()
        old = check_pins.pinned(pin)
        with tempfile.TemporaryDirectory() as workdir:
            new_text = bumped(pin, args.version, text, pathlib.Path(workdir))
    except (BumpError, check_pins.PinError) as error:
        sys.exit(f"bump-pin: {error}")

    if new_text == text:
        print(f"bump-pin: {pin.name} is already at {args.version}")
        return 0
    if args.check:
        sys.stdout.writelines(
            difflib.unified_diff(
                text.splitlines(True), new_text.splitlines(True), f"a/{pin.file}", f"b/{pin.file}"
            )
        )
        return 1

    path.write_text(new_text)
    print(f"bump-pin: {pin.name} {old} -> {args.version} in {pin.file}")
    print("\nThen:")
    for step in next_steps(pin):
        print(f"  {step}")
    places = mentions(pin, old)
    if places:
        print(f"\nThe docs name {pin.label.format(version=old)} at:")
        for place in places:
            print(f"  {place}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
