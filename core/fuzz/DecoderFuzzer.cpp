// libFuzzer target for FFmpegAudioFormat (PLAN.md H5): opens the input as an
// audio file from memory, reads its start, then seeks to a few positions the
// input chooses and reads there, as the player and the file analyser do.

#include "FFmpegAudioFormat.h"

#include <fuzzer/FuzzedDataProvider.h>

#include <cstddef>
#include <cstdint>

extern "C" int LLVMFuzzerTestOneInput (const uint8_t* data, size_t size)
{
    static anomp::FFmpegAudioFormat format;
    static const bool quiet = (anomp::FFmpegAudioFormat::silenceLog(), true); // After the constructor's level.
    juce::ignoreUnused (quiet);

    // The seek positions come from the end of the input, so a seed file
    // loses only its last few bytes to them.
    FuzzedDataProvider provider (data, size);
    uint16_t seeks[4];
    for (auto& seek : seeks)
        seek = provider.ConsumeIntegral<uint16_t>();
    const auto bytes = provider.ConsumeRemainingBytes<uint8_t>();

    auto* stream = new juce::MemoryInputStream (bytes.data(), bytes.size(), false);
    const std::unique_ptr<juce::AudioFormatReader> reader (format.createReaderFor (stream, true));
    if (reader == nullptr)
        return 0;

    constexpr int blockSize = 4096;
    juce::AudioBuffer<float> buffer (static_cast<int> (juce::jmin (reader->numChannels, 8u)), blockSize);
    const auto length = reader->lengthInSamples;

    reader->read (&buffer, 0, blockSize, 0, true, true);
    for (const auto seek : seeks)
    {
        // Anywhere from the start to just past the end.
        const auto position = length * seek / 0xfff0;
        reader->read (&buffer, 0, blockSize, position, true, true);
    }
    return 0;
}
