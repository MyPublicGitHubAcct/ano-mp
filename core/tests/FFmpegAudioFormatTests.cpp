#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>
#include <catch2/generators/catch_generators_range.hpp>

#include "FFmpegAudioFormat.h"
#include "FormatRegistry.h"
#include "TestSignal.h"

#include <cmath>
#include <vector>

namespace
{
enum class Kind
{
    lossless,     // Bit-exact against the source.
    lossyGapless, // Encoder delay and padding trimmed: exact length, no lag.
    lossyRaw      // No gapless info in the file: length and lag as FFmpeg decodes them.
};

struct Fixture
{
    const char* file;
    int sampleRate;
    int channels;
    double seconds; // Length of the encoded test signal.
    Kind kind;
    juce::int64 length;   // Samples the reader must report.
    int lagAgainstSource; // Samples by which the decoded audio trails the source.
};

// Lengths and lags for the lossyRaw files are properties of those encoded
// files, measured with the pinned FFmpeg; see scripts/make-test-fixtures.py.
const Fixture fixtures[] = {
    { "wav-s16-44k.wav", 44100, 2, 0.5, Kind::lossless, 22371, 0 },
    { "wav-mono-48k.wav", 48000, 1, 0.5, Kind::lossless, 24321, 0 },
    { "aiff-s16-44k.aiff", 44100, 2, 0.5, Kind::lossless, 22371, 0 },
    { "flac-44k.flac", 44100, 2, 0.5, Kind::lossless, 22371, 0 },
    { "alac-44k.m4a", 44100, 2, 0.5, Kind::lossless, 22371, 0 },
    { "mp3-44k.mp3", 44100, 2, 0.5, Kind::lossyGapless, 22371, 0 },
    { "mp3-vbr-44k.mp3", 44100, 2, 0.5, Kind::lossyGapless, 22371, 0 },
    { "aac-44k.m4a", 44100, 2, 0.5, Kind::lossyGapless, 22371, 0 },
    { "vorbis-44k.ogg", 44100, 2, 0.5, Kind::lossyGapless, 22371, 0 },
    { "opus-48k.opus", 48000, 2, 0.5, Kind::lossyGapless, 24321, 0 },
    { "mp3-noheader-44k.mp3", 44100, 2, 0.5, Kind::lossyRaw, 24192, 1105 },
    { "aac-adts-44k.aac", 44100, 2, 0.5, Kind::lossyRaw, 23552, 1024 },
    { "wma-44k.wma", 44100, 2, 0.5, Kind::lossyRaw, 20480, -2048 },
    // Long enough for seeks that use timestamps rather than rewinding.
    { "flac-long-48k-mono.flac", 48000, 1, 4.0, Kind::lossless, 192321, 0 },
    { "mp3-vbr-long-44k.mp3", 44100, 2, 4.0, Kind::lossyGapless, 176721, 0 },
    { "aac-long-44k.m4a", 44100, 2, 4.0, Kind::lossyGapless, 176721, 0 },
    { "vorbis-long-44k.ogg", 44100, 2, 4.0, Kind::lossyGapless, 176721, 0 },
    { "opus-long-48k.opus", 48000, 2, 4.0, Kind::lossyGapless, 192321, 0 },
    { "aac-adts-long-44k.aac", 44100, 2, 4.0, Kind::lossyRaw, 178176, 1024 },
};

juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }

std::unique_ptr<juce::AudioFormatReader> openReader (const Fixture& fixture)
{
    anomp::FFmpegAudioFormat format;
    auto stream = fixtureFile (fixture.file).createInputStream();
    if (stream == nullptr)
        return nullptr;
    return std::unique_ptr<juce::AudioFormatReader> (format.createReaderFor (stream.release(), true));
}

juce::AudioBuffer<float> readAll (juce::AudioFormatReader& reader)
{
    juce::AudioBuffer<float> buffer (static_cast<int> (reader.numChannels), static_cast<int> (reader.lengthInSamples));
    REQUIRE (reader.read (&buffer, 0, buffer.getNumSamples(), 0, true, true));
    return buffer;
}

/** Compares decoded audio with the source signal over a window, which keeps
    the lag search fast on the long fixtures. */
