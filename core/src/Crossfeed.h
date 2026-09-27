#pragma once

#include <juce_core/juce_core.h>

#include <array>

namespace anomp
{
/** Headphone crossfeed (PLAN.md O11), after Bauer's stereo-to-binaural
    method: each ear also hears the other channel, low-passed (so delayed
    and softer, as a head shadows it), while the direct signal gets a
    matching high boost so the mix stays about as bright overall. Written
    here from the method's description, with no dependency.

    Levels as in the C API: 0 off, then 700 Hz / 4.5 dB, 700 Hz / 6 dB and
    650 Hz / 9.5 dB of feed. Not thread-safe: the audio thread owns it, and
    others change it through the player. */
class Crossfeed
{
public:
    static constexpr int maxLevel = 3;

    void prepare (double sampleRate);
    void setLevel (int level);
    int getLevel() const noexcept { return level; }

    /** Processes `numSamples` stereo samples in place. */
    void process (float* left, float* right, int numSamples) noexcept;

    void reset() noexcept;

private:
    void configure();

    double rate = 44100.0;
    int level = 0;

    // One-pole lowpass for the crossed signal, one-pole high boost for the
    // direct one.
    double lowA0 = 0.0, lowB1 = 0.0;
    double highA0 = 1.0, highA1 = 0.0, highB1 = 0.0;
    double gain = 1.0;
    std::array<double, 2> low {}, high {}, previous {};
};
} // namespace anomp
