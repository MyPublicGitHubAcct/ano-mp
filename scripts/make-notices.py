#!/usr/bin/env python3
"""Builds THIRD_PARTY_NOTICES: the licences of everything the app ships.

The file is committed at the repo root, bundled into the app (tauri.conf.json
`bundle.resources`) and shown in Settings › About (PLAN.md §8.2). It covers:

- JUCE, and the code JUCE vendors into the modules the core links (from
  JUCE's own JUCE.spdx.json): each contained package either ships, with
  its licence, or is listed in NOT_SHIPPED with the reason;
- FFmpeg: the LGPL text, the version and configure flags from BUILD_INFO,
  and the source tarball they were built from;
- TagLib (and the utfcpp it bundles), Signalsmith Stretch and its FFT
  library; Catch2 is named as test-only;
- the Rust crates the app links (normal dependencies of the app's crate,
  for each macOS target, from `cargo metadata`);
- the npm packages whose code is in the built frontend (the list the
  Vite build writes, app/vite.config.js).

Each crate and npm package's licence expression must be satisfiable from
`deny.toml`'s allowed licences, or the script fails; it chooses one
alternative of an OR (by PREFERENCE) and copies the package's own licence
files for it, or the standard text from scripts/licenses/ when the package
ships none. Identical texts are printed once, after the packages that use
them.

Needs: the core configured (`cmake --preset debug`: JUCE, TagLib and the
others in build/debug/_deps), FFmpeg built (BUILD_INFO), the crates fetched
(any cargo build), and the frontend built (`npm run build` in app/).
Changes nothing with --check, which fails if the committed file differs.

Usage: scripts/make-notices.py [--check]
"""

import argparse
import dataclasses
import difflib
import json
import pathlib
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTPUT = REPO_ROOT / "THIRD_PARTY_NOTICES"
LICENCE_TEXTS = REPO_ROOT / "scripts" / "licenses"
DEPS = REPO_ROOT / "build" / "debug" / "_deps"
FFMPEG_BUILD_INFO = REPO_ROOT / "third_party" / "ffmpeg" / "macos-universal" / "BUILD_INFO"
CRATE = REPO_ROOT / "app" / "src-tauri"
DENY_TOML = CRATE / "deny.toml"
APP = REPO_ROOT / "app"
BUNDLED_PACKAGES = APP / ".svelte-kit" / "output" / "bundled-packages.json"

# The targets whose crates ship (the universal macOS app).
TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin"]

# Which alternative of an OR expression the notices use, first allowed wins.
PREFERENCE = [
    "MIT",
    "Apache-2.0",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "MIT-0",
    "CC0-1.0",
    "Unicode-3.0",
    "BSD-2-Clause",
    "Apache-2.0 WITH LLVM-exception",
    "CDLA-Permissive-2.0",
    "MPL-2.0",
]

# The JUCE modules the core links (core/CMakeLists.txt) ; their dependencies
# are added from JUCE.spdx.json.
JUCE_MODULES = ["juce_audio_formats", "juce_audio_devices", "juce_dsp"]
# Code JUCE vendors into those modules that the macOS app doesn't contain,
# and why. Anything else they contain must ship with its licence.
NOT_SHIPPED = {
    "FLAC": "compiled out, JUCE_USE_FLAC=0",
    "libogg": "compiled out, JUCE_USE_OGGVORBIS=0",
    "libvorbis": "compiled out, JUCE_USE_OGGVORBIS=0",
    "Oboe": "Android only",
    "ASIO SDK": "Windows only, and only with JUCE_ASIO",
}
# The core's settings NOT_SHIPPED relies on.
REQUIRED_CORE_DEFINITIONS = ["JUCE_USE_FLAC=0", "JUCE_USE_OGGVORBIS=0"]
# Licence files of the vendored code that ships, relative to JUCE's source.
JUCE_VENDORED_LICENCES = {"zlib": "modules/juce_core/zip/zlib/LICENSE"}

LICENCE_FILE = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)", re.IGNORECASE)
# What a licence file's name says it holds (files that say nothing, such as
# LICENSE, are always included).
FILE_KINDS = [
    ("APACHE", "Apache-2.0"),
    ("MIT", "MIT"),
    ("BSD", "BSD"),
    ("ZLIB", "Zlib"),
    ("UNLICENSE", "Unlicense"),
    ("MPL", "MPL"),
    ("ISC", "ISC"),
    ("CC0", "CC0-1.0"),
    ("UNICODE", "Unicode-3.0"),
]

