#pragma once

#include "anomp/effects/EffectChain.h"

namespace anomp::fx
{
/** One effect, as the chain runs it: it turns a chunk of stereo input
    (at most EffectChain::chunkSize samples) into its wet signal in place;
    the chain mixes that with the dry signal. Owned by the audio thread. */
class Effect
{
public:
    virtual ~Effect() = default;

    /** Allocates for `sampleRate`; not real-time safe. Leaves it cleared. */
    virtual void prepare (double sampleRate) = 0;

    /** Clears delay lines, filters and tails. */
    virtual void reset() noexcept = 0;

    /** Parameter `index`, already within its range (see describe()). With
        `snap`, it jumps there rather than gliding: as the effect starts. */
    virtual void setParameter (std::size_t index, float value, bool snap) noexcept = 0;

    /** Turns the chunk into the wet signal, in place. */
    virtual void process (float* left, float* right, int numSamples) noexcept = 0;

    /** Seconds it keeps sounding after its input stops, with the
        parameters as they are now. */
    virtual double tailSeconds() const noexcept { return 0.0; }

    /** For the spectral freeze: hold the sound, or let it go. */
    virtual void setHeld (bool) noexcept {}
};
} // namespace anomp::fx
