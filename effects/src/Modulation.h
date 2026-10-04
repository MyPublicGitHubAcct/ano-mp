#pragma once

#include "Dsp.h"
#include "Effect.h"

#include <array>

/** The effects that move a short delay or a filter with an LFO: chorus,
    flanger, phaser and tremolo. Each channel's LFO runs a quarter of a
    cycle (tremolo: up to half) apart, so they widen the image rather than
    pumping it. */
namespace anomp::fx
{
/** Two voices a channel, each a copy delayed by 7 to 25 ms as an LFO moves
    it, a third of a cycle apart: the copies drift in pitch against the
    original. Parameters: rate (Hz) and depth (how far the delay moves). */
class Chorus final : public Effect
{
public:
    enum Param : std::size_t
    {
        rate,
        depth
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;

private:
    double sampleRate = 44100.0;
    std::array<dsp::DelayLine, 2> lines;
    dsp::Lfo lfo;
    dsp::Smoothed depthGlide;
    float baseDelay = 0.0f, maxSweep = 0.0f; // Samples.
};

/** A copy delayed by 0.3 to 6 ms as an LFO sweeps it, fed back: mixed
    with the original, a comb of notches sweeps through the spectrum.
    Parameters: rate (Hz), depth and feedback (-0.95 to 0.95). */
class Flanger final : public Effect
{
public:
    enum Param : std::size_t
    {
        rate,
        depth,
        feedback
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;
    double tailSeconds() const noexcept override { return 0.1; }

private:
    double sampleRate = 44100.0;
    std::array<dsp::DelayLine, 2> lines;
    dsp::Lfo lfo;
    dsp::Smoothed depthGlide, feedbackGlide;
    float minDelay = 0.0f, maxSweep = 0.0f; // Samples.
};

/** Six first-order allpass stages a channel whose corner sweeps from
    150 Hz up to 5 kHz (as far as the depth takes it), fed back: mixed with
    the original, notches move through the spectrum. Parameters: rate (Hz),
    depth and feedback (0 to 0.9). */
class Phaser final : public Effect
{
public:
    enum Param : std::size_t
    {
        rate,
        depth,
        feedback
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;
    double tailSeconds() const noexcept override { return 0.05; }

private:
    static constexpr std::size_t numStages = 6;

    struct Channel
    {
        std::array<float, numStages> x1 {}, y1 {};
        float last = 0.0f;
    };

    double sampleRate = 44100.0;
    std::array<Channel, 2> channels;
    dsp::Lfo lfo;
    dsp::Smoothed depthGlide, feedbackGlide;
};

/** The level (tremolo) or the position (auto-pan) moving with an LFO.
    Parameters: rate (Hz), depth, and stereo: 0 moves both channels
    together, 1 moves them in opposition, panning from side to side. */
class Tremolo final : public Effect
{
public:
    enum Param : std::size_t
    {
        rate,
        depth,
        stereo
    };

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;

private:
    double sampleRate = 44100.0;
    dsp::Lfo lfo;
    dsp::Smoothed depthGlide, stereoGlide;
};
} // namespace anomp::fx
