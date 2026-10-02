import pytest


@pytest.fixture
def c_api(script):
    return script("check-c-api")


def test_header_functions_and_parameter_counts(c_api, fixtures):
    header = (fixtures / "c-api" / "anomp.h").read_text()
    assert c_api.c_functions(header) == {
        "anomp_version": 0,
        "anomp_engine_set_event_callback": 3,
        "anomp_engine_load": 5,
        "anomp_engine_play": 1,
    }


def test_extern_block_functions_and_parameter_counts(c_api, fixtures):
    bindings = (fixtures / "c-api" / "anomp.rs").read_text()
    assert c_api.rust_functions(bindings) == {
        "anomp_version": 0,
        "anomp_engine_set_event_callback": 3,
        "anomp_engine_load": 5,
        "anomp_engine_play": 1,
    }


def test_matching_sides_have_no_problems(c_api, fixtures):
    c = c_api.c_functions((fixtures / "c-api" / "anomp.h").read_text())
    rust = c_api.rust_functions((fixtures / "c-api" / "anomp.rs").read_text())
    assert c_api.compare(c, rust, not_bound={}) == []


def test_every_mismatch_is_listed(c_api, fixtures):
    c = c_api.c_functions((fixtures / "c-api" / "anomp.h").read_text())
    rust = c_api.rust_functions((fixtures / "c-api" / "anomp-broken.rs").read_text())
    assert c_api.compare(c, rust, not_bound={}) == [
        "anomp_engine_play: declared in anomp.h but not in anomp.rs",
        "anomp_engine_pause: declared in anomp.rs but not in anomp.h",
        "anomp_engine_load: 5 parameters in anomp.h, 4 in anomp.rs",
    ]


def test_not_bound_functions_are_allowed_but_kept_accurate(c_api):
    c = {"anomp_a": 0, "anomp_b": 1}
    rust = {"anomp_a": 0}
    assert c_api.compare(c, rust, not_bound={"anomp_b": "for tests"}) == []
    assert c_api.compare(c, rust, not_bound={"anomp_b": "", "anomp_gone": ""}) == [
        "anomp_gone: listed in NOT_BOUND but not declared in anomp.h"
    ]
    assert c_api.compare(c, {**rust, "anomp_b": 1}, not_bound={"anomp_b": ""}) == [
        "anomp_b: listed in NOT_BOUND but declared in anomp.rs"
    ]


def test_real_tree_passes(c_api):
    c = c_api.c_functions(c_api.HEADER.read_text(encoding="utf-8"))
    rust = c_api.rust_functions(c_api.BINDINGS.read_text(encoding="utf-8"))
    assert len(c) > 50
    assert c_api.compare(c, rust) == []
