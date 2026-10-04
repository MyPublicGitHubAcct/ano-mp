#include "Reverb.h"

#include <cmath>

namespace anomp::fx
{
namespace
{
// Freeverb's tunings, in samples at 44.1 kHz.
constexpr std::array<int, 8> combTunings { 1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617 };
constexpr std::array<int, 4> allpassTunings { 556, 441, 341, 225 };
constexpr int stereoSpread = 23;
constexpr float inputGain = 0.015f;
constexpr float allpassFeedback = 0.5f;
constexpr double maxPreDelaySeconds = 0.25;

float feedbackFor (float size) noexcept { return 0.7f + 0.28f * size; }
float dampFor (float damping) noexcept { return 0.4f * damping; }
} // namespace

void Reverb::prepare (double sampleRate)
{
    rate = sampleRate;
    const auto scale = sampleRate / 44100.0;
    for (std::size_t ch = 0; ch < channels.size(); ++ch)
    {
        const auto spread = ch == 0 ? 0 : stereoSpread;
        for (std::size_t i = 0; i < numCombs; ++i)
        {
            auto& comb = channels[ch].combs[i];
            comb.length = static_cast<std::size_t> (std::lround ((combTunings[i] + spread) * scale));
            comb.line.prepare (static_cast<int> (comb.length) + 1);
        }
        for (std::size_t i = 0; i < numAllpasses; ++i)
        {
            auto& allpass = channels[ch].allpasses[i];
            allpass.length = static_cast<std::size_t> (std::lround ((allpassTunings[i] + spread) * scale));
            allpass.line.prepare (static_cast<int> (allpass.length) + 1);
        }
    }
    longestComb = (combTunings.back() + stereoSpread) / 44100.0;
    preDelayLine.prepare (dsp::samplesIn (maxPreDelaySeconds, sampleRate) + 4);
    for (auto* glide : { &feedbackGlide, &dampGlide, &widthGlide, &preDelayGlide })
        glide->prepare (sampleRate, 0.05);
    reset();
}

void Reverb::reset() noexcept
{
    for (auto& channel : channels)
    {
        for (auto& comb : channel.combs)
        {
            comb.line.clear();
            comb.store = 0.0f;
        }
        for (auto& allpass : channel.allpasses)
            allpass.line.clear();
    }
    preDelayLine.clear();
}

void Reverb::setParameter (std::size_t index, float value, bool snap) noexcept
{
    auto set = [snap] (dsp::Smoothed& glide, float target)
    {
        if (snap)
            glide.snap (target);
        else
            glide.setTarget (target);
    };
    switch (index)
    {
        case size:     set (feedbackGlide, feedbackFor (value)); break;
        case damping:  set (dampGlide, dampFor (value)); break;
        case width:    set (widthGlide, value); break;
        case preDelay: set (preDelayGlide, static_cast<float> (value * 0.001 * rate)); break;
        default:       break;
    }
}

float Reverb::Channel::process (float input, float feedback, float damp) noexcept
{
    float sum = 0.0f;
    for (auto& comb : combs)
    {
        // A comb's delay is its length: read the oldest sample, then replace it.
        const auto output = comb.line.at (comb.length - 1);
        comb.store = output * (1.0f - damp) + comb.store * damp;
        comb.line.push (input + comb.store * feedback);
        sum += output;
    }
    for (auto& allpass : allpasses)
    {
        const auto delayed = allpass.line.at (allpass.length - 1);
        allpass.line.push (sum + delayed * allpassFeedback);
        sum = delayed - sum;
    }
    return sum;
}

void Reverb::process (float* left, float* right, int numSamples) noexcept
{
    const auto feedback = feedbackGlide.skip (numSamples);
    const auto damp = dampGlide.skip (numSamples);
    for (int i = 0; i < numSamples; ++i)
    {
        preDelayLine.push ((left[i] + right[i]) * inputGain);
        const auto delay = preDelayGlide.next();
        const auto input = delay < 1.0f ? preDelayLine.at (0) : preDelayLine.read (delay);

        const auto outLeft = channels[0].process (input, feedback, damp);
        const auto outRight = channels[1].process (input, feedback, damp);

        const auto w = widthGlide.next();
        const auto direct = 0.5f + 0.5f * w, crossed = 0.5f - 0.5f * w;
        left[i] = outLeft * direct + outRight * crossed;
        right[i] = outRight * direct + outLeft * crossed;
    }
}

double Reverb::tailSeconds() const noexcept
{
    // Until the longest comb has died away by 60 dB: 10^-3 = feedback^n.
    const auto feedback = static_cast<double> (feedbackGlide.getTarget());
    const auto passes = feedback > 0.0 ? 3.0 / -std::log10 (feedback) : 0.0;
    return passes * longestComb + preDelayGlide.getTarget() / rate;
}
} // namespace anomp::fx
