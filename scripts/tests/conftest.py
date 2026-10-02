"""Shared helpers for the scripts' tests.

The scripts are named with hyphens, so they can't be imported by name; the
`script` fixture loads one as a module.
"""

import importlib.util
import pathlib

import pytest

SCRIPTS = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = pathlib.Path(__file__).resolve().parent / "fixtures"


def load_script(name):
    path = SCRIPTS / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture
def script():
    return load_script


@pytest.fixture
def fixtures():
    return FIXTURES
