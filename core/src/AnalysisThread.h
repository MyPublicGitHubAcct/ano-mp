#pragma once

#include "SignalTap.h"
#include "SpectrumAnalyser.h"

#include <functional>

namespace anomp
{
/** Analyses a SignalTap on a thread of its own at a steady frame rate, and
    hands each frame to a callback on that thread.

    A frame is analysed only when the tap has new samples. Once none have
    arrived for staleSeconds (paused, stopped, no device), one silent frame
    is sent and then nothing until audio flows again, so an idle player
    costs no callbacks.
*/
class AnalysisThread final : private juce::Thread
{
public:
    using Callback = std::function<void (const AnalysisFrame&)>;

    static constexpr double staleSeconds = 0.15;
    static constexpr double minFramesPerSecond = 1.0, maxFramesPerSecond = 120.0;

    /** Starts at once. `framesPerSecond` is clamped to the range above. */
    AnalysisThread (const SignalTap& tap, int numBands, int waveformLength, double framesPerSecond, Callback callback);

    /** Stops the thread, waiting for a callback in progress to return. */
    ~AnalysisThread() override;

private:
    void run() override;
    void tick (double nowMs);

    const SignalTap& tap;
    const double intervalMs;
    const Callback callback;

    SpectrumAnalyser analyser;
    AnalysisFrame frame;
    std::vector<float> left, right;
    juce::uint64 lastWritten = 0;
    double lastNewAudioMs = 0.0;
    bool reportedSilence = false;

    JUCE_DECLARE_NON_COPYABLE (AnalysisThread)
};
} // namespace anomp
