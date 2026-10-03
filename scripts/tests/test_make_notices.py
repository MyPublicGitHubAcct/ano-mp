import json
import re

import pytest

ALLOWED = {"MIT", "Apache-2.0", "ISC", "Unicode-3.0", "BSD-3-Clause", "Zlib", "MPL-2.0"}


@pytest.fixture
def notices(script):
    return script("make-notices")


def test_choose(notices):
    assert notices.choose("MIT OR Apache-2.0", ALLOWED) == ["MIT"]
    assert notices.choose("Apache-2.0 OR MIT", ALLOWED) == ["MIT"]
    assert notices.choose("MIT/Apache-2.0", ALLOWED) == ["MIT"]
    assert notices.choose("Apache-2.0 / MIT", ALLOWED) == ["MIT"]
    assert notices.choose("Unlicense OR MIT", ALLOWED) == ["MIT"]
    assert notices.choose("Apache-2.0 AND ISC", ALLOWED) == ["Apache-2.0", "ISC"]
    assert notices.choose("(MIT OR Apache-2.0) AND Unicode-3.0", ALLOWED) == [
        "MIT",
        "Unicode-3.0",
    ]
    assert notices.choose("Zlib OR Apache-2.0 OR MIT", ALLOWED) == ["MIT"]
    assert notices.choose("Apache-2.0 WITH LLVM-exception OR MIT", ALLOWED) == ["MIT"]
    assert notices.choose("Apache-2.0 WITH LLVM-exception", {"Apache-2.0 WITH LLVM-exception"}) == [
        "Apache-2.0 WITH LLVM-exception"
    ]


def test_choose_refuses_unknown_and_malformed(notices):
    for expression in ["GPL-3.0-only", "MIT AND GPL-2.0", "(MIT", "MIT OR", "MIT )", "AND MIT"]:
        with pytest.raises(notices.NoticeError):
            notices.choose(expression, ALLOWED)


def test_allowed_licences(notices):
    text = """
[advisories]
ignore = []

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
]
confidence-threshold = 0.9
"""
    assert notices.allowed_licences(text) == {"MIT", "Apache-2.0"}
    with pytest.raises(notices.NoticeError):
        notices.allowed_licences("[bans]\nallow = []\n")


def test_licence_files(notices, tmp_path):
    for name in [
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "NOTICE",
        "COPYRIGHT",
        "LICENSE.spdx",
        "README.md",
        "license.rs",
    ]:
        (tmp_path / name).write_text(name)
    names = [path.name for path in notices.licence_files(tmp_path, ["MIT"])]
    assert names == ["COPYRIGHT", "LICENSE-MIT", "NOTICE"]
    names = [path.name for path in notices.licence_files(tmp_path, ["Apache-2.0"])]
    assert names == ["COPYRIGHT", "LICENSE-APACHE", "NOTICE"]


def test_package_uses_its_own_files(notices, tmp_path):
    (tmp_path / "LICENSE-MIT").write_text("Copyright (c) Someone\r\n\r\nMIT terms   \n\n")
    (tmp_path / "LICENSE-APACHE").write_text("Apache terms")
    item = notices.package("a", "1.0.0", "MIT OR Apache-2.0", tmp_path, [], ALLOWED)
    assert item.licence == "MIT"
    assert item.texts == ["Copyright (c) Someone\n\nMIT terms"]


def test_package_falls_back_to_a_standard_text(notices, tmp_path):
    texts = tmp_path / "texts"
    texts.mkdir()
    (texts / "MIT.txt").write_text("MIT License\n\n{copyright}\n\nTerms.\n")
    crate = tmp_path / "crate"
    crate.mkdir()
    item = notices.package(
        "objc2", "0.6.4", "MIT", crate, ["Mads <mads@example.com>", "Ann"], ALLOWED, texts
    )
    assert item.texts == ["MIT License\n\nCopyright (c) Mads, Ann\n\nTerms."]
    item = notices.package("x", "1.0.0", "MIT", crate, [], ALLOWED, texts)
    assert "Copyright (c) the x authors" in item.texts[0]
    with pytest.raises(notices.NoticeError, match="no text for ISC"):
        notices.package("y", "1.0.0", "ISC", crate, [], ALLOWED, texts)
    with pytest.raises(notices.NoticeError, match="z 1.0.0: 'GPL-3.0'"):
        notices.package("z", "1.0.0", "GPL-3.0", crate, [], ALLOWED, texts)
    with pytest.raises(notices.NoticeError, match="no licence expression"):
        notices.package("w", "1.0.0", None, crate, [], ALLOWED, texts)


