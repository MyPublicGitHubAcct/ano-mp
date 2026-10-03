"""Tests of the release tools: build-app.py, check-bundle.py, notarize.py,
check-signing.py and release.py (PLAN.md §8.2, §8.3, §9.2 M6)."""

import datetime
import json
import pathlib

import pytest

ADHOC = """Executable=/x/ano-mp.app/Contents/Frameworks/libavutil.61.dylib
Identifier=libavutil.61
Format=Mach-O universal (x86_64 arm64)
CodeDirectory v=20400 size=1573 flags=0x2(adhoc) hashes=44+2 location=embedded
Signature=adhoc
TeamIdentifier=not set
"""

DEVELOPER_ID = """Executable=/x/ano-mp.app/Contents/MacOS/ano-mp
CodeDirectory v=20500 size=27017 flags=0x10000(runtime) hashes=834+7 location=embedded
Authority=Developer ID Application: Someone (ABCDE12345)
TeamIdentifier=ABCDE12345
"""

RPATHS = """Load command 40
          cmd LC_RPATH
      cmdsize 48
         path @executable_path/../Frameworks (offset 12)
Load command 41
"""

LINKED = """/x/ano-mp:
\t@rpath/libavformat.63.dylib (compatibility version 63.0.0, current version 63.1.102)
\t/usr/lib/libc++.1.dylib (compatibility version 1.0.0, current version 2200.27.0)
/x/ano-mp (architecture arm64):
\t@rpath/libavutil.61.dylib (compatibility version 61.0.0, current version 61.1.102)
"""

ENTITLEMENTS = """Executable=/x/ano-mp.app/Contents/MacOS/ano-mp
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.app-sandbox</key><true/></dict></plist>
"""


@pytest.fixture
def build_app(script):
    return script("build-app")


@pytest.fixture
def bundle(script):
    return script("check-bundle")


@pytest.fixture
def notarize(script):
    return script("notarize")


@pytest.fixture
def signing(script):
    return script("check-signing")


@pytest.fixture
def release(script):
    return script("release")


# --- build-app.py --------------------------------------------------------


def test_identity(build_app):
    assert build_app.identity_from(None, {}) is None
    assert build_app.identity_from(None, {"APPLE_SIGNING_IDENTITY": "-"}) is None
    assert build_app.identity_from("-", {"APPLE_SIGNING_IDENTITY": "Developer ID"}) is None
    assert build_app.identity_from(None, {"APPLE_SIGNING_IDENTITY": "Dev ID"}) == "Dev ID"
    assert build_app.identity_from("Mine", {"APPLE_SIGNING_IDENTITY": "Dev ID"}) == "Mine"


def test_tauri_command_hardens_only_signed_builds(build_app):
    adhoc = build_app.tauri_command("universal-apple-darwin", "app,dmg", None)
    assert adhoc == [
        "npm",
        "run",
        "tauri",
        "build",
        "--",
        "--target",
        "universal-apple-darwin",
        "--bundles",
        "app,dmg",
    ]
    signed = build_app.tauri_command("universal-apple-darwin", "app", "Developer ID: X")
    overlay = json.loads(signed[signed.index("--config") + 1])
    assert overlay == {
        "bundle": {"macOS": {"signingIdentity": "Developer ID: X", "hardenedRuntime": True}}
    }


def test_local_builds_stay_adhoc_and_unhardened(build_app):
    config = json.loads((build_app.REPO_ROOT / "app/src-tauri/tauri.conf.json").read_text())
    assert config["bundle"]["macOS"]["signingIdentity"] == "-"
    assert config["bundle"]["macOS"]["hardenedRuntime"] is False


# --- check-bundle.py -----------------------------------------------------


def test_parsers(bundle):
    assert bundle.parse_archs("x86_64 arm64\n") == {"arm64", "x86_64"}
    assert bundle.parse_install_name("/x/libavutil.61.dylib:\n@rpath/libavutil.61.dylib\n") == (
        "@rpath/libavutil.61.dylib"
    )
    assert bundle.parse_install_name("/x/ano-mp:\n") is None
    assert bundle.parse_linked(LINKED) == {
        "@rpath/libavformat.63.dylib",
        "@rpath/libavutil.61.dylib",
        "/usr/lib/libc++.1.dylib",
    }
    assert bundle.parse_rpaths(RPATHS) == ["@executable_path/../Frameworks"]
    assert bundle.parse_signature(ADHOC) == {"adhoc": True, "runtime": False, "team": None}
    assert bundle.parse_signature(DEVELOPER_ID) == {
        "adhoc": False,
        "runtime": True,
        "team": "ABCDE12345",
    }
    assert bundle.parse_entitlements(ENTITLEMENTS) == {"com.apple.security.app-sandbox": True}
    assert bundle.parse_entitlements("no signature") == {}


