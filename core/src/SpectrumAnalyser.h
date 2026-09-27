#pragma once

#include <juce_core/juce_core.h>

#include <array>
#include <memory>
#include <utility>
#include <vector>

namespace juce::dsp
{
class FFT;
}

namespace anomp
{
/** One analysis of the latest audio, for the visualizer. */
struct AnalysisFrame
{
    /** No audio has flowed for a moment (paused, stopped, no device):
        everything below is zero. */
    bool silent = true;

    /** Log-spaced from lowestHz to highestHz, low first: each band's
        loudest bin, 0 at -70 dB and 1 at full scale, tilted +3 dB per
        octave around 1 kHz so typical music looks level. */
    std::vector<float> bands;
    float lowestHz = 0.0f, highestHz = 0.0f;

    /** Energy per pitch class (C, C#, … B), 0..1, the strongest as 1. */
    std::array<float, 12> chroma {};

    /** Linear, per channel (left, right), over about the last 40 ms. */
    std::array<float, 2> peak {}, rms {};

    /** The latest samples, starting at a rising zero crossing where one
        is near, so a periodic wave draws still. */
    std::vector<float> left, right;

    /** How much louder the spectrum got since the previous frame, about
        0..1, and whether that was a beat (a jump in the bass well above the
        last second and a half). */
    float onset = 0.0f;
    bool beat = false;
};

/** Turns windows of stereo samples into AnalysisFrames. Not thread-safe;
    one thread calls it. */
class SpectrumAnalyser final
{
public:
    /** Samples per channel that process() takes: enough for the pitch
        resolution of the chroma at low notes. */
    static constexpr int windowSize = 8192;

    static constexpr int minBands = 4, maxBands = 256;
    static constexpr int minWaveformLength = 16, maxWaveformLength = 2048;

    /** `numBands` and `waveformLength` are clamped to the ranges above. */
    SpectrumAnalyser (int numBands, int waveformLength);
    ~SpectrumAnalyser();

    /** Call before process() and whenever the rate changes; resets the
        beat history. */
    void prepare (double sampleRate);
    double getSampleRate() const noexcept { return sampleRate; }

    int getNumBands() const noexcept { return numBands; }
    int getWaveformLength() const noexcept { return waveformLength; }

    /** Analyses the latest `windowSize` samples of each channel, oldest
        first; `newSamples` of them arrived since the previous call, which
        drives the beat detector's clock. */
    void process (const float* left, const float* right, int newSamples, AnalysisFrame& frame);

    /** A silent frame; the next beat needs the bass to rise again. */
    void processSilence (AnalysisFrame& frame);

private:
    struct Band
    {
        float lowBin, highBin, centreBin;
        float tiltDb;
    };

    void computeBands (const float* mono, AnalysisFrame& frame);
    void computeChroma (const float* mono, AnalysisFrame& frame);
    void computeLevels (const float* left, const float* right, AnalysisFrame& frame) const;
    void computeWaveform (const float* left, const float* right, const float* mono, AnalysisFrame& frame) const;
    void detectBeat (int newSamples, AnalysisFrame& frame);
    void resize (AnalysisFrame& frame) const;
    void resetHistory();

    const int numBands, waveformLength;
    double sampleRate = 0.0;

    std::unique_ptr<juce::dsp::FFT> bandFft, chromaFft;
    std::vector<float> bandWindow, chromaWindow, fftData, mono;
    std::vector<Band> bandMap;
    std::vector<float> previousBands;
    int lowBandCount = 0; // Bands below 250 Hz, for the beat detector.

    /** The latest values pushed, oldest overwritten first. */
    template <typename T>
    struct Ring
    {
        explicit Ring (size_t capacity) : items (capacity) {}
        void push (T item)
        {
            items[next] = item;
            next = (next + 1) % items.size();
            filled = juce::jmin (filled + 1, items.size());
        }
        void clear() noexcept { next = filled = 0; }
        auto begin() const noexcept { return items.begin(); }
        auto end() const noexcept { return items.begin() + static_cast<std::ptrdiff_t> (filled); }

    private:
        std::vector<T> items;
        size_t next = 0, filled = 0;
    };

    // Beat detection: the bass flux over the last 1.5 s, by the time the
    // analysis clock (samples processed) had reached.
    Ring<std::pair<double, float>> fluxHistory;
    double clockSeconds = 0.0, lastBeatSeconds = -1.0;
    bool hasPrevious = false;

    JUCE_DECLARE_NON_COPYABLE (SpectrumAnalyser)
};
} // namespace anomp
