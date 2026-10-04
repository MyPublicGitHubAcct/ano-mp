#pragma once

#include "Dsp.h"
#include "Effect.h"

#include <array>

namespace anomp::fx
{
/** A room: Jezar's Freeverb (public domain), eight damped feedback combs
    and four allpasses a channel, the right channel's a little longer so the
    two decorrelate, its tunings scaled to the sample rate; after a
    pre-delay. Parameters: size (the combs' feedback, so the decay), damping
    (how fast the highs die), width (0 mono, 1 fully stereo) and pre-delay
    (ms). */
class Reverb final : public Effect
{
public:
    enum Param : std::size_t
    {
        size,
        damping,
        width,
        preDelay
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;
    double tailSeconds() const noexcept override;

private:
    static constexpr std::size_t numCombs = 8, numAllpasses = 4;

    struct Comb
    {
        dsp::DelayLine line;
        std::size_t length = 1;
        float store = 0.0f;
    };

    struct Allpass
    {
        dsp::DelayLine line;
        std::size_t length = 1;
    };

    struct Channel
    {
        std::array<Comb, numCombs> combs;
        std::array<Allpass, numAllpasses> allpasses;
        float process (float input, float feedback, float damp) noexcept;
    };

    double rate = 44100.0;
    std::array<Channel, 2> channels;
    dsp::DelayLine preDelayLine;
    dsp::Smoothed feedbackGlide, dampGlide, widthGlide, preDelayGlide;
    double longestComb = 0.0; // Seconds.
};
} // namespace anomp::fx