RULE = "=" * 78


class NoticeError(Exception):
    pass


def shown(path):
    """A path as messages show it: relative to the repo when inside it."""
    return path.relative_to(REPO_ROOT) if path.is_relative_to(REPO_ROOT) else path


# --- Licence expressions -------------------------------------------------


def tokens(expression):
    """The tokens of an SPDX expression, with "A/B" read as "A OR B" (an old
    crates.io form) and "X WITH Y" kept as one licence."""
    text = expression.replace("/", " OR ").replace("(", " ( ").replace(")", " ) ")
    words = text.split()
    result = []
    i = 0
    while i < len(words):
        if i + 2 < len(words) and words[i + 1] == "WITH":
            result.append(f"{words[i]} WITH {words[i + 2]}")
            i += 3
        else:
            result.append(words[i])
            i += 1
    return result


def choose(expression, allowed):
    """The licences the notices use for a package: every part of an AND, the
    preferred allowed alternative of an OR. Raises NoticeError if no
    choice is allowed."""
    words = tokens(expression)
    position = 0

    def options():
        # A list of alternatives, each a list of licences that all apply.
        nonlocal position
        alternatives = [conjunction()]
        while position < len(words) and words[position] == "OR":
            position += 1
            alternatives.append(conjunction())
        return alternatives

    def conjunction():
        nonlocal position
        parts = [term()]
        while position < len(words) and words[position] == "AND":
            position += 1
            parts.append(term())
        combined = [[]]
        for part in parts:
            combined = [left + right for left in combined for right in part]
        return combined

    def term():
        nonlocal position
        if position >= len(words):
            raise NoticeError(f"{expression!r}: incomplete licence expression")
        word = words[position]
        position += 1
        if word == "(":
            inner = options()
            if position >= len(words) or words[position] != ")":
                raise NoticeError(f"{expression!r}: unbalanced parentheses")
            position += 1
            return [choice for alternative in inner for choice in alternative]
        if word in ("AND", "OR", ")"):
            raise NoticeError(f"{expression!r}: unexpected {word}")
        return [[word]]

    alternatives = [choice for alternative in options() for choice in alternative]
    if position != len(words):
        raise NoticeError(f"{expression!r}: unexpected {words[position]}")
    usable = [choice for choice in alternatives if all(name in allowed for name in choice)]
    if not usable:
        raise NoticeError(f"{expression!r}: no alternative uses only allowed licences")

    def rank(choice):
        return sorted(
            PREFERENCE.index(name) if name in PREFERENCE else len(PREFERENCE) for name in choice
        )

    return sorted(set(min(usable, key=rank)), key=lambda name: rank([name]))


def allowed_licences(deny_toml):
    """The `allow` list of deny.toml's [licenses] table."""
    match = re.search(
        r"^\[licenses\][^\[]*?^allow\s*=\s*\[(.*?)\]", deny_toml, re.DOTALL | re.MULTILINE
    )
    if match is None:
        raise NoticeError("deny.toml: no [licenses] allow list")
    return set(re.findall(r'"([^"]+)"', match.group(1)))


# --- Licence texts -------------------------------------------------------


def file_kind(name):
    upper = name.upper()
    for marker, kind in FILE_KINDS:
        if marker in upper:
            return kind
    return None


def licence_files(directory, chosen):
    """A package's own licence files that apply to the chosen licences: those
    named for one of them, and those whose name doesn't say."""
    found = []
    for path in sorted(directory.iterdir(), key=lambda path: path.name):
        if not path.is_file() or not LICENCE_FILE.match(path.name):
            continue
        if path.suffix.lower() in (".spdx", ".json", ".toml", ".rs", ".js"):
            continue
        kind = file_kind(path.name)
        if kind is None or any(name.startswith(kind) for name in chosen):
            found.append(path)
    return found


def standard_text(licence, copyright_line, texts=LICENCE_TEXTS):
    path = texts / f"{licence}.txt"
    if not path.is_file():
        raise NoticeError(f"no text for {licence}: add {shown(path)}")
    return path.read_text(encoding="utf-8").replace("{copyright}", copyright_line)


def copyright_line(name, authors):
    names = [re.sub(r"\s*<[^>]*>", "", author).strip() for author in authors]
    names = [author for author in names if author]
    holders = ", ".join(names) if names else f"the {name} authors"
    return f"Copyright (c) {holders}"


