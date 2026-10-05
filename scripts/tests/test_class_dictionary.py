import pytest


@pytest.fixture
def dictionary(script):
    return script("class-dictionary")


def test_cpp_types_are_qualified_by_their_owners(dictionary):
    text = """
namespace anomp
{
/** A class { with a brace in its comment. */
class Player final : public juce::AudioSource,
                     private juce::Timer
{
public:
    enum class State { empty, playing };
    struct Options
    {
        float gain = 1.0f;
    };
    template <typename T>
    struct Ring
    {
        T item;
    };
    friend class Other;
    void play() { const char* s = "struct Fake {"; }
private:
    struct Track;
};
struct Player::Track
{
    struct { int unnamed; } fields;
};
enum Param : std::size_t
{
    rate
};
} // namespace anomp
"""
    assert dictionary.cpp_types(text) == [
        "Player",
        "Player::State",
        "Player::Options",
        "Player::Ring",
        "Player::Track",
        "Param",
    ]


def test_c_api_types_include_opaque_handles_callbacks_and_unnamed_enums(dictionary):
    text = """
typedef struct anomp_tags
{
    const char* title;
} anomp_tags;
typedef struct anomp_engine anomp_engine;
typedef void (*anomp_event_callback)(const anomp_event* event, void* user_data);
/** Player states. */
enum
{
    ANOMP_STATE_EMPTY = 0,
    ANOMP_STATE_PLAYING = 2
};
enum
{
    ANOMP_RECORDING_WRITE_FAILED = 1,
    ANOMP_RECORDING_DISK_FULL = 2
};
"""
    assert dictionary.c_api_types(text) == [
        "anomp_tags",
        "anomp_engine",
        "anomp_event_callback",
        "ANOMP_STATE_*",
        "ANOMP_RECORDING_*",
    ]


def test_rust_types_leave_out_test_only_code(dictionary):
    text = r"""
/// A struct.
pub struct Queue {
    items: Vec<Item>,
}
pub(crate) enum Grain { Track }
trait Host { fn recorded(&self); }
const SQL: &str = r#"CREATE TABLE x (struct Fake {"#;
pub enum Placeholders {
    System,
    #[cfg(test)]
    Listed,
}
fn read() {
    struct Row { id: i64 }
}
#[cfg(test)]
pub mod testing {
    pub struct FakeBookmarks;
}
#[cfg(test)]
#[derive(Debug)]
struct OnlyInTests;
#[cfg(test)]
mod tests {
    struct Library { s: &'static str }
    fn f() -> char { '{' }
}
"""
    assert dictionary.rust_types(text) == ["Queue", "Grain", "Host", "Placeholders", "Row"]


def test_modules_declared_for_tests_only_are_left_out(dictionary):
    lib = "#[cfg(test)]\nmod bindings;\nmod audio;\n#[cfg(debug_assertions)]\nmod dev;\n"
    assert dictionary.rust_test_modules("app/src-tauri/src/lib.rs", lib) == {
        "app/src-tauri/src/bindings.rs",
        "app/src-tauri/src/bindings/",
    }
    library = "#[cfg(test)]\npub mod test_library;\n"
    assert dictionary.rust_test_modules("app/src-tauri/src/library/mod.rs", library) == {
        "app/src-tauri/src/library/test_library.rs",
        "app/src-tauri/src/library/test_library/",
    }


def test_expected_types_by_page(dictionary):
    files = {
        "core/include/anomp/anomp.h": "typedef struct anomp_engine anomp_engine;\n",
        "core/src/anomp_c_api.cpp": "struct anomp_engine\n{\n};\nstruct TagsHandle : anomp_tags\n{\n};\n",
        "core/tests/LogTests.cpp": "struct NotListed\n{\n};\n",
        "effects/include/anomp/effects/EffectChain.h": "enum class Unit\n{\n};\n",
        "app/src-tauri/src/lib.rs": "#[cfg(test)]\nmod bindings;\npub struct App;\n",
        "app/src-tauri/src/bindings.rs": "struct Command;\n",
        "app/src/lib/api.ts": "export type ArtSize = 'list';\ntype Private = number;\n",
        "app/src/lib/generated/ipc.ts": "export type Track = {};\n",
        "app/src/lib/state/ui.svelte.ts": "export type MainView = 'home';\n",
        "app/src/lib/components/Art.svelte": "<script></script>\n",
        "app/src/routes/help/+page.svelte": "<script></script>\n",
    }
    want = dictionary.expected(sorted(files), files.get)
    names = {page: {(f.name, f.path) for f in found} for page, found in want.items()}
    assert names["c-api.md"] == {
        ("anomp_engine", "core/include/anomp/anomp.h"),
        ("anomp_engine", "core/src/anomp_c_api.cpp"),
    }
    assert names["core.md"] == {("TagsHandle", "core/src/anomp_c_api.cpp")}
    assert names["effects.md"] == {("Unit", "effects/include/anomp/effects/EffectChain.h")}
    assert names["rust.md"] == {("App", "app/src-tauri/src/lib.rs")}
    assert names["frontend.md"] == {
        ("ArtSize", "app/src/lib/api.ts"),
        ("generated/ipc.ts", "app/src/lib/generated/ipc.ts"),
        ("state/ui.svelte.ts", "app/src/lib/state/ui.svelte.ts"),
        ("MainView", "app/src/lib/state/ui.svelte.ts"),
        ("Art", "app/src/lib/components/Art.svelte"),
        ("routes/help/+page.svelte", "app/src/routes/help/+page.svelte"),
    }


