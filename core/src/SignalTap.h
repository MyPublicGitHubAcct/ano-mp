#pragma once

#include <juce_core/juce_core.h>

#include <atomic>
#include <memory>

namespace anomp
{
/** The latest stereo samples the player rendered, for the visualizer.

    Lock-free, one writer (the audio thread) and any number of readers. A
    reader copies the most recent samples without taking them: analysis
    reads overlapping windows. Samples are relaxed atomics, so a read racing
    a write is defined behaviour; it can at worst mix in samples written
    during the copy, which would need the writer to lap the ring (tens of
    milliseconds of audio) while the reader copies a few thousand floats.
*/
class SignalTap final
{
public:
    /** Samples kept per channel; readers may ask for at most half. */
    static constexpr int capacity = 1 << 15;

    SignalTap()
        : left (std::make_unique<std::atomic<float>[]> (capacity)),
          right (std::make_unique<std::atomic<float>[]> (capacity))
    {
    }

    void setSampleRate (double rate) noexcept { sampleRate.store (rate); }
    double getSampleRate() const noexcept { return sampleRate.load(); }

    /** Audio thread only. */
    void push (const float* leftSamples, const float* rightSamples, int numSamples) noexcept
    {
        auto index = written.load (std::memory_order_relaxed);
        for (int i = 0; i < numSamples; ++i, ++index)
        {
            const auto slot = static_cast<size_t> (index & mask);
            left[slot].store (leftSamples[i], std::memory_order_relaxed);
            right[slot].store (rightSamples[i], std::memory_order_relaxed);
        }
        written.store (index, std::memory_order_release);
    }

    /** Samples pushed since the tap was created. */
    juce::uint64 getWrittenCount() const noexcept { return written.load (std::memory_order_acquire); }

    /** Copies the latest `numSamples` (at most capacity / 2), oldest first,
        zero-filling the start if fewer were ever pushed. Returns the written
        count the copy ends at. */
    juce::uint64 readLatest (float* leftOut, float* rightOut, int numSamples) const noexcept
    {
        jassert (numSamples <= capacity / 2);
        const auto end = written.load (std::memory_order_acquire);
        const auto count = static_cast<juce::uint64> (numSamples);
        const auto missing = end < count ? static_cast<int> (count - end) : 0;

        for (int i = 0; i < missing; ++i)
            leftOut[i] = rightOut[i] = 0.0f;

        for (auto i = missing; i < numSamples; ++i)
        {
            const auto slot = static_cast<size_t> ((end - count + static_cast<juce::uint64> (i)) & mask);
            leftOut[i] = left[slot].load (std::memory_order_relaxed);
            rightOut[i] = right[slot].load (std::memory_order_relaxed);
        }
        return end;
    }

private:
    static constexpr juce::uint64 mask = capacity - 1;

    std::unique_ptr<std::atomic<float>[]> left, right;
    std::atomic<juce::uint64> written { 0 };
    std::atomic<double> sampleRate { 0.0 };

    JUCE_DECLARE_NON_COPYABLE (SignalTap)
};
} // namespace anomp