EXPECTED = {
    "version": "0.1.0",
    "minimum_os": "14.0",
    "frameworks": ["libavformat.63.dylib", "libavutil.61.dylib"],
    "entitlements": {"com.apple.security.app-sandbox": True},
    "notices": "notices\n",
}


def facts(signature):
    binaries = ["ano-mp", "libavformat.63.dylib", "libavutil.61.dylib"]
    return {
        "info": {"CFBundleShortVersionString": "0.1.0", "LSMinimumSystemVersion": "14.0"},
        "frameworks": ["libavformat.63.dylib", "libavutil.61.dylib"],
        "archs": {name: {"arm64", "x86_64"} for name in binaries},
        "install_names": {
            "libavformat.63.dylib": "@rpath/libavformat.63.dylib",
            "libavutil.61.dylib": "@rpath/libavutil.61.dylib",
        },
        "linked": {
            "ano-mp": {"@rpath/libavformat.63.dylib", "/usr/lib/libc++.1.dylib"},
            "libavformat.63.dylib": {"@rpath/libavutil.61.dylib", "/System/Library/X"},
            "libavutil.61.dylib": set(),
        },
        "rpaths": ["@executable_path/../Frameworks"],
        "signatures": {name: dict(signature) for name in binaries},
        "entitlements": {"com.apple.security.app-sandbox": True},
        "executable": "ano-mp",
        "verify": None,
        "notices": "notices\n",
    }


ADHOC_SIGNATURE = {"adhoc": True, "runtime": False, "team": None}
SIGNED = {"adhoc": False, "runtime": True, "team": "ABCDE12345"}


def test_a_good_bundle(bundle):
    assert bundle.problems(facts(ADHOC_SIGNATURE), EXPECTED, bundle.UNIVERSAL, False) == []
    assert bundle.problems(facts(SIGNED), EXPECTED, bundle.UNIVERSAL, True) == []
    assert bundle.problems(facts(SIGNED), EXPECTED, {"arm64"}, True) == []


def test_bundle_problems(bundle):
    bad = facts(ADHOC_SIGNATURE)
    bad["info"]["CFBundleShortVersionString"] = "0.0.9"
    bad["archs"]["ano-mp"] = {"arm64"}
    bad["install_names"]["libavutil.61.dylib"] = (
        "/Users/x/third_party/ffmpeg/lib/libavutil.61.dylib"
    )
    bad["linked"]["ano-mp"] |= {"/opt/homebrew/lib/libfoo.dylib", "@rpath/libswresample.7.dylib"}
    bad["rpaths"] = []
    bad["verify"] = "a sealed resource is missing or invalid"
    bad["entitlements"] = {}
    bad["notices"] = None
    assert bundle.problems(bad, EXPECTED, bundle.UNIVERSAL, False) == [
        "version 0.0.9, expected 0.1.0",
        "ano-mp: architectures ['arm64'], expected ['arm64', 'x86_64']",
        (
            "libavutil.61.dylib: install name /Users/x/third_party/ffmpeg/lib/libavutil.61.dylib,"
            " expected @rpath/libavutil.61.dylib"
        ),
        "ano-mp: links /opt/homebrew/lib/libfoo.dylib from outside the bundle and the OS",
        "ano-mp: links @rpath/libswresample.7.dylib, which isn't bundled",
        "ano-mp: no @executable_path/../Frameworks rpath (has [])",
        "codesign --verify --deep --strict failed: a sealed resource is missing or invalid",
        "entitlements [], expected Entitlements.plist's ['com.apple.security.app-sandbox']",
        "Contents/Resources/THIRD_PARTY_NOTICES missing",
    ]


def test_signed_bundle_problems(bundle):
    found = bundle.problems(facts(ADHOC_SIGNATURE), EXPECTED, bundle.UNIVERSAL, True)
    assert "ano-mp: not signed with a Developer ID" in found
    assert "ano-mp: hardened runtime off" in found
    mixed = facts(SIGNED)
    mixed["signatures"]["libavutil.61.dylib"]["team"] = "OTHER00000"
    assert bundle.problems(mixed, EXPECTED, bundle.UNIVERSAL, True) == [
        "signed by more than one team: ['ABCDE12345', 'OTHER00000']"
    ]
    stale = facts(SIGNED)
    stale["notices"] = "old\n"
    assert bundle.problems(stale, EXPECTED, bundle.UNIVERSAL, True) == [
        "Contents/Resources/THIRD_PARTY_NOTICES isn't the committed one"
    ]


def test_bundle_expectations_from_the_real_tree(bundle):
    expected = bundle.expected_facts()
    assert expected["frameworks"] == sorted(expected["frameworks"])
    assert all(name.startswith(("libav", "libsw")) for name in expected["frameworks"])
    assert expected["entitlements"]["com.apple.security.app-sandbox"] is True
    assert expected["minimum_os"] == "14.0"


