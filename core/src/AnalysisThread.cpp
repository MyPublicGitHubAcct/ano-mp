#include "AnalysisThread.h"

namespace anomp
{
AnalysisThread::AnalysisThread (const SignalTap& tapToRead,
                                int numBands,
                                int waveformLength,
                                double framesPerSecond,
                                Callback callbackToUse)
    : juce::Thread ("anomp analysis"),
      tap (tapToRead),
      intervalMs (1000.0 / juce::jlimit (minFramesPerSecond, maxFramesPerSecond, framesPerSecond)),
      callback (std::move (callbackToUse)),
      analyser (numBands, waveformLength),
      left (SpectrumAnalyser::windowSize),
      right (SpectrumAnalyser::windowSize),
      lastWritten (tapToRead.getWrittenCount())
{
    startThread();
}

AnalysisThread::~AnalysisThread() { stopThread (2000); }

void AnalysisThread::run()
{
    auto next = juce::Time::getMillisecondCounterHiRes();
    lastNewAudioMs = next;

    while (! threadShouldExit())
    {
        next += intervalMs;
        const auto now = juce::Time::getMillisecondCounterHiRes();

        if (next > now)
            wait (next - now);
        else
            next = now; // Fell behind (e.g. a slow callback): don't catch up in a burst.

        if (threadShouldExit())
            break;

        tick (juce::Time::getMillisecondCounterHiRes());
    }
}

void AnalysisThread::tick (double nowMs)
{
    const auto rate = tap.getSampleRate();
    const auto written = tap.readLatest (left.data(), right.data(), SpectrumAnalyser::windowSize);

    if (written != lastWritten && rate > 0.0)
    {
        if (! juce::exactlyEqual (rate, analyser.getSampleRate()))
            analyser.prepare (rate);

        const auto fresh = juce::jmin (written - lastWritten, static_cast<juce::uint64> (SpectrumAnalyser::windowSize));
        lastWritten = written;
        lastNewAudioMs = nowMs;
        reportedSilence = false;

        analyser.process (left.data(), right.data(), static_cast<int> (fresh), frame);
        callback (frame);
    }
    else if (! reportedSilence && nowMs - lastNewAudioMs >= staleSeconds * 1000.0)
    {
        reportedSilence = true;
        analyser.processSilence (frame);
        callback (frame);
    }
}
} // namespace anomp