def normalise(text):
    """A licence text with its whitespace and line ends made uniform."""
    lines = [line.rstrip() for line in text.replace("\r\n", "\n").replace("\r", "\n").split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


@dataclasses.dataclass
class Package:
    name: str
    version: str
    licence: str
    texts: list


def package(name, version, expression, directory, authors, allowed, texts=LICENCE_TEXTS):
    """A shipped package with its licence texts. Raises NoticeError for a
    licence that isn't allowed or has no text."""
    if not expression:
        raise NoticeError(f"{name} {version}: no licence expression")
    try:
        chosen = choose(expression, allowed)
    except NoticeError as error:
        raise NoticeError(f"{name} {version}: {error}") from None
    files = licence_files(directory, chosen)
    if files:
        found = [normalise(path.read_text(encoding="utf-8", errors="replace")) for path in files]
    else:
        line = copyright_line(name, authors)
        found = [normalise(standard_text(licence, line, texts)) for licence in chosen]
    return Package(name, version, " AND ".join(chosen), found)


# --- Collecting the packages ---------------------------------------------


def cargo_metadata(target):
    """`cargo metadata` for the app's crate on one target (a thin wrapper the
    tests replace)."""
    command = [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--locked",
        "--filter-platform",
        target,
    ]
    output = subprocess.run(command, cwd=CRATE, check=True, capture_output=True, text=True)
    return json.loads(output.stdout)


def shipped_crates(metadata):
    """The packages reachable from the root through normal (not dev or
    build) dependencies, the root excluded."""
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    seen = set()
    stack = [root]
    while stack:
        current = stack.pop()
        if current in seen:
            continue
        seen.add(current)
        for dependency in nodes[current]["deps"]:
            if any(kind["kind"] is None for kind in dependency["dep_kinds"]):
                stack.append(dependency["pkg"])
    seen.discard(root)
    return [package for package in metadata["packages"] if package["id"] in seen]


def crates(allowed, metadata_for=cargo_metadata, targets=TARGETS):
    found = {}
    for target in targets:
        for crate in shipped_crates(metadata_for(target)):
            found[(crate["name"], crate["version"])] = crate
    result = []
    for key in sorted(found):
        crate = found[key]
        if crate.get("license_file") and not crate.get("license"):
            raise NoticeError(f"{crate['name']} {crate['version']}: licence only as a file")
        directory = pathlib.Path(crate["manifest_path"]).parent
        result.append(
            package(*key, crate.get("license"), directory, crate.get("authors") or [], allowed)
        )
    return result


def npm_packages(allowed, app=APP, bundled=BUNDLED_PACKAGES):
    if not bundled.is_file():
        raise NoticeError(f"{shown(bundled)} missing: run `npm run build` in app/")
    result = []
    for name in json.loads(bundled.read_text(encoding="utf-8")):
        directory = app / "node_modules" / name
        manifest = json.loads((directory / "package.json").read_text(encoding="utf-8"))
        author = manifest.get("author")
        if isinstance(author, dict):
            author = author.get("name")
        authors = [author] if author else []
        expression = manifest.get("license")
        result.append(package(name, manifest["version"], expression, directory, authors, allowed))
    return result


# --- Native libraries ----------------------------------------------------


def pinned_version(text, pattern, what):
    match = re.search(pattern, text, re.MULTILINE)
    if match is None:
        raise NoticeError(f"no pinned version found for {what}")
    return match.group(1)


def build_info(text):
    """BUILD_INFO's fields (`key value` lines)."""
    fields = {}
    for line in text.splitlines():
        key, _, value = line.partition(" ")
        if key:
            fields[key] = value.strip()
    for key in ("ffmpeg", "sha256", "configure"):
        if key not in fields:
            raise NoticeError(f"BUILD_INFO has no {key}")
    return fields


def juce_vendored(spdx, modules=JUCE_MODULES):
    """The packages JUCE vendors into the linked modules and their module
    dependencies: (shipped names, not-shipped names). Raises NoticeError
    for one neither shipped with a known licence file nor in NOT_SHIPPED."""
    ids = {package["SPDXID"]: package for package in spdx["packages"]}
    by_name = {package["name"]: package["SPDXID"] for package in spdx["packages"]}
    linked = set()
    stack = [by_name[module] for module in modules]
    contained = []
    while stack:
        current = stack.pop()
        if current in linked:
            continue
        linked.add(current)
        for relationship in spdx["relationships"]:
            if relationship["spdxElementId"] != current:
                continue
            if relationship["relationshipType"] == "DEPENDS_ON":
                stack.append(relationship["relatedSpdxElement"])
            elif relationship["relationshipType"] == "CONTAINS":
                contained.append(ids[relationship["relatedSpdxElement"]])
    shipped, not_shipped = [], []
    for vendored in sorted(contained, key=lambda package: package["name"]):
        name = vendored["name"]
        if name in NOT_SHIPPED:
            not_shipped.append(name)
        elif name in JUCE_VENDORED_LICENCES:
            shipped.append(vendored)
        else:
            raise NoticeError(
                f"JUCE vendors {name} into a linked module: add it to JUCE_VENDORED_LICENCES"
                " or NOT_SHIPPED in make-notices.py"
            )
    return shipped, not_shipped


def core_definition_problems(core_cmake):
    return [
        f"core/CMakeLists.txt no longer sets {definition}, which NOT_SHIPPED relies on"
        for definition in REQUIRED_CORE_DEFINITIONS
        if definition not in core_cmake
    ]


def read(path):
    if not path.is_file():
        raise NoticeError(f"{shown(path)} missing (see make-notices.py's usage)")
    return path.read_text(encoding="utf-8")


def native_sections(root=REPO_ROOT, deps=DEPS, build_info_path=FFMPEG_BUILD_INFO):
    """The JUCE, FFmpeg, TagLib, Signalsmith and Catch2 sections."""
    top_cmake = read(root / "CMakeLists.txt")
    problems = core_definition_problems(read(root / "core" / "CMakeLists.txt"))
    if problems:
        raise NoticeError("; ".join(problems))
    juce_dir = deps / "juce-src"
    spdx = json.loads(read(juce_dir / "JUCE.spdx.json"))
    juce_version = pinned_version(top_cmake, r"^# JUCE (\S+)$", "JUCE")
    juce_licence = read(juce_dir / "LICENSE.md").split("## The JUCE Framework Dependencies")[0]
    shipped, not_shipped = juce_vendored(spdx)
    sections = []

    body = [
        f"JUCE {juce_version} (https://juce.com), linked into the app's audio core.",
        "Modules: juce_core, juce_events, juce_audio_basics, juce_audio_formats,",
        "juce_audio_devices, juce_dsp.",
        "",
        normalise(juce_licence),
    ]
    for vendored in shipped:
        text = read(juce_dir / JUCE_VENDORED_LICENCES[vendored["name"]])
        name, version = vendored["name"], vendored["versionInfo"]
        body += [
            "",
            f"JUCE includes {name} {version} ({vendored['licenseConcluded']}):",
            "",
            normalise(text),
        ]
    if not_shipped:
        listed = "; ".join(f"{name} ({NOT_SHIPPED[name]})" for name in not_shipped)
        body += ["", *wrap_words(f"Also in those modules but not in this app: {listed}.", 78)]
    sections.append(("JUCE", body))

    info = build_info(read(build_info_path))
    version = info["ffmpeg"]
    body = [
        f"FFmpeg {version} (https://ffmpeg.org): libavformat, libavcodec, libswresample",
        "and libavutil, licensed under the GNU Lesser General Public License",
        "version 2.1 or later. They are separate shared libraries in the app",
        "(Contents/Frameworks on macOS), which you may replace with your own build.",
        "",
        f"Source: https://ffmpeg.org/releases/ffmpeg-{version}.tar.xz",
        f"SHA-256: {info['sha256']}",
        "Built unmodified by ano-mp's scripts/build-ffmpeg.sh, configured with:",
        "",
        *wrap_words(info["configure"], 76, indent="  "),
        "",
        normalise(read(LICENCE_TEXTS / "LGPL-2.1.txt")),
    ]
    sections.append(("FFmpeg", body))

    taglib_dir = deps / "taglib-src"
    taglib = pinned_version(read(root / "cmake" / "TagLib.cmake"), r"/v([\d.]+)/taglib-", "TagLib")
    body = [
        f"TagLib {taglib} (https://taglib.org), linked statically. TagLib is",
        "available under the LGPL 2.1 or the Mozilla Public License 1.1; ano-mp",
        "uses it under the MPL 1.1. Its source is at",
        f"https://github.com/taglib/taglib/releases/tag/v{taglib}.",
        "",
        normalise(read(taglib_dir / "COPYING.MPL")),
        "",
        "TagLib includes utfcpp (https://github.com/nemtrif/utfcpp):",
        "",
        normalise(read(taglib_dir / "3rdparty" / "utfcpp" / "LICENSE")),
    ]
    sections.append(("TagLib", body))

    signalsmith = read(root / "cmake" / "Signalsmith.cmake")
    stretch = pinned_version(
        signalsmith, r"signalsmith-stretch/archive/refs/tags/([\d.]+)\.tar", ""
    )
    linear = pinned_version(
        signalsmith, r"Signalsmith-Audio/linear/archive/refs/tags/([\d.]+)\.", ""
    )
    body = [
        f"Signalsmith Stretch {stretch} (https://github.com/Signalsmith-Audio/signalsmith-stretch):",
        "",
        normalise(read(deps / "signalsmith_stretch-src" / "LICENSE.txt")),
        "",
        f"Signalsmith Linear {linear} (https://github.com/Signalsmith-Audio/linear):",
        "",
        normalise(read(deps / "signalsmith_linear-src" / "LICENSE.txt")),
    ]
    sections.append(("Signalsmith Stretch", body))

    catch2 = pinned_version(top_cmake, r"^\s*# Catch2 (\S+)$", "Catch2")
    body = [
        f"Catch2 {catch2} (https://github.com/catchorg/Catch2) runs the core's tests",
        "only. It is not part of the app, so no notice is needed for it.",
    ]
    sections.append(("Catch2 (not shipped)", body))
    return sections


def wrap_words(text, width, indent=""):
    lines, line = [], indent
    for word in text.split():
        if line.strip() and len(line) + 1 + len(word) > width:
            lines.append(line)
            line = indent
        line += ("" if not line.strip() else " ") + word
    if line.strip():
        lines.append(line)
    return lines


# --- The file ------------------------------------------------------------


def package_section(packages, what, source):
    """A list of the packages, then each distinct licence text once, after the
    packages that use it."""
    body = [f"{len(packages)} {what}. {source}", ""]
    body += [f"  {item.name} {item.version} ({item.licence})" for item in packages]
    groups = {}
    for item in packages:
        for text in item.texts:
            groups.setdefault(text, []).append(f"{item.name} {item.version}")
    for text, users in groups.items():
        body += ["", "-" * 78, *wrap_words("Used by: " + ", ".join(users), 78), "-" * 78, "", text]
    return body


def render(sections):
    out = [
        "ano-mp: third-party notices",
        "",
        "ano-mp includes the following third-party software, each under the",
        "licence that follows it. This file is generated by scripts/make-notices.py.",
        "",
        "Contents:",
    ]
    out += [f"  {number}. {title}" for number, (title, _) in enumerate(sections, start=1)]
    for number, (title, body) in enumerate(sections, start=1):
        out += ["", RULE, f"{number}. {title}", RULE, "", *body]
    return "\n".join(out) + "\n"


def build(root=REPO_ROOT):
    allowed = allowed_licences(read(root / "app" / "src-tauri" / "deny.toml"))
    sections = native_sections(root)
    rust = package_section(
        crates(allowed),
        "crates",
        "Each one's source is at https://crates.io/crates/NAME/VERSION.",
    )
    npm = package_section(
        npm_packages(allowed),
        "npm packages",
        "Each one's source is at https://www.npmjs.com/package/NAME/v/VERSION.",
    )
    sections += [("Rust crates", rust), ("npm packages", npm)]
    return render(sections)


def main():
    parser = argparse.ArgumentParser(description="Build THIRD_PARTY_NOTICES.")
    parser.add_argument("--check", action="store_true", help="fail if the file is out of date")
    args = parser.parse_args()
    try:
        text = build()
    except (NoticeError, subprocess.CalledProcessError) as error:
        print(f"make-notices: {error}")
        return 1
    current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.is_file() else ""
    if args.check:
        if text == current:
            print("make-notices: THIRD_PARTY_NOTICES is up to date")
            return 0
        diff = difflib.unified_diff(
            current.splitlines(), text.splitlines(), "committed", "generated", lineterm="", n=1
        )
        print("\n".join(list(diff)[:60]))
        print("make-notices: THIRD_PARTY_NOTICES is out of date: run scripts/make-notices.py")
        return 1
    OUTPUT.write_text(text, encoding="utf-8")
    print(f"make-notices: wrote THIRD_PARTY_NOTICES ({len(text) // 1024} KB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
