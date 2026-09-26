#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "FormatRegistry.h"
#include "PlayerEngine.h"

#include <array>
#include <cmath>
#include <limits>
#include <string>
#include <utility>
#include <vector>

// PlayerEngine rendered offline: getNextAudioBlock is called directly, so no
// audio device is needed. Expected output comes from decoding the same
// fixtures with the same reader, so lossy files compare exactly too.

namespace
{
using Channels = std::array<std::vector<float>, 2>;
using State = anomp::PlayerEngine::State;

constexpr int blockSize = 512;

juce::File fixtureFile (const char* name)
{
    return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name);
}

/** The whole file as the player reads it: stereo, mono duplicated. */
Channels decode (const char* name)
{
    anomp::FormatRegistry registry;
    std::unique_ptr<juce::AudioFormatReader> reader (registry.manager().createReaderFor (fixtureFile (name)));
    REQUIRE (reader != nullptr);

    juce::AudioBuffer<float> buffer (2, static_cast<int> (reader->lengthInSamples));
    REQUIRE (reader->read (&buffer, 0, buffer.getNumSamples(), 0, true, true));

    Channels result;
    for (int ch = 0; ch < 2; ++ch)
        result[static_cast<size_t> (ch)].assign (buffer.getReadPointer (ch), buffer.getReadPointer (ch) + buffer.getNumSamples());
    return result;
}

Channels concat (const Channels& a, const Channels& b)
{
    auto result = a;
    for (size_t ch = 0; ch < 2; ++ch)
        result[ch].insert (result[ch].end(), b[ch].begin(), b[ch].end());
    return result;
}

int length (const Channels& audio)
{
    return static_cast<int> (audio[0].size());
}

/** Largest difference between output[outStart + i] and gain * expected[refStart + i]
    for i in [0, count); `expected` is silence outside its range. */
float maxError (const Channels& output, int outStart, const Channels& expected, int refStart, int count, float gain = 1.0f)
{
    float result = 0.0f;
    for (size_t ch = 0; ch < 2; ++ch)
        for (int i = 0; i < count; ++i)
        {
            const auto r = refStart + i;
            const auto want = r >= 0 && r < length (expected) ? gain * expected[ch][static_cast<size_t> (r)] : 0.0f;
            result = juce::jmax (result, std::abs (output[ch][static_cast<size_t> (outStart + i)] - want));
        }
    return result;
}

float peak (const Channels& output, int start, int end)
{
    float result = 0.0f;
    for (auto& channel : output)
        for (auto i = start; i < end; ++i)
            result = juce::jmax (result, std::abs (channel[static_cast<size_t> (i)]));
    return result;
}

/** A player with its own formats and optional read-ahead thread, rendered in
    fixed blocks into `output`. Records reported events in `events`. */
struct Harness
{
    Harness (double deviceRate, bool readAhead)
        : thread (readAhead ? std::make_unique<juce::TimeSliceThread> ("test read-ahead") : nullptr),
          player (registry.manager(), thread.get())
    {
        if (thread != nullptr)
            thread->startThread();

        player.prepareToPlay (blockSize, deviceRate);
        player.onTrackEnded = [this] (bool advanced) { events.push_back (advanced ? "advanced" : "ended"); };
        player.onStateChanged = [this] (State state) { events.push_back ("state " + std::to_string (static_cast<int> (state))); };
    }

