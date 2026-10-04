#pragma once

#include "anomp/effects/EffectChain.h"
#include "Effect.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <numbers>
#include <vector>

// Test signals, and running them through an effect or the chain in chunks
// as the chain would.

namespace signals
{
struct Stereo
{
    std::vector<float> left, right;
    std::size_t size() const { return left.size(); }
};

inline Stereo sine (double hertz, double sampleRate, std::size_t length, float amplitude = 0.5f)
{
    Stereo out { std::vector<float> (length), std::vector<float> (length) };
    for (std::size_t i = 0; i < length; ++i)
        out.left[i] = out.right[i] =
            amplitude
            * static_cast<float> (std::sin (2.0 * std::numbers::pi * hertz * static_cast<double> (i) / sampleRate));
    return out;
}

inline Stereo noise (std::size_t length, float amplitude = 0.5f, std::uint32_t seed = 1)
{
    Stereo out { std::vector<float> (length), std::vector<float> (length) };
    auto next = [&seed]
    {
        seed = seed * 1664525u + 1013904223u;
        return static_cast<float> (seed >> 8) / 8388608.0f - 1.0f;
    };
    for (std::size_t i = 0; i < length; ++i)
    {
        out.left[i] = amplitude * next();
        out.right[i] = amplitude * next();
    }
    return out;
}

inline Stereo impulse (std::size_t length)
{
    Stereo out { std::vector<float> (length), std::vector<float> (length) };
    out.left[0] = out.right[0] = 1.0f;
    return out;
}

inline Stereo silence (std::size_t length) { return { std::vector<float> (length), std::vector<float> (length) }; }

inline double rms (const std::vector<float>& samples, std::size_t from, std::size_t to)
{
    double sum = 0.0;
    for (auto i = from; i < to; ++i)
        sum += static_cast<double> (samples[i]) * samples[i];
    return std::sqrt (sum / static_cast<double> (to - from));
}

inline double db (double ratio) { return 20.0 * std::log10 (ratio); }

/** The largest change from one sample to the next: a click shows as a jump. */
inline float maxStep (const std::vector<float>& samples, std::size_t from = 1)
{
    float result = 0.0f;
    for (auto i = std::max<std::size_t> (from, 1); i < samples.size(); ++i)
        result = std::max (result, std::abs (samples[i] - samples[i - 1]));
    return result;
}

inline bool finite (const Stereo& audio, float limit)
{
    for (const auto* channel : { &audio.left, &audio.right })
        for (auto sample : *channel)
            if (! std::isfinite (sample) || std::abs (sample) > limit)
                return false;
    return true;
}

/** Runs `audio` through `effect` in place, in the chain's chunks. */
inline void run (anomp::fx::Effect& effect, Stereo& audio, std::size_t from = 0)
{
    constexpr auto chunk = static_cast<std::size_t> (anomp::fx::EffectChain::chunkSize);
    for (auto done = from; done < audio.size(); done += chunk)
        effect.process (audio.left.data() + done, audio.right.data() + done,
                        static_cast<int> (std::min (chunk, audio.size() - done)));
}

/** Runs `audio` through `chain` in place, in blocks of `block` samples. */
inline void run (anomp::fx::EffectChain& chain, Stereo& audio, std::size_t from = 0, std::size_t block = 512)
{
    for (auto done = from; done < audio.size(); done += block)
        chain.process (audio.left.data() + done, audio.right.data() + done,
                       static_cast<int> (std::min (block, audio.size() - done)));
}

/** Sets every parameter of `effect` to its default, snapping, as the chain
    does when it starts one. */
inline void defaults (anomp::fx::Effect& effect, anomp::fx::EffectType type)
{
    const auto& info = anomp::fx::describe (type);
    for (std::size_t i = 0; i < info.numParams; ++i)
        effect.setParameter (i, info.params[i].defaultValue, true);
}
} // namespace signals
