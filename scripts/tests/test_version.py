import json

import pytest

CMAKE = """cmake_minimum_required(VERSION 3.25)
set(CMAKE_OSX_DEPLOYMENT_TARGET "14.0" CACHE STRING "")
project(ano_mp VERSION 0.1.0 LANGUAGES C CXX)
"""

CARGO_TOML = """[package]
name = "ano-mp"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0.0", features = ["derive"] }
"""

CARGO_LOCK = """version = 4

[[package]]
name = "aho-corasick"
version = "1.1.3"

[[package]]
name = "ano-mp"
version = "0.1.0"
dependencies = [
 "serde",
]
"""

TAURI_CONF = """{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "ano-mp",
  "version": "0.1.0",
  "plugins": { "updater": { "version": "9.9.9" } }
}
"""

PACKAGE_JSON = """{
  "name": "ano-mp",
  "version": "0.1.0",
  "devDependencies": {
    "prettier": "3.6.2"
  }
}
"""

PACKAGE_LOCK = """{
  "name": "ano-mp",
  "version": "0.1.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "ano-mp",
      "version": "0.1.0"
    },
    "node_modules/prettier": {
      "version": "3.6.2"
    }
  }
}
"""


@pytest.fixture
def version(script):
    return script("version")


@pytest.fixture
def tree(tmp_path):
    files = {
        "CMakeLists.txt": CMAKE,
        "app/src-tauri/Cargo.toml": CARGO_TOML,
        "app/src-tauri/Cargo.lock": CARGO_LOCK,
        "app/src-tauri/tauri.conf.json": TAURI_CONF,
        "app/package.json": PACKAGE_JSON,
        "app/package-lock.json": PACKAGE_LOCK,
    }
    for name, text in files.items():
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
    return tmp_path


def test_reads_every_copy(version, tree):
    found = version.versions(tree)
    assert set(found) == {name for name, *_ in version.FILES}
    assert set(found.values()) == {"0.1.0"}
    assert version.problems(found) == []


def test_sets_every_copy_and_nothing_else(version, tree):
    before = {name: (tree / name).read_text() for name, *_ in version.FILES}
    version.write(tree, "1.20.3")
    assert set(version.versions(tree).values()) == {"1.20.3"}
    for name, text in before.items():
        after = (tree / name).read_text()
        assert after.replace("1.20.3", "0.1.0") == text, name
    lock = json.loads((tree / "app/package-lock.json").read_text())
    assert lock["packages"]["node_modules/prettier"]["version"] == "3.6.2"
    assert '"version": "9.9.9"' in (tree / "app/src-tauri/tauri.conf.json").read_text()
    assert 'version = "1.1.3"' in (tree / "app/src-tauri/Cargo.lock").read_text()
    assert 'serde = { version = "1.0.0"' in (tree / "app/src-tauri/Cargo.toml").read_text()


def test_refuses_a_bad_version(version, tree):
    for bad in ["1.2", "1.2.3-beta", "01.2.3", "v1.2.3", ""]:
        with pytest.raises(ValueError):
            version.write(tree, bad)
    assert set(version.versions(tree).values()) == {"0.1.0"}


def test_a_missing_copy_changes_nothing(version, tree):
    lock = tree / "app/src-tauri/Cargo.lock"
    lock.write_text(CARGO_LOCK.replace('name = "ano-mp"', 'name = "other"'))
    with pytest.raises(ValueError, match="Cargo.lock: version not found"):
        version.write(tree, "0.2.0")
    assert (tree / "CMakeLists.txt").read_text() == CMAKE


def test_reports_disagreement(version, tree):
    (tree / "app/package.json").write_text(PACKAGE_JSON.replace("0.1.0", "0.1.1"))
    lock = PACKAGE_LOCK.replace('"version": "0.1.0"\n    }', '"version": "0.0.9"\n    }')
    (tree / "app/package-lock.json").write_text(lock)
    (tree / "app/src-tauri/Cargo.toml").write_text(CARGO_TOML.replace('version = "0.1.0"\n', ""))
    assert version.problems(version.versions(tree)) == [
        "app/src-tauri/Cargo.toml: no version found",
        "app/package.json: 0.1.1, but CMakeLists.txt has 0.1.0",
        "app/package-lock.json: 0.1.0 / 0.0.9, but CMakeLists.txt has 0.1.0",
    ]


def test_a_nested_version_isnt_taken_for_the_top_level_one(version, tree):
    conf = TAURI_CONF.replace('  "version": "0.1.0",\n', "")
    (tree / "app/src-tauri/tauri.conf.json").write_text(conf)
    found = version.versions(tree)["app/src-tauri/tauri.conf.json"]
    assert found != "0.1.0"
    assert version.problems(version.versions(tree)) != []


def test_cmake_version_must_be_plain(version):
    found = {"CMakeLists.txt": "0.1", "app/package.json": "0.1"}
    assert version.problems(found) == [
        "CMakeLists.txt: no MAJOR.MINOR.PATCH version in project() (found 0.1)"
    ]


def test_tag(version):
    found = {"CMakeLists.txt": "0.2.0", "app/package.json": "0.2.0"}
    assert version.problems(found, "v0.2.0") == []
    assert version.problems(found, "v0.2.1") == [
        "tag v0.2.1 doesn't name version 0.2.0 (expected v0.2.0)"
    ]
    assert version.problems(found, "0.2.0") != []


def test_real_tree_agrees(version):
    assert version.problems(version.versions(version.REPO_ROOT)) == []