    void renderBlock()
    {
        if (thread != nullptr)
            player.waitForReadAhead (blockSize, 5000);

        juce::AudioBuffer<float> block (2, blockSize);
        block.clear();
        player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));

        for (int ch = 0; ch < 2; ++ch)
            output[static_cast<size_t> (ch)].insert (output[static_cast<size_t> (ch)].end(),
                                                     block.getReadPointer (ch),
                                                     block.getReadPointer (ch) + blockSize);
    }

    /** Renders whole blocks until at least `numSamples` more are output. */
    void render (int numSamples)
    {
        for (int done = 0; done < numSamples; done += blockSize)
            renderBlock();
    }

    /** Renders until the player stops; returns the output length at that point. */
    int renderUntilStopped (int maxSamples)
    {
        while (player.getState() == State::playing && length (output) < maxSamples)
            renderBlock();
        return length (output);
    }

    std::vector<std::string> takeEvents()
    {
        player.dispatchEvents();
        return std::exchange (events, {});
    }

    anomp::FormatRegistry registry;
    std::unique_ptr<juce::TimeSliceThread> thread;
    anomp::PlayerEngine player; // Declared after the thread: freed before it stops.
    Channels output;
    std::vector<std::string> events;
};

const std::string stateStopped = "state " + std::to_string (static_cast<int> (State::stopped));
const std::string statePlaying = "state " + std::to_string (static_cast<int> (State::playing));
} // namespace

TEST_CASE ("PlayerEngine state transitions", "[player]")
{
    Harness h (44100.0, false);
    auto& player = h.player;

    CHECK (player.getState() == State::empty);
    CHECK_FALSE (player.play());
    CHECK_FALSE (player.seek (0.1));
    CHECK (player.setNext (fixtureFile ("flac-44k.flac")) == "No track is loaded");
    CHECK (player.load (fixtureFile ("missing.flac")).startsWith ("File not found"));
    CHECK (player.load (juce::File (ANOMP_TEST_FIXTURES_DIR).getSiblingFile ("TestSignal.h")).startsWith ("Unsupported"));
    CHECK (player.getState() == State::empty);

    REQUIRE (player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    CHECK (player.getState() == State::stopped);
    CHECK (player.getDurationSeconds() == Catch::Approx (22371 / 44100.0));
    CHECK (player.getPositionSeconds() == 0.0);

    // Stopped: rendering outputs silence and holds the position.
    h.render (blockSize);
    CHECK (peak (h.output, 0, blockSize) == 0.0f);
    CHECK (player.getPositionSeconds() == 0.0);

    REQUIRE (player.play());
    CHECK (player.getState() == State::playing);
    h.render (2 * blockSize);
    CHECK (player.getPositionSeconds() == Catch::Approx (2 * blockSize / 44100.0));

    player.pause();
    CHECK (player.getState() == State::paused);
    h.render (2 * blockSize); // One block fades out; the next is silent.
    CHECK (player.getPositionSeconds() == Catch::Approx (3 * blockSize / 44100.0));
    CHECK (peak (h.output, 4 * blockSize, 5 * blockSize) == 0.0f);

    REQUIRE (player.play());
    player.stop();
    CHECK (player.getState() == State::stopped);
    CHECK (player.getPositionSeconds() == 0.0);

    CHECK (player.seek (0.25));
    CHECK (player.getPositionSeconds() == Catch::Approx (11025 / 44100.0));
    CHECK (player.seek (-1.0));
    CHECK (player.getPositionSeconds() == 0.0);
    CHECK (player.seek (100.0));
    CHECK (player.getPositionSeconds() == Catch::Approx (player.getDurationSeconds()));
    CHECK_FALSE (player.seek (std::numeric_limits<double>::quiet_NaN()));

    player.setVolume (0.25f);
    CHECK (player.getVolume() == 0.25f);
    player.setVolume (2.0f);
    CHECK (player.getVolume() == 1.0f);
    player.setVolume (-1.0f);
    CHECK (player.getVolume() == 0.0f);
    player.setVolume (std::numeric_limits<float>::quiet_NaN());
    CHECK (player.getVolume() == 0.0f);
}

TEST_CASE ("PlayerEngine plays a track exactly and stops at its end", "[player]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    Harness h (44100.0, readAhead);
    const auto expected = decode ("flac-44k.flac");

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.play());
    CHECK (h.takeEvents() == std::vector<std::string> { statePlaying });

    const auto stoppedAt = h.renderUntilStopped (100000);
    CHECK (h.player.getState() == State::stopped);
    CHECK (h.player.getPositionSeconds() == 0.0);
    CHECK (stoppedAt == (length (expected) / blockSize + 1) * blockSize);
    CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });

    // The first block fades in from silence; after it, the output is the file.
    for (int i = 0; i < blockSize; i += 37)
        CHECK (h.output[0][static_cast<size_t> (i)]
               == Catch::Approx (expected[0][static_cast<size_t> (i)] * static_cast<float> (i) / blockSize).margin (1e-6));
    CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) == 0.0f);

    h.render (blockSize);
    CHECK (peak (h.output, stoppedAt, length (h.output)) == 0.0f);
}

