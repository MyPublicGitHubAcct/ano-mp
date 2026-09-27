#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "anomp/anomp.h"
#include "FileAnalyser.h"

#include <cmath>
#include <functional>
#include <random>
#include <string_view>

// The file analysis (PLAN.md O1–O4) over synthetic signals of known
// loudness, in the spirit of EBU Tech 3341's test cases, written to WAV in
// memory and read back.

namespace
{
constexpr double pi = 3.14159265358979323846;

/** A stereo signal: `sample (channel, n)` for `seconds` at `rate`. */
juce::AudioBuffer<float> make (double rate,
                               double seconds,
                               const std::function<double (int channel, juce::int64 n)>& sample,
                               int channels = 2)
{
    const auto length = static_cast<int> (rate * seconds);
    juce::AudioBuffer<float> buffer (channels, length);
    for (int ch = 0; ch < channels; ++ch)
        for (int n = 0; n < length; ++n)
            buffer.setSample (ch, n, static_cast<float> (sample (ch, n)));
    return buffer;
}

/** A sine of `dbfs` peak level. */
std::function<double (int, juce::int64)> sine (double hz, double dbfs, double rate)
{
    const auto amplitude = std::pow (10.0, dbfs / 20.0);
    return [=] (int, juce::int64 n)
    {
        return amplitude * std::sin (2.0 * pi * hz * static_cast<double> (n) / rate);
    };
}

/** `audio` as 32-bit float WAV bytes. */
juce::MemoryBlock toWav (const juce::AudioBuffer<float>& audio, double rate)
{
    juce::MemoryBlock bytes;
    {
        std::unique_ptr<juce::OutputStream> stream = std::make_unique<juce::MemoryOutputStream> (bytes, false);
        juce::WavAudioFormat wav;
        auto writer = wav.createWriterFor (
            stream, juce::AudioFormatWriterOptions {}
                        .withSampleRate (rate)
                        .withNumChannels (audio.getNumChannels())
                        .withBitsPerSample (32)
                        .withSampleFormat (juce::AudioFormatWriterOptions::SampleFormat::floatingPoint));
        REQUIRE (writer != nullptr);
        REQUIRE (writer->writeFromAudioSampleBuffer (audio, 0, audio.getNumSamples()));
    }
    return bytes;
}

anomp::FileAnalysis analyse (const juce::AudioBuffer<float>& audio, double rate)
{
    const auto bytes = toWav (audio, rate);
    juce::WavAudioFormat wav;
    std::unique_ptr<juce::AudioFormatReader> reader (
        wav.createReaderFor (new juce::MemoryInputStream (bytes, false), true));
    REQUIRE (reader != nullptr);
    anomp::FileAnalysis result;
    const auto error = anomp::analyseReader (*reader, 0.0, 0.0, {}, result);
    INFO (error);
    REQUIRE (error.isEmpty());
    return result;
}

juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }
} // namespace

TEST_CASE ("Analysis measures the loudness of steady tones", "[file-analysis]")
{
    const auto rate = GENERATE (44100.0, 48000.0, 96000.0);
    CAPTURE (rate);

    // Tech 3341 cases 1 and 2: 1 kHz at -23 and -33 dBFS in both channels.
    CHECK (analyse (make (rate, 5.0, sine (1000.0, -23.0, rate)), rate).integratedLufs
           == Catch::Approx (-23.0).margin (0.1));
    CHECK (analyse (make (rate, 5.0, sine (1000.0, -33.0, rate)), rate).integratedLufs
           == Catch::Approx (-33.0).margin (0.1));

    // A mono file plays on both channels, so it measures as dual mono.
    CHECK (analyse (make (rate, 5.0, sine (1000.0, -23.0, rate), 1), rate).integratedLufs
           == Catch::Approx (-23.0).margin (0.1));
}

