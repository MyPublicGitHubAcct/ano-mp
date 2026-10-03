#!/usr/bin/env python3
"""Checks that the repo paths and relative links in the docs exist
(PLAN.md §9.2 M2, §9.1's docs drift row).

Reads PLAN.md, CLAUDE.md, README.md and every Markdown file under docs/.
Two kinds of reference must name something in the repo:
- a relative link, `[text](target)`: resolved from the file's folder;
- a backquoted path, `` `like/this.rs` ``: inline code that contains a
  '/' or ends with a source or config file's extension. The docs often
  shorten paths (`library/access.rs` for app/src-tauri/src/library/
  access.rs), so it may be relative to the repo, to the file's folder, or
  the tail of a tracked path, at a '/' boundary.

Skipped: fenced code blocks (commands), URLs, absolute and home paths,
placeholders (`<preset>`, `{id}`, `*`), bare extensions (`.cpp`), what
lives outside the repo or is built (IGNORED_PREFIXES: build/, target/,
the app bundle's and the container's insides, and so on), and the names
in NOT_IN_REPO (scripts §9.2 plans, files in fetched dependencies or
written at run time). The repo's files are what `git ls-files` lists,
untracked ones included unless ignored; a file that is only on disk
(ignored, or built) doesn't count, as CI's clean checkout lacks it.

With --counts, also compares the test counts PLAN.md §2 states with the
suites: `ctest --preset debug -N`, `cargo test -- --list` (less its
ignored tests), `npm test`'s pass count and the scripts' collected pytest
tests. That needs the core's debug build, the crate's tests built and
app/node_modules, so it is a full check-all step, not a quick one.

Lists every problem it finds and exits non-zero if there are any. Changes
nothing.

Usage: scripts/check-docs.py [--counts]
"""

import argparse
import pathlib
import posixpath
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DOCS = ["PLAN.md", "CLAUDE.md", "README.md"]
DOCS_DIR = "docs"

# Inline code with one of these endings is a file name even without a '/'.
EXTENSIONS = (
    ".rs",
    ".ts",
    ".mjs",
    ".js",
    ".svelte",
    ".py",
    ".sh",
    ".cpp",
    ".h",
    ".mm",
    ".cmake",
    ".txt",
    ".md",
    ".json",
    ".toml",
    ".yml",
    ".yaml",
    ".sql",
    ".plist",
    ".lock",
)

# Built, fetched or outside the repo: not checked.
IGNORED_PREFIXES = (
    "build/",
    "target/",
    "dist/",
    "third_party/",
    "node_modules/",
    "app/build",
    "app/node_modules/",
    "app/src-tauri/target",
    "src-tauri/target",
    ".svelte-kit/",
    # Inside the app bundle, the container and the user's Library.
    "Contents/",
    "Data/",
    "Library/",
    "Logs/",
    "Caches/",
    "Preferences/",
    "Frameworks/",
    "MacOS/",
)

# Named before they exist, or outside the tree: scripts §9.2 plans (remove
# each when it is written), files in the fetched dependencies, files the
# build writes, and files the app writes at run time.
NOT_IN_REPO = {
    # Planned scripts (PLAN.md §9.2).
    "audit-deps.py",
    "bump-pin.py",
    "check-pins.py",
    "doctor.py",
    "record-fixtures.py",
    "sync-ffmpeg-frameworks.py",
    # In the fetched TagLib and JUCE trees.
    "shortenfile.cpp",
    "JUCE.spdx.json",
    # Written by app/src-tauri/build.rs.
    "permissions/main-window.toml",
    # The app's own files at run time.
    "library.recovery.json",
    "Trash/files",
}

# A host name first (`bandcamp.com/developer`) is a URL without its scheme.
HOST = re.compile(r"^[\w-]+(\.[\w-]+)*\.(com|org|net|io|dev|app|fm)(/|$)")