TEST_CASE ("PlayerEngine pause fades out and play resumes where it paused", "[player]")
{
    Harness h (44100.0, false);
    const auto expected = decode ("flac-44k.flac");

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.play());
    h.render (4 * blockSize);
    h.player.pause();
    h.render (3 * blockSize);

    const auto pausedAt = 5 * blockSize;
    CHECK (h.player.getPositionSeconds() == Catch::Approx (pausedAt / 44100.0));

    for (int i = 0; i < blockSize; i += 37)
    {
        const auto n = static_cast<size_t> (4 * blockSize + i);
        CHECK (h.output[1][n] == Catch::Approx (expected[1][n] * (1.0f - static_cast<float> (i) / blockSize)).margin (1e-6));
    }
    CHECK (peak (h.output, pausedAt, 7 * blockSize) == 0.0f);

    REQUIRE (h.player.play());
    h.render (2 * blockSize);
    CHECK (maxError (h.output, 8 * blockSize, expected, pausedAt + blockSize, blockSize) == 0.0f);
}

TEST_CASE ("PlayerEngine volume scales the output", "[player]")
{
    Harness h (44100.0, false);
    const auto expected = decode ("flac-44k.flac");

    h.player.setVolume (0.5f);
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.play());
    h.render (4 * blockSize);
    CHECK (maxError (h.output, blockSize, expected, blockSize, 3 * blockSize, 0.5f) < 1e-7f);
}

TEST_CASE ("PlayerEngine seeks to the exact sample", "[player]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    // Mono at 48 kHz: plays on both channels, no resampling.
    Harness h (48000.0, readAhead);
    const auto expected = decode ("flac-long-48k-mono.flac");

    REQUIRE (h.player.load (fixtureFile ("flac-long-48k-mono.flac")).isEmpty());
    REQUIRE (h.player.seek (1.5));
    CHECK (h.player.getPositionSeconds() == 1.5);

    REQUIRE (h.player.play());
    h.render (4 * blockSize);
    CHECK (maxError (h.output, blockSize, expected, 72000 + blockSize, 3 * blockSize) == 0.0f);

    // Seeking while playing takes effect at the next block, without a fade.
    REQUIRE (h.player.seek (3.0));
    h.render (4 * blockSize);
    CHECK (maxError (h.output, 4 * blockSize, expected, 144000, 4 * blockSize) == 0.0f);
    CHECK (h.player.getPositionSeconds() == Catch::Approx ((144000 + 4 * blockSize) / 48000.0));
}

TEST_CASE ("PlayerEngine hands off to the next track gaplessly", "[player][gapless]")
{
    struct Pair
    {
        const char* first;
        const char* second;
        double rate;
    };

    const auto pair = GENERATE (Pair { "flac-44k.flac", "wav-s16-44k.wav", 44100.0 },
                                Pair { "mp3-44k.mp3", "mp3-vbr-44k.mp3", 44100.0 },
                                Pair { "vorbis-44k.ogg", "alac-44k.m4a", 44100.0 },
                                Pair { "opus-48k.opus", "opus-48k.opus", 48000.0 });
    const auto readAhead = GENERATE (false, true);
    CAPTURE (pair.first, pair.second, readAhead);

    Harness h (pair.rate, readAhead);
    const auto first = decode (pair.first);
    const auto expected = concat (first, decode (pair.second));

    REQUIRE (h.player.load (fixtureFile (pair.first)).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile (pair.second)).isEmpty());
    CHECK (h.player.hasNext());
    REQUIRE (h.player.play());
    h.takeEvents();

    h.render (length (first) + 1);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced" });
    CHECK_FALSE (h.player.hasNext());
    CHECK (h.player.getState() == State::playing);
    CHECK (h.player.getPositionSeconds() == Catch::Approx ((length (h.output) - length (first)) / pair.rate));

    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });
    CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) == 0.0f);
}

