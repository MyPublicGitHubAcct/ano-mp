#!/usr/bin/env python3
"""Checks this machine has what the build and checks need (PLAN.md §3, §9.2 M4).

Each tool in PLAN.md §3 at its minimum version, where the repo states one:
CMake's from CMakeLists.txt, Python's from what the scripts use, Node's from
.nvmrc and package.json's `engines`, Rust's from rust-toolchain.toml (with
its targets for this OS), cargo-deny's from the version CI installs. Then
FFmpeg's BUILD_INFO against what scripts/build-ffmpeg.sh would build now,
and on macOS the selected developer directory (the Command Line Tools or
Xcode) and Homebrew's llvm@22 for the fuzzers. Prints a line for each, then
what to install or run. Exits non-zero if anything is missing or too old;
a difference that still works (a newer Node patch than .nvmrc's, a Rust
toolchain rustup will fetch on first use) is a warning only.

--quick checks only what `check-all.py --quick` needs (Python, git, uv,
gitleaks, Node and npm); check-all runs it first in both modes.

Written for every OS: each tool has an install hint for macOS, Linux and
Windows, and checks that don't apply to this OS (llvm@22, the Command Line
Tools, an FFmpeg build that doesn't exist yet) are left out.

Usage: scripts/doctor.py [--quick]
"""

import argparse
import dataclasses
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tomllib

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# check-signing.py's datetime.UTC, and tomllib here.
PYTHON_MINIMUM = (3, 11)

# Where build-ffmpeg.sh puts each OS's FFmpeg (Phases 9-10 add the others).
FFMPEG_PLATFORMS = {"darwin": "macos-universal"}

# The fuzz preset's compiler (CMakePresets.json, build-ffmpeg.sh).
LLVM_FUZZ = pathlib.Path("/opt/homebrew/opt/llvm@22/bin/clang")
LLVM_FUZZ_MAJOR = 22

# The Rust targets rust-toolchain.toml lists, by the OS that builds them.
TARGET_OS = {"apple-darwin": "darwin", "linux": "linux", "windows": "win32"}


@dataclasses.dataclass(frozen=True)
class Tool:
    """A command-line tool: how to ask its version, and how to install it."""

    name: str
    command: list
    install: dict  # sys.platform -> hint; "" for the rest
    quick: bool = False


def hint(darwin, linux, win32):
    return {"darwin": darwin, "linux": linux, "win32": win32}


TOOLS = [
    Tool(
        "git",
        ["git", "--version"],
        hint("xcode-select --install", "sudo apt install git", "winget install Git.Git"),
        quick=True,
    ),
    Tool(
        "uv",
        ["uv", "--version"],
        hint(
            "brew install uv",
            "curl -LsSf https://astral.sh/uv/install.sh | sh",
            "winget install astral-sh.uv",
        ),
        quick=True,
    ),
    Tool(
        "gitleaks",
        ["gitleaks", "version"],
        hint(
            "brew install gitleaks",
            "https://github.com/gitleaks/gitleaks/releases",
            "https://github.com/gitleaks/gitleaks/releases",
        ),
        quick=True,
    ),
    Tool(
        "node",
        ["node", "--version"],
        hint(
            "brew install node (or fnm/nvm with .nvmrc)",
            "fnm or nvm with .nvmrc",
            "fnm or nvm-windows with .nvmrc",
        ),
        quick=True,
    ),
    Tool(
        "npm",
        ["npm", "--version"],
        hint("comes with Node", "comes with Node", "comes with Node"),
        quick=True,
    ),
    Tool(
        "cmake",
        ["cmake", "--version"],
        hint("brew install cmake", "sudo apt install cmake", "winget install Kitware.CMake"),
    ),
    Tool(
        "ninja",
        ["ninja", "--version"],
        hint(
            "brew install ninja", "sudo apt install ninja-build", "winget install Ninja-build.Ninja"
        ),
    ),
    Tool(
        "nasm",
        ["nasm", "-v"],
        hint("brew install nasm", "sudo apt install nasm", "pacman -S nasm (MSYS2)"),
    ),
    Tool(
        "pkg-config",
        ["pkg-config", "--version"],
        hint("brew install pkg-config", "sudo apt install pkg-config", "pacman -S pkgconf (MSYS2)"),
    ),
    Tool(
        "rustup",
        ["rustup", "--version"],
        hint(
            "curl https://sh.rustup.rs -sSf | sh",
            "curl https://sh.rustup.rs -sSf | sh",
            "winget install Rustlang.Rustup",
        ),
    ),
    Tool(
        "cargo-deny",
        ["cargo", "deny", "--version"],
        hint(*["cargo install cargo-deny --version {deny} --locked"] * 3),
    ),
]


@dataclasses.dataclass(frozen=True)
class Finding:
    level: str  # "ok", "warn" or "fail"
    name: str
    detail: str
    fix: str = ""


def version(text):
    """The first dotted version in `text`, as a tuple of ints, or None."""
    match = re.search(r"(\d+)\.(\d+)(?:\.(\d+))?", text)
    if match is None:
        return None
    return tuple(int(part) for part in match.groups() if part is not None)


