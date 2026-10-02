#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "Crossfeed.h"
#include "Equaliser.h"
#include "FormatRegistry.h"
#include "PlayerEngine.h"

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cmath>
#include <limits>
#include <string>
#include <thread>
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

juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }

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
        result[static_cast<size_t> (ch)].assign (buffer.getReadPointer (ch),
                                                 buffer.getReadPointer (ch) + buffer.getNumSamples());
    return result;
}

Channels concat (const Channels& a, const Channels& b)
{
    auto result = a;
    for (size_t ch = 0; ch < 2; ++ch)
        result[ch].insert (result[ch].end(), b[ch].begin(), b[ch].end());
    return result;
}

int length (const Channels& audio) { return static_cast<int> (audio[0].size()); }

/** Largest difference between output[outStart + i] and gain * expected[refStart + i]
    for i in [0, count); `expected` is silence outside its range. */
float maxError (const Channels& output,
                int outStart,
                const Channels& expected,
                int refStart,
                int count,
                float gain = 1.0f)
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
        player.onTrackEnded = [this] (bool advanced)
        {
            events.push_back (advanced ? "advanced" : "ended");
        };
        player.onStateChanged = [this] (State state)
        {
            events.push_back ("state " + std::to_string (static_cast<int> (state)));
        };
    }

    void renderBlock()
    {
        if (thread != nullptr)
            player.waitForReadAhead (blockSize, 5000);

        juce::AudioBuffer<float> block (2, blockSize);
        block.clear();
        player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));

        for (int ch = 0; ch < 2; ++ch)
            output[static_cast<size_t> (ch)].insert (output[static_cast<size_t> (ch)].end(), block.getReadPointer (ch),
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
    CHECK (
        player.load (juce::File (ANOMP_TEST_FIXTURES_DIR).getSiblingFile ("TestSignal.h")).startsWith ("Unsupported"));
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
        CHECK (
            h.output[0][static_cast<size_t> (i)]
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
        CHECK (h.output[1][n]
               == Catch::Approx (expected[1][n] * (1.0f - static_cast<float> (i) / blockSize)).margin (1e-6));
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

TEST_CASE ("PlayerEngine applies each track's gain, switching at the hand-off", "[player][gapless][gain]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    Harness h (44100.0, readAhead);
    auto first = decode ("flac-44k.flac");
    auto second = decode ("wav-s16-44k.wav");
    for (size_t ch = 0; ch < 2; ++ch)
    {
        for (auto& sample : first[ch])
            sample *= 0.5f;
        for (auto& sample : second[ch])
            sample *= 2.0f;
    }
    const auto expected = concat (first, second);

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac"), 0.5f).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), 2.0f).isEmpty());
    REQUIRE (h.player.play());
    h.takeEvents();
    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced", "ended", stateStopped });
    CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) < 1e-6f);
}

