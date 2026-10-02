import pytest


@pytest.fixture
def format_frontend(script):
    return script("format-frontend")


def test_check_mode_changes_nothing(format_frontend):
    assert format_frontend.prettier_command("npx", check=True) == [
        "npx",
        "--no",
        "--",
        "prettier",
        "--check",
        ".",
    ]
    assert "--write" in format_frontend.prettier_command("npx", check=False)


def test_prettier_is_pinned_exactly(format_frontend):
    import json

    package = json.loads((format_frontend.APP / "package.json").read_text())
    for name in ("prettier", "prettier-plugin-svelte", "eslint"):
        version = package["devDependencies"][name]
        assert version[0].isdigit(), f"{name} {version} isn't an exact pin"