def shown(numbers):
    return ".".join(str(number) for number in numbers)


def run(command, cwd=REPO_ROOT):
    """Runs `command`; returns (exit status, output), or None if it isn't
    installed."""
    if shutil.which(command[0]) is None:
        return None
    try:
        result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    except OSError:
        return None
    return result.returncode, result.stdout + result.stderr


# The repo's own statements of what it needs.


def cmake_minimum(root=REPO_ROOT):
    text = (root / "CMakeLists.txt").read_text()
    return version(re.search(r"cmake_minimum_required\s*\(\s*VERSION\s+([\d.]+)", text).group(1))


def node_pins(root=REPO_ROOT):
    """(.nvmrc's version, package.json's engines range as (minimum, below))."""
    nvmrc = version((root / ".nvmrc").read_text())
    engines = json.loads((root / "app" / "package.json").read_text())["engines"]["node"]
    minimum = re.search(r">=\s*([\d.]+)", engines)
    below = re.search(r"<\s*([\d.]+)", engines)
    return nvmrc, (
        version(minimum.group(1)) if minimum else None,
        _padded(below.group(1)) if below else None,
    )


def _padded(text):
    """A bound like "27" as (27, 0, 0), so tuples compare."""
    numbers = [int(part) for part in text.split(".")]
    return tuple(numbers + [0] * (3 - len(numbers)))


def rust_pin(root=REPO_ROOT):
    """rust-toolchain.toml's channel and targets."""
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]
    return toolchain["channel"], toolchain.get("targets", [])


def cargo_deny_pin(root=REPO_ROOT):
    """The cargo-deny version CI installs."""
    text = (root / ".github" / "workflows" / "ci.yml").read_text()
    return version(re.search(r"cargo install cargo-deny --version ([\d.]+)", text).group(1))


def targets_for(targets, platform):
    """The targets in `targets` this OS builds."""
    return [
        target
        for target in targets
        if any(key in target and os == platform for key, os in TARGET_OS.items())
    ]


# The checks.


def check_python(info=sys.version_info):
    found = tuple(info[:3])
    if found[:2] < PYTHON_MINIMUM:
        return Finding(
            "fail",
            "python",
            f"{shown(found)}, older than {shown(PYTHON_MINIMUM)}",
            f"install Python {shown(PYTHON_MINIMUM)} or later",
        )
    return Finding("ok", "python", shown(found))


def check_tool(tool, platform, run=run, minimum=None, pins=None):
    """Whether `tool` is installed (at `minimum` or later); its version."""
    fix = tool.install.get(platform, "").format(**(pins or {}))
    result = run(tool.command)
    if result is None or result[0] != 0:
        return Finding("fail", tool.name, "not found", fix)
    found = version(result[1])
    if minimum is not None and (found is None or found < minimum):
        return Finding(
            "fail",
            tool.name,
            f"{shown(found) if found else 'unknown version'}, older than {shown(minimum)}",
            fix,
        )
    detail = shown(found) if found else "installed"
    if minimum is not None:
        detail += f" (>= {shown(minimum)})"
    return Finding("ok", tool.name, detail)


def check_node(found, nvmrc, engines):
    """Node against package.json's engines (npm refuses others) and .nvmrc
    (what CI runs)."""
    minimum, below = engines
    fix = f"install Node {shown(nvmrc)} (.nvmrc), e.g. `fnm use` or `nvm use`"
    if found is None:
        return Finding("fail", "node version", "unknown", fix)
    if (minimum and found < minimum) or (below and found >= below):
        return Finding(
            "fail", "node version", f"{shown(found)}, outside package.json's engines", fix
        )
    if found != nvmrc:
        return Finding("warn", "node version", f"{shown(found)}, not .nvmrc's {shown(nvmrc)}", fix)
    return Finding("ok", "node version", f"{shown(found)} (.nvmrc)")


def check_rust(channel, targets, platform, run=run):
    """rust-toolchain.toml's toolchain and this OS's targets, installed."""
    fix = "rustup toolchain install (in the repo; it reads rust-toolchain.toml)"
    result = run(["rustup", "toolchain", "list"])
    if result is None:
        return [Finding("fail", "rust toolchain", "rustup not found", fix)]
    if not any(line.startswith(f"{channel}-") for line in result[1].splitlines()):
        return [
            Finding(
                "warn",
                "rust toolchain",
                f"{channel} not installed (rustup fetches it on first use)",
                fix,
            )
        ]
    findings = [Finding("ok", "rust toolchain", f"{channel} (rust-toolchain.toml)")]
    wanted = targets_for(targets, platform)
    if wanted:
        installed = run(["rustup", "target", "list", "--installed", "--toolchain", channel])
        have = set(installed[1].split()) if installed and installed[0] == 0 else set()
        missing = [target for target in wanted if target not in have]
        if missing:
            findings.append(Finding("warn", "rust targets", f"missing {', '.join(missing)}", fix))
        else:
            findings.append(Finding("ok", "rust targets", ", ".join(wanted)))
    return findings


