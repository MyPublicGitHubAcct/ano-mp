import hashlib
import json
import re
import shutil

import pytest


@pytest.fixture
def bump(script):
    return script("bump-pin")


@pytest.fixture
def tree(bump, tmp_path):
    """Copies of the real files each pin lives in."""
    for pin in bump.check_pins.PINS:
        target = tmp_path / pin.file
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(bump.REPO_ROOT / pin.file, target)
    return tmp_path


SHA = "f" * 64


def fake_download(contents=b"tarball"):
    fetched = []

    def download(url, destination):
        fetched.append(url)
        destination.write_bytes(contents)
        return hashlib.sha256(contents).hexdigest()

    return download, fetched


def ls_remote(peeled):
    def run(command, env=None):
        assert command[:2] == ["git", "ls-remote"]
        tag = command[3]
        lines = [f"{'a' * 40}\t{tag}"]
        if peeled:
            lines.append(f"{'c' * 40}\t{tag}^{{}}")
        return 0, "\n".join(lines) + "\n", ""

    return run


def bumped_file(bump, tree, name, version, **fakes):
    pin = bump.check_pins.by_name(name)
    text = (tree / pin.file).read_text()
    return text, bump.bumped(pin, version, text, tree, **fakes)


def test_juce_moves_to_the_commit_its_tag_names(bump, tree):
    download, fetched = fake_download()
    old, new = bumped_file(bump, tree, "juce", "9.0.3", download=download, run=ls_remote(True))
    digest = hashlib.sha256(b"tarball").hexdigest()
    assert fetched == [f"https://github.com/juce-framework/JUCE/archive/{'c' * 40}.tar.gz"]
    assert "# JUCE 9.0.3\n" in new
    assert f"JUCE/archive/{'c' * 40}.tar.gz" in new
    assert f"URL_HASH SHA256={digest})\nFetchContent_MakeAvailable(JUCE)" in new
    # Catch2's pin, in the same file, is untouched.
    catch2 = bump.check_pins.by_name("catch2")
    assert re.search(catch2.pattern, new).group(0) in old
    assert new.count("URL_HASH") == old.count("URL_HASH")
    assert len(new.splitlines()) == len(old.splitlines())


def test_a_lightweight_tag_names_its_commit_itself(bump, tree):
    download, fetched = fake_download()
    bumped_file(bump, tree, "catch2", "3.17.0", download=download, run=ls_remote(False))
    assert fetched == [f"https://github.com/catchorg/Catch2/archive/{'a' * 40}.tar.gz"]


def test_a_missing_tag_is_refused(bump, tree):
    def run(command, env=None):
        return 0, "", ""

    with pytest.raises(bump.BumpError):
        bumped_file(bump, tree, "catch2", "9.9.9", download=fake_download()[0], run=run)


def test_tarball_pins_move_url_and_hash(bump, tree):
    download, fetched = fake_download()
    old, new = bumped_file(bump, tree, "taglib", "2.4.0", download=download)
    assert fetched == [
        "https://github.com/taglib/taglib/releases/download/v2.4.0/taglib-2.4.0.tar.gz"
    ]
    assert "v2.4.0/taglib-2.4.0.tar.gz" in new
    assert hashlib.sha256(b"tarball").hexdigest() in new

    old, new = bumped_file(bump, tree, "signalsmith-linear", "0.7.0", download=download)
    assert "linear/archive/refs/tags/0.7.0.tar.gz" in new
    # The other Signalsmith pin keeps its URL and hash.
    stretch = [line for line in old.splitlines() if "signalsmith-stretch/archive" in line]
    assert stretch[0] in new
    assert new.count(hashlib.sha256(b"tarball").hexdigest()) == 1


GOOD_STATUS = (
    "[GNUPG:] NEWSIG\n"
    "[GNUPG:] VALIDSIG 1111222233334444555566667777888899990000 2026-01-01 1 0 4 0 1 10 01 "
    "FCF986EA15E6E293A5644F10B4322F04D67658D8\n"
)


