import pathlib

import pytest


@pytest.fixture
def doctor(script):
    return script("doctor")


def fake(outputs):
    """A `run` answering each command by its first words; None (not
    installed) for the rest."""

    def run(command, cwd=None):
        for prefix, answer in outputs.items():
            if " ".join(command).startswith(prefix):
                return answer
        return None

    return run


def test_version_reads_the_first_dotted_number(doctor):
    assert doctor.version("cmake version 4.4.3\n") == (4, 4, 3)
    assert doctor.version("v26.10.0") == (26, 10, 0)
    assert doctor.version("NASM version 3.02 compiled on") == (3, 2)
    assert doctor.version("gitleaks version 8.30.1") == (8, 30, 1)
    assert doctor.version("no version here") is None


def test_a_missing_tool_fails_with_this_oss_hint(doctor):
    tool = doctor.Tool("ninja", ["ninja", "--version"], doctor.hint("brew a", "apt b", "winget c"))
    for platform, fix in (("darwin", "brew a"), ("linux", "apt b"), ("win32", "winget c")):
        finding = doctor.check_tool(tool, platform, fake({}))
        assert (finding.level, finding.detail, finding.fix) == ("fail", "not found", fix)
    failing = fake({"ninja": (1, "")})
    assert doctor.check_tool(tool, "darwin", failing).level == "fail"


def test_a_tool_below_its_minimum_fails(doctor):
    tool = doctor.Tool("cmake", ["cmake", "--version"], doctor.hint("brew", "apt", "winget"))
    old = doctor.check_tool(tool, "darwin", fake({"cmake": (0, "cmake version 3.24.9")}), (3, 25))
    assert old.level == "fail"
    assert "older than 3.25" in old.detail
    new = doctor.check_tool(tool, "darwin", fake({"cmake": (0, "cmake version 3.25.0")}), (3, 25))
    assert (new.level, new.detail) == ("ok", "3.25.0 (>= 3.25)")


def test_hints_take_the_pinned_versions(doctor):
    deny = next(tool for tool in doctor.TOOLS if tool.name == "cargo-deny")
    finding = doctor.check_tool(deny, "linux", fake({}), pins={"deny": "0.20.2"})
    assert finding.fix == "cargo install cargo-deny --version 0.20.2 --locked"


def test_every_tool_has_a_hint_for_every_os(doctor):
    for tool in doctor.TOOLS:
        assert set(tool.install) == {"darwin", "linux", "win32"}, tool.name
        assert all(tool.install.values()), tool.name


def test_python_below_the_minimum_fails(doctor):
    assert doctor.check_python((3, 11, 0)).level == "ok"
    assert doctor.check_python((3, 10, 14)).level == "fail"


def test_node_against_engines_and_nvmrc(doctor):
    nvmrc, engines = (26, 10, 0), ((26, 10, 0), (27, 0, 0))
    assert doctor.check_node((26, 10, 0), nvmrc, engines).level == "ok"
    assert doctor.check_node((26, 11, 2), nvmrc, engines).level == "warn"
    assert doctor.check_node((26, 9, 0), nvmrc, engines).level == "fail"
    assert doctor.check_node((27, 0, 0), nvmrc, engines).level == "fail"
    assert doctor.check_node(None, nvmrc, engines).level == "fail"


def test_cargo_deny_at_cis_version(doctor):
    ok = doctor.Finding("ok", "cargo-deny", "0.20.2")
    assert doctor.check_cargo_deny(ok, (0, 20, 2)).level == "ok"
    assert doctor.check_cargo_deny(ok, (0, 21, 0)).level == "fail"
    assert doctor.check_cargo_deny(ok, (0, 19, 0)).level == "warn"
    missing = doctor.Finding("fail", "cargo-deny", "not found")
    assert doctor.check_cargo_deny(missing, (0, 20, 2)) == missing


def test_rust_toolchain_and_this_oss_targets(doctor):
    targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"]
    assert doctor.targets_for(targets, "darwin") == targets[:2]
    assert doctor.targets_for(targets, "linux") == targets[2:]
    assert doctor.targets_for(targets, "win32") == []

    listed = {
        "rustup toolchain list": (
            0,
            "stable-aarch64-apple-darwin\n1.98.1-aarch64-apple-darwin (active)\n",
        )
    }
    run = fake({**listed, "rustup target list": (0, "aarch64-apple-darwin\n")})
    toolchain, missing = doctor.check_rust("1.98.1", targets, "darwin", run)
    assert toolchain.level == "ok"
    assert (missing.level, missing.detail) == ("warn", "missing x86_64-apple-darwin")

    run = fake({**listed, "rustup target list": (0, "aarch64-apple-darwin\nx86_64-apple-darwin\n")})
    assert [finding.level for finding in doctor.check_rust("1.98.1", targets, "darwin", run)] == [
        "ok",
        "ok",
    ]

    absent = fake({"rustup toolchain list": (0, "stable-aarch64-apple-darwin\n")})
    assert [
        finding.level for finding in doctor.check_rust("1.98.1", targets, "darwin", absent)
    ] == ["warn"]


