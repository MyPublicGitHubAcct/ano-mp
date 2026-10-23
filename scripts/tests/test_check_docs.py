import pytest


@pytest.fixture
def docs(script):
    return script("check-docs")


@pytest.fixture
def tree(tmp_path):
    """A small repo: two source files and a docs folder."""
    for path in [
        "app/src-tauri/src/library/access.rs",
        "scripts/bench.py",
        "docs/design/phase-1-engine.md",
    ]:
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / path).write_text("")
    known = {
        "app/src-tauri/src/library/access.rs",
        "scripts/bench.py",
        "docs/design/phase-1-engine.md",
        "app",
        "app/src-tauri",
        "app/src-tauri/src",
        "app/src-tauri/src/library",
        "scripts",
        "docs",
        "docs/design",
    }
    return tmp_path, known


def problems(docs, tree, text, doc="PLAN.md"):
    _, known = tree
    return docs.check_text(doc, text, known, docs.tails(known))


@pytest.mark.parametrize(
    "code, path",
    [
        ("library/access.rs", "library/access.rs"),
        ("scripts/bench.py:42", "scripts/bench.py"),
        ("app/src-tauri/src/", "app/src-tauri/src"),
        ("en.json", "en.json"),
        ("Cargo.lock", "Cargo.lock"),
        ("docs/design/x.md#anchor", "docs/design/x.md"),
    ],
)
def test_paths_are_recognised(docs, code, path):
    assert docs.candidate_path(code) == path


@pytest.mark.parametrize(
    "code",
    [
        "cargo test",
        "anomp_read_tags",
        "metadata-changed",
        "build/<preset>/_deps",
        "third_party/ffmpeg/macos-universal",
        "~/Library/Logs",
        "/usr/bin/ld",
        "https://example.com/a.json",
        "bandcamp.com/developer",
        "queue::run",
        ".cpp",
        "--ld-path=x/y",
        "core/src/PlayerEngine.*",
        "library.recovery.json",
        "Contents/MacOS/ano-mp",
    ],
)
def test_other_inline_code_is_not_a_path(docs, code):
    assert docs.candidate_path(code) is None


def test_shortened_paths_match_the_tail_of_a_tracked_one(docs, tree):
    assert problems(docs, tree, "See `library/access.rs` and `src/library/access.rs`.") == []
    assert problems(docs, tree, "See `brary/access.rs`.") == [
        "PLAN.md:1: `brary/access.rs` isn't in the repo"
    ]


def test_a_missing_path_is_reported_with_its_line(docs, tree):
    text = "one\n`scripts/bench.py` is here\n`scripts/gone.py` is not\n"
    assert problems(docs, tree, text) == ["PLAN.md:3: `scripts/gone.py` isn't in the repo"]


def test_files_only_on_disk_are_not_in_the_repo(docs, tree):
    # An ignored file here (CI's run of 2026-10-03 failed on a
    # `scripts/__pycache__` that existed only on this Mac).
    root, _ = tree
    (root / "scripts/__pycache__").mkdir()
    (root / "docs/design/ignored.md").write_text("")
    text = "`scripts/__pycache__`\n[x](design/ignored.md)\n"
    assert problems(docs, tree, text, doc="docs/index.md") == [
        "docs/index.md:1: `scripts/__pycache__` isn't in the repo",
        "docs/index.md:2: links to design/ignored.md, which doesn't exist",
    ]


def test_fenced_code_is_skipped(docs, tree):
    text = "```sh\nscripts/gone.py `scripts/gone.py`\n```\nafter\n"
    assert problems(docs, tree, text) == []


def test_relative_links_resolve_from_the_doc(docs, tree):
    text = "[engine](design/phase-1-engine.md) [gone](design/gone.md) [web](https://x.org) [here](#top)"
    assert problems(docs, tree, text, doc="docs/index.md") == [
        "docs/index.md:1: links to design/gone.md, which doesn't exist"
    ]
    assert problems(docs, tree, "[engine](docs/design/phase-1-engine.md#exit)") == []
    assert problems(docs, tree, "[up](../scripts/bench.py) [root](../)", doc="docs/index.md") == []


@pytest.mark.parametrize(
    "heading, anchor",
    [
        ("Folders that can't be found", "folders-that-cant-be-found"),
        ("Match the device’s sample rate", "match-the-devices-sample-rate"),
        ("2. Finding music", "2-finding-music"),
        ("`anomp_analysis_callback`", "anomp_analysis_callback"),
        ("`ANOMP_EFFECT_*`", "anomp_effect_"),
        ("X1 Themes: the look", "x1-themes-the-look"),
        ("See [the map](03-repository-map.md)", "see-the-map"),
    ],
)
def test_slugs_are_githubs(docs, heading, anchor):
    assert docs.slug(heading) == anchor


