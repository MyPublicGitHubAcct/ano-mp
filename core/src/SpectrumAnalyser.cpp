#include "SpectrumAnalyser.h"

#include <juce_dsp/juce_dsp.h>

#include <algorithm>
#include <cmath>

namespace anomp
{
namespace
{
// Bands: a short window, so the bars follow the music closely (about 43 ms
// at 48 kHz).
constexpr int bandFftOrder = 11;
constexpr int bandFftSize = 1 << bandFftOrder;
// Chroma: the whole window, for semitone resolution down to about 100 Hz.
constexpr int chromaFftOrder = 13;
static_assert ((1 << chromaFftOrder) == SpectrumAnalyser::windowSize);

constexpr float lowestBandHz = 30.0f;
constexpr float highestBandHz = 16000.0f;
constexpr float floorDb = -70.0f;
constexpr float tiltDbPerOctave = 3.0f;

constexpr float chromaLowestHz = 80.0f;
constexpr float chromaHighestHz = 5000.0f;
constexpr float chromaSilence = 1.0e-4f; // Summed normalised magnitude.

// Levels and the waveform trigger search, in samples.
constexpr int levelSamples = 2048;
constexpr int triggerSearch = 2048;

// Beats: a bass jump well above the recent ones, at most 4 a second.
constexpr float beatBassHz = 250.0f;
constexpr double beatHistorySeconds = 1.5;
constexpr size_t beatHistoryCapacity = 256;
constexpr float beatDeviations = 1.5f;
constexpr float beatMinimumFlux = 0.03f;
constexpr double beatMinimumGap = 0.25;
constexpr float onsetScale = 5.0f;

/** Periodic Hann, whose coherent gain is 1/2: a full-scale sine centred on a
    bin has magnitude size / 4. */
std::vector<float> hann (int size)
{
    std::vector<float> window (static_cast<size_t> (size));
    for (int i = 0; i < size; ++i)
        window[static_cast<size_t> (i)] =
            0.5f
            - 0.5f * std::cos (juce::MathConstants<float>::twoPi * static_cast<float> (i) / static_cast<float> (size));
    return window;
}

float toUnit (float magnitude, float tiltDb)
{
    const auto db = 20.0f * std::log10 (magnitude + 1.0e-9f) + tiltDb;
    return juce::jlimit (0.0f, 1.0f, (db - floorDb) / -floorDb);
}
} // namespace

SpectrumAnalyser::SpectrumAnalyser (int bands, int waveform)
    : numBands (juce::jlimit (minBands, maxBands, bands)),
      waveformLength (juce::jlimit (minWaveformLength, maxWaveformLength, waveform)),
      bandFft (std::make_unique<juce::dsp::FFT> (bandFftOrder)),
      chromaFft (std::make_unique<juce::dsp::FFT> (chromaFftOrder)),
      bandWindow (hann (bandFftSize)),
      chromaWindow (hann (windowSize)),
      fftData (2 * windowSize),
      mono (windowSize),
      previousBands (static_cast<size_t> (numBands)),
      fluxHistory (beatHistoryCapacity)
{
}

SpectrumAnalyser::~SpectrumAnalyser() = default;

void SpectrumAnalyser::prepare (double rate)
{
    sampleRate = rate;
    bandMap.clear();
    lowBandCount = 0;

    const auto binHz = static_cast<float> (rate) / static_cast<float> (bandFftSize);
    const auto top = juce::jmin (highestBandHz, 0.45f * static_cast<float> (rate));
    const auto ratio = top / lowestBandHz;

    for (int i = 0; i < numBands; ++i)
    {
        const auto low = lowestBandHz * std::pow (ratio, static_cast<float> (i) / static_cast<float> (numBands));
        const auto high = lowestBandHz * std::pow (ratio, static_cast<float> (i + 1) / static_cast<float> (numBands));
        const auto centre = std::sqrt (low * high);
        bandMap.push_back (
            { low / binHz, high / binHz, centre / binHz, tiltDbPerOctave * std::log2 (centre / 1000.0f) });
        if (centre < beatBassHz)
            ++lowBandCount;
    }

    resetHistory();
}

void SpectrumAnalyser::process (const float* left, const float* right, int newSamples, AnalysisFrame& frame)
{
    jassert (sampleRate > 0.0);
    resize (frame);
    frame.silent = false;

    for (int i = 0; i < windowSize; ++i)
        mono[static_cast<size_t> (i)] = 0.5f * (left[i] + right[i]);

    computeBands (mono.data() + windowSize - bandFftSize, frame);
    computeChroma (mono.data(), frame);
    computeLevels (left, right, frame);
    computeWaveform (left, right, mono.data(), frame);
    detectBeat (newSamples, frame);
}

void SpectrumAnalyser::processSilence (AnalysisFrame& frame)
{
    resize (frame);
    frame.silent = true;
    std::fill (frame.bands.begin(), frame.bands.end(), 0.0f);
    frame.chroma.fill (0.0f);
    frame.peak.fill (0.0f);
    frame.rms.fill (0.0f);
    std::fill (frame.left.begin(), frame.left.end(), 0.0f);
    std::fill (frame.right.begin(), frame.right.end(), 0.0f);
    frame.onset = 0.0f;
    frame.beat = false;
    resetHistory();
}

void SpectrumAnalyser::resetHistory()
{
    hasPrevious = false;
    fluxHistory.clear();
    lastBeatSeconds = -1.0;
}

void SpectrumAnalyser::resize (AnalysisFrame& frame) const
{
    frame.bands.resize (static_cast<size_t> (numBands));
    frame.left.resize (static_cast<size_t> (waveformLength));
    frame.right.resize (static_cast<size_t> (waveformLength));

    frame.lowestHz = lowestBandHz;
    frame.highestHz =
        sampleRate > 0.0 ? juce::jmin (highestBandHz, 0.45f * static_cast<float> (sampleRate)) : highestBandHz;
}

void SpectrumAnalyser::computeBands (const float* samples, AnalysisFrame& frame)
{
    std::fill (fftData.begin(), fftData.end(), 0.0f);
    for (int i = 0; i < bandFftSize; ++i)
        fftData[static_cast<size_t> (i)] = samples[i] * bandWindow[static_cast<size_t> (i)];
    bandFft->performFrequencyOnlyForwardTransform (fftData.data(), true);

    const auto scale = 4.0f / static_cast<float> (bandFftSize);
    const auto lastBin = bandFftSize / 2;
    const auto magnitude = [&] (int bin)
    {
        return fftData[static_cast<size_t> (juce::jlimit (0, lastBin, bin))] * scale;
    };

    for (size_t b = 0; b < bandMap.size(); ++b)
    {
        const auto& band = bandMap[b];
        const auto first = static_cast<int> (std::ceil (band.lowBin));
        const auto last = juce::jmin (lastBin, static_cast<int> (std::ceil (band.highBin)) - 1);

        float value = 0.0f;
        if (first <= last)
        {
            for (auto bin = first; bin <= last; ++bin)
                value = juce::jmax (value, magnitude (bin));
        }
        else
        {
            // Narrower than a bin: interpolate at the band's centre.
            const auto below = static_cast<int> (std::floor (band.centreBin));
            const auto fraction = band.centreBin - static_cast<float> (below);
            value = magnitude (below) * (1.0f - fraction) + magnitude (below + 1) * fraction;
        }
        frame.bands[b] = toUnit (value, band.tiltDb);
    }
}

void SpectrumAnalyser::computeChroma (const float* samples, AnalysisFrame& frame)
{
    std::fill (fftData.begin(), fftData.end(), 0.0f);
    for (int i = 0; i < windowSize; ++i)
        fftData[static_cast<size_t> (i)] = samples[i] * chromaWindow[static_cast<size_t> (i)];
    chromaFft->performFrequencyOnlyForwardTransform (fftData.data(), true);

    const auto binHz = static_cast<float> (sampleRate) / static_cast<float> (windowSize);
    const auto scale = 4.0f / static_cast<float> (windowSize);
    const auto first = juce::jmax (1, static_cast<int> (std::ceil (chromaLowestHz / binHz)));
    const auto last = juce::jmin (windowSize / 2, static_cast<int> (chromaHighestHz / binHz));

    std::array<float, 12> energy {};
    float total = 0.0f;

    for (auto bin = first; bin <= last; ++bin)
    {
        // Each bin counts towards its nearest semitone, less the further off
        // it is.
        const auto note = 69.0f + 12.0f * std::log2 (static_cast<float> (bin) * binHz / 440.0f);
        const auto nearest = std::round (note);
        const auto weight = 1.0f - 2.0f * std::abs (note - nearest);
        const auto magnitude = fftData[static_cast<size_t> (bin)] * scale;
        const auto pitchClass = ((static_cast<int> (nearest) % 12) + 12) % 12;
        energy[static_cast<size_t> (pitchClass)] += weight * magnitude;
        total += magnitude;
    }

    const auto strongest = *std::max_element (energy.begin(), energy.end());
    for (size_t i = 0; i < 12; ++i)
        frame.chroma[i] = total > chromaSilence && strongest > 0.0f ? energy[i] / strongest : 0.0f;
}

void SpectrumAnalyser::computeLevels (const float* left, const float* right, AnalysisFrame& frame) const
{
    const float* channels[] = { left, right };
    for (size_t ch = 0; ch < 2; ++ch)
    {
        const auto* samples = channels[ch] + windowSize - levelSamples;
        float peak = 0.0f;
        double sum = 0.0;
        for (int i = 0; i < levelSamples; ++i)
        {
            peak = juce::jmax (peak, std::abs (samples[i]));
            sum += static_cast<double> (samples[i]) * samples[i];
        }
        frame.peak[ch] = peak;
        frame.rms[ch] = static_cast<float> (std::sqrt (sum / levelSamples));
    }
}

void SpectrumAnalyser::computeWaveform (const float* left,
                                        const float* right,
                                        const float* samples,
                                        AnalysisFrame& frame) const
{
    // The latest rising zero crossing that leaves a whole waveform after it.
    const auto latest = windowSize - waveformLength;
    auto start = latest;
    for (auto i = latest; i > juce::jmax (1, latest - triggerSearch); --i)
    {
        if (samples[i - 1] < 0.0f && samples[i] >= 0.0f)
        {
            start = i;
            break;
        }
    }

    std::copy (left + start, left + start + waveformLength, frame.left.begin());
    std::copy (right + start, right + start + waveformLength, frame.right.begin());
}

void SpectrumAnalyser::detectBeat (int newSamples, AnalysisFrame& frame)
{
    clockSeconds += static_cast<double> (juce::jmax (0, newSamples)) / sampleRate;

    float flux = 0.0f, bassFlux = 0.0f;
    if (hasPrevious)
    {
        for (size_t b = 0; b < frame.bands.size(); ++b)
        {
            const auto rise = juce::jmax (0.0f, frame.bands[b] - previousBands[b]);
            flux += rise;
            if (b < static_cast<size_t> (lowBandCount))
                bassFlux += rise;
        }
        flux /= static_cast<float> (frame.bands.size());
        bassFlux /= static_cast<float> (juce::jmax (1, lowBandCount));
    }
    std::copy (frame.bands.begin(), frame.bands.end(), previousBands.begin());
    hasPrevious = true;
    frame.onset = juce::jmin (1.0f, flux * onsetScale);

    // Mean and deviation of the bass flux over the recent history.
    double sum = 0.0, squares = 0.0;
    int count = 0;
    for (const auto& [time, recorded] : fluxHistory)
    {
        if (clockSeconds - time > beatHistorySeconds)
            continue;
        const auto value = static_cast<double> (recorded);
        sum += value;
        squares += value * value;
        ++count;
    }
    const auto mean = count > 0 ? sum / count : 0.0;
    const auto deviation = count > 0 ? std::sqrt (juce::jmax (0.0, squares / count - mean * mean)) : 0.0;

    frame.beat = bassFlux > beatMinimumFlux && bassFlux > mean + beatDeviations * deviation
                 && (lastBeatSeconds < 0.0 || clockSeconds - lastBeatSeconds >= beatMinimumGap);
    if (frame.beat)
        lastBeatSeconds = clockSeconds;

    fluxHistory.push ({ clockSeconds, bassFlux });
}
} // namespace anomp
