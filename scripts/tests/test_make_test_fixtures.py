"""make-test-fixtures.py's signal must be the one TestSignal.h regenerates."""

import math
import re

import pytest


@pytest.fixture
def fixtures_script(script):
    return script("make-test-fixtures")


@pytest.fixture
def test_signal_h(fixtures_script):
    return (fixtures_script.REPO_ROOT / "core" / "tests" / "TestSignal.h").read_text()


def test_amplitude_matches(fixtures_script, test_signal_h):
    match = re.search(r"signalAmplitude\s*=\s*([\d.]+)", test_signal_h)
    assert float(match.group(1)) == fixtures_script.AMPLITUDE


def test_chirps_match(fixtures_script, test_signal_h):
    match = re.search(r"signalChirps\s*\{\s*\{(.*?)\}\s*\};", test_signal_h, re.DOTALL)
    pairs = re.findall(r"\{\s*([\d.]+)\s*,\s*([\d.]+)\s*\}", match.group(1))
    assert [(float(a), float(b)) for a, b in pairs] == fixtures_script.CHIRPS


def test_length_remainder_matches(fixtures_script, test_signal_h):
    match = re.search(r"sampleRate \* seconds\)\s*\+\s*(\d+);", test_signal_h)
    remainder = int(match.group(1))
    assert fixtures_script.signal_length(44100, 0.5) == 22050 + remainder
    assert fixtures_script.signal_length(48000, 4.0) == 192000 + remainder


def test_quantisation_matches(fixtures_script, test_signal_h):
    # TestSignal.h: floor(x * 32767.0 + 0.5).
    assert "32767.0 + 0.5" in test_signal_h
    for x in (-0.5, -0.25, 0.0, 0.123456, 0.5):
        assert fixtures_script.to_int16(x) == math.floor(x * 32767.0 + 0.5)


def test_signal_is_a_linear_chirp(fixtures_script):
    rate, seconds = 44100, 0.5
    duration = fixtures_script.signal_length(rate, seconds) / rate
    n = 1000
    t = n / rate
    phase = 2.0 * math.pi * (200.0 * t + 4800.0 * t * t / (2.0 * duration))
    assert fixtures_script.sample(0, n, rate, seconds) == pytest.approx(0.5 * math.sin(phase))
    assert fixtures_script.sample(0, 0, rate, seconds) == 0.0