TEST_CASE ("PlayerEngine changes a track's gain by its file", "[player][gain]")
{
    Harness h (44100.0, false);
    const auto expected = decode ("flac-44k.flac");
    const auto file = fixtureFile ("flac-44k.flac");

    REQUIRE (h.player.load (file).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.play());
    h.render (2 * blockSize);
    CHECK (maxError (h.output, blockSize, expected, blockSize, blockSize) < 1e-7f);

    // Only the track opened from that file changes; one block ramps to it.
    CHECK (h.player.setTrackGain (fixtureFile ("mp3-44k.mp3"), 0.25f) == 0);
    CHECK (h.player.setTrackGain (file, 0.25f) == 1);
    h.render (3 * blockSize);
    CHECK (maxError (h.output, 3 * blockSize, expected, 3 * blockSize, 2 * blockSize, 0.25f) < 1e-7f);

    // Gains are clamped: NaN and negatives to silence, large ones to the maximum.
    CHECK (h.player.setTrackGain (file, 100.0f) == 1);
    h.render (2 * blockSize);
    CHECK (maxError (h.output, 6 * blockSize, expected, 6 * blockSize, blockSize, anomp::PlayerEngine::maxTrackGain)
           < 1e-5f);
    CHECK (h.player.setTrackGain (file, std::numeric_limits<float>::quiet_NaN()) == 1);
    h.render (2 * blockSize);
    CHECK (peak (h.output, 8 * blockSize, 9 * blockSize) == 0.0f);

    // The same file as current and next changes both.
    REQUIRE (h.player.setNext (file).isEmpty());
    CHECK (h.player.setTrackGain (file, 1.0f) == 2);
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

    const auto pair = GENERATE (
        Pair { "flac-44k.flac", "wav-s16-44k.wav", 44100.0 }, Pair { "mp3-44k.mp3", "mp3-vbr-44k.mp3", 44100.0 },
        Pair { "vorbis-44k.ogg", "alac-44k.m4a", 44100.0 }, Pair { "opus-48k.opus", "opus-48k.opus", 48000.0 });
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

TEST_CASE ("PlayerEngine counts hand-offs before it reports them", "[player][gapless]")
{
    Harness h (44100.0, false);
    const auto first = decode ("flac-44k.flac");
    const auto second = decode ("wav-s16-44k.wav");

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.play());
    h.takeEvents();
    CHECK (h.player.getAdvanceCount() == 0);

    h.render (length (first) + 1);
    CHECK (h.player.getAdvanceCount() == 1); // Counted before dispatchEvents() reports it.
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced" });
    CHECK (h.player.getAdvanceCount() == 1);

    // A load keeps the count, and a pending hand-off is still reported.
    REQUIRE (h.player.setNext (fixtureFile ("flac-44k.flac")).isEmpty());
    h.render (length (second));
    CHECK (h.player.getAdvanceCount() == 2);
    REQUIRE (h.player.load (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    CHECK (h.player.getAdvanceCount() == 2);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced", stateStopped });
}

TEST_CASE ("PlayerEngine load drops an end it has not reported", "[player]")
{
    Harness h (44100.0, false);

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.play());
    h.takeEvents();
    h.renderUntilStopped (200000);

    // Loaded before the end was dispatched: the end belonged to the old track.
    REQUIRE (h.player.load (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.play());
    CHECK (h.takeEvents().empty()); // Stopped and playing again since the last report.

    h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });
    CHECK (h.player.getAdvanceCount() == 0);
}

TEST_CASE ("PlayerEngine event callbacks may send it commands", "[player][gapless]")
{
    // The host's queue arms the next track from inside onTrackEnded (the
    // Rust queue does, through the C API's callback).
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);
    Harness h (44100.0, readAhead);
    const auto first = decode ("flac-44k.flac");
    const auto second = decode ("wav-s16-44k.wav");

    SECTION ("setting the next track after a hand-off keeps it gapless")
    {
        int advances = 0;
        h.player.onTrackEnded = [&] (bool advanced)
        {
            h.events.push_back (advanced ? "advanced" : "ended");
            if (advanced && ++advances == 1)
                CHECK (h.player.setNext (fixtureFile ("flac-44k.flac")).isEmpty());
        };

        REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
        REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
        REQUIRE (h.player.play());
        h.takeEvents();

        h.render (length (first) + 1);
        CHECK (h.takeEvents() == std::vector<std::string> { "advanced" });
        CHECK (h.player.hasNext());

        const auto stoppedAt = h.renderUntilStopped (300000);
        CHECK (h.takeEvents() == std::vector<std::string> { "advanced", "ended", stateStopped });
        const auto expected = concat (concat (first, second), first);
        CHECK (stoppedAt >= length (expected));
        CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) == 0.0f);
    }

    SECTION ("loading and playing when the last track ends")
    {
        bool restarted = false;
        h.player.onTrackEnded = [&] (bool advanced)
        {
            h.events.push_back (advanced ? "advanced" : "ended");
            if (! advanced && ! std::exchange (restarted, true))
            {
                CHECK (h.player.load (fixtureFile ("wav-s16-44k.wav")).isEmpty());
                CHECK (h.player.play());
            }
        };

        REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
        REQUIRE (h.player.play());
        h.takeEvents();
        const auto endedAt = h.renderUntilStopped (200000);

        // Playing again before the state was reported: no state change.
        CHECK (h.takeEvents() == std::vector<std::string> { "ended" });
        CHECK (h.player.getState() == State::playing);
        CHECK (h.player.getPositionSeconds() == 0.0);

        const auto stoppedAt = h.renderUntilStopped (200000);
        CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });
        CHECK (stoppedAt - endedAt >= length (second));
        // The first block fades in.
        CHECK (maxError (h.output, endedAt + blockSize, second, blockSize, length (second) - blockSize) == 0.0f);
    }
}