TEST_CASE ("PlayerEngine resamples to the device rate without a gap between tracks", "[player][gapless]")
{
    // 44.1 kHz files on a 48 kHz device: the output must be the two files
    // joined and then resampled as one stream.
    Harness h (48000.0, false);
    const auto input = concat (decode ("flac-44k.flac"), decode ("wav-s16-44k.wav"));

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced", "ended", stateStopped });

    const auto ratio = 44100.0 / 48000.0;
    CHECK (stoppedAt >= static_cast<int> ((length (input) - 3) / ratio));
    CHECK (stoppedAt < static_cast<int> (length (input) / ratio) + blockSize);

    Channels expected;
    for (size_t ch = 0; ch < 2; ++ch)
    {
        auto padded = input[ch];
        padded.resize (padded.size() + static_cast<size_t> (stoppedAt), 0.0f);
        expected[ch].resize (static_cast<size_t> (stoppedAt));
        juce::WindowedSincInterpolator interpolator;
        interpolator.process (ratio, padded.data(), expected[ch].data(), stoppedAt);
    }

    CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) < 1e-5f);
}

TEST_CASE ("PlayerEngine switches to a next track at a different rate", "[player][gapless]")
{
    Harness h (48000.0, false);
    const auto second = decode ("wav-mono-48k.wav");

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-mono-48k.wav")).isEmpty());
    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced", "ended", stateStopped });

    // The second track plays unresampled, starting at a chunk boundary soon
    // after the first one ends.
    const auto firstEnd = static_cast<int> (22371 * 48000.0 / 44100.0);
    int start = -1;
    for (int k = firstEnd - 16; k < firstEnd + 2 * blockSize && start < 0; ++k)
        if (k + length (second) <= stoppedAt && maxError (h.output, k, second, 0, length (second)) == 0.0f)
            start = k;

    CHECK (start >= firstEnd - 16);
    CHECK (start - firstEnd < blockSize + 200);
}

TEST_CASE ("PlayerEngine next track can be cleared or replaced", "[player][gapless]")
{
    Harness h (44100.0, false);
    const auto first = decode ("flac-44k.flac");

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    h.player.clearNext();
    CHECK_FALSE (h.player.hasNext());

    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    CHECK_FALSE (h.player.setNext (fixtureFile ("missing.wav")).isEmpty());
    CHECK (h.player.hasNext()); // A failed setNext keeps the previous one.

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    CHECK_FALSE (h.player.hasNext()); // Loading clears it.

    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (stoppedAt < length (first) + blockSize);
    CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });
}

TEST_CASE ("PlayerEngine reports position changes", "[player]")
{
    Harness h (44100.0, false);
    std::vector<std::pair<double, double>> positions;
    h.player.onPositionChanged = [&] (double position, double duration) { positions.emplace_back (position, duration); };

    h.player.dispatchEvents();
    REQUIRE (positions.size() == 1); // The initial report.
    CHECK (positions.back() == std::pair (0.0, 0.0));

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    h.player.dispatchEvents();
    REQUIRE (positions.size() == 2);
    CHECK (positions.back().second == Catch::Approx (22371 / 44100.0));

    h.player.dispatchEvents();
    CHECK (positions.size() == 2); // Unchanged: nothing reported.

    REQUIRE (h.player.play());
    h.render (blockSize);
    h.player.dispatchEvents();
    REQUIRE (positions.size() == 3);
    CHECK (positions.back().first == Catch::Approx (blockSize / 44100.0));
}
