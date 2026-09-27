#include "Equaliser.h"

#include <algorithm>
#include <cmath>
#include <complex>

namespace anomp
{
namespace
{
/** An octave-wide peak: Q = sqrt(2) / (2^1 - 1) ≈ 1.41. */
constexpr double bandQ = 1.41421356237;

double toLinear (double db) { return std::pow (10.0, db / 20.0); }
} // namespace

void Equaliser::prepare (double sampleRate)
{
    rate = sampleRate > 0.0 ? sampleRate : 44100.0;
    for (int band = 0; band < numBands; ++band)
        configure (band);
    reset();
}

void Equaliser::set (bool on, const Gains& gainsDb, double preampDb)
{
    enabled = on;
    for (int band = 0; band < numBands; ++band)
    {
        const auto gain = gainsDb[static_cast<size_t> (band)];
        target[static_cast<size_t> (band)] =
            on && std::isfinite (gain) ? juce::jlimit (-maxGainDb, maxGainDb, gain) : 0.0;
    }
    targetPreamp = on && std::isfinite (preampDb) ? juce::jlimit (-maxGainDb, maxGainDb, preampDb) : 0.0;
    if (on)
        active = true;
}

void Equaliser::configure (int band)
{
    auto& filter = filters[static_cast<size_t> (band)];
    const auto gainDb = current[static_cast<size_t> (band)];
    // A band above the Nyquist frequency (16 kHz at a 22.05 kHz rate) stays flat.
    const auto frequency = frequencies[static_cast<size_t> (band)];
    if (juce::exactlyEqual (gainDb, 0.0) || frequency >= rate * 0.49)
    {
        filter.b0 = 1.0;
        filter.b1 = filter.b2 = filter.a1 = filter.a2 = 0.0;
        return;
    }

    const auto a = std::pow (10.0, gainDb / 40.0);
    const auto w0 = juce::MathConstants<double>::twoPi * frequency / rate;
    const auto alpha = std::sin (w0) / (2.0 * bandQ);
    const auto cosW0 = std::cos (w0);
    const auto a0 = 1.0 + alpha / a;
    filter.b0 = (1.0 + alpha * a) / a0;
    filter.b1 = -2.0 * cosW0 / a0;
    filter.b2 = (1.0 - alpha * a) / a0;
    filter.a1 = -2.0 * cosW0 / a0;
    filter.a2 = (1.0 - alpha / a) / a0;
}

bool Equaliser::glide (int numSamples)
{
    const auto step = glideDbPerSecond * numSamples / rate;
    bool moved = false;
    for (int band = 0; band < numBands; ++band)
    {
        auto& now = current[static_cast<size_t> (band)];
        const auto wanted = target[static_cast<size_t> (band)];
        if (juce::exactlyEqual (now, wanted))
            continue;
        now = std::abs (wanted - now) <= step ? wanted : now + (wanted > now ? step : -step);
        configure (band);
        moved = true;
    }
    return moved;
}

void Equaliser::process (float* left, float* right, int numSamples) noexcept
{
    if (! active || numSamples <= 0)
        return;

    const auto preampBefore = currentPreamp;
    glide (numSamples);
    const auto step = glideDbPerSecond * numSamples / rate;
    currentPreamp = std::abs (targetPreamp - currentPreamp) <= step
                        ? targetPreamp
                        : currentPreamp + (targetPreamp > currentPreamp ? step : -step);

    for (auto& filter : filters)
    {
        if (juce::exactlyEqual (filter.b0, 1.0) && juce::exactlyEqual (filter.a1, 0.0)
            && juce::exactlyEqual (filter.b1, 0.0))
            continue;
        std::array<float*, 2> channels { left, right };
        for (size_t ch = 0; ch < 2; ++ch)
        {
            auto* data = channels[ch];
            auto z1 = filter.z1[ch], z2 = filter.z2[ch];
            for (int i = 0; i < numSamples; ++i)
            {
                // Transposed direct form II.
                const double in = data[i];
                const double out = filter.b0 * in + z1;
                z1 = filter.b1 * in - filter.a1 * out + z2;
                z2 = filter.b2 * in - filter.a2 * out;
                data[i] = static_cast<float> (out);
            }
            filter.z1[ch] = z1;
            filter.z2[ch] = z2;
        }
    }

    // The preamp, ramped over the block.
    const auto from = toLinear (preampBefore), to = toLinear (currentPreamp);
    if (! juce::exactlyEqual (from, 1.0) || ! juce::exactlyEqual (to, 1.0))
    {
        for (int i = 0; i < numSamples; ++i)
        {
            const auto gain = static_cast<float> (from + (to - from) * (i + 1) / numSamples);
            left[i] *= gain;
            right[i] *= gain;
        }
    }

    // Off and flat again: bypass until turned on.
    if (! enabled && juce::exactlyEqual (currentPreamp, 0.0)
        && std::all_of (current.begin(), current.end(), [] (double gain) { return juce::exactlyEqual (gain, 0.0); }))
    {
        active = false;
        reset();
    }
}

void Equaliser::reset() noexcept
{
    for (auto& filter : filters)
        filter.z1 = filter.z2 = {};
}

double Equaliser::responseDb (double frequency) const
{
    const auto w = juce::MathConstants<double>::twoPi * frequency / rate;
    const std::complex<double> z1 = std::polar (1.0, -w), z2 = std::polar (1.0, -2.0 * w);
    std::complex<double> response = 1.0;
    for (const auto& filter : filters)
        response *= (filter.b0 + filter.b1 * z1 + filter.b2 * z2) / (1.0 + filter.a1 * z1 + filter.a2 * z2);
    return 20.0 * std::log10 (std::abs (response));
}
} // namespace anomp
