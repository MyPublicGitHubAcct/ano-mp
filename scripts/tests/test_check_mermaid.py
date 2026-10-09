import pathlib
import shutil

import pytest

APP = pathlib.Path(__file__).resolve().parents[2] / "app"


@pytest.fixture
def mermaid(script):
    return script("check-mermaid")


DOC = """# A page

```mermaid
flowchart LR
  A --> B
```

- A list item:

  ```mermaid
  sequenceDiagram
    A->>B: Vec<Track>
  ```

Inline ```` ```mermaid ```` isn't a block.

```sh
echo mermaid
```
"""


def test_finds_blocks_and_takes_off_their_indent(mermaid):
    diagrams = mermaid.diagrams_in("page.md", DOC)
    assert [(d.doc, d.line) for d in diagrams] == [("page.md", 3), ("page.md", 10)]
    assert diagrams[0].source == "flowchart LR\n  A --> B"
    assert diagrams[1].source == "sequenceDiagram\n  A->>B: Vec<Track>"


def test_reports_only_the_diagrams_that_fail(mermaid):
    diagrams = mermaid.diagrams_in("page.md", DOC)
    assert mermaid.problems(diagrams, [None, "Parse error on line 2"]) == [
        "page.md:10: Mermaid diagram doesn't parse: Parse error on line 2"
    ]


def test_mermaid_and_jsdom_are_pinned_exactly(mermaid):
    import json

    package = json.loads((mermaid.APP / "package.json").read_text())
    for name in ("mermaid", "jsdom"):
        version = package["devDependencies"][name]
        assert version[0].isdigit(), f"{name} {version} isn't an exact pin"


# The parser runs where check-all's quick checks run: after `npm ci` in app/.
needs_mermaid = pytest.mark.skipif(
    shutil.which("node") is None or not (APP / "node_modules" / "mermaid").is_dir(),
    reason="needs Node and `npm ci` in app/",
)


@needs_mermaid
def test_mermaid_catches_a_broken_diagram(mermaid):
    diagrams = mermaid.diagrams_in(
        "page.md", DOC + "\n```mermaid\nflowchart LR\n  A[open -->\n```\n"
    )
    errors = mermaid.parse_all(diagrams, shutil.which("node"))
    assert errors[:2] == [None, None]
    assert "Parse error" in errors[2]


@needs_mermaid
def test_every_diagram_in_the_repo_parses(mermaid):
    diagrams = mermaid.find_diagrams()
    assert diagrams, "the developer guide has diagrams"
    assert mermaid.problems(diagrams, mermaid.parse_all(diagrams, shutil.which("node"))) == []
