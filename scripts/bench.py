#!/usr/bin/env python3
"""Runs the benchmarks and checks them against H18's budgets and a committed
baseline (PLAN.md §9.2 M4, H18; a §8.7 release step).

Two sets, each printing `bench <key> <value> <unit>` lines:
- the Rust benchmarks on a synthetic 50,000-track library
  (app/src-tauri/src/library/bench.rs: search, browse, the queue's list,
  inserts, a 50,000-file scan), run with `cargo test --release`;
- the core's (core/tests/BenchTests.cpp: playback through the engine and the
  visualizer's analysis, as multiples of real time), built by the Release
  `bench` CMake preset.

Each result must be within its budget (scripts/bench-baseline.json's
"budgets", whatever the machine), and no more than --margin (25%) worse
than the baseline's result for that key, unless the difference is under the
unit's floor (2 ms, 0.05 s, 0.1 MB), so tiny timings don't flap. Results
in "x" (times real time) are better higher; every other unit, lower. The
baseline is only compared on the machine it was measured on (CPU, cores,
memory, OS version): elsewhere only the budgets are checked.

--update rewrites the baseline's results and machine from this run,
keeping its budgets. --from FILE reads saved output instead of running
anything (the tests use it).

Not in check-all.py: it needs a release build of the crate (minutes) and the
bench preset's own JUCE build, and shared CI runners time too noisily for
the margin. The Rust benchmarks need the frontend built (app/build), as
`cargo test` does.

Usage: scripts/bench.py [--only rust|core] [--update] [--margin F] [--from FILE]
"""

import argparse
import datetime
import fnmatch
import json
import pathlib
import platform
import re
import subprocess
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
BASELINE = REPO_ROOT / "scripts" / "bench-baseline.json"
CRATE = REPO_ROOT / "app" / "src-tauri"
CORE_TESTS = REPO_ROOT / "build" / "bench" / "core" / "tests" / "anomp_core_tests"

MARGIN = 0.25
# Differences smaller than these never fail the baseline comparison.
FLOORS = {"ms": 2.0, "s": 0.05, "MB": 0.1}
# Units where a larger number is better.
HIGHER_IS_BETTER = {"x"}

LINE = re.compile(r"^bench (\S+) (-?[0-9.]+(?:e[-+]?\d+)?) (\S+)\s*$", re.MULTILINE)

RUST_COMMAND = [
    "cargo",
    "test",
    "--release",
    "--lib",
    "bench",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads",
    "1",
]
CORE_BUILD = [["cmake", "--preset", "bench"], ["cmake", "--build", "--preset", "bench"]]
CORE_COMMAND = [str(CORE_TESTS), "[bench]"]


def parse(output):
    """{key: (value, unit)} from the `bench` lines in `output`; a key
    printed twice keeps its last value."""
    return {key: (float(value), unit) for key, value, unit in LINE.findall(output)}


def in_set(key, only):
    """Whether `key` belongs to the set --only names: the core's keys start
    with "core.", the rest are the Rust benchmarks'."""
    return key.startswith("core.") == (only == "core")


def better_is_higher(unit):
    return unit in HIGHER_IS_BETTER


def budget_for(key, budgets):
    """The budget whose pattern matches `key` (fnmatch; an exact key wins
    over a pattern), or None."""
    if key in budgets:
        return budgets[key]
    for pattern, budget in budgets.items():
        if fnmatch.fnmatchcase(key, pattern):
            return budget
    return None


def check_budget(key, value, unit, budget):
    """A problem with `key`'s budget, or None."""
    if budget["unit"] != unit:
        return f"{key}: measured in {unit}, but its budget is in {budget['unit']}"
    if "max" in budget and value > budget["max"]:
        return f"{key}: {value:g} {unit}, over its budget of {budget['max']:g} {unit}"
    if "min" in budget and value < budget["min"]:
        return f"{key}: {value:g} {unit}, under its budget of {budget['min']:g} {unit}"
    return None


def check_baseline(key, value, unit, base, margin):
    """A problem comparing `key` with its baseline result, or None."""
    base_value, base_unit = base["value"], base["unit"]
    if base_unit != unit:
        return f"{key}: measured in {unit}, but the baseline has {base_unit}"
    if better_is_higher(unit):
        worse = value < base_value * (1 - margin)
    else:
        worse = value > base_value * (1 + margin) and value - base_value >= FLOORS.get(unit, 0.0)
    if worse:
        change = (value - base_value) / base_value * 100 if base_value else float("inf")
        return (
            f"{key}: {value:g} {unit}, {change:+.0f}% against the baseline's {base_value:g} {unit}"
        )
    return None


