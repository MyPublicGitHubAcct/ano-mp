#!/usr/bin/env python3
"""Checks that the Rust FFI declarations match the core's C API.

Every function declared in core/include/anomp/anomp.h must be declared in the
`extern "C"` block of app/src-tauri/src/anomp.rs with the same number of
parameters, and that block must declare nothing the header lacks. The few
functions Rust has no use for are listed in NOT_BOUND, with the reason. Lists every
problem it finds and exits non-zero if there are any. Changes nothing.

Usage: scripts/check-c-api.py
"""

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
HEADER = REPO_ROOT / "core" / "include" / "anomp" / "anomp.h"
BINDINGS = REPO_ROOT / "app" / "src-tauri" / "src" / "anomp.rs"

# Functions in anomp.h that anomp.rs deliberately doesn't declare.
NOT_BOUND = {
    # Rust builds the struct itself (TrackOptions::to_raw).
    "anomp_track_options_default": "for C callers",
    # Stands in for the OS in the core's tests.
    "anomp_media_controls_perform": "for tests",
}


def strip_c_comments(text):
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", " ", text)


def strip_rust_comments(text):
    return re.sub(r"//[^\n]*", " ", text)


def split_top_level(text, separator):
    """Splits on `separator` outside any (), [] or {}."""
    parts, depth, start = [], 0, 0
    for i, char in enumerate(text):
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
        elif char == separator and depth == 0:
            parts.append(text[start:i])
            start = i + 1
    parts.append(text[start:])
    return parts


def count_params(params):
    params = params.strip()
    if params in ("", "void"):
        return 0
    return len([p for p in split_top_level(params, ",") if p.strip()])


def matching_close(text, open_index):
    """The index of the bracket closing the one at `open_index`."""
    pairs = {"(": ")", "{": "}", "[": "]"}
    stack = []
    for i in range(open_index, len(text)):
        if text[i] in pairs:
            stack.append(pairs[text[i]])
        elif stack and text[i] == stack[-1]:
            stack.pop()
            if not stack:
                return i
    raise ValueError(f"unbalanced bracket at offset {open_index}")


C_FUNCTION = re.compile(r"\b(anomp_\w+)\s*\(")


def c_functions(header_text):
    """{name: parameter count} for each function the header declares."""
    text = strip_c_comments(header_text)
    # The C++ guards: an extern "C" brace opened and closed in #ifdefs.
    text = re.sub(r'#ifdef\s+__cplusplus\s+extern\s+"C"\s*\{\s*#endif', " ", text)
    text = re.sub(r"#ifdef\s+__cplusplus\s+\}\s*#endif", " ", text)
    text = re.sub(r"^\s*#[^\n]*", " ", text, flags=re.MULTILINE)
    functions = {}
    for statement in split_top_level(text, ";"):
        statement = statement.strip()
        # Typedefs (function pointer types among them) and struct bodies
        # are not functions.
        if not statement or statement.startswith(("typedef", "struct", "enum", "}")):
            continue
        match = C_FUNCTION.search(statement)
        if match is None:
            continue
        close = matching_close(statement, match.end() - 1)
        functions[match.group(1)] = count_params(statement[match.end() : close])
    return functions


RUST_FUNCTION = re.compile(r"\bfn\s+(anomp_\w+)\s*\(")


def rust_functions(bindings_text):
    """{name: parameter count} for each function in the `extern "C"` blocks."""
    text = strip_rust_comments(bindings_text)
    functions = {}
    for block in re.finditer(r'extern\s+"C"\s*\{', text):
        close = matching_close(text, block.end() - 1)
        body = text[block.end() : close]
        for match in RUST_FUNCTION.finditer(body):
            params_close = matching_close(body, match.end() - 1)
            functions[match.group(1)] = count_params(body[match.end() : params_close])
    return functions


def compare(c, rust, not_bound=NOT_BOUND):
    """A list of problems, one line each."""
    problems = []
    for name in sorted(c.keys() - rust.keys() - not_bound.keys()):
        problems.append(f"{name}: declared in anomp.h but not in anomp.rs")
    for name in sorted(not_bound.keys() - c.keys()):
        problems.append(f"{name}: listed in NOT_BOUND but not declared in anomp.h")
    for name in sorted(not_bound.keys() & rust.keys()):
        problems.append(f"{name}: listed in NOT_BOUND but declared in anomp.rs")
    for name in sorted(rust.keys() - c.keys()):
        problems.append(f"{name}: declared in anomp.rs but not in anomp.h")
    for name in sorted(c.keys() & rust.keys()):
        if c[name] != rust[name]:
            problems.append(f"{name}: {c[name]} parameters in anomp.h, {rust[name]} in anomp.rs")
    return problems


def main():
    parser = argparse.ArgumentParser(description="Check anomp.rs's FFI against anomp.h.")
    parser.parse_args()

    c = c_functions(HEADER.read_text(encoding="utf-8"))
    rust = rust_functions(BINDINGS.read_text(encoding="utf-8"))
    problems = compare(c, rust)
    for problem in problems:
        print(f"check-c-api: {problem}")
    if not problems:
        print(f"check-c-api: {len(rust)} functions match ({len(NOT_BOUND)} not bound)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
