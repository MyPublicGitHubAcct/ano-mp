#pragma once

#include <juce_audio_formats/juce_audio_formats.h>

namespace anomp
{
/** A read-only JUCE AudioFormat that decodes every supported file type with
    FFmpeg (PLAN.md §4.3). Nothing outside FFmpegAudioFormat.cpp includes
    FFmpeg headers.

    Readers produce 32-bit float samples at the file's own sample rate, trim
    encoder delay and padding where the file records them (gapless MP3, AAC,
    Opus, Vorbis), and seek to exact samples.
*/
class FFmpegAudioFormat final : public juce::AudioFormat
{
public:
    FFmpegAudioFormat();
    ~FFmpegAudioFormat() override;

    juce::Array<int> getPossibleSampleRates() override;
    juce::Array<int> getPossibleBitDepths() override;
    bool canDoStereo() override;
    bool canDoMono() override;
    bool isCompressed() override;

    /** Takes ownership of `sourceStream` if a reader is returned. The stream
        must support setPosition() for seeking and for most containers. */
    juce::AudioFormatReader* createReaderFor (juce::InputStream* sourceStream,
                                              bool deleteStreamIfOpeningFails) override;

    /** Always returns nullptr: the player never encodes. */
    std::unique_ptr<juce::AudioFormatWriter> createWriterFor (std::unique_ptr<juce::OutputStream>& streamToWriteTo,
                                                              const juce::AudioFormatWriterOptions& options) override;

    using juce::AudioFormat::createWriterFor;

private:
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (FFmpegAudioFormat)
};
} // namespace anomp