def compare(results, baseline, margin, same_machine):
    """(problems, notes) for `results` against the baseline's budgets and,
    on the same machine, its results."""
    problems, notes = [], []
    budgets = baseline.get("budgets", {})
    base_results = baseline.get("results", {})
    for key, (value, unit) in sorted(results.items()):
        budget = budget_for(key, budgets)
        if budget is not None:
            problem = check_budget(key, value, unit, budget)
            if problem:
                problems.append(problem)
        if not same_machine:
            continue
        if key in base_results:
            problem = check_baseline(key, value, unit, base_results[key], margin)
            if problem:
                problems.append(problem)
        else:
            notes.append(f"{key}: new, not in the baseline (--update adds it)")
    for pattern in budgets:
        if not any(fnmatch.fnmatchcase(key, pattern) for key in results):
            problems.append(f"{pattern}: has a budget but no result")
    if same_machine:
        for key in sorted(set(base_results) - set(results)):
            problems.append(f"{key}: in the baseline but not measured")
    return problems, notes


def updated(baseline, results, machine, today):
    """The baseline with this run's results and machine, budgets kept."""
    return {
        "machine": machine,
        "measured": today,
        "budgets": baseline.get("budgets", {}),
        "results": {
            key: {"value": value, "unit": unit} for key, (value, unit) in sorted(results.items())
        },
    }


def run(command, cwd=REPO_ROOT):
    """A command's stdout and stderr, echoed (a thin wrapper the tests replace)."""
    print("$ " + " ".join(command), flush=True)
    result = subprocess.run(command, cwd=cwd, check=False, capture_output=True, text=True)
    print(result.stdout, end="")
    print(result.stderr, end="", file=sys.stderr)
    if result.returncode != 0:
        raise RuntimeError(f"{command[0]} failed ({result.returncode})")
    return result.stdout


def sysctl(name):
    try:
        return subprocess.run(
            ["sysctl", "-n", name], check=True, capture_output=True, text=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def machine():
    """What the timings depend on: the CPU, its cores, the memory and the OS."""
    if sys.platform == "darwin":
        memory = sysctl("hw.memsize")
        return {
            "cpu": sysctl("machdep.cpu.brand_string"),
            "cores": int(sysctl("hw.ncpu") or 0),
            "memory_gb": round(int(memory) / 2**30) if memory else 0,
            # The major version: a point update doesn't move the timings.
            "os": f"macOS {platform.mac_ver()[0].split('.')[0]}",
        }
    return {
        "cpu": platform.processor() or platform.machine(),
        "cores": 0,
        "memory_gb": 0,
        "os": platform.platform(),
    }


def measure(only):
    output = ""
    if only in (None, "rust"):
        if not (REPO_ROOT / "app" / "build").is_dir():
            raise RuntimeError("app/build is missing: run `npm run build` in app/ first")
        output += run(RUST_COMMAND, cwd=CRATE)
    if only in (None, "core"):
        for command in CORE_BUILD:
            run(command)
        output += run(CORE_COMMAND)
    return output


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Run the benchmarks against the budgets and baseline."
    )
    parser.add_argument("--only", choices=["rust", "core"], help="run only one set")
    parser.add_argument("--update", action="store_true", help="rewrite the baseline from this run")
    parser.add_argument(
        "--margin", type=float, default=MARGIN, help="how much worse than the baseline fails"
    )
    parser.add_argument(
        "--from", dest="saved", type=pathlib.Path, help="read saved output instead of running"
    )
    parser.add_argument("--baseline", type=pathlib.Path, default=BASELINE, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    baseline = json.loads(args.baseline.read_text()) if args.baseline.exists() else {}
    try:
        output = args.saved.read_text() if args.saved else measure(args.only)
    except RuntimeError as error:
        print(f"bench: {error}")
        return 1
    results = parse(output)
    if not results:
        print("bench: no `bench` lines in the output")
        return 1

    here = machine()
    full = baseline
    if args.only:
        # One set run: compare only its keys, and keep the other set's.
        baseline = {
            **baseline,
            "budgets": {
                k: v for k, v in baseline.get("budgets", {}).items() if in_set(k, args.only)
            },
            "results": {
                k: v for k, v in baseline.get("results", {}).items() if in_set(k, args.only)
            },
        }

    same_machine = baseline.get("machine") == here
    # An update replaces the baseline, so only the budgets are checked.
    problems, notes = compare(results, baseline, args.margin, same_machine and not args.update)
    for key, (value, unit) in sorted(results.items()):
        print(f"  {key:32} {value:12g} {unit}")
    if args.update:
        kept = {
            k: (v["value"], v["unit"])
            for k, v in full.get("results", {}).items()
            if args.only and not in_set(k, args.only)
        }
        new = updated(
            full,
            {**kept, **results},
            here,
            datetime.datetime.now(tz=datetime.timezone.utc).date().isoformat(),
        )
        args.baseline.write_text(json.dumps(new, indent=2, ensure_ascii=False) + "\n")
        print(f"bench: wrote {len(new['results'])} results to {args.baseline.name}")
    elif not same_machine:
        print(
            f"bench: another machine than the baseline's ({baseline.get('machine')}): budgets only"
        )
    for note in notes:
        print(f"note: {note}")
    for problem in problems:
        print(f"FAIL: {problem}")
    print(f"bench: {len(results)} results, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
