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
    root, known = tree
    return docs.check_text(doc, text, root, known, docs.tails(known))


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
        "doctor.py",
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


def test_fenced_code_is_skipped(docs, tree):
    text = "```sh\nscripts/gone.py `scripts/gone.py`\n```\nafter\n"
    assert problems(docs, tree, text) == []


def test_relative_links_resolve_from_the_doc(docs, tree):
    text = "[engine](design/phase-1-engine.md) [gone](design/gone.md) [web](https://x.org) [here](#top)"
    assert problems(docs, tree, text, doc="docs/index.md") == [
        "docs/index.md:1: links to design/gone.md, which doesn't exist"
    ]
    assert problems(docs, tree, "[engine](docs/design/phase-1-engine.md#exit)") == []


def test_stated_counts_come_from_section_2(docs):
    plan = (
        "# Plan\n## 2. Current state\n"
        "| 115 passing Catch2 tests, also clean | `core/tests` |\n"
        "| 25 frontend tests (`npm test`) | `app/tests` |\n"
        "| 406 passing `cargo test` tests (…) | `app/src-tauri/src` |\n"
        "| One entry point …, the scripts' 102 pytest tests (`test-python.py`) | `scripts/` |\n"
        "## 3. Prerequisites\n| 999 passing Catch2 tests |\n"
    )
    assert docs.stated_counts(plan) == {
        "Catch2 tests": 115,
        "frontend tests": 25,
        "cargo tests": 406,
        "script tests": 102,
    }


def test_suite_outputs_are_counted(docs):
    assert docs.ctest_count("Test #115: last\n\nTotal Tests: 115\n") == 115
    listed = "a: test\n417 tests, 0 benchmarks\n0 tests, 0 benchmarks\n1 test, 0 benchmarks\n"
    assert docs.cargo_list_count(listed) == 418
    assert docs.node_pass_count("ℹ tests 25\nℹ pass 25\nℹ fail 0\n") == 25
    assert docs.pytest_collected("...\n102 tests collected in 0.2s\n") == 102


def test_count_problems_name_each_stale_count(docs):
    stated = {"Catch2 tests": 115, "cargo tests": 405, "frontend tests": 25}
    actual = {"Catch2 tests": 115, "cargo tests": 406, "frontend tests": 25, "script tests": 102}
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
        "script tests": 102,
    }


def test_the_real_docs_have_every_path_and_link(docs):
    assert docs.check() == []
