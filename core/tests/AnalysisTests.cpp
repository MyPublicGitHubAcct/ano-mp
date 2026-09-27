#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "anomp/anomp.h"
#include "AnalysisThread.h"
#include "FormatRegistry.h"
#include "PlayerEngine.h"
#include "SignalTap.h"
#include "SpectrumAnalyser.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <mutex>
#include <numeric>
#include <random>
#include <vector>

// The visualizer's analysis: the tap on the player's output, the analyser's
// measurements on synthetic signals, and the thread that runs it.

namespace
{
constexpr double twoPi = 6.283185307179586;
constexpr int window = anomp::SpectrumAnalyser::windowSize;

std::vector<float> sine (double frequency, double amplitude, double sampleRate, int length, double phase = 0.0)
{
    std::vector<float> samples (static_cast<size_t> (length));
    for (int i = 0; i < length; ++i)
        samples[static_cast<size_t> (i)] =
            static_cast<float> (amplitude * std::sin (twoPi * frequency * i / sampleRate + phase));
    return samples;
}

std::vector<float> mix (std::initializer_list<std::vector<float>> signals)
{
    std::vector<float> result (signals.begin()->size());
    for (auto& signal : signals)
        for (size_t i = 0; i < result.size(); ++i)
            result[i] += signal[i];
    return result;
}

/** The band whose range holds `frequency`. */
size_t bandOf (const anomp::AnalysisFrame& frame, double frequency)
{
    const auto bands = static_cast<double> (frame.bands.size());
    const auto position = std::log (frequency / frame.lowestHz) / std::log (frame.highestHz / frame.lowestHz);
    return static_cast<size_t> (std::floor (position * bands));
}

size_t loudestBand (const anomp::AnalysisFrame& frame)
{
    return static_cast<size_t> (std::max_element (frame.bands.begin(), frame.bands.end()) - frame.bands.begin());
}

anomp::AnalysisFrame analyse (const std::vector<float>& left, const std::vector<float>& right, double sampleRate)
{
    anomp::SpectrumAnalyser analyser (64, 512);
    analyser.prepare (sampleRate);
    anomp::AnalysisFrame frame;
    analyser.process (left.data() + left.size() - window, right.data() + right.size() - window, window, frame);
    return frame;
}

/** Runs the analyser over `signal` (mono) in hops of `hop` samples, as the
    thread would at hop / rate frames a second; returns the frames' beat
    times in seconds. */
std::vector<double> beatTimes (const std::vector<float>& signal, double sampleRate, int hop)
{
    anomp::SpectrumAnalyser analyser (64, 256);
    analyser.prepare (sampleRate);
    anomp::AnalysisFrame frame;
    std::vector<double> beats;

    for (auto end = window; end <= static_cast<int> (signal.size()); end += hop)
    {
        const auto* start = signal.data() + end - window;
        analyser.process (start, start, hop, frame);
        if (frame.beat)
            beats.push_back (static_cast<double> (end) / sampleRate);
    }
    return beats;
}

/** Low thumps every `period` seconds over quiet noise: a 60 Hz burst
    decaying over 0.1 s. */
std::vector<float> kicks (double sampleRate, double seconds, double period)
{
    std::mt19937 random (42);
    std::uniform_real_distribution<float> noise (-0.02f, 0.02f);
    std::vector<float> signal (static_cast<size_t> (sampleRate * seconds));

    for (size_t i = 0; i < signal.size(); ++i)
    {
        const auto t = static_cast<double> (i) / sampleRate;
        const auto sinceKick = std::fmod (t, period);
        const auto envelope = std::exp (-sinceKick / 0.05);
        signal[i] = static_cast<float> (0.8 * envelope * std::sin (twoPi * 60.0 * sinceKick)) + noise (random);
    }
    return signal;
}
} // namespace

TEST_CASE ("SignalTap keeps the latest samples", "[analysis]")
{
    anomp::SignalTap tap;
    std::vector<float> left (8), right (8);

    // Fewer than asked for: zeros first.
    const float a[] = { 1, 2, 3 }, b[] = { -1, -2, -3 };
    tap.push (a, b, 3);
    CHECK (tap.readLatest (left.data(), right.data(), 5) == 3);
    CHECK (left == std::vector<float> { 0, 0, 1, 2, 3, 0, 0, 0 });
    CHECK (right == std::vector<float> { 0, 0, -1, -2, -3, 0, 0, 0 });

    // Across the end of the ring.
    std::vector<float> ramp (anomp::SignalTap::capacity - 1);
    std::iota (ramp.begin(), ramp.end(), 10.0f);
    tap.push (ramp.data(), ramp.data(), static_cast<int> (ramp.size()));
    CHECK (tap.getWrittenCount() == anomp::SignalTap::capacity + 2);
    std::vector<float> latest (4), unused (4);
    tap.readLatest (latest.data(), unused.data(), 4);
    CHECK (latest == std::vector<float> (ramp.end() - 4, ramp.end()));
}

