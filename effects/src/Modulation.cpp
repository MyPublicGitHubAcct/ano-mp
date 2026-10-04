#include "Modulation.h"

#include <cmath>

namespace anomp::fx
{
namespace
{
/** The right channel's LFO, this many cycles behind the left's. */
constexpr float channelOffset = 0.25f;

void set (dsp::Smoothed& glide, float value, bool snap) noexcept
{
    if (snap)
        glide.snap (value);
    else
        glide.setTarget (value);
}

float samplesOf (double milliseconds, double sampleRate) noexcept
{
    return static_cast<float> (milliseconds * 0.001 * sampleRate);
}
} // namespace

//==============================================================================
void Chorus::prepare (double newRate)
{
    sampleRate = newRate;
    baseDelay = samplesOf (7.0, sampleRate);
    maxSweep = samplesOf (18.0, sampleRate);
    for (auto& line : lines)
        line.prepare (static_cast<int> (baseDelay + maxSweep) + 4);
    depthGlide.prepare (sampleRate, 0.05);
    reset();
}

void Chorus::reset() noexcept
{
    for (auto& line : lines)
        line.clear();
    lfo.reset();
}

void Chorus::setParameter (std::size_t index, float value, bool snap) noexcept
{
    if (index == rate)
        lfo.setRate (value, sampleRate);
    else if (index == depth)
        set (depthGlide, value, snap);
}

void Chorus::process (float* left, float* right, int numSamples) noexcept
{
    std::array<float*, 2> data { left, right };
    for (int i = 0; i < numSamples; ++i)
    {
        const auto phase = lfo.advance();
        const auto sweep = depthGlide.next() * maxSweep;
        for (std::size_t ch = 0; ch < 2; ++ch)
        {
            auto& line = lines[ch];
            line.push (data[ch][i]);
            const auto offset = ch == 0 ? 0.0f : channelOffset;
            const auto first = line.read (baseDelay + sweep * dsp::Lfo::unipolar (phase, offset));
            const auto second = line.read (baseDelay + sweep * dsp::Lfo::unipolar (phase, offset + 1.0f / 3.0f));
            data[ch][i] = 0.6f * (first + second);
        }
    }
}

//==============================================================================
void Flanger::prepare (double newRate)
{
    sampleRate = newRate;
    minDelay = std::max (1.0f, samplesOf (0.3, sampleRate));
    maxSweep = samplesOf (5.7, sampleRate);
    for (auto& line : lines)
        line.prepare (static_cast<int> (minDelay + maxSweep) + 4);
    depthGlide.prepare (sampleRate, 0.05);
    feedbackGlide.prepare (sampleRate, 0.05);
    reset();
}

void Flanger::reset() noexcept
{
    for (auto& line : lines)
        line.clear();
    lfo.reset();
}

void Flanger::setParameter (std::size_t index, float value, bool snap) noexcept
{
    if (index == rate)
        lfo.setRate (value, sampleRate);
    else if (index == depth)
        set (depthGlide, value, snap);
    else if (index == feedback)
        set (feedbackGlide, value, snap);
}

void Flanger::process (float* left, float* right, int numSamples) noexcept
{
    std::array<float*, 2> data { left, right };
    for (int i = 0; i < numSamples; ++i)
    {
        const auto phase = lfo.advance();
        const auto sweep = depthGlide.next() * maxSweep;
        const auto gain = feedbackGlide.next();
        for (std::size_t ch = 0; ch < 2; ++ch)
        {
            auto& line = lines[ch];
            const auto delayed =
                line.read (minDelay + sweep * dsp::Lfo::unipolar (phase, ch == 0 ? 0.0f : channelOffset));
            line.push (data[ch][i] + gain * delayed);
            data[ch][i] = delayed;
        }
    }
}

//==============================================================================
void Phaser::prepare (double newRate)
{
    sampleRate = newRate;
    depthGlide.prepare (sampleRate, 0.05);
    feedbackGlide.prepare (sampleRate, 0.05);
    reset();
}

void Phaser::reset() noexcept
{
    channels = {};
    lfo.reset();
}

void Phaser::setParameter (std::size_t index, float value, bool snap) noexcept
{
    if (index == rate)
        lfo.setRate (value, sampleRate);
    else if (index == depth)
        set (depthGlide, value, snap);
    else if (index == feedback)
        set (feedbackGlide, value, snap);
}

void Phaser::process (float* left, float* right, int numSamples) noexcept
{
    constexpr float lowest = 150.0f;
    const auto range = std::log (5000.0f / lowest);
    const auto nyquistSafe = static_cast<float> (sampleRate * 0.45);
    std::array<float*, 2> data { left, right };
    for (int i = 0; i < numSamples; ++i)
    {
        const auto phase = lfo.advance();
        const auto sweep = depthGlide.next() * range;
        const auto gain = feedbackGlide.next();
        for (std::size_t ch = 0; ch < 2; ++ch)
        {
            const auto position = dsp::Lfo::unipolar (phase, ch == 0 ? 0.0f : channelOffset);
            const auto corner = std::min (lowest * std::exp (sweep * position), nyquistSafe);
            const auto t = std::tan (dsp::pi * corner / static_cast<float> (sampleRate));
            const auto a = (t - 1.0f) / (t + 1.0f);

            auto& state = channels[ch];
            auto signal = data[ch][i] + gain * state.last;
            for (std::size_t stage = 0; stage < numStages; ++stage)
            {
                const auto out = a * signal + state.x1[stage] - a * state.y1[stage];
                state.x1[stage] = signal;
                state.y1[stage] = out;
                signal = out;
            }
            state.last = signal;
            data[ch][i] = signal;
        }
    }
}

//==============================================================================
void Tremolo::prepare (double newRate)
{
    sampleRate = newRate;
    depthGlide.prepare (sampleRate, 0.05);
    stereoGlide.prepare (sampleRate, 0.05);
    reset();
}

void Tremolo::reset() noexcept { lfo.reset(); }

void Tremolo::setParameter (std::size_t index, float value, bool snap) noexcept
{
    if (index == rate)
        lfo.setRate (value, sampleRate);
    else if (index == depth)
        set (depthGlide, value, snap);
    else if (index == stereo)
        set (stereoGlide, value, snap);
}

void Tremolo::process (float* left, float* right, int numSamples) noexcept
{
    for (int i = 0; i < numSamples; ++i)
    {
        const auto phase = lfo.advance();
        const auto amount = depthGlide.next();
        const auto opposition = 0.5f * stereoGlide.next();
        left[i] *= 1.0f - amount * dsp::Lfo::unipolar (phase);
        right[i] *= 1.0f - amount * dsp::Lfo::unipolar (phase, opposition);
    }
}
} // namespace anomp::fx