def metadata(tmp_path, licences):
    """A `cargo metadata` result: root -> a (normal) -> b (normal); root -> c
    (dev) and d (build); a -> e (normal and dev)."""
    packages = []
    for name in ["root", "a", "b", "c", "d", "e"]:
        directory = tmp_path / name
        directory.mkdir(exist_ok=True)
        (directory / "LICENSE").write_text(f"licence of {name}")
        packages.append(
            {
                "id": name,
                "name": name,
                "version": "1.0.0",
                "license": licences.get(name, "MIT"),
                "license_file": None,
                "authors": [],
                "manifest_path": str(directory / "Cargo.toml"),
            }
        )

    def edge(target, *kinds):
        return {"pkg": target, "dep_kinds": [{"kind": kind} for kind in kinds]}

    nodes = [
        {"id": "root", "deps": [edge("a", None), edge("c", "dev"), edge("d", "build")]},
        {"id": "a", "deps": [edge("b", None), edge("e", "dev", None)]},
        {"id": "b", "deps": []},
        {"id": "c", "deps": []},
        {"id": "d", "deps": []},
        {"id": "e", "deps": []},
    ]
    return {"packages": packages, "resolve": {"root": "root", "nodes": nodes}}


def test_shipped_crates(notices, tmp_path):
    shipped = notices.shipped_crates(metadata(tmp_path, {}))
    assert sorted(crate["name"] for crate in shipped) == ["a", "b", "e"]


def test_crates(notices, tmp_path):
    found = notices.crates(ALLOWED, lambda target: metadata(tmp_path, {}), ["t1", "t2"])
    assert [(item.name, item.texts) for item in found] == [
        ("a", ["licence of a"]),
        ("b", ["licence of b"]),
        ("e", ["licence of e"]),
    ]
    # A crate only a dev or build dependency may have any licence.
    notices.crates(ALLOWED, lambda target: metadata(tmp_path, {"c": "GPL-3.0"}), ["t"])
    with pytest.raises(notices.NoticeError, match="b 1.0.0"):
        notices.crates(ALLOWED, lambda target: metadata(tmp_path, {"b": "GPL-3.0"}), ["t"])


def test_npm_packages(notices, tmp_path):
    bundled = tmp_path / "bundled-packages.json"
    with pytest.raises(notices.NoticeError, match="npm run build"):
        notices.npm_packages(ALLOWED, tmp_path, bundled)
    bundled.write_text(json.dumps(["@scope/pkg", "svelte"]))
    for name, licence, author in [
        ("@scope/pkg", "MIT OR Apache-2.0", {"name": "Scope"}),
        ("svelte", "MIT", None),
    ]:
        directory = tmp_path / "node_modules" / name
        directory.mkdir(parents=True)
        manifest = {"name": name, "version": "2.0.0", "license": licence}
        if author:
            manifest["author"] = author
        (directory / "package.json").write_text(json.dumps(manifest))
        (directory / "LICENSE").write_text(f"{name} licence")
    found = notices.npm_packages(ALLOWED, tmp_path, bundled)
    assert [(item.name, item.version, item.licence) for item in found] == [
        ("@scope/pkg", "2.0.0", "MIT"),
        ("svelte", "2.0.0", "MIT"),
    ]


