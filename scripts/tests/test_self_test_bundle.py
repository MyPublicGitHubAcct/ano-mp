"""Tests of self-test-bundle.py (PLAN.md H14)."""

import plistlib

import pytest

PASSED = """JUCE v9.0.2
self-test: sandbox: ok: sandboxed
self-test: fixtures: ok: 21 files
self-test: scan: ok: 21 tracks
self-test: bookmark: ok: 832 bytes, resolved
self-test: covers: ok: 2 covers
self-test: decoding: ok: 21 files
self-test: playback: skipped: no output device: none
self-test: passed
"""


@pytest.fixture
def bundle_test(script):
    return script("self-test-bundle")


def test_it_runs_on_ci_or_when_asked(bundle_test):
    assert bundle_test.should_run({"CI": "true"}, local=False)
    assert bundle_test.should_run({}, local=True)
    assert not bundle_test.should_run({}, local=False)
    assert not bundle_test.should_run({"CI": "false"}, local=False)
    assert not bundle_test.should_run({"CI": ""}, local=False)


def test_the_build_has_the_feature_in_its_own_target_dir_ad_hoc(bundle_test):
    command = bundle_test.build_command("aarch64-apple-darwin")
    assert command[:4] == ["npm", "run", "tauri", "build"]
    assert command[command.index("--features") + 1] == "self-test"
    assert command[command.index("--bundles") + 1] == "app"
    environment = bundle_test.build_environment(
        {"APPLE_SIGNING_IDENTITY": "Developer ID", "PATH": "/bin"}
    )
    assert "APPLE_SIGNING_IDENTITY" not in environment
    assert environment["PATH"] == "/bin"
    assert environment["CARGO_TARGET_DIR"].endswith("target/self-test")
    bundle = bundle_test.bundle_path("aarch64-apple-darwin", "ano-mp")
    assert bundle.parts[-6:] == (
        "self-test",
        "aarch64-apple-darwin",
        "release",
        "bundle",
        "macos",
        "ano-mp.app",
    )


def test_the_executable_comes_from_the_info_plist(bundle_test, tmp_path):
    contents = tmp_path / "x.app" / "Contents"
    contents.mkdir(parents=True)
    (contents / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "ano-mp"}))
    found = bundle_test.executable(tmp_path / "x.app")
    assert found == contents / "MacOS" / "ano-mp"
    assert bundle_test.run_command(found, require_audio=False)[1:] == [
        "--self-test",
        "--require-sandbox",
    ]
    assert bundle_test.run_command(found, require_audio=True)[-1] == "--require-audio"


def test_a_sandboxed_run_with_every_stage_passing_or_skipped_passes(bundle_test):
    assert bundle_test.verdict(PASSED, 0) == []


def test_a_failed_stage_fails(bundle_test):
    output = PASSED.replace("covers: ok: 2 covers", "covers: FAILED: tagged-vorbis.flac: no cover")
    output = output.replace("self-test: passed", "self-test: FAILED")
    problems = bundle_test.verdict(output, 1)
    assert "self-test: covers: FAILED: tagged-vorbis.flac: no cover" in problems
    assert "the self-test didn't finish" in problems
    assert "exit status 1" in problems


def test_a_run_outside_the_sandbox_fails(bundle_test):
    output = PASSED.replace("sandbox: ok: sandboxed", "sandbox: skipped: not sandboxed")
    assert bundle_test.verdict(output, 0) == ["the app didn't report running in the sandbox"]


def test_a_crash_fails(bundle_test):
    problems = bundle_test.verdict("self-test: sandbox: ok: sandboxed\n", -6)
    assert problems == ["the self-test didn't finish", "exit status -6"]
