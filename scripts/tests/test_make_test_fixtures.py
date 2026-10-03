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


def test_vorbis_files_get_a_fixed_serial_from_their_name(fixtures_script):
    serial = fixtures_script.serial("/tmp/x/vorbis-44k.ogg")
    assert serial == fixtures_script.serial("vorbis-44k.ogg")
    assert serial != fixtures_script.serial("vorbis-long-44k.ogg")
    assert 0 <= serial < 2**31
    command = fixtures_script.oggenc("5")("in.wav", "out/vorbis-44k.ogg")
    assert command[command.index("--serial") + 1] == str(fixtures_script.serial("vorbis-44k.ogg"))
    assert command[-3:] == ["-o", "out/vorbis-44k.ogg", "in.wav"]


def test_every_oggenc_fixture_has_a_serial(fixtures_script):
    for name, _, _, _, command in fixtures_script.FIXTURES_SPEC:
        if command is not None and command("s.wav", name)[0] == "oggenc":
            assert "--serial" in command("s.wav", name), name


def test_only_selects_named_fixtures(fixtures_script):
    spec = fixtures_script.FIXTURES_SPEC
    assert fixtures_script.selected(spec, []) == spec
    chosen = fixtures_script.selected(spec, ["vorbis-44k.ogg", "flac-44k.flac"])
    assert [entry[0] for entry in chosen] == ["flac-44k.flac", "vorbis-44k.ogg"]
    with pytest.raises(ValueError, match="nope.ogg"):
        fixtures_script.selected(spec, ["nope.ogg"])


def test_decoded_length_counts_samples_per_channel(fixtures_script):
    import subprocess

    def run(command, **kwargs):
        assert command[:2] == ["ffmpeg", "-v"]
        assert command[command.index("-ac") + 1] == "2"
        return subprocess.CompletedProcess(command, 0, stdout=b"\0" * (4 * 22371))

    assert fixtures_script.decoded_length("x.mp3", 2, run) == 22371


def test_the_tests_table_names_only_real_fixtures(fixtures_script):
    table = (
        fixtures_script.REPO_ROOT / "core" / "tests" / "FFmpegAudioFormatTests.cpp"
    ).read_text()
    names = {entry[0] for entry in fixtures_script.FIXTURES_SPEC}
    for name in re.findall(r'\{ "([\w.-]+)", \d+, \d, [\d.]+, Kind::', table):
        assert name in names, name
