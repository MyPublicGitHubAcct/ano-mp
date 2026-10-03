"""cmake/Fetch.cmake, run in script mode against file:// URLs."""

import hashlib
import pathlib
import re
import shutil
import subprocess

import pytest

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FETCH = REPO_ROOT / "cmake" / "Fetch.cmake"

pytestmark = pytest.mark.skipif(shutil.which("cmake") is None, reason="needs cmake")


def download(tmp_path, url, sha256, attempts=3):
    """Runs anomp_download in a script; returns (ok, output)."""
    script = tmp_path / "run.cmake"
    script.write_text(
        f'set(ANOMP_DOWNLOAD_DIR "{tmp_path / "downloads"}")\n'
        f"set(ANOMP_DOWNLOAD_ATTEMPTS {attempts})\n"
        "set(ANOMP_DOWNLOAD_RETRY_DELAY 0)\n"
        f'include("{FETCH}")\n'
        f'anomp_download("{url}" "{sha256}" file)\n'
        'message(STATUS "RESULT=${file}")\n'
    )
    result = subprocess.run(
        ["cmake", "-P", str(script)], capture_output=True, text=True, check=False
    )
    return result.returncode == 0, result.stdout + result.stderr


def tarball(tmp_path, content=b"pinned"):
    source = tmp_path / "source" / "pkg-1.0.tar.gz"
    source.parent.mkdir(parents=True, exist_ok=True)
    source.write_bytes(content)
    return source, hashlib.sha256(content).hexdigest()


def result_path(output):
    return pathlib.Path(re.search(r"RESULT=(.*)", output).group(1).strip())


def test_a_download_lands_under_its_hash(tmp_path):
    source, sha = tarball(tmp_path)
    ok, output = download(tmp_path, source.as_uri(), sha)
    assert ok, output
    path = result_path(output)
    assert path == tmp_path / "downloads" / f"{sha}-pkg-1.0.tar.gz"
    assert path.read_bytes() == b"pinned"
    assert not list((tmp_path / "downloads").glob("*.part"))


def test_a_copy_with_the_right_hash_is_used_without_downloading(tmp_path):
    source, sha = tarball(tmp_path)
    assert download(tmp_path, source.as_uri(), sha)[0]
    source.unlink()
    ok, output = download(tmp_path, source.as_uri(), sha)
    assert ok, output
    assert "Downloading" not in output


def test_a_damaged_copy_is_downloaded_again(tmp_path):
    source, sha = tarball(tmp_path)
    (tmp_path / "downloads").mkdir()
    (tmp_path / "downloads" / f"{sha}-pkg-1.0.tar.gz").write_bytes(b"truncated")
    ok, output = download(tmp_path, source.as_uri(), sha)
    assert ok, output
    assert result_path(output).read_bytes() == b"pinned"


def test_a_failed_download_is_retried_then_fails_naming_each_attempt(tmp_path):
    missing = (tmp_path / "source" / "gone.tar.gz").as_uri()
    ok, output = download(tmp_path, missing, "0" * 64, attempts=3)
    assert not ok
    assert output.count("Downloading") == 3
    assert "Couldn't download" in output and "attempt 3:" in output


def test_a_wrong_hash_fails_at_once(tmp_path):
    source, _ = tarball(tmp_path)
    ok, output = download(tmp_path, source.as_uri(), "0" * 64)
    assert not ok
    assert output.count("Downloading") == 1
    assert "expected " + "0" * 64 in output
    assert not list((tmp_path / "downloads").iterdir())


def test_every_pinned_tarball_goes_through_the_helper():
    for path in [REPO_ROOT / "CMakeLists.txt", *sorted((REPO_ROOT / "cmake").glob("*.cmake"))]:
        text = path.read_text()
        assert "FetchContent_Declare(" not in text or path == FETCH, path
