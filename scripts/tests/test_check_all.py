import pytest


@pytest.fixture
def check_all(script):
    return script("check-all")


def test_quick_leaves_out_the_builds(check_all):
    quick = check_all.select(check_all.STEPS, quick=True)
    full = check_all.select(check_all.STEPS, quick=False)
    assert full == check_all.STEPS
    assert [step for step in full if step not in quick] == [
        step for step in check_all.STEPS if not step.quick
    ]
    assert "core build" not in [step.name for step in quick]
    assert "C API bindings" in [step.name for step in quick]


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