PLACEHOLDER = re.compile(r"[<>{}*$…|\\]|\.\.\.")
INLINE_CODE = re.compile(r"(?<!`)`([^`\n]+)`(?!`)")
LINK = re.compile(r"(?<!!)\[[^\]\n]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")


def strip_fences(text):
    """The text with fenced code blocks blanked, keeping line numbers."""
    lines, inside = [], False
    for line in text.split("\n"):
        if line.lstrip().startswith("```"):
            inside = not inside
            lines.append("")
        else:
            lines.append("" if inside else line)
    return lines


def candidate_path(code):
    """The repo path a piece of inline code names, or None if it isn't one."""
    code = code.strip()
    if not code or " " in code or "://" in code or "::" in code or "=" in code:
        return None
    if code.startswith(("-", "~", "/", "@", "#", "$", "%", ".")) or PLACEHOLDER.search(code):
        return None
    # A `path:line` or `path#anchor` reference names the path.
    code = re.sub(r":\d+(-\d+)?$", "", code).split("#")[0]
    if not re.fullmatch(r"[\w.@+-]+(/[\w.@+-]*)*", code):
        return None
    if "/" not in code and not code.endswith(tuple(EXTENSIONS)):
        return None
    if code.startswith(IGNORED_PREFIXES) or HOST.match(code):
        return None
    code = code.rstrip("/")
    if not code or code in NOT_IN_REPO:
        return None
    return code


def tracked_paths(root):
    """Every file git tracks or would (not ignored), and every folder that
    holds one, '/'-separated."""
    output = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    files = {path for path in output.split("\0") if path}
    folders = {str(pathlib.PurePosixPath(path).parent) for path in files}
    for folder in list(folders):
        parts = folder.split("/")
        folders.update("/".join(parts[:i]) for i in range(1, len(parts)))
    folders.discard(".")
    return files | folders


def tails(paths):
    """Every '/'-boundary tail of `paths`: `a/b/c` gives `b/c` and `c`."""
    result = set()
    for path in paths:
        parts = path.split("/")
        result.update("/".join(parts[i:]) for i in range(len(parts)))
    return result


def in_repo(path, known):
    """Whether a repo-relative path is a tracked file or folder. Only git's
    list counts, never the disk: an ignored file here (`__pycache__`, a
    build's output) is missing from CI's clean checkout."""
    path = posixpath.normpath(path)
    return path == "." or path in known


def path_exists(path, doc_dir, known, known_tails):
    """Whether a backquoted path names something in the repo."""
    return path in known_tails or in_repo(posixpath.join(doc_dir, path), known)


def link_problem(target, doc, known):
    """A problem with a relative link's target, or None."""
    if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith("#"):
        return None
    path = target.split("#")[0]
    if not path:
        return None
    if not in_repo(posixpath.join(posixpath.dirname(doc), path), known):
        return f"links to {target}, which doesn't exist"
    return None


def check_text(doc, text, known, known_tails):
    """Problems in one doc, each `doc:line: message`."""
    problems = []
    doc_dir = posixpath.dirname(doc)
    for number, line in enumerate(strip_fences(text), start=1):
        for match in LINK.finditer(line):
            problem = link_problem(match.group(1), doc, known)
            if problem:
                problems.append(f"{doc}:{number}: {problem}")
        for match in INLINE_CODE.finditer(line):
            path = candidate_path(match.group(1))
            if path and not path_exists(path, doc_dir, known, known_tails):
                problems.append(f"{doc}:{number}: `{match.group(1)}` isn't in the repo")
    return problems


def docs(root):
    found = [name for name in DOCS if (root / name).exists()]
    found += sorted(p.relative_to(root).as_posix() for p in (root / DOCS_DIR).rglob("*.md"))
    return found


def check(root=REPO_ROOT, known=None):
    known = tracked_paths(root) if known is None else known
    known_tails = tails(known)
    problems = []
    for doc in docs(root):
        text = (root / doc).read_text(encoding="utf-8")
        problems += check_text(doc, text, known, known_tails)
    return problems