class SourceWindow
{
public:
    SourceWindow (const Fixture& fixture, int ch)
    {
        const auto length = anomp::test::signalLength (fixture.sampleRate, fixture.seconds);
        for (auto n = start; n < juce::jmin (length, start + windowSize); ++n)
            samples.push_back (anomp::test::signalSample (ch, n, fixture.sampleRate, fixture.seconds));
    }

    /** Normalised correlation of `decoded` with the source, shifted by `lag`. */
    double correlation (const float* decoded, int decodedLength, int lag) const
    {
        double sum = 0, sourceEnergy = 0, decodedEnergy = 0;

        for (size_t i = 0; i < samples.size(); ++i)
        {
            const auto j = start + static_cast<juce::int64> (i) + lag;
            if (j < 0 || j >= decodedLength)
                continue;
            const auto d = static_cast<double> (decoded[j]);
            sum += samples[i] * d;
            sourceEnergy += samples[i] * samples[i];
            decodedEnergy += d * d;
        }

        return sum / std::sqrt (sourceEnergy * decodedEnergy + 1e-12);
    }

    int bestLag (const float* decoded, int decodedLength) const
    {
        int best = 0;
        double bestValue = -2.0;
        for (int lag = -2500; lag <= 2500; ++lag)
        {
            const auto value = correlation (decoded, decodedLength, lag);
            if (value > bestValue)
            {
                bestValue = value;
                best = lag;
            }
        }
        return best;
    }

private:
    static constexpr juce::int64 start = 4096;
    static constexpr juce::int64 windowSize = 16384;
    std::vector<double> samples;
};

/** Largest difference between `block` (read at `position`) and the reference
    decode shifted by `shift`; past the end the reference is silence. */
float blockError (const juce::AudioBuffer<float>& block,
                  const juce::AudioBuffer<float>& reference,
                  int position,
                  int shift)
{
    float maxError = 0.0f;
    for (int ch = 0; ch < block.getNumChannels(); ++ch)
        for (int i = 0; i < block.getNumSamples(); ++i)
        {
            const auto n = position + shift + i;
            const auto want = n >= 0 && n < reference.getNumSamples() ? reference.getSample (ch, n) : 0.0f;
            maxError = juce::jmax (maxError, std::abs (block.getSample (ch, i) - want));
        }
    return maxError;
}
} // namespace

TEST_CASE ("FFmpegAudioFormat decodes every fixture", "[ffmpeg-format]")
{
    const auto& fixture = GENERATE (from_range (std::begin (fixtures), std::end (fixtures)));
    INFO ("fixture " << fixture.file);

    auto reader = openReader (fixture);
    REQUIRE (reader != nullptr);
    CHECK (reader->sampleRate == fixture.sampleRate);
    CHECK (reader->numChannels == static_cast<unsigned int> (fixture.channels));
    CHECK (reader->usesFloatingPointData);
    REQUIRE (reader->lengthInSamples == fixture.length);

    const auto decoded = readAll (*reader);

    for (int ch = 0; ch < fixture.channels; ++ch)
    {
        INFO ("channel " << ch);

        if (fixture.kind == Kind::lossless)
        {
            float maxError = 0.0f;
            for (int n = 0; n < decoded.getNumSamples(); ++n)
                maxError = juce::jmax (
                    maxError, std::abs (decoded.getSample (ch, n)
                                        - anomp::test::signalSample16 (ch, n, fixture.sampleRate, fixture.seconds)));
            CHECK (maxError <= 1.5f / 32768.0f);
        }
        else
        {
            // A sample-exact lag proves encoder delay was trimmed (or, for
            // lossyRaw, left exactly as FFmpeg decodes it); a channel swap
            // would lower the correlation, since each channel's chirp differs.
            const SourceWindow source (fixture, ch);
            const auto* samples = decoded.getReadPointer (ch);
            CHECK (source.bestLag (samples, decoded.getNumSamples()) == fixture.lagAgainstSource);
            CHECK (source.correlation (samples, decoded.getNumSamples(), fixture.lagAgainstSource) > 0.97);
        }
    }
}

