#!/usr/bin/env python3
"""Checks that every Mermaid diagram in the docs parses (PLAN.md D4).

GitHub draws a ```mermaid block with a syntax error as an error box, and
nothing else would notice. This finds every such block in the repo's
Markdown (what `git ls-files` lists, untracked files included unless
ignored), indented ones in lists too, and parses each with `mermaid.parse`
from the Mermaid pinned in app/package.json (installed by `npm ci` in
app/), run by Node with jsdom's window. Parsing needs no browser, so
nothing is rendered: a diagram that parses but draws badly isn't caught.

Lists each diagram that fails, with its file, line and Mermaid's message,
and exits non-zero if there are any. Changes nothing.

Usage: scripts/check-mermaid.py
"""

import dataclasses
import json
import pathlib
import re
import shutil
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
APP = REPO_ROOT / "app"

OPENING = re.compile(r"^(\s*)```mermaid\s*$")

# Reads the diagrams as a JSON list of strings on stdin and prints a JSON
# list holding, for each, null or Mermaid's error message. Run from app/,
# so `mermaid` resolves to its node_modules. Mermaid sanitises labels with
# DOMPurify, which needs a window: jsdom's, set before Mermaid loads.
PARSER = """
import { JSDOM } from "jsdom";
const { window } = new JSDOM("<!doctype html><body></body>");
globalThis.window = window;
globalThis.document = window.document;
const { default: mermaid } = await import("mermaid");
mermaid.initialize({ startOnLoad: false });
let input = "";
for await (const chunk of process.stdin) input += chunk;
const results = [];
for (const source of JSON.parse(input)) {
  try {
    await mermaid.parse(source);
    results.push(null);
  } catch (error) {
    results.push(String(error?.message ?? error));
  }
}
console.log(JSON.stringify(results));
"""


@dataclasses.dataclass(frozen=True)
class Diagram:
    doc: str
    line: int  # 1-based, the opening fence's
    source: str


def diagrams_in(doc, text):
    """The ```mermaid blocks in a Markdown text, each with its fence's
    indentation taken off its lines."""
    found, lines = [], text.split("\n")
    index = 0
    while index < len(lines):
        opening = OPENING.match(lines[index])
        if opening is None:
            index += 1
            continue
        indent, body = len(opening.group(1)), []
        start = index
        index += 1
        while index < len(lines) and not lines[index].lstrip().startswith("```"):
            line = lines[index]
            body.append(line[indent:] if not line[:indent].strip() else line.lstrip())
            index += 1
        found.append(Diagram(doc, start + 1, "\n".join(body)))
        index += 1
    return found


def markdown_files(root):
    output = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "*.md"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return sorted(path for path in output.split("\0") if path and (root / path).is_file())


def find_diagrams(root=REPO_ROOT):
    diagrams = []
    for doc in markdown_files(root):
        diagrams += diagrams_in(doc, (root / doc).read_text(encoding="utf-8"))
    return diagrams


def parse_all(diagrams, node, app=APP):
    """Mermaid's error message for each diagram, or None where it parses."""
    result = subprocess.run(
        [node, "--input-type=module", "-e", PARSER],
        cwd=app,
        input=json.dumps([diagram.source for diagram in diagrams]),
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(f"Mermaid's parser failed:\n{result.stderr.strip()}")
    return json.loads(result.stdout)


def problems(diagrams, errors):
    return [
        f"{diagram.doc}:{diagram.line}: Mermaid diagram doesn't parse: {error}"
        for diagram, error in zip(diagrams, errors, strict=True)
        if error is not None
    ]


def main():
    node = shutil.which("node")
    if node is None:
        sys.exit("error: node not found; install Node.js (see .nvmrc)")
    if not (APP / "node_modules" / "mermaid").is_dir():
        sys.exit("error: Mermaid isn't installed; run `npm ci` in app/")
    diagrams = find_diagrams()
    try:
        found = problems(diagrams, parse_all(diagrams, node))
    except RuntimeError as error:
        sys.exit(f"check-mermaid: {error}")
    for problem in found:
        print(problem)
    if found:
        print(f"check-mermaid: {len(found)} of {len(diagrams)} diagrams don't parse")
        return 1
    print(f"check-mermaid: all {len(diagrams)} diagrams parse")
    return 0


if __name__ == "__main__":
    sys.exit(main())