def spdx(*contained):
    packages = [
        {"SPDXID": "m1", "name": "juce_dsp"},
        {"SPDXID": "m2", "name": "juce_core"},
        {"SPDXID": "m3", "name": "juce_graphics"},
        {"SPDXID": "z", "name": "zlib", "versionInfo": "1.3", "licenseConcluded": "Zlib"},
        {"SPDXID": "p", "name": "libpng"},
    ]
    relationships = [
        {"spdxElementId": "m1", "relationshipType": "DEPENDS_ON", "relatedSpdxElement": "m2"},
        {"spdxElementId": "m3", "relationshipType": "CONTAINS", "relatedSpdxElement": "p"},
    ]
    for module, package in contained:
        packages.append({"SPDXID": package, "name": package})
        relationships.append(
            {"spdxElementId": module, "relationshipType": "CONTAINS", "relatedSpdxElement": package}
        )
    relationships.append(
        {"spdxElementId": "m2", "relationshipType": "CONTAINS", "relatedSpdxElement": "z"}
    )
    return {"packages": packages, "relationships": relationships}


def test_juce_vendored(notices):
    shipped, not_shipped = notices.juce_vendored(spdx(("m1", "Oboe")), ["juce_dsp"])
    assert [package["name"] for package in shipped] == ["zlib"]
    assert not_shipped == ["Oboe"]
    # libpng is in a module the core doesn't link.
    with pytest.raises(notices.NoticeError, match="JUCE vendors QuickJS"):
        notices.juce_vendored(spdx(("m2", "QuickJS")), ["juce_dsp"])


def test_core_definitions(notices):
    assert notices.core_definition_problems("JUCE_USE_FLAC=0\nJUCE_USE_OGGVORBIS=0") == []
    assert notices.core_definition_problems("JUCE_USE_FLAC=1\nJUCE_USE_OGGVORBIS=0") == [
        "core/CMakeLists.txt no longer sets JUCE_USE_FLAC=0, which NOT_SHIPPED relies on"
    ]


def test_build_info(notices):
    info = notices.build_info("ffmpeg 9.0.2\nsha256 abc\nconfigure --disable-everything --x\n")
    assert info == {"ffmpeg": "9.0.2", "sha256": "abc", "configure": "--disable-everything --x"}
    with pytest.raises(notices.NoticeError, match="no configure"):
        notices.build_info("ffmpeg 9.0.2\nsha256 abc\n")


def test_package_section_prints_each_text_once(notices):
    items = [
        notices.Package("a", "1.0.0", "MIT", ["MIT text"]),
        notices.Package("b", "2.0.0", "MIT", ["MIT text"]),
        notices.Package("c", "3.0.0", "ISC", ["ISC text"]),
    ]
    text = "\n".join(notices.package_section(items, "crates", "Source: here."))
    assert text.count("MIT text") == 1
    assert "Used by: a 1.0.0, b 2.0.0" in text
    assert "Used by: c 3.0.0" in text
    assert text.startswith("3 crates. Source: here.")


def test_wrap_words(notices):
    assert notices.wrap_words("aa bb cc dd", 5) == ["aa bb", "cc dd"]
    assert notices.wrap_words("aa bb", 6, indent="  ") == ["  aa", "  bb"]


def test_real_tree(notices):
    root = notices.REPO_ROOT
    allowed = notices.allowed_licences((root / "app/src-tauri/deny.toml").read_text())
    assert {"MIT", "Apache-2.0"} <= allowed
    assert notices.core_definition_problems((root / "core/CMakeLists.txt").read_text()) == []
    for name in notices.PREFERENCE:
        assert name in allowed or name == "BSD-2-Clause", name
    committed = (root / "THIRD_PARTY_NOTICES").read_text()
    # The committed file names the pinned versions.
    juce = re.search(r"^# JUCE (\S+)$", (root / "CMakeLists.txt").read_text(), re.MULTILINE)
    ffmpeg = re.search(
        r'^FFMPEG_VERSION="([^"]+)"', (root / "scripts/build-ffmpeg.sh").read_text(), re.MULTILINE
    )
    assert f"JUCE {juce.group(1)} " in committed
    assert f"ffmpeg-{ffmpeg.group(1)}.tar.xz" in committed
    for licence in ["MIT", "BSD-3-Clause", "MPL-2.0", "LGPL-2.1"]:
        assert (root / "scripts/licenses" / f"{licence}.txt").is_file()
