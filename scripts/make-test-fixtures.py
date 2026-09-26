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
import zlib

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


def write_cover_png(path):
    """A deterministic 16x16 PNG for the tagged fixtures' embedded art."""
    width = height = 16
    rows = b"".join(
        b"\x00" + b"".join(bytes((x * 16, y * 16, 128)) for x in range(width))
        for y in range(height))

    def chunk(kind, data):
        return (struct.pack(">I", len(data)) + kind + data
                + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))

    path.write_bytes(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows, 9))
                     + chunk(b"IEND", b""))


def tagged(codec_args, metadata):
    """Like ffmpeg(), plus tags and cover.png (next to the source) as front cover."""
    tags = [arg for key, value in metadata for arg in ("-metadata", f"{key}={value}")]
    return lambda source, output: [
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", source,
        "-i", str(pathlib.Path(source).parent / "cover.png"),
        "-map", "0:a", "-map", "1:v", "-map_metadata", "-1", "-bitexact",
        *codec_args, "-c:v", "copy", "-disposition:v", "attached_pic",
        "-metadata:s:v", "comment=Cover (front)", *tags, output,
    ]


# Tags for the tagged fixtures; core/tests/TagReaderTests.cpp checks them.
# ID3v2.3 can't hold the recording ID (a UFID frame, which ffmpeg doesn't
# write), so only the FLAC file has one.
MP3_TAGS = [
    ("title", "Café Déjà Vu"),
    ("artist", "Ano Artist"),
    ("album", "東京 Sessions"),
    ("album_artist", "Various Artists"),
    ("track", "3/12"),
    ("disc", "1/2"),
    ("date", "2004"),
    ("genre", "Electronic"),
    ("MusicBrainz Album Id", "a1b2c3d4-0000-4000-8000-000000000001"),
    ("MusicBrainz Release Group Id", "a1b2c3d4-0000-4000-8000-000000000002"),
    ("MusicBrainz Release Track Id", "a1b2c3d4-0000-4000-8000-000000000003"),
    ("MusicBrainz Artist Id", "a1b2c3d4-0000-4000-8000-000000000004"),
    ("MusicBrainz Album Artist Id", "89ad4ac3-39f7-470e-963a-56509c546377"),
]
FLAC_TAGS = [
    ("title", "Café Déjà Vu"),
    ("artist", "Ano Artist"),
    ("album", "東京 Sessions"),
    ("album_artist", "Various Artists"),
    ("track", "3"),
    ("TRACKTOTAL", "12"),
    ("disc", "1"),
    ("DISCTOTAL", "2"),
    ("date", "2004-05-01"),
    ("genre", "Electronic"),
    ("MUSICBRAINZ_TRACKID", "a1b2c3d4-0000-4000-8000-000000000000"),
    ("MUSICBRAINZ_ALBUMID", "a1b2c3d4-0000-4000-8000-000000000001"),
    ("MUSICBRAINZ_RELEASEGROUPID", "a1b2c3d4-0000-4000-8000-000000000002"),
    ("MUSICBRAINZ_RELEASETRACKID", "a1b2c3d4-0000-4000-8000-000000000003"),
    ("MUSICBRAINZ_ARTISTID", "a1b2c3d4-0000-4000-8000-000000000004"),
    ("MUSICBRAINZ_ALBUMARTISTID", "89ad4ac3-39f7-470e-963a-56509c546377"),
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
    # Tagged by another tool than TagLib, for the tag reader tests.
    ("tagged-id3v23.mp3", 44100, 2, 0.5,
     tagged(["-c:a", "libmp3lame", "-b:a", "192k", "-id3v2_version", "3"], MP3_TAGS)),
    ("tagged-vorbis.flac", 44100, 2, 0.5, tagged(["-c:a", "flac"], FLAC_TAGS)),
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
                    write_cover_png(source.parent / "cover.png")
                subprocess.run(command(str(source), str(output)), check=True)
            print(f"{name:24} {output.stat().st_size:>8} bytes")


if __name__ == "__main__":
    main()