TEST_CASE ("SpectrumAnalyser finds a tone's band", "[analysis]")
{
    const auto rate = GENERATE (44100.0, 48000.0, 96000.0);
    const auto frequency = GENERATE (100.0, 1000.0, 6000.0);
    CAPTURE (rate, frequency);

    // -20 dB, so the tilt doesn't push the band past the top of the scale.
    const auto tone = sine (frequency, 0.1, rate, window);
    const auto frame = analyse (tone, tone, rate);

    REQUIRE (frame.bands.size() == 64);
    CHECK_FALSE (frame.silent);
    CHECK (frame.lowestHz == 30.0f);
    CHECK (frame.highestHz == Catch::Approx (std::min (16000.0, 0.45 * rate)));

    // Bands narrower than a bin (the lowest, at high rates) are read at their
    // centres, so the loudest can be the neighbour whose centre is nearer.
    const auto band = bandOf (frame, frequency);
    const auto loudest = loudestBand (frame);
    CHECK (loudest + 1 >= band);
    CHECK (loudest <= band + 1);

    // -20 dB plus the tilt (3 dB an octave from 1 kHz), on the 70 dB scale,
    // less up to 1.5 dB where the tone falls between bins.
    const auto expected = (-20.0 + 3.0 * std::log2 (frequency / 1000.0) + 70.0) / 70.0;
    CHECK (frame.bands[loudest] == Catch::Approx (expected).margin (0.03));

    // Two octaves away there's next to nothing.
    for (size_t b = 0; b < frame.bands.size(); ++b)
    {
        const auto centre =
            frame.lowestHz * std::pow (frame.highestHz / frame.lowestHz, (static_cast<double> (b) + 0.5) / 64.0);
        if (std::abs (std::log2 (centre / frequency)) > 2.0)
            CHECK (frame.bands[b] < 0.2f);
    }
}

TEST_CASE ("SpectrumAnalyser reads silence as zero", "[analysis]")
{
    const std::vector<float> silence (window);
    const auto frame = analyse (silence, silence, 48000.0);

    CHECK_FALSE (frame.silent); // Silent audio is still audio; only the thread reports none.
    CHECK (std::all_of (frame.bands.begin(), frame.bands.end(), [] (float v) { return v == 0.0f; }));
    CHECK (std::all_of (frame.chroma.begin(), frame.chroma.end(), [] (float v) { return v == 0.0f; }));
    CHECK (frame.peak == std::array<float, 2> {});
    CHECK (frame.rms == std::array<float, 2> {});
    CHECK_FALSE (frame.beat);
}

TEST_CASE ("SpectrumAnalyser names the pitch classes played", "[analysis]")
{
    constexpr double rate = 48000.0;
    const auto note = [] (int midi)
    {
        return 440.0 * std::pow (2.0, (midi - 69) / 12.0);
    };

    SECTION ("A 440 Hz tone is an A")
    {
        const auto tone = sine (440.0, 0.5, rate, window);
        const auto frame = analyse (tone, tone, rate);
        CHECK (frame.chroma[9] == 1.0f);
        for (size_t pc = 0; pc < 12; ++pc)
            if (pc != 9)
                CHECK (frame.chroma[pc] < 0.25f);
    }

    SECTION ("A C major chord lights C, E and G")
    {
        const auto chord = mix ({ sine (note (60), 0.3, rate, window, 0.1), sine (note (64), 0.3, rate, window, 0.7),
                                  sine (note (67), 0.3, rate, window, 1.9), sine (note (48), 0.3, rate, window, 2.3) });
        const auto frame = analyse (chord, chord, rate);

        std::vector<size_t> order (12);
        std::iota (order.begin(), order.end(), size_t { 0 });
        std::sort (order.begin(), order.end(), [&] (size_t a, size_t b) { return frame.chroma[a] > frame.chroma[b]; });
        std::sort (order.begin(), order.begin() + 3);
        CHECK (std::vector<size_t> (order.begin(), order.begin() + 3) == std::vector<size_t> { 0, 4, 7 });
        CHECK (frame.chroma[order[3]] < 0.35f);
    }
}