TEST_CASE ("Analysis gates quiet passages out of the loudness", "[file-analysis]")
{
    // Tech 3341 case 3, shortened: -36, -23, -36 dBFS; the relative gate
    // leaves only the loud part.
    constexpr double rate = 48000.0;
    const auto loud = sine (1000.0, -23.0, rate), quiet = sine (1000.0, -36.0, rate);
    const auto audio = make (rate, 30.0,
                             [&] (int ch, juce::int64 n)
                             {
                                 const auto t = static_cast<double> (n) / rate;
                                 return t < 5.0 || t >= 25.0 ? quiet (ch, n) : loud (ch, n);
                             });
    const auto result = analyse (audio, rate);
    CHECK (result.integratedLufs == Catch::Approx (-23.0).margin (0.1));

    // The histogram holds every block above -70 LUFS, at its level.
    juce::uint32 blocks = 0;
    for (auto count : result.histogram)
        blocks += count;
    CHECK (blocks == Catch::Approx (30.0 * 10.0 - 3.0).margin (1.0));
    const auto binOf = [] (double lufs)
    {
        return static_cast<size_t> ((lufs - anomp::FileAnalysis::histogramFloor) / anomp::FileAnalysis::histogramStep);
    };
    CHECK (result.histogram[binOf (-23.0)] + result.histogram[binOf (-23.0) - 1] >= 190);

    // Digital silence has no loudness at all.
    const auto silent = analyse (make (rate, 2.0, [] (int, juce::int64) { return 0.0; }), rate);
    CHECK (std::isnan (silent.integratedLufs));
    CHECK (silent.truePeak == 0.0);
}

TEST_CASE ("Analysis finds peaks between samples", "[file-analysis]")
{
    // A quarter of the sample rate, 45° out: every sample is at 0.707 of the
    // wave's peak, which falls between them.
    constexpr double rate = 48000.0;
    const auto audio = make (rate, 1.0, [] (int, juce::int64 n)
                             { return 0.5 * std::sin (pi * static_cast<double> (n) / 2.0 + pi / 4.0); });
    const auto result = analyse (audio, rate);
    CHECK (result.samplePeak == Catch::Approx (0.5 * std::sqrt (0.5)).margin (1e-4));
    CHECK (result.truePeak == Catch::Approx (0.5).margin (0.02));
}

TEST_CASE ("Analysis finds silence at the ends and a gap inside", "[file-analysis]")
{
    constexpr double rate = 44100.0;
    const auto tone = sine (440.0, -12.0, rate);
    // 1 s silence, 2 s tone, 3 s silence, 2 s tone (a hidden track), 1.5 s silence.
    const auto audio = make (rate, 9.5,
                             [&] (int ch, juce::int64 n)
                             {
                                 const auto t = static_cast<double> (n) / rate;
                                 return (t >= 1.0 && t < 3.0) || (t >= 6.0 && t < 8.0) ? tone (ch, n) : 0.0;
                             });
    const auto result = analyse (audio, rate);
    CHECK (result.leadingSilence == Catch::Approx (1.0).margin (0.011));
    CHECK (result.trailingSilence == Catch::Approx (1.5).margin (0.011));
    CHECK (result.gapStart == Catch::Approx (3.0).margin (0.011));
    CHECK (result.gapLength == Catch::Approx (3.0).margin (0.011));
    CHECK (std::isinf (result.startLevelDb));
    CHECK (std::isinf (result.endLevelDb));

    // A track that is sound to its very edges could segue.
    const auto full = analyse (make (rate, 3.0, tone), rate);
    CHECK (full.leadingSilence == 0.0);
    CHECK (full.trailingSilence == 0.0);
    CHECK (full.gapLength == 0.0);
    CHECK (full.startLevelDb == Catch::Approx (-15.0).margin (0.2)); // A sine's RMS is 3 dB under its peak.
    CHECK (full.endLevelDb == Catch::Approx (-15.0).margin (0.2));
}

TEST_CASE ("Analysis finds a lossy encoder's lowpass", "[file-analysis]")
{
    constexpr double rate = 44100.0;
    // Tones every 50 Hz with random phases: up to 16 kHz, or all the way.
    const auto noise = [] (double topHz)
    {
        std::mt19937 random (7);
        std::uniform_real_distribution<double> phase (0.0, 2.0 * pi);
        std::vector<std::pair<double, double>> partials;
        for (double hz = 50.0; hz <= topHz; hz += 50.0)
            partials.emplace_back (hz, phase (random));
        const auto amplitude = 0.5 / std::sqrt (static_cast<double> (partials.size()));
        return make (rate, 3.0,
                     [partials, amplitude] (int, juce::int64 n)
                     {
                         double sum = 0.0;
                         for (const auto& [hz, offset] : partials)
                             sum += std::sin (2.0 * pi * hz * static_cast<double> (n) / rate + offset);
                         return amplitude * sum;
                     });
    };
    CHECK (analyse (noise (16000.0), rate).cutoffHz == Catch::Approx (16000.0).margin (300.0));
    CHECK (analyse (noise (21900.0), rate).cutoffHz == 0.0);
}

