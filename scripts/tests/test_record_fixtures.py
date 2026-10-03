import json

import pytest


@pytest.fixture
def rf(script):
    return script("record-fixtures")


RESPONSE = {
    "created": "2026-10-03T00:00:00Z",
    "count": 3,
    "releases": [
        {
            "id": "a",
            "title": "A",
            "extra": 1,
            "genres": [{"id": "g1", "n": 1}, {"id": "g2", "n": 9}],
        },
        {"id": "b", "title": "B", "extra": 2, "genres": []},
        {"id": "c", "title": "C", "extra": 3, "genres": [{"id": "g2", "n": 4}]},
    ],
    "relations": [
        {"type": "wikidata", "url": {"id": "u1", "resource": "https://w/Q1"}},
        {"type": "discogs", "url": {"id": "u2", "resource": "https://d/1"}},
    ],
}


def test_select_keeps_the_listed_items_in_the_listed_order(rf):
    rule = {"select": {"releases": {"by": ["id"], "items": [["c"], ["a"]]}}}
    value, missing = rf.trim(RESPONSE, rule)
    assert [release["id"] for release in value["releases"]] == ["c", "a"]
    assert missing == []
    assert [release["id"] for release in RESPONSE["releases"]] == ["a", "b", "c"], "unchanged"


def test_select_by_nested_fields_and_inside_lists(rf):
    rule = {
        "select": {
            "relations": {"by": ["type", "url.id"], "items": [["discogs", "u2"]]},
            "releases.genres": {"by": ["id"], "items": [["g2"]]},
        }
    }
    value, _ = rf.trim(RESPONSE, rule)
    assert value["relations"] == [RESPONSE["relations"][1]]
    assert [[genre["n"] for genre in release["genres"]] for release in value["releases"]] == [
        [9],
        [],
        [4],
    ]


def test_select_reports_items_no_longer_upstream(rf):
    rule = {"select": {"releases": {"by": ["id"], "items": [["a"], ["gone"]]}}}
    value, missing = rf.trim(RESPONSE, rule)
    assert [release["id"] for release in value["releases"]] == ["a"]
    assert missing == ["releases ['gone']"]


def test_keep_takes_only_the_shapes_keys(rf):
    rule = {"keep": {"count": True, "releases": {"id": True, "genres": {"id": True}}}}
    value, _ = rf.trim(RESPONSE, rule)
    assert value == {
        "count": 3,
        "releases": [
            {"id": "a", "genres": [{"id": "g1"}, {"id": "g2"}]},
            {"id": "b", "genres": []},
            {"id": "c", "genres": [{"id": "g2"}]},
        ],
    }


def test_set_replaces_values_after_trimming(rf):
    rule = {
        "select": {"releases": {"by": ["id"], "items": [["b"]]}},
        "keep": {"count": True, "releases": {"title": True}},
        "set": {"count": 1, "releases.title": "Stand-in", "missing.path": 0},
    }
    value, _ = rf.trim(RESPONSE, rule)
    assert value == {"count": 1, "releases": [{"title": "Stand-in"}]}


def test_output_is_indented_utf8_with_a_newline(rf):
    assert rf.dumps({"a": "Café", "b": [1]}) == '{\n  "a": "Café",\n  "b": [\n    1\n  ]\n}\n'


def test_the_throttle_waits_a_second_between_requests(rf):
    now = [100.0]
    slept = []

    def sleep(seconds):
        slept.append(round(seconds, 3))
        now[0] += seconds

    throttle = rf.Throttle(1.0, clock=lambda: now[0], sleep=sleep)
    throttle.wait()
    now[0] += 0.25
    throttle.wait()
    now[0] += 3.0
    throttle.wait()
    assert slept == [0.75]


def test_record_fetches_with_the_agent_and_skips_hand_made_files(rf):
    manifest = {
        "x/a.json": {"url": "https://h/a", "keep": {"id": True}},
        "x/b.json": {"made_by_hand": "no token"},
    }
    calls, lines = [], []

    def fetch(url, agent):
        calls.append((url, agent))
        return json.dumps({"id": 1, "noise": 2})

    throttle = rf.Throttle(0, clock=lambda: 0.0, sleep=lambda s: None)
    recorded = rf.record(
        manifest, ["x/a.json", "x/b.json"], "agent/1", fetch, throttle, lines.append
    )
    assert recorded == {"x/a.json": '{\n  "id": 1\n}\n'}
    assert calls == [("https://h/a", "agent/1")]
    assert lines == ["x/b.json: made by hand (no token)"]


def test_coverage_and_problems(rf, tmp_path):
    (tmp_path / "s").mkdir()
    for name in ("s/a.json", "s/b.jpg", "manifest.json", ".DS_Store"):
        (tmp_path / name).write_text("{}")
    manifest = {"s/a.json": {"url": "https://h"}, "s/gone.json": {"made_by_hand": "x"}}
    assert rf.coverage(manifest, tmp_path) == (["s/b.jpg"], ["s/gone.json"])
    assert rf.problems(
        {
            "a": {"url": "http://h"},
            "b": {},
            "c": {"url": "https://h", "made_by_hand": "x"},
            "d": {"url": "https://h", "trim": {}},
        }
    ) == [
        "a: not an https URL",
        "b: needs either a url or made_by_hand",
        "c: needs either a url or made_by_hand",
        "d: unknown rule trim",
    ]


def test_the_user_agent_is_the_apps(rf):
    agent = rf.user_agent()
    assert agent.startswith("ano-mp/")
    assert "( https://" in agent and agent.endswith(" )")


def test_the_real_manifest_covers_every_fixture(rf):
    manifest = rf.load_manifest()
    assert rf.coverage(manifest) == ([], [])
    assert rf.problems(manifest) == []


def test_the_real_fixtures_are_their_own_trimming(rf):
    """Each fetched file, trimmed again by its rule, is unchanged: the rules
    keep everything the recorded files hold."""
    manifest = rf.load_manifest()
    for name, entry in manifest.items():
        if "url" not in entry:
            continue
        text = (rf.FIXTURES / name).read_text()
        value, missing = rf.trim(json.loads(text), entry)
        assert missing == [], name
        assert rf.dumps(value) == text, name
