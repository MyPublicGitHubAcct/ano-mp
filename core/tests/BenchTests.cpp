#include <catch2/catch_test_macros.hpp>

#include "FormatRegistry.h"
#include "PlayerEngine.h"
#include "SpectrumAnalyser.h"

#include <chrono>
#include <iostream>
#include <vector>

// How fast the core does its work while playing (PLAN.md H18), as a multiple
// of real time: 100 means a second of audio takes 10 ms of one core. Hidden
// ([.]), so ctest and a plain run skip them; scripts/bench.py runs them from
// the Release `bench` preset and reads the `bench <key> <value> x` lines.
// Debug timings mean nothing here.

namespace
{
constexpr int blockSize = 512;
constexpr double deviceRate = 48000.0;
// Seconds of audio each benchmark renders: long enough to steady the timing.
constexpr double seconds = 60.0;

juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }

using Clock = std::chrono::steady_clock;

double elapsedSeconds (Clock::time_point start) { return std::chrono::duration<double> (Clock::now() - start).count(); }

void report (const std::string& key, double realTime)
{
    std::cout << "bench " << key << " " << realTime << " x" << std::endl;
}

/** Renders `seconds` of `name` through the engine as the device would pull it,
    replaying the file whenever it ends; returns the multiple of real time. */
double renderFile (const char* name, bool effects)
{
    anomp::FormatRegistry registry;
    // No read-ahead thread: decoding happens inside getNextAudioBlock, so its
    // cost is counted, as the read-ahead thread's would be while playing.
    anomp::PlayerEngine player (registry.manager(), nullptr);
    player.prepareToPlay (blockSize, deviceRate);
    if (effects)
    {
        player.setCrossfeed (2);
        anomp::Equaliser::Gains gains {};
        for (size_t i = 0; i < gains.size(); ++i)
            gains[i] = (i % 2 == 0) ? 3.0 : -3.0;
        player.setEqualiser (true, gains, -3.0);
    }
    REQUIRE (player.load (fixtureFile (name)).isEmpty());
    REQUIRE (player.play());

    juce::AudioBuffer<float> block (2, blockSize);
    const auto blocks = static_cast<int> (seconds * deviceRate / blockSize);
    double busy = 0.0;
    for (int i = 0; i < blocks; ++i)
    {
        if (player.getState() != anomp::PlayerEngine::State::playing)
        {
            player.dispatchEvents();
            REQUIRE (player.seek (0.0));
            REQUIRE (player.play());
        }
        block.clear();
        const auto start = Clock::now();
        player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));
        busy += elapsedSeconds (start);
    }
    return seconds / busy;
}
} // namespace

TEST_CASE ("Bench: playback through the engine", "[.][bench]")
{
    // A lossless file at the device rate, and lossy ones resampled to it.
    report ("core.play.flac", renderFile ("flac-long-48k-mono.flac", false));
    report ("core.play.mp3", renderFile ("mp3-vbr-long-44k.mp3", false));
    report ("core.play.aac", renderFile ("aac-long-44k.m4a", false));
    report ("core.play.opus", renderFile ("opus-long-48k.opus", false));
    // With crossfeed and the equaliser on.
    report ("core.play.mp3_effects", renderFile ("mp3-vbr-long-44k.mp3", true));
}

TEST_CASE ("Bench: the visualizer's analysis", "[.][bench]")
{
    // As visualizer.rs configures it: 64 bands, a 512-sample waveform, 60
    // frames a second, each over the latest window of the tap.
    constexpr double framesPerSecond = 60.0;
    constexpr int hop = static_cast<int> (deviceRate / framesPerSecond);
    anomp::SpectrumAnalyser analyser (64, 512);
    analyser.prepare (deviceRate);

    juce::Random random (42);
    std::vector<float> left (anomp::SpectrumAnalyser::windowSize), right (anomp::SpectrumAnalyser::windowSize);
    for (size_t i = 0; i < left.size(); ++i)
    {
        left[i] = random.nextFloat() * 2.0f - 1.0f;
        right[i] = random.nextFloat() * 2.0f - 1.0f;
    }

    anomp::AnalysisFrame frame;
    const auto frames = static_cast<int> (seconds * framesPerSecond);
    const auto start = Clock::now();
    for (int i = 0; i < frames; ++i)
        analyser.process (left.data(), right.data(), hop, frame);
    report ("core.analysis", seconds / elapsedSeconds (start));
}
