#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>

#include "Signals.h"

#include <atomic>
#include <cmath>
#include <cstring>
#include <set>
#include <string>
#include <thread>

using anomp::fx::EffectChain;
using anomp::fx::EffectType;
using namespace signals;

namespace
{
constexpr double rate = 48000.0;

std::size_t samples (double seconds) { return static_cast<std::size_t> (seconds * rate); }
} // namespace

TEST_CASE ("The catalogue names each effect once, with defaults in range", "[effects][chain]")
{
    std::set<std::string> ids;
    std::set<EffectType> ordered (anomp::fx::chainOrder.begin(), anomp::fx::chainOrder.end());
    CHECK (ordered.size() == anomp::fx::effectCount);
    for (std::size_t t = 0; t < anomp::fx::effectCount; ++t)
    {
        const auto type = static_cast<EffectType> (t);
        const auto& info = anomp::fx::describe (type);
        CHECK (info.type == type);
        CHECK (ids.insert (info.id).second);
        EffectType found {};
        CHECK (anomp::fx::typeOf (info.id, found));
        CHECK (found == type);
        CHECK (info.defaultMix >= 0.0f);
        CHECK (info.defaultMix <= 1.0f);
        CHECK (info.numParams >= 1);
        CHECK (info.numParams <= anomp::fx::maxParams);
        std::set<std::string> params;
        for (std::size_t i = 0; i < info.numParams; ++i)
        {
            const auto& param = info.params[i];
            CHECK (params.insert (param.id).second);
            CHECK (param.min < param.max);
            CHECK (param.defaultValue >= param.min);
            CHECK (param.defaultValue <= param.max);
            CHECK ((! param.logarithmic || param.min > 0.0f));
        }
    }
    EffectType unused {};
    CHECK_FALSE (anomp::fx::typeOf ("wah", unused));
    CHECK_FALSE (anomp::fx::typeOf (nullptr, unused));
    CHECK (ids.size() >= 7);
}

TEST_CASE ("The chain leaves the signal untouched while every effect is off", "[effects][chain]")
{
    EffectChain chain;
    chain.prepare (rate);
    const auto input = noise (samples (1.0));
    auto audio = input;
    run (chain, audio);
    CHECK (audio.left == input.left);
    CHECK (audio.right == input.right);

    // On, then off again: once its tail has gone, untouched again.
    chain.setEnabled (EffectType::echo, true);
    run (chain, audio);
    CHECK (chain.isActive (EffectType::echo));
    chain.setEnabled (EffectType::echo, false);
    auto tail = noise (samples (4.0));
    run (chain, tail);
    CHECK_FALSE (chain.isActive (EffectType::echo));
    audio = input;
    run (chain, audio);
    CHECK (audio.left == input.left);
}

TEST_CASE ("The chain clamps its settings and refuses unknown parameters", "[effects][chain]")
{
    EffectChain chain;
    CHECK (chain.getMix (EffectType::reverb) == anomp::fx::describe (EffectType::reverb).defaultMix);
    chain.setMix (EffectType::reverb, 3.0f);
    CHECK (chain.getMix (EffectType::reverb) == 1.0f);
    chain.setMix (EffectType::reverb, NAN);
    CHECK (chain.getMix (EffectType::reverb) == 1.0f);

    CHECK (chain.setParameter (EffectType::echo, 0, 99999.0f));
    CHECK (chain.getParameter (EffectType::echo, 0) == 2000.0f);
    CHECK (chain.setParameter (EffectType::echo, 0, NAN));
    CHECK (chain.getParameter (EffectType::echo, 0) == 2000.0f);
    CHECK_FALSE (chain.setParameter (EffectType::echo, 4, 1.0f));
    CHECK_FALSE (chain.setParameter (EffectType::freeze, 1, 1.0f));
}

TEST_CASE ("The chain fades an effect in and out without a click", "[effects][chain]")
{
    EffectChain chain;
    chain.prepare (rate);
    chain.setParameter (EffectType::tremolo, 1, 1.0f);
    chain.setParameter (EffectType::lofi, 0, 3.0f);
    auto audio = sine (220.0, rate, samples (2.0));
    const auto input = audio;
    for (std::size_t done = 0; done < audio.size(); done += 512)
    {
        if (done == 24064)
        {
            chain.setEnabled (EffectType::tremolo, true);
            chain.setEnabled (EffectType::lofi, true);
        }
        if (done == 72192)
            chain.setEnabled (EffectType::lofi, false);
        chain.process (audio.left.data() + done, audio.right.data() + done,
                       static_cast<int> (std::min<std::size_t> (512, audio.size() - done)));
    }
    // Lo-fi's 3 bits step by 0.25 at once; anything bigger is a click.
    CHECK (maxStep (audio.left) < 0.3f);
    CHECK (audio.left != input.left);
}

