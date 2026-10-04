#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "Echo.h"
#include "Modulation.h"
#include "Reverb.h"
#include "Signals.h"

#include <cmath>
#include <set>

// Each effect on its own, run as the chain runs it: wet only, in chunks.

using anomp::fx::EffectType;
using namespace signals;

namespace
{
constexpr double rate = 44100.0;

/** The index of the largest sample in [from, to). */
std::size_t peakAt (const std::vector<float>& samples, std::size_t from, std::size_t to)
{
    auto best = from;
    for (auto i = from; i < to; ++i)
        if (std::abs (samples[i]) > std::abs (samples[best]))
            best = i;
    return best;
}

double sum (const std::vector<float>& samples, std::size_t from, std::size_t to)
{
    double total = 0.0;
    for (auto i = from; i < to; ++i)
        total += samples[i];
    return total;
}
} // namespace

TEST_CASE ("Every effect stays finite and bounded at every rate and setting", "[effects]")
{
    const auto sampleRate = GENERATE (22050.0, 44100.0, 96000.0, 192000.0);
    const auto setting = GENERATE (0, 1, 2); // Defaults, every minimum, every maximum.
    anomp::fx::EffectChain chain;
    chain.prepare (sampleRate);
    for (std::size_t t = 0; t < anomp::fx::effectCount; ++t)
    {
        const auto type = static_cast<EffectType> (t);
        const auto& info = anomp::fx::describe (type);
        chain.setEnabled (type, true);
        chain.setMix (type, 1.0f);
        for (std::size_t i = 0; i < info.numParams; ++i)
        {
            const auto& param = info.params[i];
            chain.setParameter (type, i, setting == 0 ? param.defaultValue : setting == 1 ? param.min : param.max);
        }
    }
    chain.setFreezeHeld (true);

    auto audio = noise (static_cast<std::size_t> (sampleRate * 2.0));
    run (chain, audio);
    CHECK (finite (audio, 100.0f));
    // Into silence: the tails die away without denormal trouble or NaNs.
    auto after = silence (static_cast<std::size_t> (sampleRate));
    run (chain, after);
    CHECK (finite (after, 100.0f));
}

TEST_CASE ("Reverb rings on after an impulse, longer in a larger room", "[effects][reverb]")
{
    auto tailEnergy = [] (float size)
    {
        anomp::fx::Reverb reverb;
        reverb.prepare (rate);
        defaults (reverb, EffectType::reverb);
        reverb.setParameter (anomp::fx::Reverb::size, size, true);
        reverb.setParameter (anomp::fx::Reverb::preDelay, 0.0f, true);
        auto audio = impulse (static_cast<std::size_t> (rate * 2.0));
        run (reverb, audio);
        CHECK (finite (audio, 1.0f));
        return rms (audio.left, static_cast<std::size_t> (rate), audio.size());
    };
    const auto small = tailEnergy (0.1f), large = tailEnergy (0.95f);
    CHECK (small > 0.0);
    CHECK (db (large / small) > 20.0);

    anomp::fx::Reverb reverb;
    reverb.prepare (rate);
    defaults (reverb, EffectType::reverb);
    CHECK (reverb.tailSeconds() > 1.0);
    CHECK (reverb.tailSeconds() < 30.0);
}

TEST_CASE ("Reverb's width runs from mono to stereo, after its pre-delay", "[effects][reverb]")
{
    anomp::fx::Reverb reverb;
    reverb.prepare (rate);
    defaults (reverb, EffectType::reverb);
    reverb.setParameter (anomp::fx::Reverb::width, 0.0f, true);
    reverb.setParameter (anomp::fx::Reverb::preDelay, 100.0f, true);
    auto audio = impulse (static_cast<std::size_t> (rate));
    run (reverb, audio);

    // Nothing comes out before the pre-delay and the shortest comb.
    CHECK (rms (audio.left, 0, static_cast<std::size_t> (rate * 0.1)) == 0.0);
    for (std::size_t i = 0; i < audio.size(); ++i)
        REQUIRE (audio.left[i] == Catch::Approx (audio.right[i]).margin (1e-6));

    reverb.reset();
    reverb.setParameter (anomp::fx::Reverb::width, 1.0f, true);
    audio = impulse (static_cast<std::size_t> (rate));
    run (reverb, audio);
    double difference = 0.0;
    for (std::size_t i = 0; i < audio.size(); ++i)
        difference += std::abs (audio.left[i] - audio.right[i]);
    CHECK (difference > 1.0);
}

