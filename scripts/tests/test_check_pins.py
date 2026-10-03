import json

import pytest


@pytest.fixture
def pins(script):
    return script("check-pins")


LS_REMOTE = """\
aaa\trefs/tags/v3.15.0
bbb\trefs/tags/v3.16.0
ccc\trefs/tags/v3.16.0^{}
ddd\trefs/tags/v3.17.0-preview1
eee\trefs/tags/v3.9.9
fff\trefs/tags/devel
"""


def test_tags_keep_release_numbers_after_the_prefix(pins):
    assert pins.tags(LS_REMOTE, "v") == {"3.15.0", "3.16.0", "3.9.9"}
    assert pins.tags("x\trefs/tags/9.0.2\nx\trefs/tags/v1.0\n", "") == {"9.0.2"}


def test_versions_compare_as_numbers(pins):
    assert pins.key("3.16.0") > pins.key("3.9.9")
    assert pins.key("22.1.0.1") > pins.key("22.1.0")
    assert max(pins.tags(LS_REMOTE, "v"), key=pins.key) == "3.16.0"


def test_ffmpeg_releases_from_the_listing(pins):
    listing = (
        '<a href="ffmpeg-9.0.2.tar.xz">x</a> <a href="ffmpeg-9.0.2.tar.xz.asc">x</a>'
        '<a href="ffmpeg-10.0.tar.xz">x</a> <a href="ffmpeg-snapshot.tar.bz2">x</a>'
        '<a href="ffmpeg-8.1.2.tar.gz">x</a>'
    )
    assert pins.ffmpeg_releases(listing) == {"9.0.2", "10.0"}


def fake_upstream(tags="", pypi="1.0", ffmpeg=""):
    def fetch(url):
        if "pypi.org" in url:
            return json.dumps({"info": {"version": pypi}})
        assert url == "https://ffmpeg.org/releases/"
        return ffmpeg

    def run(command, cwd=None):
        assert command[:3] == ["git", "ls-remote", "--tags"]
        return 0, tags, ""

    return fetch, run


def test_latest_asks_each_kind_of_upstream(pins):
    fetch, run = fake_upstream(LS_REMOTE, "0.17.0", '<a href="ffmpeg-9.1.tar.xz">')
    assert pins.latest(pins.by_name("catch2"), fetch, run) == "3.16.0"
    assert pins.latest(pins.by_name("ruff"), fetch, run) == "0.17.0"
    assert pins.latest(pins.by_name("ffmpeg"), fetch, run) == "9.1"


def test_latest_refuses_no_releases_and_pre_releases(pins):
    fetch, run = fake_upstream("", "1.0rc1")
    with pytest.raises(pins.PinError):
        pins.latest(pins.by_name("catch2"), fetch, run)
    with pytest.raises(pins.PinError):
        pins.latest(pins.by_name("ruff"), fetch, run)


def test_rows_report_updates_and_failed_lookups(pins, tmp_path):
    (tmp_path / "a.cmake").write_text("# Lib v1.2.0\n")
    (tmp_path / "b.py").write_text('TOOL_VERSION = "2.0"\n')
    table = [
        pins.Pin("lib", "a.cmake", r"# Lib v(?P<version>[\d.]+)", "github:o/lib", tag_prefix="v"),
        pins.Pin("tool", "b.py", r'TOOL_VERSION = "(?P<version>[\d.]+)"', "pypi:tool"),
    ]

    def fetch(url):
        raise OSError("offline")

    def run(command, cwd=None):
        return 0, "x\trefs/tags/v1.3.0\n", ""

    rows = pins.pin_rows(table, tmp_path, fetch, run)
    assert rows[0] == pins.Row("lib", "1.2.0", "1.3.0", "update available")
    assert rows[1].latest == "?"
    assert rows[1].status.startswith("lookup failed")


def test_a_pin_missing_from_its_file_is_an_error(pins, tmp_path):
    (tmp_path / "a.cmake").write_text("nothing\n")
    pin = pins.Pin("lib", "a.cmake", r"# Lib v(?P<version>[\d.]+)", "github:o/lib")
    with pytest.raises(pins.PinError):
        pins.pinned(pin, tmp_path)


def test_cargo_and_npm_summaries(pins):
    stderr = (
        "    Updating crates.io index\n"
        "     Locking 2 packages to latest compatible versions\n"
        "    Updating cc v1.5.1 -> v1.6.0\n"
        "    Updating libc v0.2.189 -> v0.2.190\n"
        "warning: not updating lockfile due to dry run\n"
    )
    assert pins.cargo_updates(stderr) == ["cc v1.5.1 -> v1.6.0", "libc v0.2.189 -> v0.2.190"]
    outdated = json.dumps(
        {
            "vite": {"current": "8.3.1", "wanted": "8.3.2", "latest": "8.3.2"},
            "eslint": {"wanted": "10.12.0", "latest": "10.12.0"},
        }
    )
    assert pins.npm_outdated(outdated) == ["eslint missing -> 10.12.0", "vite 8.3.1 -> 8.3.2"]
    assert pins.npm_outdated("") == []


def test_reports_as_text_and_markdown(pins):
    rows = [pins.Row("juce", "9.0.2", "9.0.3", "update available")]
    sections = {"npm outdated": ["vite 8.3.1 -> 8.3.2"], "cargo update --dry-run": []}
    text = pins.text_report(rows, sections)
    assert "juce  9.0.2      9.0.3      update available" in text
    assert "npm outdated: 1 to update\n  vite 8.3.1 -> 8.3.2" in text
    assert "cargo update --dry-run: nothing to update" in text
    markdown = pins.markdown_report(rows, sections)
    assert "| juce | 9.0.2 | 9.0.3 | update available |" in markdown
    assert "- vite 8.3.1 -> 8.3.2" in markdown
    assert "Nothing to update." in markdown


def test_every_real_pin_is_found(pins):
    names = [pin.name for pin in pins.PINS]
    assert len(set(names)) == len(names)
    for pin in pins.PINS:
        assert pins.RELEASE.match(pins.pinned(pin)), pin.name
        kind = pin.upstream.partition(":")[0]
        assert kind in {"github", "pypi", "ffmpeg"}, pin.name
        if kind == "github" or pin.name == "ffmpeg":
            assert pin.url, pin.name
        if kind == "github":
            assert pin.fetch_id, pin.name