TEST_CASE ("PlayerEngine reports position changes", "[player]")
{
    Harness h (44100.0, false);
    std::vector<std::pair<double, double>> positions;
    h.player.onPositionChanged = [&] (double position, double duration)
    {
        positions.emplace_back (position, duration);
    };

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

namespace
{
using Options = anomp::PlayerEngine::TrackOptions;

/** Samples [from, to) of `audio`. */
Channels slice (const Channels& audio, int from, int to)
{
    Channels result;
    for (size_t ch = 0; ch < 2; ++ch)
        result[ch].assign (audio[ch].begin() + from, audio[ch].begin() + to);
    return result;
}

Options part (double start, double end)
{
    Options options;
    options.start = start;
    options.end = end;
    return options;
}
} // namespace

TEST_CASE ("PlayerEngine plays part of a file", "[player][range]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    Harness h (44100.0, readAhead);
    const auto whole = decode ("flac-44k.flac");
    const auto file = fixtureFile ("flac-44k.flac");

    REQUIRE (h.player.load (file, part (0.1, 0.3)).isEmpty());
    CHECK (h.player.getDurationSeconds() == Catch::Approx (0.2));
    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (100000);
    CHECK (maxError (h.output, blockSize, slice (whole, 4410, 13230), blockSize, stoppedAt - blockSize) == 0.0f);

    // Seeks and positions are within the part.
    CHECK (h.player.seek (0.05));
    CHECK (h.player.getPositionSeconds() == Catch::Approx (0.05));
    CHECK (h.player.seek (1.0));
    CHECK (h.player.getPositionSeconds() == Catch::Approx (0.2));

    // An end past the file's is its end; a start past it is an error.
    REQUIRE (h.player.load (file, part (0.3, 99.0)).isEmpty());
    CHECK (h.player.getDurationSeconds() == Catch::Approx ((length (whole) - 13230) / 44100.0));
    CHECK (h.player.load (file, part (5.0, 0.0)).startsWith ("The track starts after the end of the file"));
    CHECK (h.player.load (file, part (std::numeric_limits<double>::quiet_NaN(), 0.0)).isNotEmpty());
}

TEST_CASE ("PlayerEngine hands off between parts of one file gaplessly", "[player][range][gapless]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    Harness h (44100.0, readAhead);
    const auto whole = decode ("flac-44k.flac");
    const auto file = fixtureFile ("flac-44k.flac");

    REQUIRE (h.player.load (file, part (0.0, 0.2)).isEmpty());
    REQUIRE (h.player.setNext (file, part (0.2, 0.0)).isEmpty());
    REQUIRE (h.player.play());
    h.takeEvents();

    h.render (8820 + 1);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced" });
    CHECK (h.player.getPositionSeconds() == Catch::Approx ((length (h.output) - 8820) / 44100.0));
    CHECK (h.player.getDurationSeconds() == Catch::Approx ((length (whole) - 8820) / 44100.0));

    const auto stoppedAt = h.renderUntilStopped (100000);
    CHECK (maxError (h.output, blockSize, whole, blockSize, stoppedAt - blockSize) == 0.0f);
}

TEST_CASE ("PlayerEngine changes the gain of one part of a file", "[player][range][gain]")
{
    Harness h (44100.0, false);
    const auto file = fixtureFile ("flac-44k.flac");
    REQUIRE (h.player.load (file, part (0.0, 0.2)).isEmpty());
    REQUIRE (h.player.setNext (file, part (0.2, 0.0)).isEmpty());
    CHECK (h.player.setTrackGainAt (file, 0.2, 0.5f) == 1);
    CHECK (h.player.setTrackGainAt (file, 0.0, 0.5f) == 1);
    CHECK (h.player.setTrackGainAt (file, 0.1, 0.5f) == 0);
    CHECK (h.player.setTrackGain (file, 0.5f) == 2);
}

TEST_CASE ("PlayerEngine skips a stretch of a track once", "[player][range]")
{
    // Without read-ahead only: with it, the jump lands before the reader has
    // caught up, a moment of silence that is fine for the silences it skips.
    Harness h (44100.0, false);
    const auto whole = decode ("flac-44k.flac");

    auto options = Options {};
    options.skipFrom = 0.1;
    options.skipTo = 0.3;
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac"), options).isEmpty());
    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (100000);
    const auto expected = concat (slice (whole, 0, 4410), slice (whole, 13230, length (whole)));
    CHECK (maxError (h.output, blockSize, expected, blockSize, stoppedAt - blockSize) == 0.0f);

    // Once: after a seek back it plays through.
    h.output = {};
    REQUIRE (h.player.play());
    CHECK (h.player.seek (0.0));
    h.renderUntilStopped (100000);
    CHECK (length (h.output) > length (whole));
}

TEST_CASE ("PlayerEngine loops between two points on the exact sample", "[player][loop]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);

    Harness h (44100.0, readAhead);
    const auto whole = decode ("flac-44k.flac");
    auto& player = h.player;

    CHECK (player.setLoop (0.1, 0.35) == "No track is loaded");
    REQUIRE (player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    CHECK (player.setLoop (0.1, 0.2).startsWith ("A loop must"));
    CHECK (player.setLoop (-1.0, 0.4).startsWith ("A loop must"));
    REQUIRE (player.setLoop (0.1, 0.35).isEmpty());

    double start = 0.0, end = 0.0;
    REQUIRE (player.getLoop (start, end));
    CHECK (start == Catch::Approx (0.1));
    CHECK (end == Catch::Approx (0.35));

    REQUIRE (player.play());
    // Three times round, sending the spare reader back after each block.
    const auto loopLength = 15435 - 4410;
    while (length (h.output) < 15435 + 3 * loopLength)
    {
        h.renderBlock();
        player.dispatchEvents();
    }
    CHECK (player.getState() == State::playing);
    auto expected = slice (whole, 0, 15435);
    for (int i = 0; i < 4; ++i)
        expected = concat (expected, slice (whole, 4410, 15435));
    CHECK (maxError (h.output, blockSize, expected, blockSize, length (h.output) - blockSize) == 0.0f);
    CHECK (player.getPositionSeconds() >= 0.1);
    CHECK (player.getPositionSeconds() <= 0.35);

    // Cleared, it plays on to the end.
    player.clearLoop();
    CHECK_FALSE (player.getLoop (start, end));
    h.renderUntilStopped (200000);
    CHECK (player.getState() == State::stopped);

    // A load ends the loop.
    REQUIRE (player.setLoop (0.1, 0.35).isEmpty());
    REQUIRE (player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    CHECK_FALSE (player.getLoop (start, end));
}

TEST_CASE ("PlayerEngine changes tempo and pitch independently", "[player][tempo]")
{
    Harness h (44100.0, false);
    const auto whole = decode ("flac-44k.flac");
    auto& player = h.player;

    CHECK_FALSE (player.setTempo (0.25, 0.0));
    CHECK_FALSE (player.setTempo (1.75, 0.0));
    CHECK_FALSE (player.setTempo (1.0, 13.0));
    CHECK_FALSE (player.setTempo (std::numeric_limits<double>::quiet_NaN(), 0.0));
    CHECK (player.setTempo (1.0, 0.0)); // Nothing to do.

    REQUIRE (player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (player.setTempo (1.25, 0.0));
    REQUIRE (player.play());
    h.render (8192);
    // The file is read a quarter faster than the device plays.
    CHECK (player.getPositionSeconds() == Catch::Approx (8192 * 1.25 / 44100.0).margin (2 / 44100.0));
    CHECK (peak (h.output, 4096, 8192) > 0.05f);
    CHECK (player.getSignalInfo().tempo == 1.25);

    // Transposed at the normal tempo.
    REQUIRE (player.setTempo (1.0, 12.0));
    const auto before = player.getPositionSeconds();
    h.render (4096);
    CHECK (player.getPositionSeconds() - before == Catch::Approx (4096 / 44100.0).margin (2 / 44100.0));
    CHECK (player.getSignalInfo().semitones == 12.0);

    // Back to normal: the file itself again.
    REQUIRE (player.setTempo (1.0, 0.0));
    const auto at = static_cast<int> (std::llround (player.getPositionSeconds() * 44100.0));
    const auto from = length (h.output);
    h.render (blockSize);
    CHECK (maxError (h.output, from, whole, at, blockSize) == 0.0f);
    CHECK (player.getSignalInfo().tempo == 1.0);
}

TEST_CASE ("PlayerEngine describes the signal path", "[player][signal]")
{
    Harness h (48000.0, false);
    CHECK_FALSE (h.player.getSignalInfo().loaded);

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac"), 0.5f).isEmpty());
    auto info = h.player.getSignalInfo();
    CHECK (info.loaded);
    CHECK (info.codec == "flac");
    CHECK (info.lossless);
    CHECK (info.bitsPerSample == 16);
    CHECK (info.fileSampleRate == 44100.0);
    CHECK (info.channels == 2);
    CHECK (info.gain == 0.5f);
    CHECK (info.deviceSampleRate == 48000.0);

    REQUIRE (h.player.load (fixtureFile ("mp3-44k.mp3")).isEmpty());
    info = h.player.getSignalInfo();
    CHECK (info.codec == "mp3");
    CHECK_FALSE (info.lossless);
    CHECK (info.bitsPerSample == 0);
    CHECK (info.bitrateKbps > 0);
}

TEST_CASE ("PlayerEngine crossfeeds after the tap", "[player][crossfeed]")
{
    Harness h (44100.0, false);
    const auto whole = decode ("flac-44k.flac");
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    h.player.setCrossfeed (2);
    CHECK (h.player.getCrossfeed() == 2);
    REQUIRE (h.player.play());
    h.render (4096);
    // Each channel now carries some of the other.
    CHECK (maxError (h.output, blockSize, whole, blockSize, 2048) > 0.01f);
    h.player.setCrossfeed (99);
    CHECK (h.player.getCrossfeed() == anomp::Crossfeed::maxLevel);
}

TEST_CASE ("Crossfeed blends the channels at low frequencies", "[crossfeed]")
{
    anomp::Crossfeed crossfeed;
    crossfeed.prepare (44100.0);

    const auto settle = [&crossfeed] (float left, float right)
    {
        std::vector<float> l (44100, left), r (44100, right);
        crossfeed.process (l.data(), r.data(), static_cast<int> (l.size()));
        return std::pair { l.back(), r.back() };
    };

    // Off: untouched.
    CHECK (settle (1.0f, 0.0f) == std::pair { 1.0f, 0.0f });

    for (int level = 1; level <= anomp::Crossfeed::maxLevel; ++level)
    {
        CAPTURE (level);
        crossfeed.setLevel (level);
        // A centred (mono) signal keeps its level at DC.
        const auto [monoL, monoR] = settle (1.0f, 1.0f);
        CHECK (monoL == Catch::Approx (1.0).margin (1e-3));
        CHECK (monoR == Catch::Approx (1.0).margin (1e-3));
        // A hard-left one reaches the right ear too, more with each level.
        crossfeed.reset();
        const auto [left, right] = settle (1.0f, 0.0f);
        CHECK (right > 0.1f);
        CHECK (right < left);
    }
}

TEST_CASE ("PlayerEngine crossfades into a next track with equal-power curves", "[player][crossfade]")
{
    const auto readAhead = GENERATE (false, true);
    CAPTURE (readAhead);
    Harness h (44100.0, readAhead);
    const auto first = decode ("flac-44k.flac");
    const auto second = decode ("wav-s16-44k.wav");
    constexpr int fade = 4410; // 0.1 s.

    anomp::PlayerEngine::TrackOptions options;
    options.crossfade = 0.1;
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), options).isEmpty());
    CHECK (h.player.getSignalInfo().crossfade == Catch::Approx (0.1));
    REQUIRE (h.player.play());
    h.takeEvents();

    const auto fadeStart = length (first) - fade;
    h.render (length (first) + 1);
    CHECK (h.takeEvents() == std::vector<std::string> { "advanced" });
    // The next track took over already a fade's length in.
    CHECK (h.player.getPositionSeconds() == Catch::Approx ((length (h.output) - length (first) + fade) / 44100.0));

    // The first track alone up to the fade (after the fade-in of the first block).
    CHECK (maxError (h.output, blockSize, first, blockSize, fadeStart - blockSize) == 0.0f);

    // Both, the first fading out as the second fades in.
    float error = 0.0f;
    for (size_t ch = 0; ch < 2; ++ch)
        for (int i = 0; i < fade; ++i)
        {
            const auto angle = juce::MathConstants<double>::halfPi * (i + 0.5) / fade;
            const auto want = first[ch][static_cast<size_t> (fadeStart + i)] * std::cos (angle)
                              + second[ch][static_cast<size_t> (i)] * std::sin (angle);
            error = juce::jmax (
                error, static_cast<float> (std::abs (h.output[ch][static_cast<size_t> (fadeStart + i)] - want)));
        }
    CHECK (error < 1e-6f);

    // Then the rest of the second track.
    const auto stoppedAt = h.renderUntilStopped (200000);
    CHECK (h.takeEvents() == std::vector<std::string> { "ended", stateStopped });
    CHECK (maxError (h.output, length (first), second, fade, stoppedAt - length (first)) == 0.0f);
}

TEST_CASE ("PlayerEngine hands off without a crossfade across rates or while looping", "[player][crossfade]")
{
    Harness h (44100.0, false);
    anomp::PlayerEngine::TrackOptions options;
    options.crossfade = 0.1;

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("opus-48k.opus"), options).isEmpty());
    CHECK (h.player.getSignalInfo().crossfade == 0.0);

    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), options).isEmpty());
    CHECK (h.player.getSignalInfo().crossfade == Catch::Approx (0.1));
    REQUIRE (h.player.setLoop (0.05, 0.3).isEmpty());
    CHECK (h.player.getSignalInfo().crossfade == 0.0);
    h.player.clearLoop();

    // No longer than half of either track.
    options.crossfade = 11.0;
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), options).isEmpty());
    CHECK (h.player.getSignalInfo().crossfade == Catch::Approx ((22371 / 2) / 44100.0));
}