# What PLAN.md §2 says about each suite: (name, pattern for its count).
STATED_COUNTS = [
    ("Catch2 tests", re.compile(r"^\| (\d+) passing Catch2 tests", re.MULTILINE)),
    ("cargo tests", re.compile(r"^\| (\d+) passing `cargo test` tests", re.MULTILINE)),
    ("frontend tests", re.compile(r"^\| (\d+) frontend tests", re.MULTILINE)),
    ("script tests", re.compile(r"the scripts' (\d+) pytest tests")),
]


def stated_counts(plan_text):
    """{suite: count} as PLAN.md §2 states them; a suite §2 doesn't
    mention is missing from the result."""
    section = plan_text.split("\n## 2.", 1)[-1].split("\n## 3.", 1)[0]
    counts = {}
    for name, pattern in STATED_COUNTS:
        match = pattern.search(section)
        if match:
            counts[name] = int(match.group(1))
    return counts


def ctest_count(output):
    """`ctest -N`: "Total Tests: N"."""
    match = re.search(r"Total Tests: (\d+)", output)
    return int(match.group(1)) if match else None


def cargo_list_count(output):
    """`cargo test -- --list`: the sum of each binary's "N tests, M benchmarks"."""
    return sum(int(n) for n in re.findall(r"^(\d+) tests?, \d+ benchmarks?$", output, re.MULTILINE))


def node_pass_count(output):
    """`node --test`: "ℹ pass N"."""
    match = re.search(r"^\S* ?pass (\d+)$", output, re.MULTILINE)
    return int(match.group(1)) if match else None


def pytest_collected(output):
    """`pytest --collect-only -q`: "N tests collected"."""
    match = re.search(r"(\d+) tests? collected", output)
    return int(match.group(1)) if match else None


def run(command, cwd):
    """A command's stdout (a thin wrapper the tests replace)."""
    result = subprocess.run(command, cwd=cwd, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(f"{' '.join(command)} failed: {result.stderr.strip()[-500:]}")
    return result.stdout


def suite_counts(root=REPO_ROOT):
    """{suite: count} from the suites themselves."""
    crate = root / "app" / "src-tauri"
    listed = cargo_list_count(run(["cargo", "test", "--", "--list"], crate))
    ignored = cargo_list_count(run(["cargo", "test", "--", "--list", "--ignored"], crate))
    return {
        "Catch2 tests": ctest_count(run(["ctest", "--preset", "debug", "-N"], root)),
        "cargo tests": listed - ignored,
        "frontend tests": node_pass_count(run(["npm", "test"], root / "app")),
        "script tests": pytest_collected(
            run([sys.executable, "scripts/test-python.py", "--collect-only", "-q"], root)
        ),
    }


def count_problems(stated, actual):
    problems = []
    for name, _ in STATED_COUNTS:
        if name not in stated:
            problems.append(f"PLAN.md §2: no count of {name}")
        elif actual.get(name) is None:
            problems.append(f"couldn't count the {name}")
        elif stated[name] != actual[name]:
            problems.append(f"PLAN.md §2: {stated[name]} {name}, but the suite has {actual[name]}")
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description="Check the docs' repo paths and relative links.")
    parser.add_argument(
        "--counts", action="store_true", help="also compare §2's test counts with the suites"
    )
    args = parser.parse_args(argv)
    problems = check()
    if args.counts:
        try:
            actual = suite_counts()
        except RuntimeError as error:
            problems.append(str(error))
        else:
            problems += count_problems(stated_counts((REPO_ROOT / "PLAN.md").read_text()), actual)
    for problem in problems:
        print(problem)
    if problems:
        print(f"check-docs: {len(problems)} problems")
        return 1
    print(f"check-docs: {len(docs(REPO_ROOT))} docs, every path and link found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
