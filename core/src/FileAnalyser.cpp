#include "FileAnalyser.h"

#include <juce_dsp/juce_dsp.h>

#include <array>
#include <cmath>
#include <numeric>

namespace anomp
{
namespace
{
constexpr double pi = 3.14159265358979323846;
constexpr int readChunk = 16384;
constexpr int analysedChannels = 2;

/** The level below which audio counts as silence, -60 dBFS, as a linear RMS. */
constexpr double silenceRms = 0.001;
constexpr double silenceWindowSeconds = 0.01;
constexpr int levelWindows = 5; // 50 ms of silence windows.

// The spectrum for the cutoff: a 4096-point FFT twice a second.
constexpr int fftOrder = 12;
constexpr int fftSize = 1 << fftOrder;
constexpr double spectrumHopSeconds = 0.5;

/** Loudness of a mean square energy, as BS.1770 defines it. */
double loudnessOf (double energy) { return -0.691 + 10.0 * std::log10 (energy); }

/** 4× oversampling for the true peak: a windowed-sinc lowpass at the
    original Nyquist frequency, 12 taps per phase. */
class TruePeakMeter
{
public:
    TruePeakMeter()
    {
        constexpr int taps = phases * tapsPerPhase;
        constexpr double centre = (taps - 1) / 2.0;
        for (int i = 0; i < taps; ++i)
        {
            const auto t = (i - centre) / phases;
            const auto sinc = std::abs (t) < 1e-9 ? 1.0 : std::sin (pi * t) / (pi * t);
            const auto window = 0.5 - 0.5 * std::cos (2.0 * pi * (i + 0.5) / taps); // Hann
            coefficients[static_cast<size_t> (i % phases)][static_cast<size_t> (i / phases)] = sinc * window;
        }
        // Each phase passes DC at unity.
        for (auto& phase : coefficients)
        {
            const auto sum = std::accumulate (phase.begin(), phase.end(), 0.0);
            for (auto& c : phase)
                c /= sum;
        }
    }

    void process (float sample) noexcept
    {
        history[static_cast<size_t> (head)] = sample;
        for (const auto& phase : coefficients)
        {
            double y = 0.0;
            for (int k = 0; k < tapsPerPhase; ++k)
                y += phase[static_cast<size_t> (k)]
                     * history[static_cast<size_t> ((head - k + tapsPerPhase) % tapsPerPhase)];
            peak = juce::jmax (peak, std::abs (y));
        }
        head = (head + 1) % tapsPerPhase;
    }

