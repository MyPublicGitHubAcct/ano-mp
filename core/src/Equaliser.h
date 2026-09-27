#pragma once

#include <juce_core/juce_core.h>

#include <array>

namespace anomp
{
/** A 10-band graphic equaliser with a preamp (PLAN.md F15): peaking filters
    (RBJ's cookbook biquads, an octave wide) at the ISO octave centres from
    31 Hz to 16 kHz, written here with no GUI dependency.

    Changes glide: each band's gain and the preamp move towards their
    targets by at most `glideDbPerSecond`, recomputing the filters once per
    block, so a new setting never clicks. Turned off, it glides to flat and
    then bypasses itself.

    Not thread-safe: the audio thread processes it, and the player changes
    its targets under the lock the audio thread holds. */
class Equaliser
{
public:
    static constexpr int numBands = 10;
    static constexpr double maxGainDb = 12.0;
    static constexpr std::array<double, numBands> frequencies { 31.25,  62.5,   125.0,  250.0,  500.0,
                                                                1000.0, 2000.0, 4000.0, 8000.0, 16000.0 };
    static constexpr double glideDbPerSecond = 120.0;

    using Gains = std::array<double, numBands>;

    void prepare (double sampleRate);

    /** New targets; `enabled` false glides to flat, then bypasses. Values
        are clamped to ±maxGainDb. */
    void set (bool enabled, const Gains& gainsDb, double preampDb);

    bool isEnabled() const noexcept { return enabled; }

    /** Whether process() changes anything: on, or still gliding to flat. */
    bool isActive() const noexcept { return active; }

    /** Processes `numSamples` stereo samples in place. */
    void process (float* left, float* right, int numSamples) noexcept;

    void reset() noexcept;

    /** The response in dB at `frequency` Hz of the filters as they are now,
        without the preamp, for tests. */
    double responseDb (double frequency) const;

private:
    struct Biquad
    {
        double b0 = 1.0, b1 = 0.0, b2 = 0.0, a1 = 0.0, a2 = 0.0;
        std::array<double, 2> z1 {}, z2 {};
    };

    void configure (int band);
    bool glide (int numSamples);

    double rate = 44100.0;
    bool enabled = false, active = false;
    Gains target {}, current {};
    double targetPreamp = 0.0, currentPreamp = 0.0;
    std::array<Biquad, numBands> filters;
};
} // namespace anomp
