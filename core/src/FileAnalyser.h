#pragma once

#include <juce_audio_formats/juce_audio_formats.h>

#include <functional>
#include <limits>
#include <vector>

namespace anomp
{
/** What one pass over a file measures (PLAN.md O1–O4): its loudness and
    peaks for ReplayGain, silence at its ends and inside it, where its
    spectrum stops, and a coarse waveform for the seek bar. */
struct FileAnalysis
{
    /** Samples decoded, at the file's rate, and seconds. */
    juce::int64 decodedSamples = 0;
    double sampleRate = 0.0;
    int channels = 0;

    /** EBU R128 / ITU-R BS.1770-4 integrated loudness in LUFS; NaN when every
        block is below the absolute gate (digital silence). A mono file
        counts as dual mono, since the player plays it on both channels. */
    double integratedLufs = std::numeric_limits<double>::quiet_NaN();
    /** Linear, 1 is full scale: the largest sample, and the largest value
        between samples too (4× oversampled, BS.1770 Annex 2). */
    double samplePeak = 0.0, truePeak = 0.0;

    /** The loudness of every 400 ms block above the absolute gate, counted
        in `histogramStep` LU bins from `histogramFloor` LUFS up, so an
        album's loudness can be gated over all of its tracks later. */
    static constexpr double histogramFloor = -70.0;
    static constexpr double histogramStep = 0.5;
    static constexpr int histogramBins = 150; // Up to +5 LUFS.
    std::vector<juce::uint32> histogram = std::vector<juce::uint32> (histogramBins);

    /** Seconds below -60 dBFS at the start and the end. */
    double leadingSilence = 0.0, trailingSilence = 0.0;
    /** The longest silence inside the track, touching neither end and at
        least `minGapSeconds` long (the gap before a hidden track), or 0. */
    static constexpr double minGapSeconds = 2.0;
    double gapStart = 0.0, gapLength = 0.0;
    /** RMS of the first and last 50 ms in dBFS (the louder channel), -inf
        for digital silence. A track that segues into the next has sound at
        its end, and the next at its start. */
    double startLevelDb = -std::numeric_limits<double>::infinity();
    double endLevelDb = -std::numeric_limits<double>::infinity();

    /** Where the average spectrum falls off a cliff, in Hz, as a lossy
        encoder's lowpass leaves it; 0 if it doesn't. */
    double cutoffHz = 0.0;

    /** The smallest and largest sample (of either channel) in each of up
        to `envelopePoints` equal slices of the track. */
    static constexpr int envelopePoints = 1000;
    std::vector<float> envelopeMin, envelopeMax;
};

/** Measures everything in FileAnalysis from `reader` between `start` and
    `end` seconds (an end not after the start is the end of the file),
    decoding that part once. `progress` (may be empty) is called after the first chunk
    and then about four times a second with the fraction done; returning
    false cancels. Returns an error message, or an empty string on success. */
juce::String analyseReader (juce::AudioFormatReader& reader,
                            double start,
                            double end,
                            const std::function<bool (double fraction)>& progress,
                            FileAnalysis& result);

/** analyseReader over `file`, opened with its own reader from `formats`.
    Safe to call from any thread, concurrently. */
juce::String analyseFile (const juce::File& file,
                          juce::AudioFormatManager& formats,
                          double start,
                          double end,
                          const std::function<bool (double fraction)>& progress,
                          FileAnalysis& result);

/** BS.1770's K-weighting (a high shelf, then a high pass) for one channel. */
class KWeighting
{
public:
    explicit KWeighting (double sampleRate);
    float process (float sample) noexcept;
    void reset() noexcept;

private:
    struct Biquad
    {
        double b0, b1, b2, a1, a2;
        double z1 = 0.0, z2 = 0.0;
        double process (double x) noexcept;
    };
    Biquad shelf, highPass;
};
} // namespace anomp