TEST_CASE ("FFmpegAudioFormat seeks to exact samples", "[ffmpeg-format]")
{
    const auto& fixture = GENERATE (from_range (std::begin (fixtures), std::end (fixtures)));
    INFO ("fixture " << fixture.file);

    auto reference = openReader (fixture);
    REQUIRE (reference != nullptr);
    const auto expected = readAll (*reference);

    auto reader = openReader (fixture);
    REQUIRE (reader != nullptr);

    const auto length = static_cast<int> (fixture.length);
    constexpr int blockSize = 300;
    // Forward and backward jumps, frame-boundary neighbours, the very start
    // and a block that runs past the end.
    const int positions[] = { length / 2 + 7,
                              0,
                              length - 1000,
                              1,
                              1151,
                              1024,
                              length / 3,
                              4097,
                              length - blockSize / 2,
                              13,
                              length / 2 - 2000,
                              length / 2 + 7 };

    for (const auto position : positions)
    {
        INFO ("position " << position);
        juce::AudioBuffer<float> block (fixture.channels, blockSize);
        block.clear();
        REQUIRE (reader->read (&block, 0, blockSize, position, true, true));

        const auto error = blockError (block, expected, position, 0);

        if (fixture.kind == Kind::lossless)
        {
            CHECK (error == 0.0f);
        }
        else if (error > 1.0e-4f)
        {
            // Lossy decoders match a straight decode to within 1e-4 after the
            // seek preroll, except AAC's noise substitution (PNS), which is
            // random. Then the block must still line up exactly: any shift
            // of up to 64 samples must match worse. (Only where there is
            // signal to line up; ADTS ends in near-silent encoder padding.)
            INFO ("error " << error);
            CHECK (error < 0.1f);

            juce::AudioBuffer<float> silence (fixture.channels, blockSize);
            silence.clear();
            if (blockError (silence, expected, position, 0) > 0.1f)
                for (int shift = -64; shift <= 64; ++shift)
                    if (shift != 0)
                        CHECK (blockError (block, expected, position, shift) > 2.0f * error);
        }
    }
}

TEST_CASE ("FFmpegAudioFormat reads from a memory stream", "[ffmpeg-format]")
{
    juce::MemoryBlock data;
    REQUIRE (fixtureFile ("flac-44k.flac").loadFileAsData (data));

    anomp::FFmpegAudioFormat format;
    std::unique_ptr<juce::AudioFormatReader> reader (
        format.createReaderFor (new juce::MemoryInputStream (data, false), true));

    REQUIRE (reader != nullptr);
    CHECK (reader->lengthInSamples == 22371);

    juce::AudioBuffer<float> block (2, 64);
    REQUIRE (reader->read (&block, 0, 64, 1000, true, true));
    CHECK (std::abs (block.getSample (1, 10) - anomp::test::signalSample16 (1, 1010, 44100, 0.5)) <= 1.5f / 32768.0f);
}

TEST_CASE ("FFmpegAudioFormat rejects data it cannot decode", "[ffmpeg-format]")
{
    anomp::FFmpegAudioFormat format;
    const char text[] = "This is not audio, just some text that FFmpeg should refuse to open.";

    SECTION ("keeps the stream when asked to")
    {
        juce::MemoryInputStream stream (text, sizeof (text), false);
        CHECK (format.createReaderFor (&stream, false) == nullptr);
        CHECK (stream.getTotalLength() == static_cast<juce::int64> (sizeof (text))); // Still alive.
    }

    SECTION ("deletes the stream otherwise")
    {
        CHECK (format.createReaderFor (new juce::MemoryInputStream (text, sizeof (text), false), true) == nullptr);
    }

    SECTION ("truncated file")
    {
        juce::MemoryBlock data;
        REQUIRE (fixtureFile ("flac-44k.flac").loadFileAsData (data));
        data.setSize (100);
        CHECK (format.createReaderFor (new juce::MemoryInputStream (data, false), true) == nullptr);
    }
}

TEST_CASE ("FormatRegistry decodes every format through FFmpeg", "[ffmpeg-format][formats]")
{
    anomp::FormatRegistry registry;

    for (const auto& fixture : fixtures)
    {
        INFO ("fixture " << fixture.file);
        std::unique_ptr<juce::AudioFormatReader> reader (
            registry.manager().createReaderFor (fixtureFile (fixture.file)));
        REQUIRE (reader != nullptr);
        CHECK (reader->getFormatName() == "FFmpeg");
        CHECK (reader->lengthInSamples == fixture.length);
    }

    for (auto* ext : { "m4a", "aac", "opus", "wma", "aif" })
    {
        INFO ("extension " << ext);
        CHECK (registry.canDecodeExtension (ext));
    }
}
