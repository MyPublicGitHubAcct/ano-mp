import pytest


@pytest.fixture
def guide(script):
    return script("check-developer-guide")


def test_cpp_units_fold_headers_and_platform_files(guide):
    assert guide.cpp_unit("PlayerEngine.h") == "PlayerEngine"
    assert guide.cpp_unit("PlayerEngine.cpp") == "PlayerEngine"
    assert guide.cpp_unit("FolderAccess_apple.mm") == "FolderAccess"
    assert guide.cpp_unit("FolderAccess_unsandboxed.cpp") == "FolderAccess"
    assert guide.cpp_unit("MediaControls_none.cpp") == "MediaControls"
    assert guide.cpp_unit("anomp_c_api.cpp") == "anomp_c_api"


def test_expected_names_per_tree(guide):
    files = [
        "README.md",
        ".github/workflows/ci.yml",
        "core/src/Log.h",
        "core/src/Log.cpp",
        "core/src/DockMenu_apple.mm",
        "core/src/DockMenu_none.cpp",
        "core/tests/LogTests.cpp",
        "effects/src/Fft.h",
        "app/src-tauri/src/lib.rs",
        "app/src-tauri/src/queue/mod.rs",
        "app/src-tauri/src/library/migrations/001_initial.sql",
        "app/src/app.html",
        "app/src/lib/api.ts",
        "app/src/lib/generated/ipc.ts",
        "app/src/lib/generated/commands.ts",
        "app/src/lib/components/Art.svelte",
        "app/tests/links.test.mjs",
    ]
    want = guide.expected(files)
    assert want["Top-level folders"] == {".github/", "core/", "effects/", "app/"}
    assert want["core/src/"] == {"Log", "DockMenu"}
    assert want["effects/src/"] == {"Fft"}
    assert want["app/src-tauri/src/"] == {"lib.rs", "queue/mod.rs"}
    assert want["app/src/"] == {
        "app.html",
        "lib/api.ts",
        "lib/generated/",
        "lib/components/Art.svelte",
    }


MAP = """# 3. Repository map

## Top-level folders

- `app/`: the app.
- `core/`: the core.

## The core: `core/src/`

- `Log`: the log, in a sentence
  that goes on to a second line.
- `DockMenu` and `MediaControls`: platform code.

## Notes

- `Elsewhere`: not a section the check reads.

## The Rust backend: `app/src-tauri/src/`

- `lib.rs`: start-up.

### `queue/`

- `mod.rs`, `model.rs`: the queue.

## The frontend: `app/src/`

- `lib/generated/`: generated.

### `lib/visualizer/renderers/`

- `bars.ts`, `scope.ts`
  and `vu.ts`: three visualizations.
"""


def test_listed_names_by_section_and_folder(guide):
    have = guide.listed(MAP)
    assert have["Top-level folders"] == {"app/", "core/"}
    assert have["core/src/"] == {"Log", "DockMenu", "MediaControls"}
    assert have["app/src-tauri/src/"] == {"lib.rs", "queue/mod.rs", "queue/model.rs"}
    assert have["app/src/"] == {
        "lib/generated/",
        "lib/visualizer/renderers/bars.ts",
        "lib/visualizer/renderers/scope.ts",
        "lib/visualizer/renderers/vu.ts",
    }
    assert "Notes" not in have


def test_map_problems_name_what_is_missing_and_what_is_gone(guide):
    want = {"Top-level folders": {"app/"}, "core/src/": {"Log", "Recorder"}}
    have = {"Top-level folders": {"app/"}, "core/src/": {"Log", "OldThing"}}
    assert guide.map_problems(want, have) == [
        "03-repository-map.md doesn't name Recorder in `core/src/`",
        "03-repository-map.md names OldThing in `core/src/`, which isn't in the tree",
    ]
    assert guide.map_problems({"effects/src/": {"Fft"}}, {}) == [
        "03-repository-map.md has no section for `effects/src/`"
    ]


def test_the_index_links_every_page(guide):
    index = "1. [Overview](01-overview.md)\n3. [Map](03-repository-map.md#top-level-folders)\n"
    pages = {"01-overview.md", "02-getting-set-up.md", "03-repository-map.md"}
    assert guide.index_problems(pages, index) == ["README.md doesn't link 02-getting-set-up.md"]


def test_the_real_guide_maps_the_tree(guide):
    files = guide.tracked_files()
    assert guide.problems_in(lambda path: path.read_text("utf-8"), files) == []