TEST_CASE ("SpectrumAnalyser measures each channel's level", "[analysis]")
{
    const auto left = sine (1000.0, 0.5, 48000.0, window);
    const std::vector<float> right (window);
    const auto frame = analyse (left, right, 48000.0);

    CHECK (frame.peak[0] == Catch::Approx (0.5).margin (0.001));
    CHECK (frame.rms[0] == Catch::Approx (0.5 / std::sqrt (2.0)).margin (0.002));
    CHECK (frame.peak[1] == 0.0f);
    CHECK (frame.rms[1] == 0.0f);
}

TEST_CASE ("SpectrumAnalyser starts the waveform at a rising zero crossing", "[analysis]")
{
    const auto length = GENERATE (256, 512, 2048);
    CAPTURE (length);

    anomp::SpectrumAnalyser analyser (32, length);
    analyser.prepare (48000.0);
    anomp::AnalysisFrame frame;

    // The same wave at several phases always starts at the same point.
    for (const auto phase : { 0.3, 1.7, 4.0 })
    {
        const auto left = sine (220.0, 0.8, 48000.0, window, phase);
        const auto right = sine (220.0, 0.4, 48000.0, window, phase);
        analyser.process (left.data(), right.data(), window, frame);

        REQUIRE (frame.left.size() == static_cast<size_t> (length));
        REQUIRE (frame.right.size() == static_cast<size_t> (length));
        CHECK (frame.left[0] >= 0.0f);
        CHECK (frame.left[0] < 0.03f);
        CHECK (frame.left[1] > frame.left[0]);
        CHECK (frame.right[5] == Catch::Approx (frame.left[5] / 2.0f));
    }
}

TEST_CASE ("SpectrumAnalyser finds beats and only beats", "[analysis]")
{
    constexpr double rate = 48000.0;
    const auto hop = GENERATE (800, 1600); // 60 and 30 frames a second.
    CAPTURE (hop);

    SECTION ("Kicks twice a second")
    {
        const auto beats = beatTimes (kicks (rate, 6.0, 0.5), rate, hop);
        // The window fills in the first 0.17 s; every kick after that is found,
        // each within 60 ms of its start.
        CHECK (beats.size() >= 11);
        CHECK (beats.size() <= 12);
        for (const auto time : beats)
        {
            const auto late = std::fmod (time, 0.5);
            CHECK (late < 0.06);
        }
    }

    SECTION ("A steady tone has no beats once it has started")
    {
        const auto tone = mix ({ sine (80.0, 0.5, rate, static_cast<int> (rate * 4)),
                                 sine (1200.0, 0.2, rate, static_cast<int> (rate * 4)) });
        const auto beats = beatTimes (tone, rate, hop);
        CHECK (std::none_of (beats.begin(), beats.end(), [] (double time) { return time > 0.3; }));
    }
}

TEST_CASE ("SpectrumAnalyser clamps its sizes and zeroes a silent frame", "[analysis]")
{
    anomp::SpectrumAnalyser analyser (1, 100000);
    CHECK (analyser.getNumBands() == anomp::SpectrumAnalyser::minBands);
    CHECK (analyser.getWaveformLength() == anomp::SpectrumAnalyser::maxWaveformLength);

    analyser.prepare (44100.0);
    const auto tone = sine (500.0, 0.5, 44100.0, window);
    anomp::AnalysisFrame frame;
    analyser.process (tone.data(), tone.data(), window, frame);
    CHECK (frame.peak[0] > 0.0f);

    analyser.processSilence (frame);
    CHECK (frame.silent);
    CHECK (frame.bands.size() == 4);
    CHECK (std::all_of (frame.bands.begin(), frame.bands.end(), [] (float v) { return v == 0.0f; }));
    CHECK (std::all_of (frame.left.begin(), frame.left.end(), [] (float v) { return v == 0.0f; }));
    CHECK (frame.peak == std::array<float, 2> {});
    CHECK (frame.onset == 0.0f);
}