INFO = "ffmpeg 9.0.2\nsha256 abc\nplatform macos-universal\nmin_os 14.0\nconfigure --x\n"


def ffmpeg_tree(tmp_path, built):
    if built is not None:
        path = tmp_path / "third_party" / "ffmpeg" / "macos-universal" / "BUILD_INFO"
        path.parent.mkdir(parents=True)
        path.write_text(built)
    return tmp_path


def test_ffmpeg_build_info_against_the_script(doctor, tmp_path):
    run = fake({"bash": (0, INFO)})
    current = doctor.check_ffmpeg("darwin", ffmpeg_tree(tmp_path / "a", INFO), run)
    assert (current.level, current.detail) == (
        "ok",
        "9.0.2 macos-universal, matches build-ffmpeg.sh",
    )

    stale = doctor.check_ffmpeg(
        "darwin", ffmpeg_tree(tmp_path / "b", INFO.replace("9.0.2", "8.1")), run
    )
    assert (stale.level, stale.fix) == ("fail", "scripts/build-ffmpeg.sh")

    missing = doctor.check_ffmpeg("darwin", ffmpeg_tree(tmp_path / "c", None), run)
    assert missing.level == "fail"
    assert "not built" in missing.detail

    assert doctor.check_ffmpeg("linux", tmp_path, run).level == "ok"


def test_developer_directory_on_macos(doctor):
    clt = fake(
        {"xcode-select": (0, "/Library/Developer/CommandLineTools\n"), "xcrun": (0, "/sdk\n")}
    )
    assert (
        doctor.check_developer_dir(clt).detail
        == "Command Line Tools (/Library/Developer/CommandLineTools)"
    )
    xcode = fake(
        {"xcode-select": (0, "/Applications/Xcode.app/Contents/Developer\n"), "xcrun": (0, "/sdk")}
    )
    assert doctor.check_developer_dir(xcode).detail.startswith("Xcode")
    assert doctor.check_developer_dir(fake({"xcode-select": (2, "error")})).level == "fail"
    assert (
        doctor.check_developer_dir(fake({"xcode-select": (0, "/x\n"), "xcrun": (1, "")})).level
        == "fail"
    )


def test_llvm_for_the_fuzzers(doctor, tmp_path):
    clang = tmp_path / "clang"
    assert doctor.check_llvm_fuzz(fake({}), clang).level == "fail"
    clang.write_text("")
    good = fake({str(clang): (0, "Homebrew clang version 22.1.8\n")})
    assert doctor.check_llvm_fuzz(good, clang).level == "ok"
    other = fake({str(clang): (0, "Homebrew clang version 21.1.0\n")})
    assert doctor.check_llvm_fuzz(other, clang).level == "fail"


def test_quick_checks_only_what_the_quick_steps_need(doctor):
    findings = doctor.checks(quick=True, platform="linux", run=fake({}))
    names = [finding.name for finding in findings]
    assert names == ["python", "git", "uv", "gitleaks", "node", "npm"]
    assert all(finding.level == "fail" for finding in findings[1:])


def test_full_checks_on_each_os(doctor):
    for platform, extra in (
        ("darwin", ["developer tools", "llvm@22", "ffmpeg"]),
        ("linux", ["ffmpeg"]),
        ("win32", ["ffmpeg"]),
    ):
        names = [finding.name for finding in doctor.checks(False, platform, fake({}))]
        assert names[-len(extra) :] == extra, platform
        assert "cmake" in names and "cargo-deny" in names


def test_report_lists_each_fix_once_and_fails_on_a_failure(doctor):
    lines = []
    findings = [
        doctor.Finding("ok", "git", "2.5"),
        doctor.Finding("warn", "node version", "26.11.0", "fnm use"),
        doctor.Finding("fail", "rust", "none", "rustup toolchain install"),
        doctor.Finding("warn", "rust targets", "missing", "rustup toolchain install"),
    ]
    assert doctor.report(findings, lines.append) == 1
    assert lines[:4] == [
        "ok    git: 2.5",
        "warn  node version: 26.11.0",
        "FAIL  rust: none",
        "warn  rust targets: missing",
    ]
    assert lines.count("  rustup toolchain install") == 1
    assert "  fnm use" in lines
    assert doctor.report(findings[:2], [].append) == 0


def test_the_repos_pins_parse(doctor):
    assert doctor.cmake_minimum() >= (3, 25)
    nvmrc, (minimum, below) = doctor.node_pins()
    assert minimum <= nvmrc < below
    channel, targets = doctor.rust_pin()
    assert doctor.version(channel) is not None
    assert targets
    assert doctor.cargo_deny_pin() is not None


def test_build_ffmpeg_prints_its_build_info(doctor):
    script = pathlib.Path(doctor.REPO_ROOT, "scripts", "build-ffmpeg.sh").read_text()
    assert "--info) INFO=1" in script