    double peak = 0.0;

private:
    static constexpr int phases = 4, tapsPerPhase = 12;
    std::array<std::array<double, tapsPerPhase>, phases> coefficients {};
    std::array<float, tapsPerPhase> history {};
    int head = 0;
};

/** Where the averaged power spectrum `power` (bins of `binHz`, a full-scale
    sine at 0 dB) drops by 30 dB or more within 300 Hz between 11 kHz and
    Nyquist; 0 if it doesn't. */
double findCutoff (const std::vector<double>& power, double binHz)
{
    std::vector<double> db (power.size());
    for (size_t i = 0; i < power.size(); ++i)
        db[i] = 10.0 * std::log10 (juce::jmax (power[i], 1e-30));

    const auto bins = [binHz] (double hz)
    {
        return static_cast<int> (std::lround (hz / binHz));
    };
    const auto mean = [&db] (int from, int to)
    {
        double sum = 0.0;
        for (int i = from; i < to; ++i)
            sum += db[static_cast<size_t> (i)];
        return sum / juce::jmax (1, to - from);
    };

    const auto size = static_cast<int> (db.size());
    const auto width = juce::jmax (2, bins (1500.0));
    const auto guard = juce::jmax (1, bins (300.0));
    double bestDrop = 0.0;
    int best = 0;
    for (int c = bins (11000.0); c + guard + width <= size; ++c)
    {
        const auto below = mean (c - width, c);
        const auto above = mean (c + guard, c + guard + width);
        if (below > -110.0 && below - above > bestDrop)
        {
            bestDrop = below - above;
            best = c;
        }
    }
    return bestDrop >= 30.0 ? best * binHz : 0.0;
}
} // namespace

//==============================================================================
double KWeighting::Biquad::process (double x) noexcept
{
    // Transposed direct form II.
    const auto y = b0 * x + z1;
    z1 = b1 * x - a1 * y + z2;
    z2 = b2 * x - a2 * y;
    return y;
}

KWeighting::KWeighting (double sampleRate)
{
    // BS.1770's two stages, re-derived for any sample rate (as libebur128
    // does) rather than the 48 kHz coefficients the standard lists.
    {
        const auto f0 = 1681.974450955533, gainDb = 3.999843853973347, q = 0.7071752369554196;
        const auto k = std::tan (pi * f0 / sampleRate);
        const auto vh = std::pow (10.0, gainDb / 20.0);
        const auto vb = std::pow (vh, 0.4996667741545416);
        const auto a0 = 1.0 + k / q + k * k;
        shelf = { (vh + vb * k / q + k * k) / a0, 2.0 * (k * k - vh) / a0, (vh - vb * k / q + k * k) / a0,
                  2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0 };
    }
    {
        const auto f0 = 38.13547087602444, q = 0.5003270373238773;
        const auto k = std::tan (pi * f0 / sampleRate);
        const auto a0 = 1.0 + k / q + k * k;
        highPass = { 1.0, -2.0, 1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0 };
    }
}

float KWeighting::process (float sample) noexcept
{
    return static_cast<float> (highPass.process (shelf.process (sample)));
}

void KWeighting::reset() noexcept { shelf.z1 = shelf.z2 = highPass.z1 = highPass.z2 = 0.0; }

//==============================================================================
juce::String analyseReader (juce::AudioFormatReader& reader,
                            double start,
                            double end,
                            const std::function<bool (double)>& progress,
                            FileAnalysis& result)
{
    result = {};
    const auto rate = reader.sampleRate;
    if (rate <= 0.0 || reader.lengthInSamples <= 0 || reader.numChannels == 0)
        return "No audio in the file";
    if (! std::isfinite (start) || ! std::isfinite (end))
        return "The part's start or end isn't a number";

    const auto first = juce::jlimit (juce::int64 { 0 }, reader.lengthInSamples,
                                     static_cast<juce::int64> (std::llround (juce::jmax (0.0, start) * rate)));
    const auto last =
        end > start ? juce::jlimit (first, reader.lengthInSamples, static_cast<juce::int64> (std::llround (end * rate)))
                    : reader.lengthInSamples;
    const auto length = last - first;
    if (length <= 0)
        return "The part starts after the end of the file";

    result.sampleRate = rate;
    result.channels = static_cast<int> (reader.numChannels);

    // Loudness: 400 ms blocks every 100 ms, from 100 ms sub-blocks.
    std::array<KWeighting, analysedChannels> weighting { KWeighting (rate), KWeighting (rate) };
    const auto subBlockLength = juce::jmax (1, static_cast<int> (std::lround (rate * 0.1)));
    std::array<double, analysedChannels> subBlockSum {};
    int subBlockFill = 0;
    std::array<double, 4> recentSubBlocks {};
    int subBlocks = 0;
    std::vector<double> blockEnergies;
    blockEnergies.reserve (static_cast<size_t> (length / subBlockLength + 1));

    std::array<TruePeakMeter, analysedChannels> truePeak;
    float samplePeak = 0.0f;

    // Silence, in 10 ms windows.
    const auto windowLength = juce::jmax (1, static_cast<int> (std::lround (rate * silenceWindowSeconds)));
    std::array<double, analysedChannels> windowSum {};
    int windowFill = 0;
    juce::int64 windows = 0, silentRun = 0, runStart = 0;
    bool leadingDone = false;
    std::vector<double> firstLevels, lastLevels; // Mean squares of the louder channel per window.

    // Spectrum.
    juce::dsp::FFT fft (fftOrder);
    std::vector<float> fftBuffer (static_cast<size_t> (2 * fftSize));
    std::vector<float> ring (static_cast<size_t> (fftSize));
    int ringHead = 0;
    juce::int64 ringFilled = 0;
    const auto hop =
        juce::jmax (static_cast<juce::int64> (fftSize), static_cast<juce::int64> (rate * spectrumHopSeconds));
    juce::int64 sinceFrame = 0;
    std::vector<double> spectrum (static_cast<size_t> (fftSize / 2));
    int frames = 0;
    std::vector<float> window (static_cast<size_t> (fftSize));
    for (int i = 0; i < fftSize; ++i)
        window[static_cast<size_t> (i)] = static_cast<float> (0.5 - 0.5 * std::cos (2.0 * pi * i / fftSize));

    // Envelope.
    const auto points = static_cast<int> (juce::jmin (static_cast<juce::int64> (FileAnalysis::envelopePoints), length));
    result.envelopeMin.assign (static_cast<size_t> (points), 0.0f);
    result.envelopeMax.assign (static_cast<size_t> (points), 0.0f);

    juce::AudioBuffer<float> buffer (analysedChannels, readChunk);
    auto lastReport = juce::Time::getMillisecondCounter();

    for (juce::int64 position = 0; position < length;)
    {
        const auto count = static_cast<int> (juce::jmin (static_cast<juce::int64> (readChunk), length - position));
        if (! reader.read (&buffer, 0, count, first + position, true, true))
            return "Cannot decode the audio at " + juce::String (static_cast<double> (first + position) / rate, 1)
                   + " s";

        const auto* left = buffer.getReadPointer (0);
        const auto* right = buffer.getReadPointer (1);

        for (int i = 0; i < count; ++i)
        {
            const std::array<float, analysedChannels> sample { left[i], right[i] };

            for (size_t ch = 0; ch < analysedChannels; ++ch)
            {
                const auto weighted = weighting[ch].process (sample[ch]);
                subBlockSum[ch] += static_cast<double> (weighted) * weighted;
                truePeak[ch].process (sample[ch]);
                samplePeak = juce::jmax (samplePeak, std::abs (sample[ch]));
                windowSum[ch] += static_cast<double> (sample[ch]) * sample[ch];
            }

            if (++subBlockFill == subBlockLength)
            {
                recentSubBlocks[static_cast<size_t> (subBlocks % 4)] =
                    (subBlockSum[0] + subBlockSum[1]) / subBlockLength;
                subBlockSum = {};
                subBlockFill = 0;
                if (++subBlocks >= 4)
                    blockEnergies.push_back (std::accumulate (recentSubBlocks.begin(), recentSubBlocks.end(), 0.0)
                                             / 4.0);
            }

            if (++windowFill == windowLength)
            {
                const auto meanSquare = juce::jmax (windowSum[0], windowSum[1]) / windowLength;
                const auto silent = meanSquare < silenceRms * silenceRms;
                windowSum = {};
                windowFill = 0;

                if (static_cast<int> (firstLevels.size()) < levelWindows)
                    firstLevels.push_back (meanSquare);
                lastLevels.push_back (meanSquare);
                if (static_cast<int> (lastLevels.size()) > levelWindows)
                    lastLevels.erase (lastLevels.begin());

                if (silent)
                {
                    if (silentRun++ == 0)
                        runStart = windows;
                }
                else
                {
                    if (! leadingDone)
                    {
                        result.leadingSilence = static_cast<double> (silentRun) * windowLength / rate;
                        leadingDone = true;
                    }
                    else if (silentRun > 0)
                    {
                        const auto seconds = static_cast<double> (silentRun) * windowLength / rate;
                        if (seconds >= FileAnalysis::minGapSeconds && seconds > result.gapLength)
                        {
                            result.gapStart = static_cast<double> (runStart) * windowLength / rate;
                            result.gapLength = seconds;
                        }
                    }
                    silentRun = 0;
                }
                ++windows;
            }

            ring[static_cast<size_t> (ringHead)] = 0.5f * (sample[0] + sample[1]);
            ringHead = (ringHead + 1) % fftSize;
            ++ringFilled;
            if (++sinceFrame >= hop && ringFilled >= fftSize)
            {
                sinceFrame = 0;
                std::fill (fftBuffer.begin(), fftBuffer.end(), 0.0f);
                for (int k = 0; k < fftSize; ++k)
                    fftBuffer[static_cast<size_t> (k)] =
                        ring[static_cast<size_t> ((ringHead + k) % fftSize)] * window[static_cast<size_t> (k)];
                fft.performFrequencyOnlyForwardTransform (fftBuffer.data(), true);
                // A full-scale sine peaks at fftSize / 4 through the Hann window.
                const auto scale = 4.0 / fftSize;
                for (size_t k = 0; k < spectrum.size(); ++k)
                {
                    const auto magnitude = fftBuffer[k] * scale;
                    spectrum[k] += magnitude * magnitude;
                }
                ++frames;
            }

            const auto point = static_cast<size_t> ((position + i) * points / length);
            const auto low = juce::jmin (sample[0], sample[1]), high = juce::jmax (sample[0], sample[1]);
            result.envelopeMin[point] = juce::jmin (result.envelopeMin[point], low);
            result.envelopeMax[point] = juce::jmax (result.envelopeMax[point], high);
        }

        position += count;
        result.decodedSamples = position;

        if (progress && (position == count || juce::Time::getMillisecondCounter() - lastReport >= 250))
        {
            lastReport = juce::Time::getMillisecondCounter();
            if (! progress (static_cast<double> (position) / static_cast<double> (length)))
                return "Cancelled";
        }
    }

    // A track shorter than one block still gets one, over what there is.
    if (blockEnergies.empty() && subBlocks + subBlockFill > 0)
    {
        double sum = std::accumulate (recentSubBlocks.begin(), recentSubBlocks.begin() + juce::jmin (subBlocks, 4), 0.0)
                     * subBlockLength;
        sum += subBlockSum[0] + subBlockSum[1];
        blockEnergies.push_back (sum / (juce::jmin (subBlocks, 4) * subBlockLength + subBlockFill));
    }

    // Integrated loudness: blocks above -70 LUFS, then those within 10 LU
    // of their mean.
    const auto absoluteGate = std::pow (10.0, (FileAnalysis::histogramFloor + 0.691) / 10.0);
    double gatedSum = 0.0;
    int gatedCount = 0;
    for (const auto energy : blockEnergies)
    {
        if (energy <= absoluteGate)
            continue;
        gatedSum += energy;
        ++gatedCount;
        const auto bin =
            static_cast<int> ((loudnessOf (energy) - FileAnalysis::histogramFloor) / FileAnalysis::histogramStep);
        result.histogram[static_cast<size_t> (juce::jlimit (0, FileAnalysis::histogramBins - 1, bin))]++;
    }
    if (gatedCount > 0)
    {
        const auto relativeGate = gatedSum / gatedCount * 0.1; // -10 LU
        double sum = 0.0;
        int n = 0;
        for (const auto energy : blockEnergies)
        {
            if (energy > absoluteGate && energy > relativeGate)
            {
                sum += energy;
                ++n;
            }
        }
        if (n > 0)
            result.integratedLufs = loudnessOf (sum / n);
    }

    result.samplePeak = samplePeak;
    result.truePeak = juce::jmax (static_cast<double> (samplePeak), truePeak[0].peak, truePeak[1].peak);

    // The last silent run reaches the end; if nothing had sound, the whole
    // track is leading silence.
    const auto runSeconds = static_cast<double> (silentRun) * windowLength / rate;
    if (! leadingDone)
        result.leadingSilence = runSeconds;
    else
        result.trailingSilence = runSeconds;

    const auto levelDb = [] (const std::vector<double>& levels)
    {
        if (levels.empty())
            return -std::numeric_limits<double>::infinity();
        const auto mean = std::accumulate (levels.begin(), levels.end(), 0.0) / static_cast<double> (levels.size());
        return mean > 0.0 ? 10.0 * std::log10 (mean) : -std::numeric_limits<double>::infinity();
    };
    result.startLevelDb = levelDb (firstLevels);
    result.endLevelDb = levelDb (lastLevels);

    if (frames > 0)
    {
        for (auto& power : spectrum)
            power /= frames;
        result.cutoffHz = findCutoff (spectrum, rate / fftSize);
    }

    return {};
}

juce::String analyseFile (const juce::File& file,
                          juce::AudioFormatManager& formats,
                          double start,
                          double end,
                          const std::function<bool (double)>& progress,
                          FileAnalysis& result)
{
    if (! file.existsAsFile())
        return "File not found: " + file.getFullPathName();

    const std::unique_ptr<juce::AudioFormatReader> reader (formats.createReaderFor (file));
    if (reader == nullptr)
        return "Unsupported or unreadable file: " + file.getFullPathName();

    return analyseReader (*reader, start, end, progress, result);
}
} // namespace anomp