TEST_CASE ("PlayerEngine restarts a crossfade after a seek back", "[player][crossfade]")
{
    Harness h (44100.0, false);
    const auto first = decode ("flac-44k.flac");
    const auto second = decode ("wav-s16-44k.wav");
    constexpr int fade = 4410;
    anomp::PlayerEngine::TrackOptions options;
    options.crossfade = 0.1;
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), options).isEmpty());
    REQUIRE (h.player.play());

    // Halfway into the fade, back to the start.
    h.render (length (first) - fade / 2);
    REQUIRE (h.player.seek (0.0));
    const auto from = length (h.output);
    h.render (length (first));
    // The fade begins again with the next track from its start.
    const auto fadeStart = from + length (first) - fade;
    const auto angle = juce::MathConstants<double>::halfPi * 0.5 / fade;
    const auto want =
        first[0][static_cast<size_t> (length (first) - fade)] * std::cos (angle) + second[0][0] * std::sin (angle);
    CHECK (h.output[0][static_cast<size_t> (fadeStart)] == Catch::Approx (want).margin (1e-6));
}

TEST_CASE ("Equaliser boosts and cuts each band, gliding to its settings", "[equaliser]")
{
    anomp::Equaliser eq;
    eq.prepare (44100.0);
    CHECK_FALSE (eq.isActive());

    anomp::Equaliser::Gains gains {};
    gains[5] = 6.0;  // 1 kHz
    gains[1] = -6.0; // 62.5 Hz
    eq.set (true, gains, 0.0);
    CHECK (eq.isActive());

    std::vector<float> left (512), right (512);
    const auto run = [&] (int blocks)
    {
        for (int b = 0; b < blocks; ++b)
        {
            std::fill (left.begin(), left.end(), 0.0f);
            std::fill (right.begin(), right.end(), 0.0f);
            eq.process (left.data(), right.data(), 512);
        }
    };

    // Part of the way after one block…
    run (1);
    CHECK (eq.responseDb (1000.0) > 0.5);
    CHECK (eq.responseDb (1000.0) < 5.0);
    // …and there after 100 ms.
    run (10);
    CHECK (eq.responseDb (1000.0) == Catch::Approx (6.0).margin (0.05));
    CHECK (eq.responseDb (62.5) == Catch::Approx (-6.0).margin (0.1));
    CHECK (std::abs (eq.responseDb (8000.0)) < 0.3);

    // A 1 kHz sine comes out twice as loud (+6 dB).
    std::vector<float> sineL (44100), sineR (44100);
    for (size_t i = 0; i < sineL.size(); ++i)
        sineL[i] = sineR[i] =
            0.25f * static_cast<float> (std::sin (juce::MathConstants<double>::twoPi * 1000.0 * i / 44100.0));
    for (size_t i = 0; i < sineL.size(); i += 512)
        eq.process (sineL.data() + i, sineR.data() + i, static_cast<int> (juce::jmin<size_t> (512, sineL.size() - i)));
    const auto peakOut = *std::max_element (sineL.begin() + 22050, sineL.end());
    CHECK (peakOut == Catch::Approx (0.5).margin (0.01));

    // Clamped, and off glides to flat and then bypasses.
    gains.fill (40.0);
    eq.set (true, gains, -40.0);
    run (20);
    CHECK (eq.responseDb (1000.0) > 10.0);
    eq.set (false, gains, 0.0);
    CHECK_FALSE (eq.isEnabled());
    run (30);
    CHECK_FALSE (eq.isActive());
    CHECK (std::abs (eq.responseDb (1000.0)) < 1e-9);
}