TEST_CASE ("Analysis keeps a coarse waveform", "[file-analysis]")
{
    constexpr double rate = 44100.0;
    // Loud first half, quiet second half.
    const auto audio = make (rate, 4.0,
                             [] (int, juce::int64 n)
                             {
                                 const auto level = n < static_cast<juce::int64> (2 * rate) ? 0.8 : 0.1;
                                 return level * std::sin (2.0 * pi * 1000.0 * static_cast<double> (n) / rate);
                             });
    const auto result = analyse (audio, rate);
    REQUIRE (result.envelopeMax.size() == anomp::FileAnalysis::envelopePoints);
    REQUIRE (result.envelopeMin.size() == anomp::FileAnalysis::envelopePoints);
    CHECK (result.envelopeMax[100] == Catch::Approx (0.8).margin (0.01));
    CHECK (result.envelopeMin[100] == Catch::Approx (-0.8).margin (0.01));
    CHECK (result.envelopeMax[900] == Catch::Approx (0.1).margin (0.01));
    CHECK (result.decodedSamples == static_cast<juce::int64> (4 * rate));

    // A track shorter than the envelope gets a point per sample.
    const auto tiny = analyse (make (rate, 0.01, [] (int, juce::int64) { return 0.25; }), rate);
    CHECK (tiny.envelopeMax.size() == 441);
}

TEST_CASE ("Analysis through the C API, with progress and cancelling", "[file-analysis][c-api]")
{
    char error[256] = "unchanged";
    auto* analysis = anomp_analyse_file (fixtureFile ("flac-44k.flac").getFullPathName().toRawUTF8(), 0.0, 0.0, nullptr,
                                         nullptr, error, sizeof (error));
    INFO (error);
    REQUIRE (analysis != nullptr);
    CHECK (std::string_view (error).empty());
    CHECK (analysis->duration == Catch::Approx (22371 / 44100.0));
    CHECK (analysis->sample_rate == 44100.0);
    CHECK (analysis->channels == 2);
    CHECK (std::isfinite (analysis->integrated_lufs));
    CHECK (analysis->true_peak >= analysis->sample_peak);
    CHECK (analysis->sample_peak == Catch::Approx (0.5).margin (0.01));
    CHECK (analysis->histogram_count == anomp::FileAnalysis::histogramBins);
    CHECK (analysis->histogram_floor == -70.0);
    CHECK (analysis->envelope_length == 1000);
    CHECK (analysis->envelope_max[500] > 0.3f);
    anomp_file_analysis_free (analysis);

    // Cancelled at the first report.
    int calls = 0;
    const anomp_analysis_progress cancel = [] (double, void* count)
    {
        ++*static_cast<int*> (count);
        return 0;
    };
    CHECK (anomp_analyse_file (fixtureFile ("flac-44k.flac").getFullPathName().toRawUTF8(), 0.0, 0.0, cancel, &calls,
                               error, sizeof (error))
           == nullptr);
    CHECK (calls == 1);
    CHECK (std::string_view (error) == "Cancelled");

    CHECK (anomp_analyse_file ("relative.flac", 0.0, 0.0, nullptr, nullptr, error, sizeof (error)) == nullptr);
    CHECK (std::string_view (error).starts_with ("Path is not absolute"));
    CHECK (anomp_analyse_file (fixtureFile ("missing.flac").getFullPathName().toRawUTF8(), 0.0, 0.0, nullptr, nullptr,
                               error, sizeof (error))
           == nullptr);
    CHECK (std::string_view (error).starts_with ("File not found"));
    anomp_file_analysis_free (nullptr);
}

TEST_CASE ("Analysis measures part of a file", "[file-analysis]")
{
    const auto path = fixtureFile ("flac-44k.flac").getFullPathName();
    char error[256] = "";
    auto* part = anomp_analyse_file (path.toRawUTF8(), 0.1, 0.3, nullptr, nullptr, error, sizeof (error));
    INFO (error);
    REQUIRE (part != nullptr);
    CHECK (part->duration == Catch::Approx (0.2));
    CHECK (part->envelope_length == 1000);
    anomp_file_analysis_free (part);

    CHECK (anomp_analyse_file (path.toRawUTF8(), 5.0, 0.0, nullptr, nullptr, error, sizeof (error)) == nullptr);
    CHECK (std::string_view (error) == "The part starts after the end of the file");
}
