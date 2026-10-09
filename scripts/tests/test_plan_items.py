import textwrap

import pytest

# One excerpt per table shape, each as PLAN.md writes it.
PLAN = textwrap.dedent(
    """\
    # Plan

    #### Optional features (§4.6)

    | # | Feature | Touches | Size | Decision (2026-09-27) |
    |---|---|---|---|---|
    | O1 | Loudness analysis | core | M | Built; off by default |

    #### Expected features (§4.7)

    | # | Feature | Touches | Size | Priority |
    |---|---|---|---|---|
    | F1 | Playlists | Rust | M | P1 |

    ### Phase 7 — Hardening (macOS)

    | # | Item | Area | Size | Priority | Status |
    |---|---|---|---|---|---|
    | H1 | Content Security Policy | security | S | P1 | Done 2026-10-02 |
    | H19 | Split the largest modules | maintenance | S | P3 | Open, as touched |

    - **H19 Split the largest modules**, when they are next changed.

    ### Phase 7b — Personalisation

    | # | Feature | Touches | Size |
    |---|---|---|---|
    | X1 | Themes: the look and feel | UI | M |

    - [x] **X1 Themes.** Built 2026-10-03; to check in the app (the
      phase's exit). Record: elsewhere.

    ### Phase 7c — Documentation

    | # | Document | Reader | Priority |
    |---|---|---|---|
    | D1 | User guide | anyone | P1 |

    - [ ] **D1 User guide.** Written 2026-10-04: `docs/user-guide/`.

    ### Phase 7d — Proposals

    | # | Feature | Kind | Touches | Size |
    |---|---|---|---|---|
    | | **Playing and the queue** | | | |
    | N1 | Spoken-word mode | common | Rust | M |

    #### Playing and the queue

    - [ ] **N1 Spoken-word mode.** Audiobooks play differently.

    ### 9.2 Scripts to develop and test

    - [x] M1 Test harness (2026-10-02): `scripts/test-python.py`.
    - [ ] M2 Repo checks. Done: `check-c-api.py`. Open:
      - `sync-ffmpeg-frameworks.py`.
    """
)

DESIGN = {
    "docs/design/phase-6b.md": textwrap.dedent(
        """\
        # Phase 6b

        ## Proposals and decisions (§4.6)

        - **O1 Loudness analysis.** Analyse it.
          - **Decision:** accepted and built 2026-09-27.
        """
    ),
    "docs/design/phase-6c.md": textwrap.dedent(
        """\
        # Phase 6c

        ## Proposals and decisions (§4.7)

        - **F1 Playlists.** No playlists exist.
          - **Decision:** built 2026-09-27. Migration 007.
        """
    ),
    "docs/design/phase-7.md": textwrap.dedent(
        """\
        # Phase 7

        ## Hardening items (H1–H22) as proposed and built

        - **H1 Content Security Policy.** None today.

        ## Scripts (§9.2 M1–M6) as specified

        - [x] M1 Test harness (2026-10-02). Pytest.
        - [ ] M2 Repo checks, each read-only.
        """
    ),
    "docs/design/phase-7b.md": textwrap.dedent(
        """\
        # Phase 7b

        ## X1 Themes

        - [x] **X1 Themes.** Design tokens.
        """
    ),
}


@pytest.fixture
def items(script):
    return script("plan-items")


def by_number(found):
    return {item.number: item for item in found}


def test_reads_each_table_shape(items):
    found, problems = items.collect(PLAN, DESIGN)
    assert problems == []
    got = by_number(found)
    assert [item.number for item in found] == [
        "O1",
        "F1",
        "H1",
        "H19",
        "X1",
        "D1",
        "N1",
        "M1",
        "M2",
    ]
    # O: the Decision column; F: its record's Decision line.
    assert got["O1"].status == "Built; off by default"
    assert got["F1"].status == "Built 2026-09-27"
    assert got["F1"].extra == {"Priority": "P1"}
    # H: Priority and Status; an open item may have no record.
    assert got["H1"].status == "Done 2026-10-02"
    assert got["H19"].status == "Open, as touched"
    assert got["H19"].record is None
    # X: the name from the table, the status from its line.
    assert got["X1"].name == "Themes: the look and feel"
    assert got["X1"].status == "Built 2026-10-03; to check in the app"
    assert got["D1"].status == "Open: written 2026-10-04"
    assert got["N1"].status == "Proposal"
    assert got["N1"].extra == {"Kind": "common"}
    # M: its line only.
    assert got["M1"].name == "Test harness"
    assert got["M1"].status == "Done 2026-10-02"
    assert got["M2"].status == "Open, part done"


def test_places_items_in_the_plan_and_their_records(items):
    got = by_number(items.collect(PLAN, DESIGN)[0])
    assert got["O1"].plan.anchor == "optional-features-46"
    assert got["H1"].plan.anchor == "phase-7--hardening-macos"
    assert got["N1"].plan.anchor == "playing-and-the-queue"
    assert got["M1"].plan.anchor == "92-scripts-to-develop-and-test"
    assert got["H1"].record.path == "docs/design/phase-7.md"
    assert got["H1"].record.anchor == "hardening-items-h1h22-as-proposed-and-built"
    # A heading naming the item, with its line under it, is one record.
    assert got["X1"].record.anchor == "x1-themes"
    assert got["D1"].record is None