PAGE = """# The core

Intro, with a [link](../developer-guide/04-core.md).

### `AudioEngine`

Class · [`core/src/AudioEngine.h`](../../core/src/AudioEngine.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine)

Owns the device.

### `Equaliser::Biquad`

Helper · [`core/src/Equaliser.h`](../../core/src/Equaliser.h)

### `Entry`

Struct · [`core/src/A.cpp`](../../core/src/A.cpp), [`core/src/B.cpp`](../../core/src/B.cpp)

### `Entry`

Struct · [`core/src/C.cpp`](../../core/src/C.cpp)

Something.
"""


def test_entries_read_names_files_guide_links_and_anchors(dictionary):
    entries = dictionary.entries_of(PAGE)
    assert [(e.name, e.anchor) for e in entries] == [
        ("AudioEngine", "audioengine"),
        ("Equaliser::Biquad", "equaliserbiquad"),
        ("Entry", "entry"),
        ("Entry", "entry-1"),
    ]
    assert entries[0].kind == "Class"
    assert entries[0].paths == ["core/src/AudioEngine.h"]
    assert entries[0].guide_links == [("04-core.md", "the-engine")]
    assert entries[0].described and not entries[1].described
    assert entries[2].paths == ["core/src/A.cpp", "core/src/B.cpp"]


def test_page_problems_name_missing_gone_undescribed_and_out_of_order(dictionary):
    found = dictionary.Found
    entries = dictionary.entries_of(PAGE)
    want = {
        found("AudioEngine", "core/src/AudioEngine.h"),
        found("Equaliser::Biquad", "core/src/Equaliser.h"),
        found("Entry", "core/src/A.cpp"),
        found("Entry", "core/src/B.cpp"),
        found("Recorder", "core/src/Recorder.h"),
    }
    assert dictionary.page_problems("core.md", entries, want) == [
        "core.md has no entry for `Recorder` (core/src/Recorder.h)",
        "core.md's entry `Entry` names core/src/C.cpp, which doesn't declare it",
        "core.md's entry `Entry` has no description",
        "core.md: `Entry` comes before `Equaliser::Biquad` alphabetically",
    ]


def test_guide_links_must_reach_a_heading(dictionary):
    entries = dictionary.entries_of(PAGE)
    pages = {"docs/developer-guide/04-core.md": "# 4. The core\n\n## The engine\n\n## The engine\n"}

    def read(path):
        if path not in pages:
            raise OSError(path)
        return pages[path]

    assert dictionary.guide_problems("core.md", entries, read) == []
    pages["docs/developer-guide/04-core.md"] = "# 4. The core\n\n## Something else\n"
    assert dictionary.guide_problems("core.md", entries, read) == [
        "core.md's entry `AudioEngine` links 04-core.md#the-engine, which isn't a heading there"
    ]


def test_the_index_lists_every_entry_by_folder_and_letter(dictionary):
    pages = {"core.md": dictionary.entries_of(PAGE)}
    index = dictionary.build_index(pages)
    assert "- `core/src/`: [`AudioEngine`](core.md#audioengine)" in index
    assert "[`Entry`](core.md#entry) (A.cpp, B.cpp), [`Entry`](core.md#entry-1) (C.cpp)" in index
    assert "### E\n\n- [`Entry`](core.md#entry): Core (C++), `core/src/A.cpp`" in index
    text = f"# Index\n\n{dictionary.INDEX_START}\nold\n{dictionary.INDEX_END}\n"
    assert dictionary.with_index(text, pages) == f"# Index\n\n{index}\n"
    assert dictionary.with_index("# No markers\n", pages) is None


def test_the_real_dictionary_covers_the_tree(dictionary):
    root = dictionary.REPO_ROOT

    def read_page(name):
        path = dictionary.DICTIONARY / name
        return path.read_text("utf-8") if path.exists() else None

    problems, index_text, new_index = dictionary.problems_in(
        read_page, dictionary.tracked_files(), lambda path: (root / path).read_text("utf-8")
    )
    assert problems == []
    assert new_index == index_text
