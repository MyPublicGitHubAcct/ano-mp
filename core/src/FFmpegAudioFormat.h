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

    /** Keys a reader sets in its metadataValues, describing the file before
        it is decoded to float: FFmpeg's codec name ("flac", "mp3"…), "1"
        for a lossless codec, the source's bit depth (lossless only), and
        the bit rate in bits a second, where known. */
    static constexpr const char* codecKey = "anomp.codec";
    static constexpr const char* losslessKey = "anomp.lossless";
    static constexpr const char* bitsKey = "anomp.bits";
    static constexpr const char* bitRateKey = "anomp.bitRate";

    /** A chapter a container records: MP4 and Matroska chapters, ID3v2
        CHAP frames, Ogg CHAPTERxxx comments, a FLAC cue sheet's tracks. */
    struct Chapter
    {
        double start = 0.0; ///< Seconds.
        double end = -1.0;  ///< Seconds; -1 for the end of the file.
        juce::String title;
    };

    /** Reads `file`'s chapters from its header, without decoding. Safe to
        call from any thread. Returns an error message, or an empty string
        on success (with no chapters if it has none). */
    static juce::String readChapters (const juce::File& file, juce::Array<Chapter>& chapters);

private:
    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (FFmpegAudioFormat)
};
} // namespace anomp
