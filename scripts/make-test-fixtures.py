#!/usr/bin/env python3
"""Generates the audio fixtures in core/tests/fixtures/ for the decoder tests.

Each fixture encodes the same deterministic test signal, which the C++ tests
regenerate (core/tests/TestSignal.h) to check the decoded audio. Keep the two
in sync.

The fixtures are committed, so this only needs re-running when they change.
Needs Homebrew's `ffmpeg` (for LAME, libopus and the AAC/ALAC/FLAC/WMA
encoders) and `oggenc` (vorbis-tools). These tools only make test data; the
app never uses them.

Usage: scripts/make-test-fixtures.py
"""

import math
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile
import wave

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
FIXTURES = REPO_ROOT / "core" / "tests" / "fixtures"

# Must match core/tests/TestSignal.h.
AMPLITUDE = 0.5
CHIRPS = [(200.0, 5000.0), (300.0, 7000.0)]  # (start Hz, end Hz) per channel


def signal_length(sample_rate, seconds):
    # Plus an odd remainder, so the length is not a multiple of any codec's
    # frame size and end-padding trimming is visible.
    return int(sample_rate * seconds) + 321


def sample(channel, n, sample_rate, seconds):
    f0, f1 = CHIRPS[channel]
    t = n / sample_rate
    duration = signal_length(sample_rate, seconds) / sample_rate
    phase = 2.0 * math.pi * (f0 * t + (f1 - f0) * t * t / (2.0 * duration))
    return AMPLITUDE * math.sin(phase)


def to_int16(x):
    return int(math.floor(x * 32767.0 + 0.5))


def write_source_wav(path, sample_rate, channels, seconds):
    frames = bytearray()
    for n in range(signal_length(sample_rate, seconds)):
        for c in range(channels):
            frames += struct.pack("<h", to_int16(sample(c, n, sample_rate, seconds)))
    with wave.open(str(path), "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        w.writeframes(bytes(frames))


def ffmpeg(*output_args):
    """An ffmpeg command for (source, output), without metadata or version tags."""
    return lambda source, output: [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", source,
        "-map_metadata", "-1", "-bitexact", *output_args, output,
    ]


# (file name, sample rate, channels, seconds, command given (source, output)).
# Short files cover decoding; the 4-second ones are long enough for seeks
# that use timestamps rather than rewinding (see FFmpegAudioFormat.cpp).
FIXTURES_SPEC = [
    ("wav-s16-44k.wav", 44100, 2, 0.5, None),
    ("wav-mono-48k.wav", 48000, 1, 0.5, None),
    ("aiff-s16-44k.aiff", 44100, 2, 0.5, ffmpeg("-c:a", "pcm_s16be")),
    ("flac-44k.flac", 44100, 2, 0.5, ffmpeg("-c:a", "flac")),
    ("alac-44k.m4a", 44100, 2, 0.5, ffmpeg("-c:a", "alac")),
    # ffmpeg's MP3 muxer writes a LAME header with encoder delay and padding.
    ("mp3-44k.mp3", 44100, 2, 0.5, ffmpeg("-c:a", "libmp3lame", "-b:a", "192k")),
    # VBR: positions must come from an exact index, not the approximate Xing TOC.
    ("mp3-vbr-44k.mp3", 44100, 2, 0.5, ffmpeg("-c:a", "libmp3lame", "-q:a", "2")),
    # No Xing/LAME header: no gapless info, and the duration must be measured.
    ("mp3-noheader-44k.mp3", 44100, 2, 0.5,
     ffmpeg("-c:a", "libmp3lame", "-b:a", "192k", "-write_xing", "0")),
    ("aac-44k.m4a", 44100, 2, 0.5, ffmpeg("-c:a", "aac", "-b:a", "160k")),
    # Raw ADTS AAC: no container timestamps or gapless info.
    ("aac-adts-44k.aac", 44100, 2, 0.5, ffmpeg("-c:a", "aac", "-b:a", "160k")),
    ("vorbis-44k.ogg", 44100, 2, 0.5, lambda s, o: ["oggenc", "--quiet", "-q", "5", "-o", o, s]),
    ("opus-48k.opus", 48000, 2, 0.5, ffmpeg("-c:a", "libopus", "-b:a", "128k")),
    ("wma-44k.wma", 44100, 2, 0.5, ffmpeg("-c:a", "wmav2", "-b:a", "192k")),
    ("flac-long-48k-mono.flac", 48000, 1, 4.0, ffmpeg("-c:a", "flac")),
    ("mp3-vbr-long-44k.mp3", 44100, 2, 4.0, ffmpeg("-c:a", "libmp3lame", "-q:a", "4")),
    ("aac-long-44k.m4a", 44100, 2, 4.0, ffmpeg("-c:a", "aac", "-b:a", "96k")),
    ("aac-adts-long-44k.aac", 44100, 2, 4.0, ffmpeg("-c:a", "aac", "-b:a", "96k")),
    ("vorbis-long-44k.ogg", 44100, 2, 4.0, lambda s, o: ["oggenc", "--quiet", "-q", "2", "-o", o, s]),
    ("opus-long-48k.opus", 48000, 2, 4.0, ffmpeg("-c:a", "libopus", "-b:a", "64k")),
]


def main():
    for tool in ("ffmpeg", "oggenc"):
        if shutil.which(tool) is None:
            sys.exit(f"error: {tool} not found (brew install ffmpeg vorbis-tools)")

    FIXTURES.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        for name, sample_rate, channels, seconds, command in FIXTURES_SPEC:
            output = FIXTURES / name
            if command is None:
                write_source_wav(output, sample_rate, channels, seconds)
            else:
                source = pathlib.Path(tmp) / f"source-{sample_rate}-{channels}-{seconds}.wav"
                if not source.exists():
                    write_source_wav(source, sample_rate, channels, seconds)
                subprocess.run(command(str(source), str(output)), check=True)
            print(f"{name:24} {output.stat().st_size:>8} bytes")


if __name__ == "__main__":
    main()