TEST_CASE ("PlayerEngine equalises after the tap, with its preamp", "[player][equaliser]")
{
    Harness h (44100.0, false);
    const auto whole = decode ("flac-44k.flac");
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    CHECK_FALSE (h.player.getSignalInfo().equaliser);
    // Flat bands and -6.02 dB: half the level, once it has glided there.
    h.player.setEqualiser (true, anomp::Equaliser::Gains {}, -20.0 * std::log10 (2.0));
    CHECK (h.player.getSignalInfo().equaliser);
    REQUIRE (h.player.play());
    h.render (8192);
    CHECK (maxError (h.output, 4096, whole, 4096, 4096, 0.5f) < 1e-5f);
}

TEST_CASE ("PlayerEngine hands off while an audio thread renders", "[player][gapless][threads]")
{
    // As with a device: one thread renders while the message thread queues
    // next tracks, seeks, changes gains and collects events. The rest of
    // this file renders on the calling thread, so this is the test that
    // lets TSan (the tsan preset, PLAN.md H6) see the two threads meet.
    anomp::FormatRegistry registry;
    juce::TimeSliceThread readAhead ("test read-ahead");
    readAhead.startThread();
    {
        anomp::PlayerEngine player (registry.manager(), &readAhead);
        player.prepareToPlay (blockSize, 44100.0);
        int advanced = 0;
        player.onTrackEnded = [&advanced] (bool next)
        {
            advanced += next ? 1 : 0;
        };

        const auto first = fixtureFile ("flac-44k.flac");
        const auto second = fixtureFile ("wav-s16-44k.wav");
        REQUIRE (player.load (first).isEmpty());
        REQUIRE (player.play());

        std::atomic<bool> running { true };
        std::thread audio (
            [&]
            {
                juce::AudioBuffer<float> block (2, blockSize);
                while (running)
                {
                    block.clear();
                    player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));
                    std::this_thread::sleep_for (std::chrono::milliseconds (1));
                }
            });

        // Each round queues a next track and seeks close to the current
        // one's end, so the next takes over within a few blocks.
        constexpr int rounds = 20;
        for (int round = 0; round < rounds; ++round)
        {
            const auto advances = player.getAdvanceCount();
            REQUIRE (player.setNext (round % 2 == 0 ? second : first, round % 3 == 0 ? 0.5f : 1.0f).isEmpty());
            player.setVolume (round % 2 == 0 ? 0.5f : 1.0f);
            player.setTrackGain (first, 0.75f);
            REQUIRE (player.seek (player.getDurationSeconds() - 0.02));
            for (int wait = 0; wait < 1000 && player.getAdvanceCount() == advances; ++wait)
            {
                player.dispatchEvents();
                juce::Thread::sleep (2);
            }
            REQUIRE (player.getAdvanceCount() == advances + 1);
        }

        running = false;
        audio.join();
        player.dispatchEvents();
        CHECK (player.getAdvanceCount() == rounds);
        CHECK (advanced >= 1);
        CHECK (player.getState() == State::playing);
    }
    readAhead.stopThread (1000);
}
