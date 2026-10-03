import json

import pytest

MACHINE = {"cpu": "Test CPU", "cores": 4, "memory_gb": 8, "os": "macOS 26"}


@pytest.fixture
def bench(script):
    return script("bench")


def baseline(results, budgets=None, machine=MACHINE):
    return {
        "machine": machine,
        "measured": "2026-10-03",
        "budgets": budgets or {},
        "results": {key: {"value": value, "unit": unit} for key, (value, unit) in results.items()},
    }


def test_parse_reads_only_the_bench_lines(bench, fixtures):
    results = bench.parse((fixtures / "bench-output.txt").read_text())
    assert results == {
        "browse.page.album-artist": (22.426, "ms"),
        "browse.node.album-artist": (0.121, "ms"),
        "scan.first": (9.448, "s"),
        "scan.rescan": (0.269, "s"),
        "search.love.all": (11.85, "ms"),
        "queue.json_size": (8.052, "MB"),
        "core.play.mp3": (30.2163, "x"),
        "core.analysis": (403.396, "x"),
    }


def test_an_exact_budget_wins_over_a_pattern(bench):
    budgets = {"search.*": {"max": 100, "unit": "ms"}, "search.k.all": {"max": 60, "unit": "ms"}}
    assert bench.budget_for("search.k.all", budgets)["max"] == 60
    assert bench.budget_for("search.love.all", budgets)["max"] == 100
    assert bench.budget_for("scan.first", budgets) is None


def test_budgets_fail_past_their_max_or_under_their_min(bench):
    budgets = {"search.*": {"max": 100, "unit": "ms"}, "core.play.*": {"min": 20, "unit": "x"}}
    results = {
        "search.k.all": (120.0, "ms"),
        "core.play.mp3": (19.0, "x"),
        "core.play.flac": (900.0, "x"),
    }
    problems, _ = bench.compare(results, baseline({}, budgets), 0.25, same_machine=False)
    assert problems == [
        "core.play.mp3: 19 x, under its budget of 20 x",
        "search.k.all: 120 ms, over its budget of 100 ms",
    ]


def test_a_slower_result_fails_past_the_margin(bench):
    base = baseline({"search.love.all": (10.0, "ms"), "scan.first": (8.0, "s")})
    ok, _ = bench.compare(
        {"search.love.all": (12.4, "ms"), "scan.first": (9.9, "s")}, base, 0.25, True
    )
    assert ok == []
    problems, _ = bench.compare(
        {"search.love.all": (12.6, "ms"), "scan.first": (10.1, "s")}, base, 0.25, True
    )
    assert problems == [
        "scan.first: 10.1 s, +26% against the baseline's 8 s",
        "search.love.all: 12.6 ms, +26% against the baseline's 10 ms",
    ]


def test_small_differences_never_fail(bench):
    # 0.08 ms to 0.2 ms is +150%, but under the 2 ms floor.
    problems, _ = bench.compare(
        {"browse.one_artist": (0.2, "ms")},
        baseline({"browse.one_artist": (0.08, "ms")}),
        0.25,
        True,
    )
    assert problems == []


def test_real_time_multiples_are_better_higher(bench):
    base = baseline({"core.play.mp3": (30.0, "x")})
    assert bench.compare({"core.play.mp3": (60.0, "x")}, base, 0.25, True)[0] == []
    problems, _ = bench.compare({"core.play.mp3": (22.0, "x")}, base, 0.25, True)
    assert problems == ["core.play.mp3: 22 x, -27% against the baseline's 30 x"]


def test_on_another_machine_only_budgets_count(bench):
    base = baseline({"search.love.all": (1.0, "ms")}, {"search.*": {"max": 100, "unit": "ms"}})
    problems, notes = bench.compare(
        {"search.love.all": (50.0, "ms"), "new.key": (1.0, "ms")}, base, 0.25, False
    )
    assert problems == [] and notes == []


def test_missing_and_new_results_are_reported(bench):
    base = baseline({"search.love.all": (10.0, "ms")}, {"scan.*": {"max": 90, "unit": "s"}})
    problems, notes = bench.compare({"browse.page.year": (70.0, "ms")}, base, 0.25, True)
    assert problems == [
        "scan.*: has a budget but no result",
        "search.love.all: in the baseline but not measured",
    ]
    assert notes == ["browse.page.year: new, not in the baseline (--update adds it)"]


def test_a_unit_change_is_a_problem(bench):
    problems, _ = bench.compare(
        {"scan.first": (9000.0, "ms")}, baseline({"scan.first": (9.0, "s")}), 0.25, True
    )
    assert problems == ["scan.first: measured in ms, but the baseline has s"]


def test_main_passes_against_a_matching_baseline(bench, fixtures, tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(bench, "machine", lambda: MACHINE)
    path = tmp_path / "baseline.json"
    results = bench.parse((fixtures / "bench-output.txt").read_text())
    path.write_text(json.dumps(baseline(results, {"core.play.*": {"min": 20, "unit": "x"}})))
    assert bench.main(["--from", str(fixtures / "bench-output.txt"), "--baseline", str(path)]) == 0
    assert "8 results, 0 problems" in capsys.readouterr().out


def test_update_rewrites_results_and_machine_but_keeps_budgets(
    bench, fixtures, tmp_path, monkeypatch
):
    monkeypatch.setattr(bench, "machine", lambda: MACHINE)
    path = tmp_path / "baseline.json"
    budgets = {"scan.first": {"max": 90, "unit": "s"}}
    path.write_text(json.dumps(baseline({"old.key": (1.0, "ms")}, budgets, machine={"cpu": "old"})))
    assert (
        bench.main(
            ["--from", str(fixtures / "bench-output.txt"), "--baseline", str(path), "--update"]
        )
        == 0
    )
    written = json.loads(path.read_text())
    assert written["machine"] == MACHINE
    assert written["budgets"] == budgets
    assert "old.key" not in written["results"]
    assert written["results"]["scan.first"] == {"value": 9.448, "unit": "s"}


def test_updating_one_set_keeps_the_others_results(bench, tmp_path, monkeypatch):
    monkeypatch.setattr(bench, "machine", lambda: MACHINE)
    saved = tmp_path / "core.txt"
    saved.write_text("bench core.play.mp3 31 x\n")
    path = tmp_path / "baseline.json"
    path.write_text(json.dumps(baseline({"scan.first": (9.0, "s"), "core.play.mp3": (30.0, "x")})))
    assert (
        bench.main(["--only", "core", "--from", str(saved), "--baseline", str(path), "--update"])
        == 0
    )
    written = json.loads(path.read_text())["results"]
    assert written == {
        "core.play.mp3": {"value": 31.0, "unit": "x"},
        "scan.first": {"value": 9.0, "unit": "s"},
    }


def test_the_committed_baseline_has_every_budget_measured(bench):
    committed = json.loads(bench.BASELINE.read_text())
    keys = list(committed["results"])
    for pattern, budget in committed["budgets"].items():
        assert budget["unit"] in {"ms", "s", "MB", "x"}
        assert ("max" in budget) != ("min" in budget), pattern
        assert any(bench.fnmatch.fnmatchcase(key, pattern) for key in keys), pattern
    problems, _ = bench.compare(
        {k: (v["value"], v["unit"]) for k, v in committed["results"].items()},
        committed,
        bench.MARGIN,
        True,
    )
    assert problems == []
