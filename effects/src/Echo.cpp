#include "Echo.h"

#include <cmath>

namespace anomp::fx
{
void Echo::prepare (double rate)
{
    sampleRate = rate;
    for (auto& line : lines)
        line.prepare (dsp::samplesIn (maxTimeSeconds, rate) + 4);
    feedbackGlide.prepare (rate, 0.05);
    spreadGlide.prepare (rate, 0.05);
    fadeLength = dsp::samplesIn (0.05, rate);
    for (auto& filter : filters)
        filter.setCutoff (1000.0f * std::pow (20.0f, toneValue), sampleRate);
    reset();
}

void Echo::reset() noexcept
{
    for (auto& line : lines)
        line.clear();
    for (auto& filter : filters)
        filter.reset();
    delay = oldDelay = wantedDelay;
    fadeLeft = 0;
}

void Echo::setParameter (std::size_t index, float value, bool snap) noexcept
{
    switch (index)
    {
        case time:
            wantedDelay = static_cast<std::size_t> (dsp::samplesIn (value * 0.001, sampleRate));
            if (snap)
            {
                delay = oldDelay = wantedDelay;
                fadeLeft = 0;
            }
            break;
        case feedback:
            if (snap)
                feedbackGlide.snap (value);
            else
                feedbackGlide.setTarget (value);
            break;
        case tone:
            // 1 kHz to 20 kHz.
            toneValue = value;
            for (auto& filter : filters)
                filter.setCutoff (1000.0f * std::pow (20.0f, value), sampleRate);
            break;
        case spread:
            if (snap)
                spreadGlide.snap (value);
            else
                spreadGlide.setTarget (value);
            break;
        default: break;
    }
}

float Echo::readLine (std::size_t channel) const noexcept
{
    // Read before this sample is pushed: a delay of n is lines[].at (n - 1).
    const auto now = lines[channel].at (delay - 1);
    if (fadeLeft <= 0)
        return now;
    const auto before = lines[channel].at (oldDelay - 1);
    const auto t = static_cast<float> (fadeLeft) / static_cast<float> (fadeLength);
    return now + (before - now) * t;
}

void Echo::process (float* left, float* right, int numSamples) noexcept
{
    for (int i = 0; i < numSamples; ++i)
    {
        // The next change of time waits for the last one's crossfade.
        if (fadeLeft <= 0 && wantedDelay != delay)
        {
            oldDelay = delay;
            delay = wantedDelay;
            fadeLeft = fadeLength;
        }

        const auto repeatLeft = filters[0].process (readLine (0));
        const auto repeatRight = filters[1].process (readLine (1));
        if (fadeLeft > 0)
            --fadeLeft;

        const auto gain = feedbackGlide.next();
        const auto bounce = spreadGlide.next();
        const auto inLeft = left[i], inRight = right[i];

        // Straight: each channel into its own line. Ping-pong: both into the
        // left line, which feeds the right, which feeds the left.
        const auto straightLeft = inLeft + gain * repeatLeft;
        const auto straightRight = inRight + gain * repeatRight;
        const auto bouncedLeft = 0.5f * (inLeft + inRight) + gain * repeatRight;
        const auto bouncedRight = gain * repeatLeft;
        lines[0].push (straightLeft + (bouncedLeft - straightLeft) * bounce);
        lines[1].push (straightRight + (bouncedRight - straightRight) * bounce);

        left[i] = repeatLeft;
        right[i] = repeatRight;
    }
}

double Echo::tailSeconds() const noexcept
{
    // Repeats until they are 60 dB down, at most 20 s.
    const auto gain = static_cast<double> (feedbackGlide.getTarget());
    const auto repeats = gain > 0.001 ? 1.0 + 3.0 / -std::log10 (gain) : 1.0;
    return std::min (20.0, repeats * static_cast<double> (std::max (delay, wantedDelay)) / sampleRate);
}

//==============================================================================
void LoFi::prepare (double rate)
{
    sampleRate = rate;
    bitsGlide.prepare (rate, 0.05);
    reset();
}

void LoFi::reset() noexcept
{
    phase = 1.0f;
    held = {};
}

void LoFi::setParameter (std::size_t index, float value, bool snap) noexcept
{
    if (index == bits)
    {
        if (snap)
            bitsGlide.snap (value);
        else
            bitsGlide.setTarget (value);
    }
    else if (index == holdRate)
    {
        increment = static_cast<float> (std::min (1.0, static_cast<double> (value) / sampleRate));
    }
}

void LoFi::process (float* left, float* right, int numSamples) noexcept
{
    for (int i = 0; i < numSamples; ++i)
    {
        // Steps of 2 / 2^bits across -1..1.
        const auto step = std::exp2 (1.0f - bitsGlide.next());
        if (phase >= 1.0f)
        {
            phase -= std::floor (phase);
            held[0] = std::round (left[i] / step) * step;
            held[1] = std::round (right[i] / step) * step;
        }
        phase += increment;
        left[i] = held[0];
        right[i] = held[1];
    }
}
} // namespace anomp::fx
