#!/usr/bin/env python3
"""Runs the tests of the Python scripts (scripts/tests/) with pytest.

pytest is pinned and run through uvx (https://docs.astral.sh/uv/), so the
scripts themselves stay standard-library only. Extra arguments go to pytest,
e.g. `scripts/test-python.py -k c_api`.

Usage: scripts/test-python.py [PYTEST_ARGS...]
"""

import pathlib
import shutil
import subprocess
import sys

PYTEST_VERSION = "9.1.1"

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def main():
    uvx = shutil.which("uvx")
    if uvx is None:
        sys.exit("error: uvx not found; install uv: https://docs.astral.sh/uv/")

    command = [uvx, "--from", f"pytest=={PYTEST_VERSION}", "pytest", "scripts/tests"]
    return subprocess.run([*command, *sys.argv[1:]], cwd=REPO_ROOT, check=False).returncode


if __name__ == "__main__":
    sys.exit(main())