TEST_CASE ("The chain's mix runs from dry to wet", "[effects][chain]")
{
    EffectChain chain;
    chain.prepare (rate);
    chain.setEnabled (EffectType::lofi, true);
    chain.setParameter (EffectType::lofi, 0, 2.0f);
    chain.setMix (EffectType::lofi, 0.0f);
    const auto input = sine (220.0, rate, samples (1.0));
    auto audio = input;
    run (chain, audio);
    // Dry once the fade-in is over.
    const auto settled = samples (EffectChain::glideSeconds) + 512;
    for (auto i = settled; i < audio.size(); ++i)
        REQUIRE (audio.left[i] == input.left[i]);

    chain.setMix (EffectType::lofi, 1.0f);
    audio = input;
    run (chain, audio);
    std::set<float> levels (audio.left.begin() + static_cast<std::ptrdiff_t> (settled), audio.left.end());
    CHECK (levels.size() <= 3);
}

TEST_CASE ("The chain lets a reverb ring out after it is switched off", "[effects][chain]")
{
    EffectChain chain;
    chain.prepare (rate);
    chain.setEnabled (EffectType::reverb, true);
    chain.setMix (EffectType::reverb, 0.5f);
    auto audio = noise (samples (1.0));
    run (chain, audio);

    chain.setEnabled (EffectType::reverb, false);
    auto tail = silence (samples (1.0));
    run (chain, tail);
    // No new input, but the room still sounds.
    CHECK (rms (tail.left, samples (0.3), samples (0.4)) > 1e-3);
    CHECK (chain.isActive (EffectType::reverb));
    auto rest = silence (samples (10.0));
    run (chain, rest);
    CHECK_FALSE (chain.isActive (EffectType::reverb));
}

TEST_CASE ("The chain holds the freeze only while it is on, until the track changes", "[effects][chain]")
{
    EffectChain chain;
    chain.prepare (rate);
    CHECK_FALSE (chain.setFreezeHeld (true));
    CHECK_FALSE (chain.isFreezeHeld());

    chain.setEnabled (EffectType::freeze, true);
    auto audio = sine (440.0, rate, samples (0.5));
    run (chain, audio);
    CHECK (chain.setFreezeHeld (true));
    CHECK (chain.isFreezeHeld());

    auto held = silence (samples (1.0));
    run (chain, held);
    CHECK (rms (held.left, samples (0.5), samples (1.0)) > 0.1);

    chain.trackChanged();
    CHECK_FALSE (chain.isFreezeHeld());
    auto released = silence (samples (1.0));
    run (chain, released);
    CHECK (rms (released.left, samples (0.5), samples (1.0)) == 0.0);

    CHECK (chain.setFreezeHeld (true));
    chain.setEnabled (EffectType::freeze, false);
    CHECK_FALSE (chain.isFreezeHeld());

    // A new device rate clears the history there was to hold.
    chain.setEnabled (EffectType::freeze, true);
    CHECK (chain.setFreezeHeld (true));
    chain.prepare (96000.0);
    CHECK_FALSE (chain.isFreezeHeld());
}

TEST_CASE ("The chain takes settings from another thread while it runs", "[effects][chain][threads]")
{
    EffectChain chain;
    chain.prepare (rate);
    std::atomic<bool> done { false };
    std::thread controls (
        [&]
        {
            for (int i = 0; ! done; ++i)
            {
                const auto type = static_cast<EffectType> (static_cast<std::size_t> (i) % anomp::fx::effectCount);
                chain.setEnabled (type, i % 3 != 0);
                chain.setMix (type, static_cast<float> (i % 10) / 10.0f);
                chain.setParameter (type, 0, static_cast<float> (i % 1000));
                chain.setFreezeHeld (i % 2 == 0);
                std::this_thread::yield();
            }
        });
    auto audio = noise (samples (3.0));
    run (chain, audio, 0, 256);
    done = true;
    controls.join();
    CHECK (finite (audio, 100.0f));
}