TEST_CASE ("Chorus keeps the level and moves the copies", "[effects][chorus]")
{
    anomp::fx::Chorus chorus;
    chorus.prepare (rate);
    defaults (chorus, EffectType::chorus);
    const auto input = sine (440.0, rate, static_cast<std::size_t> (rate * 2.0));
    auto audio = input;
    run (chorus, audio);
    const auto from = static_cast<std::size_t> (rate * 0.1);
    CHECK (std::abs (db (rms (audio.left, from, audio.size()) / rms (input.left, from, input.size()))) < 3.0);
    // The two channels' LFOs are apart.
    double difference = 0.0;
    for (auto i = from; i < audio.size(); ++i)
        difference += std::abs (audio.left[i] - audio.right[i]);
    CHECK (difference > 10.0);
}

TEST_CASE ("Flanger repeats an impulse at its delay, fed back", "[effects][flanger]")
{
    anomp::fx::Flanger flanger;
    flanger.prepare (rate);
    defaults (flanger, EffectType::flanger);
    flanger.setParameter (anomp::fx::Flanger::depth, 0.0f, true); // Held at 0.3 ms.
    flanger.setParameter (anomp::fx::Flanger::feedback, 0.5f, true);
    auto audio = impulse (256);
    run (flanger, audio);

    const auto first = peakAt (audio.left, 0, 256);
    CHECK (first == Catch::Approx (rate * 0.0003).margin (1.0));
    const auto second = peakAt (audio.left, first + 3, 256);
    CHECK (second == Catch::Approx (2.0 * static_cast<double> (first)).margin (2.0));
    // Interpolation spreads each repeat over a few samples but keeps its sum.
    CHECK (sum (audio.left, second - 4, second + 4)
           == Catch::Approx (0.5 * sum (audio.left, first - 4, first + 4)).margin (0.02));
}

TEST_CASE ("Phaser's allpasses keep the level without feedback", "[effects][phaser]")
{
    anomp::fx::Phaser phaser;
    phaser.prepare (rate);
    defaults (phaser, EffectType::phaser);
    phaser.setParameter (anomp::fx::Phaser::feedback, 0.0f, true);
    const auto input = noise (static_cast<std::size_t> (rate * 2.0));
    auto audio = input;
    run (phaser, audio);
    CHECK (std::abs (db (rms (audio.left, 0, audio.size()) / rms (input.left, 0, input.size()))) < 0.5);
    // But it changes the waveform: the phase moves.
    CHECK (rms (audio.left, 0, audio.size()) > 0.0);
    CHECK (std::abs (audio.left[1000] - input.left[1000]) + std::abs (audio.left[2000] - input.left[2000]) > 0.0f);
}

TEST_CASE ("Echo repeats at its time, each repeat quieter, bouncing when spread", "[effects][echo]")
{
    anomp::fx::Echo echo;
    echo.prepare (rate);
    defaults (echo, EffectType::echo);
    echo.setParameter (anomp::fx::Echo::time, 100.0f, true);
    echo.setParameter (anomp::fx::Echo::feedback, 0.5f, true);
    echo.setParameter (anomp::fx::Echo::tone, 1.0f, true);
    echo.setParameter (anomp::fx::Echo::spread, 0.0f, true);
    const auto spacing = static_cast<std::size_t> (rate * 0.1);
    auto audio = impulse (spacing * 4);
    run (echo, audio);

    const auto first = peakAt (audio.left, 0, spacing + spacing / 2);
    const auto second = peakAt (audio.left, spacing + spacing / 2, spacing * 2 + spacing / 2);
    CHECK (first == Catch::Approx (spacing).margin (3.0));
    CHECK (second == Catch::Approx (2 * spacing).margin (6.0));
    // The tone's lowpass spreads each repeat but keeps its sum.
    CHECK (sum (audio.left, second - 50, second + 50)
           == Catch::Approx (0.5 * sum (audio.left, first - 50, first + 50)).margin (0.01));
    CHECK (echo.tailSeconds() == Catch::Approx (0.1 * (1.0 + 3.0 / std::log10 (2.0))).margin (0.01));

    // Ping-pong: an impulse on the left comes back left, then right.
    echo.setParameter (anomp::fx::Echo::spread, 1.0f, true);
    echo.reset();
    audio = silence (spacing * 4);
    audio.left[0] = 1.0f;
    run (echo, audio);
    const auto window = [&] (const std::vector<float>& channel, std::size_t centre)
    {
        return rms (channel, centre - 100, centre + 100);
    };
    CHECK (window (audio.left, spacing) > 10.0 * window (audio.right, spacing));
    CHECK (window (audio.right, 2 * spacing) > 10.0 * window (audio.left, 2 * spacing));
}

