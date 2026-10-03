import pytest


@pytest.fixture
def fuzzers(script):
    return script("run-fuzzers")


def test_each_target_runs_for_its_time_over_its_corpus_and_the_fixtures(fuzzers, tmp_path):
    command = fuzzers.fuzz_command("tags", 60, build=tmp_path, fixtures=tmp_path / "fixtures")
    assert command[0] == str(tmp_path / "core" / "fuzz" / "anomp_tagreaderfuzzer")
    assert "-max_total_time=60" in command
    assert f"-artifact_prefix={tmp_path / 'crashes'}/tags-" in command
    # New inputs go to the first directory: the corpus, never the fixtures.
    assert command[-2:] == [str(tmp_path / "corpus" / "tags"), str(tmp_path / "fixtures")]


def test_every_target_is_built_by_the_fuzz_cmake_list(fuzzers):
    cmake = (fuzzers.REPO_ROOT / "core" / "fuzz" / "CMakeLists.txt").read_text()
    for executable in fuzzers.TARGETS.values():
        assert f"add_executable({executable} " in cmake


def test_the_preset_and_ffmpeg_build_use_the_same_compiler(fuzzers):
    presets = (fuzzers.REPO_ROOT / "CMakePresets.json").read_text()
    script = (fuzzers.REPO_ROOT / "scripts" / "build-ffmpeg.sh").read_text()
    assert f"{fuzzers.LLVM}/bin/clang++" in presets
    assert f'FUZZ_LLVM="{fuzzers.LLVM}"' in script


def test_the_build_uses_the_fuzz_developer_dir_when_set(fuzzers, tmp_path):
    env = fuzzers.build_env({"PATH": "/usr/bin", "ANOMP_FUZZ_DEVELOPER_DIR": str(tmp_path)})
    assert env == {
        "PATH": "/usr/bin",
        "ANOMP_FUZZ_DEVELOPER_DIR": str(tmp_path),
        "DEVELOPER_DIR": str(tmp_path),
    }


def test_the_build_keeps_the_environment_without_it(fuzzers):
    assert fuzzers.build_env({"PATH": "/usr/bin"}) == {"PATH": "/usr/bin"}


def test_a_missing_fuzz_developer_dir_fails_the_build(fuzzers, tmp_path):
    assert fuzzers.build_env({"ANOMP_FUZZ_DEVELOPER_DIR": str(tmp_path / "Xcode_0.app")}) is None


def test_the_workflows_point_the_fuzz_build_at_the_same_xcode(fuzzers):
    workflows = fuzzers.REPO_ROOT / ".github" / "workflows"
    dirs = {
        line.split(":", 1)[1].strip()
        for name in ("ci.yml", "fuzz.yml")
        for line in (workflows / name).read_text().splitlines()
        if line.strip().startswith("ANOMP_FUZZ_DEVELOPER_DIR:")
    }
    assert len(dirs) == 1 and next(iter(dirs)).endswith(".app/Contents/Developer")