def test_status_from_an_item_line(items):
    assert items.line_status("Built 2026-10-03; to check (the exit).", True) == (
        "Built 2026-10-03; to check"
    )
    assert items.line_status("(with §8.2). Done 2026-10-02: x.", True) == "Done 2026-10-02"
    assert items.line_status("(2026-10-02): x.", True) == "Done 2026-10-02"
    assert items.line_status("Pick a file.", True) == "Done"
    assert items.line_status("Added 2026-10-08 at the owner's request, from", False) == (
        "Open: added 2026-10-08 at the owner's request"
    )
    assert items.line_status("Done: one. Open: two.", False) == "Open, part done"
    assert items.line_status("Audiobooks play differently.", False) == "Open"


def test_a_done_item_without_a_record_fails(items):
    design = {path: text for path, text in DESIGN.items() if not path.endswith("phase-7.md")}
    problems = items.collect(PLAN, design)[1]
    assert any(p.startswith("H1 is done (Done 2026-10-02) but nothing") for p in problems)
    assert any(p.startswith("M1 is done") for p in problems)
    # M2 is open, so no record is needed.
    assert not any(p.startswith("M2") for p in problems)


def test_an_item_without_any_status_fails(items):
    design = {path: text for path, text in DESIGN.items() if not path.endswith("phase-6c.md")}
    problems = items.collect(PLAN, design)[1]
    assert any(p.startswith("F1 has no status") for p in problems)


def test_a_duplicate_number_fails(items):
    plan = PLAN.replace(
        "| O1 | Loudness analysis | core | M | Built; off by default |",
        "| O1 | A | B | C | Built |\n| O1 | D | E | F | Built |",
    )
    problems = items.collect(plan, DESIGN)[1]
    assert any(p.startswith("O1 is in a plan table twice") for p in problems)

    design = dict(DESIGN, **{"docs/design/other.md": "## X1 Themes again\n"})
    problems = items.collect(PLAN, design)[1]
    assert any(p.startswith("X1 is recorded in docs/design/ twice") for p in problems)


def test_a_series_item_missing_its_line_or_row_fails(items):
    plan = PLAN.replace("- [ ] **D1 User guide.** Written 2026-10-04: `docs/user-guide/`.\n", "")
    problems = items.collect(plan, DESIGN)[1]
    assert "D1 has no item line in PLAN.md, as D items do" in problems
    # H19's line is a note, not a source H items have: H1 needs none.
    assert not any(p.startswith("H1 has no") for p in items.collect(PLAN, DESIGN)[1])


def test_a_table_and_a_line_that_disagree_fail(items):
    plan = PLAN.replace(
        "| # | Document | Reader | Priority |\n|---|---|---|---|",
        "| # | Document | Reader | Priority | Status |\n|---|---|---|---|---|",
    )
    plan = plan.replace(
        "| D1 | User guide | anyone | P1 |", "| D1 | User guide | anyone | P1 | Done 2026-10-05 |"
    )
    problems = items.collect(plan, DESIGN)[1]
    assert "D1's table says 'Done 2026-10-05' but its item line is unticked" in problems


def test_render_one_table_per_series_in_plan_order(items):
    text = items.render(items.collect(PLAN, DESIGN)[0])
    headings = [line for line in text.split("\n") if line.startswith("## ")]
    assert headings[0] == "## O1: Optional features (Phase 6b, §4 #6)"
    assert headings[-1] == "## M1–M2: Maintenance scripts (§9.2)"
    assert len(headings) == len(items.SERIES)
    assert "generated by `scripts/plan-items.py`" in text
    assert (
        "| H1 | Content Security Policy | P1 | Done 2026-10-02 "
        "| [Phase 7 — Hardening (macOS)](../PLAN.md#phase-7--hardening-macos) "
        "| [phase-7.md: Hardening items (H1–H22) as proposed and built]"
        "(design/phase-7.md#hardening-items-h1h22-as-proposed-and-built) |"
    ) in text
    assert "| N1 | Spoken-word mode | common | Proposal |" in text
    assert text.count("| — |") == 3  # H19, D1, N1


def write_tree(root):
    (root / "docs" / "design").mkdir(parents=True)
    (root / "PLAN.md").write_text(PLAN, "utf-8")
    for path, text in DESIGN.items():
        (root / path).write_text(text, "utf-8")


def test_a_stale_page_fails_the_check_and_is_rewritten(items, tmp_path):
    write_tree(tmp_path)
    page = tmp_path / "docs" / "plan-items.md"
    page.write_text("# Plan items\n\nOld.\n", "utf-8")
    said = []
    assert items.run(tmp_path, check=True, out=said.append) == 1
    assert said == ["plan-items: docs/plan-items.md is stale: run scripts/plan-items.py"]
    assert page.read_text("utf-8") == "# Plan items\n\nOld.\n"

    assert items.run(tmp_path, out=said.append) == 0
    assert items.run(tmp_path, check=True, out=said.append) == 0
    assert said[-1] == "plan-items: 9 items indexed"


def test_a_problem_writes_nothing(items, tmp_path):
    write_tree(tmp_path)
    (tmp_path / "docs" / "design" / "phase-7.md").unlink()
    assert items.run(tmp_path, out=lambda _: None) == 1
    assert not (tmp_path / "docs" / "plan-items.md").exists()


def test_the_real_page_is_current(items):
    assert items.run(check=True, out=lambda _: None) == 0