TEST_CASE ("Echo crossfades to a new time without a click", "[effects][echo]")
{
    anomp::fx::Echo echo;
    echo.prepare (rate);
    defaults (echo, EffectType::echo);
    echo.setParameter (anomp::fx::Echo::feedback, 0.0f, true);
    echo.setParameter (anomp::fx::Echo::tone, 1.0f, true);
    auto audio = sine (440.0, rate, static_cast<std::size_t> (rate * 2.0));
    const auto change = static_cast<std::size_t> (rate);
    for (std::size_t done = 0; done < audio.size(); done += 64)
    {
        if (done == change - change % 64)
            echo.setParameter (anomp::fx::Echo::time, 1234.5f, false);
        echo.process (audio.left.data() + done, audio.right.data() + done,
                      static_cast<int> (std::min<std::size_t> (64, audio.size() - done)));
    }
    // A 440 Hz sine at 0.5 moves at most 0.031 a sample; a jump to the other
    // read position could move it by up to 1.
    CHECK (maxStep (audio.left, static_cast<std::size_t> (rate * 0.5)) < 0.05f);
}

TEST_CASE ("Tremolo moves the level, or pans when stereo", "[effects][tremolo]")
{
    anomp::fx::Tremolo tremolo;
    tremolo.prepare (rate);
    defaults (tremolo, EffectType::tremolo);
    tremolo.setParameter (anomp::fx::Tremolo::depth, 1.0f, true);
    tremolo.setParameter (anomp::fx::Tremolo::rate, 5.0f, true);
    Stereo audio { std::vector<float> (44100, 1.0f), std::vector<float> (44100, 1.0f) };
    run (tremolo, audio);
    const auto [low, high] = std::minmax_element (audio.left.begin(), audio.left.end());
    CHECK (*low == Catch::Approx (0.0).margin (0.001));
    CHECK (*high == Catch::Approx (1.0).margin (0.001));
    CHECK (audio.left == audio.right);

    tremolo.reset();
    tremolo.setParameter (anomp::fx::Tremolo::stereo, 1.0f, true);
    audio = { std::vector<float> (44100, 1.0f), std::vector<float> (44100, 1.0f) };
    run (tremolo, audio);
    for (std::size_t i = 0; i < audio.size(); i += 97)
        REQUIRE (audio.left[i] + audio.right[i] == Catch::Approx (1.0).margin (0.001));
}

TEST_CASE ("Lo-fi rounds to its bits and holds at its rate", "[effects][lofi]")
{
    anomp::fx::LoFi lofi;
    lofi.prepare (rate);
    lofi.setParameter (anomp::fx::LoFi::bits, 2.0f, true);
    lofi.setParameter (anomp::fx::LoFi::holdRate, 11025.0f, true);
    auto audio = sine (100.0, rate, 4410, 0.9f);
    run (lofi, audio);

    std::set<float> levels (audio.left.begin(), audio.left.end());
    CHECK (levels == std::set<float> { -1.0f, -0.5f, 0.0f, 0.5f, 1.0f });
    // Each value held for four samples: 44.1 kHz down to 11.025 kHz.
    for (std::size_t i = 0; i + 4 <= audio.size(); i += 4)
        for (std::size_t j = 1; j < 4; ++j)
            REQUIRE (audio.left[i + j] == audio.left[i]);
}