# --- notarize.py ---------------------------------------------------------


def test_credentials(notarize):
    full = {"NOTARY_KEY_PATH": "/k.p8", "NOTARY_KEY_ID": "ID", "NOTARY_ISSUER": "ISS"}
    assert notarize.credentials(full) == ("/k.p8", "ID", "ISS")
    assert notarize.credentials({**full, "NOTARY_ISSUER": ""}) is None
    assert notarize.credentials({}) is None


def test_plan(notarize, tmp_path):
    creds = ("/k.p8", "ID", "ISS")
    steps = notarize.plan(pathlib.Path("/d/ano-mp.dmg"), creds, tmp_path)
    assert [what for what, _ in steps] == ["submit", "staple", "validate", "assess"]
    submit = steps[0][1]
    assert submit[:4] == ["xcrun", "notarytool", "submit", "/d/ano-mp.dmg"]
    assert ["--key", "/k.p8", "--key-id", "ID", "--issuer", "ISS"] == submit[4:10]
    assert "context:primary-signature" in steps[3][1]
    steps = notarize.plan(pathlib.Path("/d/ano-mp.app"), None, tmp_path)
    assert [what for what, _ in steps] == ["zip", "submit", "staple", "validate", "assess"]
    assert steps[1][1][3] == str(tmp_path / "ano-mp.zip")
    assert "$NOTARY_KEY_PATH" in steps[1][1]


def test_submission(notarize):
    assert notarize.submission('{"id": "abc", "status": "Accepted", "message": "ok"}') == (
        "abc",
        "Accepted",
    )
    assert notarize.submission('{"id": "abc", "status": "Invalid"}') == ("abc", "Invalid")


# --- check-signing.py ----------------------------------------------------

FIND_IDENTITY = """Policy: Code Signing
  Matching identities
  1) 0123456789ABCDEF0123456789ABCDEF01234567 "Developer ID Application: Someone (ABCDE12345)"
     1 valid identities found
"""

FIND_CERTIFICATE = """SHA-1 hash: FFFF456789ABCDEF0123456789ABCDEF01234567
-----BEGIN CERTIFICATE-----
OTHER
-----END CERTIFICATE-----
SHA-1 hash: 0123456789ABCDEF0123456789ABCDEF01234567
-----BEGIN CERTIFICATE-----
MINE
-----END CERTIFICATE-----
"""


def test_signing(signing):
    assert signing.identities(FIND_IDENTITY) == [
        (
            "0123456789ABCDEF0123456789ABCDEF01234567",
            "Developer ID Application: Someone (ABCDE12345)",
        )
    ]
    assert signing.identities("     0 valid identities found\n") == []
    pem = signing.certificate_pem(FIND_CERTIFICATE, "0123456789ABCDEF0123456789ABCDEF01234567")
    assert "MINE" in pem and "OTHER" not in pem
    assert signing.certificate_pem(FIND_CERTIFICATE, "AAAA") is None
    expires = signing.not_after("notAfter=Oct  1 12:00:00 2031 GMT\n")
    assert expires == datetime.datetime(2031, 10, 1, 12, tzinfo=datetime.UTC)
    now = datetime.datetime(2031, 8, 15, tzinfo=datetime.UTC)
    assert signing.status(expires, now, 60) == "expires soon"
    assert signing.status(expires, now, 30) == "ok"
    assert signing.status(expires, expires, 60) == "expired"


# --- release.py ----------------------------------------------------------

CHANGELOG = """# Changelog

Intro.

## [Unreleased]

### Added
- Next thing.

## [0.2.0] - 2027-01-10

### Fixed
- A bug.

## [0.1.0] - 2026-11-01

## [0.0.1] - 2026-10-01
- First.
"""


def test_notes(release):
    assert release.notes(CHANGELOG, "0.2.0") == "### Fixed\n- A bug.\n"
    assert release.notes(CHANGELOG, "0.0.1") == "- First.\n"
    assert release.notes(CHANGELOG, "0.1.0") is None  # empty
    assert release.notes(CHANGELOG, "0.3.0") is None
    assert release.notes(CHANGELOG, "0.2") is None


def test_checksums(release, tmp_path):
    (tmp_path / "b.dmg").write_bytes(b"abc")
    (tmp_path / "a.zip").write_bytes(b"")
    text = release.checksums([tmp_path / "b.dmg", tmp_path / "a.zip"])
    assert text == (
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  a.zip\n"
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  b.dmg\n"
    )


def test_real_changelog_has_an_unreleased_section(release):
    text = (release.REPO_ROOT / "CHANGELOG.md").read_text()
    assert release.notes(text, "Unreleased") is not None
