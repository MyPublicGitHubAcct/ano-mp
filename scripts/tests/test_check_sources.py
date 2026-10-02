import pytest


@pytest.fixture
def sources(script):
    return script("check-sources")


def make_tree(root, cmake_text, files):
    (root / "core" / "src").mkdir(parents=True)
    (root / "core" / "CMakeLists.txt").write_text(cmake_text)
    for name in files:
        path = root / "core" / "src" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("")


def test_listed_sources_ignores_comments_and_headers(sources):
    text = """
    add_library(core STATIC src/A.cpp
        src/B.cpp)  # src/Commented.cpp
    # src/Old.cpp
    target_sources(core PRIVATE src/C_apple.mm src/D.h)
    set_source_files_properties(src/C_apple.mm PROPERTIES COMPILE_OPTIONS -fobjc-arc)
    """
    assert sources.listed_sources(text) == {"src/A.cpp", "src/B.cpp", "src/C_apple.mm"}


def test_matching_lists_pass(sources, tmp_path):
    make_tree(tmp_path, "add_library(c src/A.cpp src/sub/B.mm)", ["A.cpp", "sub/B.mm", "A.h"])
    assert sources.compare("core/CMakeLists.txt", "core/src", root=tmp_path) == []


def test_unlisted_and_missing_files_are_reported(sources, tmp_path):
    make_tree(tmp_path, "add_library(c src/A.cpp src/Gone.cpp)", ["A.cpp", "New.cpp"])
    assert sources.compare("core/CMakeLists.txt", "core/src", root=tmp_path) == [
        "core/src/New.cpp: not listed in core/CMakeLists.txt",
        "core/src/Gone.cpp: listed in core/CMakeLists.txt but doesn't exist",
    ]


def test_real_tree_passes(sources):
    for cmake_path, source_dir in sources.SOURCE_LISTS:
        assert sources.compare(cmake_path, source_dir) == []