def gpg(status, imported=0, verified=0):
    calls = []

    def run(command, env=None):
        calls.append(command)
        assert env["GNUPGHOME"], "a throwaway keyring"
        if "--import" in command:
            return imported, "", "import failed" if imported else ""
        return verified, status, ""

    return run, calls


def test_ffmpeg_is_checked_against_the_recorded_key(bump, tree):
    download, fetched = fake_download()
    run, calls = gpg(GOOD_STATUS)
    old, new = bumped_file(bump, tree, "ffmpeg", "9.0.3", download=download, run=run)
    assert fetched == [
        "https://ffmpeg.org/releases/ffmpeg-9.0.3.tar.xz",
        "https://ffmpeg.org/releases/ffmpeg-9.0.3.tar.xz.asc",
        bump.FFMPEG_KEY_URL,
    ]
    assert [call[2] for call in calls] == ["--import", "--status-fd"]
    assert 'FFMPEG_VERSION="9.0.3"' in new
    assert f'FFMPEG_SHA256="{hashlib.sha256(b"tarball").hexdigest()}"' in new
    assert len(new.splitlines()) == len(old.splitlines())


@pytest.mark.parametrize(
    "status, verified",
    [
        (GOOD_STATUS.replace("FCF986EA15E6E293A5644F10B4322F04D67658D8", "0" * 40), 0),
        ("[GNUPG:] BADSIG 1234 someone\n", 1),
        ("", 2),
    ],
)
def test_ffmpeg_with_another_or_a_bad_signature_is_refused(bump, tree, status, verified):
    run, _ = gpg(status, verified=verified)
    with pytest.raises(bump.BumpError, match="signature"):
        bumped_file(bump, tree, "ffmpeg", "9.0.3", download=fake_download()[0], run=run)


def test_the_recorded_fingerprint_is_read(bump):
    script = (bump.REPO_ROOT / "scripts" / "build-ffmpeg.sh").read_text()
    assert bump.ffmpeg_fingerprint(script) == "FCF986EA15E6E293A5644F10B4322F04D67658D8"
    with pytest.raises(bump.BumpError):
        bump.ffmpeg_fingerprint("# no key here\n")


def test_pypi_pins_need_the_release_to_exist(bump, tree):
    def fetch(url):
        assert url == "https://pypi.org/pypi/ruff/json"
        return json.dumps({"releases": {"0.16.9": [], "0.17.0": []}})

    _, new = bumped_file(bump, tree, "ruff", "0.17.0", fetch=fetch)
    assert 'RUFF_VERSION = "0.17.0"' in new
    with pytest.raises(bump.BumpError):
        bumped_file(bump, tree, "ruff", "0.99.0", fetch=fetch)


def test_mentions_find_the_docs_naming_a_version(bump, tmp_path):
    (tmp_path / "docs" / "design").mkdir(parents=True)
    (tmp_path / "CLAUDE.md").write_text("Pins: JUCE 9.0.2 and more\n")
    (tmp_path / "docs" / "design" / "x.md").write_text("a\nbuilt on JUCE 9.0.2.\n")
    (tmp_path / "docs" / "notes.txt").write_text("JUCE 9.0.2\n")
    juce = bump.check_pins.by_name("juce")
    assert bump.mentions(juce, "9.0.2", tmp_path) == ["CLAUDE.md:1", "docs/design/x.md:2"]
    assert bump.mentions(bump.check_pins.by_name("ruff"), "0.16.9", tmp_path) == []


def test_next_steps_name_real_scripts(bump):
    for pin in bump.check_pins.PINS:
        for step in bump.next_steps(pin):
            for word in step.replace(",", " ").split():
                if word.startswith("scripts/"):
                    assert (bump.REPO_ROOT / word).exists(), (pin.name, word)
