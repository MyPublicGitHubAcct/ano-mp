#include <catch2/catch_test_macros.hpp>

#include "FormatRegistry.h"

TEST_CASE ("FormatRegistry recognises the core formats", "[formats]")
{
    anomp::FormatRegistry registry;

    for (auto* ext : { "mp3", "flac", "wav", "aiff", "ogg" })
    {
        INFO ("extension: " << ext);
        CHECK (registry.canDecodeExtension (ext));
    }
}

TEST_CASE ("FormatRegistry normalises extensions", "[formats]")
{
    anomp::FormatRegistry registry;

    CHECK (registry.canDecodeExtension (".FLAC"));
    CHECK (registry.canDecodeExtension ("  Mp3 "));
    CHECK_FALSE (registry.canDecodeExtension (""));
    CHECK_FALSE (registry.canDecodeExtension ("txt"));
}

TEST_CASE ("FormatRegistry round-trips a WAV file", "[formats]")
{
    anomp::FormatRegistry registry;
    juce::MemoryBlock data;

    {
        juce::WavAudioFormat wav;
        std::unique_ptr<juce::OutputStream> out = std::make_unique<juce::MemoryOutputStream> (data, false);
        auto writer = wav.createWriterFor (
            out, juce::AudioFormatWriterOptions {}.withSampleRate (44100.0).withNumChannels (2).withBitsPerSample (16));
        REQUIRE (writer != nullptr);

        juce::AudioBuffer<float> buffer (2, 441);
        buffer.clear();
        REQUIRE (writer->writeFromAudioSampleBuffer (buffer, 0, buffer.getNumSamples()));
    }

    std::unique_ptr<juce::AudioFormatReader> reader (
        registry.manager().createReaderFor (std::make_unique<juce::MemoryInputStream> (data, false)));
    REQUIRE (reader != nullptr);
    CHECK (reader->sampleRate == 44100.0);
    CHECK (reader->numChannels == 2);
    CHECK (reader->lengthInSamples == 441);
}
