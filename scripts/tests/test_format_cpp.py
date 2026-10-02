import pytest


@pytest.fixture
def format_cpp(script):
    return script("format-cpp")


def test_source_files_selects_cpp_sources_only(format_cpp, tmp_path):
    for name in ["src/B.cpp", "src/A.h", "src/C_apple.mm", "include/x.h", "notes.txt", "x.py"]:
        path = tmp_path / "core" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("")
    (tmp_path / "core" / "dir.cpp").mkdir()
    assert format_cpp.source_files(tmp_path / "core", tmp_path) == [
        "core/include/x.h",
        "core/src/A.h",
        "core/src/B.cpp",
        "core/src/C_apple.mm",
    ]


def test_batches_cover_every_file_in_order(format_cpp):
    files = [f"f{i}" for i in range(7)]
    assert format_cpp.batches(files, size=3) == [["f0", "f1", "f2"], ["f3", "f4", "f5"], ["f6"]]
    assert format_cpp.batches([], size=3) == []
    assert format_cpp.batches(files[:3], size=3) == [files[:3]]


def test_real_core_fits_in_batches(format_cpp):
    files = format_cpp.source_files(format_cpp.CORE, format_cpp.REPO_ROOT)
    assert "core/include/anomp/anomp.h" in files
    batches = format_cpp.batches(files)
    assert [file for batch in batches for file in batch] == files
    # Well inside Windows' 32K-character command line.
    assert all(len(" ".join(batch)) < 16_000 for batch in batches)