TEST_CASE ("AnalysisThread sends frames while audio flows, then one silent frame", "[analysis]")
{
    anomp::SignalTap tap;
    tap.setSampleRate (48000.0);

    std::mutex mutex;
    std::vector<anomp::AnalysisFrame> frames;
    std::atomic<bool> feeding { true };

    {
        anomp::AnalysisThread thread (tap, 32, 128, 100.0,
                                      [&] (const anomp::AnalysisFrame& frame)
                                      {
                                          const std::scoped_lock lock (mutex);
                                          frames.push_back (frame);
                                      });

        // 0.3 s of a 1 kHz tone in 10 ms blocks, as an audio device would.
        const auto tone = sine (1000.0, 0.5, 48000.0, 48000);
        for (int block = 0; block < 30; ++block)
        {
            tap.push (tone.data() + block * 480, tone.data() + block * 480, 480);
            juce::Thread::sleep (10);
        }
        feeding = false;
        juce::Thread::sleep (400); // Well past staleSeconds.
    }

    const std::scoped_lock lock (mutex);
    REQUIRE (frames.size() >= 10);
    CHECK_FALSE (frames.front().silent);
    CHECK (frames.front().bands.size() == 32);
    CHECK (frames.front().left.size() == 128);
    CHECK (loudestBand (frames[frames.size() / 2]) == bandOf (frames.front(), 1000.0));

    // Exactly one silent frame, the last.
    CHECK (frames.back().silent);
    CHECK (std::count_if (frames.begin(), frames.end(), [] (const auto& frame) { return frame.silent; }) == 1);
}

TEST_CASE ("PlayerEngine taps its output before the volume, only while playing", "[analysis][player]")
{
    anomp::FormatRegistry registry;
    anomp::PlayerEngine player (registry.manager(), nullptr);
    player.prepareToPlay (512, 44100.0);
    CHECK (player.getTap().getSampleRate() == 44100.0);

    const auto render = [&]
    {
        juce::AudioBuffer<float> block (2, 512);
        block.clear();
        player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));
        return block;
    };

    REQUIRE (player.load (juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile ("flac-44k.flac")).isEmpty());
    render(); // Stopped.
    CHECK (player.getTap().getWrittenCount() == 0);

    player.setVolume (0.25f);
    REQUIRE (player.play());
    render(); // Ramps from silence to the volume.
    const auto block = render();
    CHECK (player.getTap().getWrittenCount() == 1024);

    std::vector<float> left (512), right (512);
    player.getTap().readLatest (left.data(), right.data(), 512);
    for (int i = 0; i < 512; i += 37)
    {
        CHECK (block.getSample (0, i) == Catch::Approx (0.25f * left[static_cast<size_t> (i)]).margin (1.0e-6));
        CHECK (block.getSample (1, i) == Catch::Approx (0.25f * right[static_cast<size_t> (i)]).margin (1.0e-6));
    }

    player.pause();
    render(); // Fades out.
    render();
    CHECK (player.getTap().getWrittenCount() == 1024);
}

TEST_CASE ("C API analysis callback", "[c-api][analysis]")
{
    const anomp_analysis_config config { 64, 512, 60.0 };
    CHECK (anomp_engine_set_analysis_callback (
               nullptr, &config, [] (const anomp_analysis_frame*, void*) {}, nullptr)
           == 0);
    CHECK (anomp_engine_set_analysis_callback (nullptr, nullptr, nullptr, nullptr) == 0);

    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);

    struct Received
    {
        std::atomic<int> frames { 0 }, silent { 0 }, bands { 0 }, waveform { 0 };
    } received;

    const auto callback = [] (const anomp_analysis_frame* frame, void* userData)
    {
        auto& r = *static_cast<Received*> (userData);
        r.bands = frame->band_count;
        r.waveform = frame->waveform_length;
        r.silent += frame->silent;
        ++r.frames;
    };

    for (const auto bad : { anomp_analysis_config { 3, 512, 60.0 }, anomp_analysis_config { 64, 4096, 60.0 },
                            anomp_analysis_config { 64, 512, 0.0 }, anomp_analysis_config { 64, 512, 1.0 / 0.0 } })
        CHECK (anomp_engine_set_analysis_callback (engine, &bad, callback, &received) == 0);
    CHECK (anomp_engine_set_analysis_callback (engine, nullptr, callback, &received) == 0);

    // Nothing plays (no device), so the one frame is the silent one.
    REQUIRE (anomp_engine_set_analysis_callback (engine, &config, callback, &received) == 1);
    juce::Thread::sleep (400);
    CHECK (anomp_engine_set_analysis_callback (engine, nullptr, nullptr, nullptr) == 1);
    const auto frames = received.frames.load();
    CHECK (frames == 1);
    CHECK (received.silent == 1);
    CHECK (received.bands == 64);
    CHECK (received.waveform == 512);

    // Stopped: nothing more arrives.
    juce::Thread::sleep (300);
    CHECK (received.frames == frames);

    // Destroying the engine stops a running analysis.
    REQUIRE (anomp_engine_set_analysis_callback (engine, &config, callback, &received) == 1);
    anomp_engine_destroy (engine);
}