def test_heading_anchors_number_repeats_and_skip_fences(docs):
    text = "# Plan\n## Exit\n```sh\n# a comment\n```\n### Exit\n## Closed ##\n#hashtag\n"
    assert docs.heading_anchors(text) == {"plan", "exit", "exit-1", "closed"}


def test_link_anchors_must_be_headings(docs, tree):
    anchors = {
        "docs/design/phase-1-engine.md": {"exit", "seek-margins"},
        "PLAN.md": {"phase-7"},
        "docs/README.md": {"index"},
        "docs/index.md": {"phase-7"},
    }
    _, known = tree
    known = known | {"docs/README.md"}
    text = (
        "[ok](design/phase-1-engine.md#exit) [gone](design/phase-1-engine.md#exits) "
        "[encoded](design/phase-1-engine.md#seek%2Dmargins) [here](#phase-7) [not](#phase-8) "
        "[folder](../docs/#index) [code](../scripts/bench.py#L3)"
    )
    found = docs.check_text("docs/index.md", text, known, docs.tails(known), anchors.get)
    assert found == [
        (
            "docs/index.md:1: links to design/phase-1-engine.md#exits, "
            "but docs/design/phase-1-engine.md has no heading #exits"
        ),
        "docs/index.md:1: links to #phase-8, but docs/index.md has no heading #phase-8",
    ]
    # Without the headings (the other tests), anchors aren't checked.
    assert docs.check_text("docs/index.md", text, known, docs.tails(known)) == []


def test_stated_counts_come_from_section_2(docs):
    plan = (
        "# Plan\n## 2. Current state\n"
        "| 115 passing Catch2 tests, also clean | `core/tests` |\n"
        "| 25 frontend tests (`npm test`) | `app/tests` |\n"
        "| 40 Vitest tests of the rune modules (`npm run test:state`) | `app/tests/state` |\n"
        "| 406 passing `cargo test` tests (…) | `app/src-tauri/src` |\n"
        "| One entry point …, the scripts' 102 pytest tests (`test-python.py`) | `scripts/` |\n"
        "## 3. Prerequisites\n| 999 passing Catch2 tests |\n"
    )
    assert docs.stated_counts(plan) == {
        "Catch2 tests": 115,
        "frontend tests": 25,
        "state tests": 40,
        "cargo tests": 406,
        "script tests": 102,
    }


def test_suite_outputs_are_counted(docs):
    assert docs.ctest_count("Test #115: last\n\nTotal Tests: 115\n") == 115
    listed = "a: test\n417 tests, 0 benchmarks\n0 tests, 0 benchmarks\n1 test, 0 benchmarks\n"
    assert docs.cargo_list_count(listed) == 418
    assert docs.node_pass_count("ℹ tests 25\nℹ pass 25\nℹ fail 0\n") == 25
    assert (
        docs.vitest_pass_count(" Test Files  15 passed (15)\n      Tests  105 passed (105)\n")
        == 105
    )
    assert docs.vitest_pass_count("      Tests  1 failed | 2 skipped | 22 passed (25)\n") == 22
    coloured = "\x1b[2m      Tests \x1b[22m \x1b[1m\x1b[32m105 passed\x1b[39m\x1b[22m\x1b[90m (105)\x1b[39m\n"
    assert docs.vitest_pass_count(coloured) == 105
    assert docs.vitest_pass_count("      Tests  3 failed (3)\n") is None
    assert docs.pytest_collected("...\n102 tests collected in 0.2s\n") == 102


def test_count_problems_name_each_stale_count(docs):
    stated = {"Catch2 tests": 115, "cargo tests": 405, "frontend tests": 25, "state tests": 40}
    actual = {
        "Catch2 tests": 115,
        "cargo tests": 406,
        "frontend tests": 25,
        "state tests": 40,
        "script tests": 102,
    }
    assert docs.count_problems(stated, actual) == [
        "PLAN.md §2: 405 cargo tests, but the suite has 406",
        "PLAN.md §2: no count of script tests",
    ]


def test_suite_counts_subtract_the_ignored_rust_tests(docs, monkeypatch, tmp_path):
    outputs = {
        ("cargo", "test", "--", "--list"): "417 tests, 0 benchmarks\n",
        ("cargo", "test", "--", "--list", "--ignored"): "11 tests, 0 benchmarks\n",
        ("ctest", "--preset", "debug", "-N"): "Total Tests: 115\n",
        ("npm", "test"): "ℹ pass 25\n",
        ("npm", "run", "test:state"): "      Tests  40 passed (40)\n",
    }

    def run(command, cwd):
        key = tuple(command)
        if key in outputs:
            return outputs[key]
        assert command[-2:] == ["--collect-only", "-q"]
        return "102 tests collected\n"

    monkeypatch.setattr(docs, "run", run)
    assert docs.suite_counts(tmp_path) == {
        "Catch2 tests": 115,
        "cargo tests": 406,
        "frontend tests": 25,
        "state tests": 40,
        "script tests": 102,
    }


def test_the_real_docs_have_every_path_and_link(docs):
    assert docs.check() == []
