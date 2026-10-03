import pytest


@pytest.fixture
def check_all(script):
    return script("check-all")


def test_quick_leaves_out_the_builds(check_all):
    quick = check_all.select(check_all.STEPS, quick=True)
    full = check_all.select(check_all.STEPS, quick=False)
    names = [step.name for step in quick]
    assert [step.name for step in full] == [step.name for step in check_all.STEPS]
    assert [step.name for step in full if step.name not in names] == [
        step.name for step in check_all.STEPS if not step.quick
    ]
    assert "core build" not in names
    assert "C API bindings" in names


def test_every_step_runs_and_failures_are_collected(check_all):
    steps = [check_all.Step(name, ["true"]) for name in ("a", "b", "c")]
    ran = []

    def fake_run(step):
        ran.append(step.name)
        return step.name != "b"

    failed = check_all.run(steps, run_step=fake_run, out=lambda *args, **kwargs: None)
    assert ran == ["a", "b", "c"]
    assert failed == ["b"]


def test_every_script_step_exists(check_all):
    for step in check_all.STEPS:
        for part in step.command:
            if str(part).endswith(".py"):
                assert (check_all.REPO_ROOT / "scripts" / part).exists(), part


def test_the_pre_commit_hook_runs_the_quick_checks(check_all):
    hook = check_all.REPO_ROOT / "scripts" / "hooks" / "pre-commit"
    assert hook.stat().st_mode & 0o111, "executable"
    assert "check-all.py" in hook.read_text()


def test_tools_are_checked_first_for_the_run_being_made(check_all):
    quick = check_all.select(check_all.STEPS, quick=True)
    full = check_all.select(check_all.STEPS, quick=False)
    assert quick[0].name == full[0].name == "tools"
    assert quick[0].command[-2:] == [str(check_all.REPO_ROOT / "scripts" / "doctor.py"), "--quick"]
    assert full[0].command[-1] == str(check_all.REPO_ROOT / "scripts" / "doctor.py")
    # Other steps run the same command in both.
    assert quick[1] == full[1]
