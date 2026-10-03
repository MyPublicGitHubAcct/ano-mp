import json
import subprocess

import pytest


@pytest.fixture
def lint_cpp(script):
    return script("lint-cpp")


def test_units_are_the_sources_compiled_once_each(lint_cpp, tmp_path):
    sources = tmp_path / "core" / "src"
    database = [
        {"directory": str(tmp_path / "build"), "file": "../core/src/B.cpp"},
        {"directory": str(tmp_path), "file": "core/src/A_apple.mm"},
        {"directory": str(tmp_path), "file": str(sources / "B.cpp")},
        {"directory": str(tmp_path), "file": "core/tests/ATests.cpp"},
        {"directory": str(tmp_path), "file": "build/_deps/juce-src/juce_core.cpp"},
    ]
    assert lint_cpp.units(database, sources.resolve()) == [
        (sources / "A_apple.mm").resolve(),
        (sources / "B.cpp").resolve(),
    ]


def test_sdk_args_name_the_sdk_on_macos_only(lint_cpp):
    calls = []

    def fake_run(command, **kwargs):
        calls.append(command)
        return subprocess.CompletedProcess(command, 0, stdout="/SDKs/MacOSX.sdk\n")

    assert lint_cpp.sdk_args("darwin", fake_run) == ["--extra-arg=-isysroot/SDKs/MacOSX.sdk"]
    assert calls == [["xcrun", "--show-sdk-path"]]
    assert lint_cpp.sdk_args("linux", fake_run) == []
    assert lint_cpp.sdk_args("win32", fake_run) == []
    assert len(calls) == 1


OUTPUT = """\
/repo/core/src/A.h:3:5: warning: something odd [bugprone-x]
    3 |     int a;
      |     ^
/repo/core/src/A.h:1:1: note: declared here
/repo/core/src/B.cpp:9:1: error: worse [bugprone-y,-warnings-as-errors]
12 warnings generated.
"""


def test_diagnostics_keep_their_notes(lint_cpp):
    found = lint_cpp.diagnostics(OUTPUT)
    assert len(found) == 2
    assert found[0].splitlines()[0] == "/repo/core/src/A.h:3:5: warning: something odd [bugprone-x]"
    assert "note: declared here" in found[0]
    assert found[1].startswith("/repo/core/src/B.cpp:9:1: error: worse")
    assert lint_cpp.diagnostics("") == []
    assert lint_cpp.diagnostics("Suppressed 3 warnings.\n") == []


def test_a_headers_finding_is_reported_once(lint_cpp):
    header = "/repo/core/src/A.h:3:5: warning: something odd [bugprone-x]\n  note"
    own = "/repo/core/src/B.cpp:1:1: warning: other [bugprone-z]"
    assert lint_cpp.unique([[header], [header, own], []]) == [header, own]


def test_real_compile_database_covers_every_compiled_source(lint_cpp):
    database_path = lint_cpp.BUILD_DIR / "compile_commands.json"
    if not database_path.is_file():
        pytest.skip("no debug configure")
    files = lint_cpp.units(json.loads(database_path.read_text()), lint_cpp.SOURCES)
    names = {path.name for path in files}
    assert "PlayerEngine.cpp" in names
    assert "anomp_c_api.cpp" in names
    assert all(path.suffix in {".cpp", ".mm"} for path in files)


def test_clang_tidy_config_keeps_the_h21_checks(lint_cpp):
    config = (lint_cpp.REPO_ROOT / ".clang-tidy").read_text()
    for check in ("bugprone-*", "performance-*", "concurrency-*"):
        assert f"  {check}," in config