def check_cargo_deny(finding, pinned):
    """cargo-deny at CI's version: older fails, another one warns."""
    if finding.level != "ok":
        return finding
    found = version(finding.detail)
    if found is None or found < pinned:
        return dataclasses.replace(
            finding, level="fail", detail=f"{finding.detail}, older than CI's {shown(pinned)}"
        )
    if found != pinned:
        return dataclasses.replace(
            finding, level="warn", detail=f"{finding.detail}, not CI's {shown(pinned)}"
        )
    return finding


def check_ffmpeg(platform, root=REPO_ROOT, run=run):
    """BUILD_INFO against what build-ffmpeg.sh would build now."""
    name = FFMPEG_PLATFORMS.get(platform)
    if name is None:
        return Finding("ok", "ffmpeg", f"no build for {platform} yet (Phases 9-10)")
    fix = "scripts/build-ffmpeg.sh"
    path = root / "third_party" / "ffmpeg" / name / "BUILD_INFO"
    if not path.is_file():
        return Finding("fail", "ffmpeg", f"not built ({path.relative_to(root)} missing)", fix)
    expected = run(["bash", str(root / "scripts" / "build-ffmpeg.sh"), "--info"])
    if expected is None or expected[0] != 0:
        return Finding("fail", "ffmpeg", "build-ffmpeg.sh --info failed", "install bash")
    if path.read_text().strip() != expected[1].strip():
        return Finding("fail", "ffmpeg", f"{name} built from another pin or flags", fix)
    pin = re.search(r"^ffmpeg (\S+)", expected[1], re.MULTILINE)
    return Finding("ok", "ffmpeg", f"{pin.group(1) if pin else ''} {name}, matches build-ffmpeg.sh")


def check_developer_dir(run=run):
    """macOS: a selected developer directory with an SDK."""
    fix = "xcode-select --install (or sudo xcode-select -s /Applications/Xcode.app)"
    selected = run(["xcode-select", "-p"])
    if selected is None or selected[0] != 0 or not selected[1].strip():
        return Finding("fail", "developer tools", "none selected", fix)
    path = selected[1].strip()
    sdk = run(["xcrun", "--show-sdk-path"])
    if sdk is None or sdk[0] != 0:
        return Finding("fail", "developer tools", f"{path} has no macOS SDK", fix)
    kind = "Xcode" if "Xcode" in path else "Command Line Tools"
    return Finding("ok", "developer tools", f"{kind} ({path})")


def check_llvm_fuzz(run=run, clang=LLVM_FUZZ):
    """macOS: Homebrew's llvm@22, the fuzzers' compiler."""
    fix = "brew install llvm@22"
    result = run([str(clang), "--version"]) if clang.exists() else None
    if result is None or result[0] != 0:
        return Finding("fail", "llvm@22", "not found (the fuzzers' compiler)", fix)
    found = version(result[1])
    if found is None or found[0] != LLVM_FUZZ_MAJOR:
        return Finding("fail", "llvm@22", f"{clang} is {shown(found) if found else 'unknown'}", fix)
    return Finding("ok", "llvm@22", shown(found))


def checks(quick, platform=sys.platform, run=run, root=REPO_ROOT):
    """Every finding for this OS, in order."""
    nvmrc, engines = node_pins(root)
    deny = cargo_deny_pin(root)
    minimums = {"cmake": cmake_minimum(root)}
    pins = {"deny": shown(deny)}

    findings = [check_python()]
    for tool in TOOLS:
        if quick and not tool.quick:
            continue
        finding = check_tool(tool, platform, run, minimums.get(tool.name), pins)
        if tool.name == "cargo-deny":
            finding = check_cargo_deny(finding, deny)
        findings.append(finding)
        if tool.name == "node" and finding.level == "ok":
            findings.append(check_node(version(finding.detail), nvmrc, engines))
        if tool.name == "rustup" and finding.level == "ok":
            channel, targets = rust_pin(root)
            findings.extend(check_rust(channel, targets, platform, run))
    if quick:
        return findings
    if platform == "darwin":
        findings.append(check_developer_dir(run))
        findings.append(check_llvm_fuzz(run))
    findings.append(check_ffmpeg(platform, root, run))
    return findings


LABELS = {"ok": "ok  ", "warn": "warn", "fail": "FAIL"}


def report(findings, out=print):
    """Prints each finding, then the fixes; returns the exit status."""
    for finding in findings:
        out(f"{LABELS[finding.level]}  {finding.name}: {finding.detail}")
    fixes = []
    for finding in findings:
        if finding.level != "ok" and finding.fix and finding.fix not in fixes:
            fixes.append(finding.fix)
    if fixes:
        out("\nTo install or update:")
        for fix in fixes:
            out(f"  {fix}")
    failed = sum(finding.level == "fail" for finding in findings)
    if failed:
        out(f"\ndoctor: {failed} missing or too old")
        return 1
    return 0


def main():
    parser = argparse.ArgumentParser(description="Check the tools the build and checks need.")
    parser.add_argument("--quick", action="store_true", help="only what check-all.py --quick needs")
    args = parser.parse_args()
    return report(checks(args.quick))


if __name__ == "__main__":
    sys.exit(main())
