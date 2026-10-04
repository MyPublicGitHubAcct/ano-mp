#pragma once

#include "Dsp.h"
#include "Effect.h"

#include <array>

namespace anomp::fx
{
/** Repeats, each darker than the one before (a lowpass in the feedback, as
    on tape). Spread moves the repeats from both channels at once (0) to
    bouncing between them (1, ping-pong). A new time doesn't glide (that
    would bend the pitch of everything in the line): the repeats crossfade
    from the old time to the new over 50 ms. Parameters: time (ms),
    feedback (0 to 0.95), tone (0 dark to 1 bright) and spread. */
class Echo final : public Effect
{
public:
    enum Param : std::size_t
    {
        time,
        feedback,
        tone,
        spread
    };

    static constexpr double maxTimeSeconds = 2.0;

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;
    double tailSeconds() const noexcept override;

private:
    float readLine (std::size_t channel) const noexcept;

    double sampleRate = 44100.0;
    std::array<dsp::DelayLine, 2> lines;
    std::array<dsp::OnePole, 2> filters;
    dsp::Smoothed feedbackGlide, spreadGlide;
    float toneValue = 1.0f;

    // The delay in samples, and while it changes the old one, fading out.
    std::size_t delay = 1, oldDelay = 1, wantedDelay = 1;
    int fadeLength = 1, fadeLeft = 0;
};

/** Fewer bits and a lower sample rate: each sample rounded to `bits` bits
    of resolution and held until the next tick of a slower clock, aliasing
    as the cheap samplers of the 1980s did. Parameters: bits (2 to 16) and
    sample rate (Hz). */
class LoFi final : public Effect
{
public:
    enum Param : std::size_t
    {
        bits,
        holdRate
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;

private:
    double sampleRate = 44100.0;
    dsp::Smoothed bitsGlide;
    float increment = 1.0f, phase = 1.0f;
    std::array<float, 2> held {};
};
} // namespace anomp::fx
