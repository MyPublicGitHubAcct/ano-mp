#include "Crossfeed.h"

#include <cmath>

namespace anomp
{
namespace
{
constexpr double pi = 3.14159265358979323846;

struct Preset
{
    double cutoffHz, feedDb;
};

constexpr std::array<Preset, Crossfeed::maxLevel> presets { { { 700.0, 4.5 }, { 700.0, 6.0 }, { 650.0, 9.5 } } };
} // namespace

void Crossfeed::prepare (double sampleRate)
{
    rate = sampleRate > 0.0 ? sampleRate : 44100.0;
    configure();
}

void Crossfeed::setLevel (int newLevel)
{
    newLevel = juce::jlimit (0, maxLevel, newLevel);
    if (newLevel == level)
        return;
    level = newLevel;
    configure();
}

void Crossfeed::configure()
{
    reset();
    if (level == 0)
        return;

    const auto& preset = presets[static_cast<size_t> (level - 1)];
    // The crossed signal is `feedDb` below the direct one at low frequencies;
    // the direct signal's shelf is placed so the two sum about flat.
    const auto lowGainDb = -preset.feedDb * 5.0 / 6.0 - 3.0;
    const auto highGainDb = preset.feedDb / 6.0 - 3.0;
    const auto lowGain = std::pow (10.0, lowGainDb / 20.0);
    const auto highGain = 1.0 - std::pow (10.0, highGainDb / 20.0);
    const auto highCutoff = preset.cutoffHz * std::pow (2.0, (lowGainDb - 20.0 * std::log10 (highGain)) / 12.0);

    const auto lowX = std::exp (-2.0 * pi * preset.cutoffHz / rate);
    lowB1 = lowX;
    lowA0 = lowGain * (1.0 - lowX);

    const auto highX = std::exp (-2.0 * pi * highCutoff / rate);
    highB1 = highX;
    highA0 = 1.0 - highGain * (1.0 - highX);
    highA1 = -highX;

    gain = 1.0 / (1.0 - highGain + lowGain);
}

void Crossfeed::process (float* left, float* right, int numSamples) noexcept
{
    if (level == 0)
        return;

    for (int i = 0; i < numSamples; ++i)
    {
        const std::array<double, 2> in { left[i], right[i] };
        for (size_t ch = 0; ch < 2; ++ch)
        {
            low[ch] = lowA0 * in[ch] + lowB1 * low[ch];
            high[ch] = highA0 * in[ch] + highA1 * previous[ch] + highB1 * high[ch];
            previous[ch] = in[ch];
        }
        left[i] = static_cast<float> ((high[0] + low[1]) * gain);
        right[i] = static_cast<float> ((high[1] + low[0]) * gain);
    }
}

void Crossfeed::reset() noexcept
{
    low = {};
    high = {};
    previous = {};
}
} // namespace anomp
