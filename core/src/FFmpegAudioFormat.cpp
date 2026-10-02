#include "FFmpegAudioFormat.h"

extern "C"
{
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libswresample/swresample.h>
}

#include <limits>
#include <vector>

namespace anomp
{
namespace
{
constexpr int ioBufferSize = 64 * 1024;

/** Samples decoded and thrown away before a seek target on lossy codecs, so
    the decoder's state has settled by then. VBR MP3 (bit reservoir) and Opus
    measurably need more than 4096; 16384 (~0.37 s) matches a straight decode
    to within 1e-4 on every test fixture. */
constexpr juce::int64 minimumLossyPreroll = 16384;

/** Seek targets stay at least this far from either end of the stream:
    timestamps there are the least reliable (the first and last Ogg pages,
    priming, start and end trimming). Nearer the start the reader rewinds;
    nearer the end it seeks earlier. Either way it decodes forward, which
    costs a millisecond or two. */
constexpr double seekMarginSeconds = 1.0;

/** How far before the end the length measurement starts decoding. */
constexpr double endMeasurementSeconds = 2.0;

//==============================================================================
// AVIOContext callbacks over a juce::InputStream, so paths, security-scoped
// bookmarks and in-memory data all go through one code path.

int readPacket (void* opaque, uint8_t* buffer, int size)
{
    auto* stream = static_cast<juce::InputStream*> (opaque);
    const auto bytesRead = stream->read (buffer, size);
    return bytesRead > 0 ? bytesRead : AVERROR_EOF;
}

int64_t seekStream (void* opaque, int64_t offset, int whence)
{
    auto* stream = static_cast<juce::InputStream*> (opaque);
    whence &= ~AVSEEK_FORCE;

    if (whence == AVSEEK_SIZE)
    {
        const auto length = stream->getTotalLength();
        return length >= 0 ? length : AVERROR (ENOSYS);
    }

    juce::int64 target = 0;

    switch (whence)
    {
        case SEEK_SET: target = offset; break;
        case SEEK_CUR: target = stream->getPosition() + offset; break;
        case SEEK_END:
        {
            const auto length = stream->getTotalLength();
            if (length < 0)
                return AVERROR (ENOSYS);
            target = length + offset;
            break;
        }
        default: return AVERROR (EINVAL);
    }

    return stream->setPosition (target) ? target : AVERROR (EIO);
}

//==============================================================================
class FFmpegAudioFormatReader final : public juce::AudioFormatReader
{
public:
    explicit FFmpegAudioFormatReader (juce::InputStream* source) : juce::AudioFormatReader (source, "FFmpeg")
    {
        opened = open();
    }

    ~FFmpegAudioFormatReader() override
    {
        swr_free (&resampler);
        av_channel_layout_uninit (&outputLayout);
        av_channel_layout_uninit (&resamplerInputLayout);
        av_frame_free (&frame);
        av_packet_free (&packet);
        closeDemuxer();

        if (io != nullptr)
            av_freep (&io->buffer);
        avio_context_free (&io);
    }

    bool isOpen() const noexcept { return opened; }

    bool readSamples (int* const* destChannels,
                      int numDestChannels,
                      int startOffsetInDestBuffer,
                      juce::int64 startSampleInFile,
                      int numSamples) override
    {
        clearSamplesBeyondAvailableLength (destChannels, numDestChannels, startOffsetInDestBuffer, startSampleInFile,
                                           numSamples, lengthInSamples);

        auto position = startSampleInFile;
        auto offset = startOffsetInDestBuffer;
        auto remaining = numSamples;

        while (remaining > 0)
        {
            if (! bufferHolds (position) && ! decodeUpTo (position))
            {
                // Decoding stopped early (truncated or corrupt file): silence.
                for (int ch = 0; ch < numDestChannels; ++ch)
                    if (destChannels[ch] != nullptr)
                        juce::FloatVectorOperations::clear (reinterpret_cast<float*> (destChannels[ch]) + offset,
                                                            remaining);
                return false;
            }

            const auto indexInFrame = static_cast<int> (position - frameStart);
            const auto count = juce::jmin (remaining, frameLength - indexInFrame);

            for (int ch = 0; ch < numDestChannels; ++ch)
                if (destChannels[ch] != nullptr)
                    juce::FloatVectorOperations::copy (reinterpret_cast<float*> (destChannels[ch]) + offset,
                                                       frameBuffer.getReadPointer (ch, indexInFrame), count);

            position += count;
            offset += count;
            remaining -= count;
        }

        return true;
    }

private:
    //==============================================================================
    bool open()
    {
        auto* buffer = static_cast<uint8_t*> (av_malloc (ioBufferSize));
        io = avio_alloc_context (buffer, ioBufferSize, 0, input, readPacket, nullptr, seekStream);
        frame = av_frame_alloc();
        packet = av_packet_alloc();

        if (io == nullptr)
            av_free (buffer);

        if (io == nullptr || frame == nullptr || packet == nullptr || ! openDemuxer())
            return false;

        const auto* parameters = stream->codecpar;
        sampleRate = parameters->sample_rate;
        numChannels = static_cast<unsigned int> (parameters->ch_layout.nb_channels);
        bitsPerSample = 32;
        usesFloatingPointData = true;

        if (av_channel_layout_copy (&outputLayout, &parameters->ch_layout) < 0)
            return false;

        const auto* descriptor = avcodec_descriptor_get (parameters->codec_id);
        const bool lossless = descriptor != nullptr && (descriptor->props & AV_CODEC_PROP_LOSSLESS) != 0;

        // What the file holds, before decoding to float (FFmpegAudioFormat.h).
        if (descriptor != nullptr)
            metadataValues.set (FFmpegAudioFormat::codecKey, descriptor->name);
        metadataValues.set (FFmpegAudioFormat::losslessKey, lossless ? "1" : "0");
        const auto sourceBits =
            parameters->bits_per_raw_sample > 0 ? parameters->bits_per_raw_sample : parameters->bits_per_coded_sample;
        if (lossless && sourceBits > 0)
            metadataValues.set (FFmpegAudioFormat::bitsKey, juce::String (sourceBits));
        const auto bitRate = parameters->bit_rate > 0 ? parameters->bit_rate : format->bit_rate;
        if (bitRate > 0)
            metadataValues.set (FFmpegAudioFormat::bitRateKey, juce::String (bitRate));
        preroll = lossless ? 0 : juce::jmax (minimumLossyPreroll, static_cast<juce::int64> (parameters->seek_preroll));

        // Timestamps coarser than one sample (ASF uses milliseconds) cannot
        // place a seek exactly; such files seek by decoding from the start.
        seekByTimestamp = av_q2d (stream->time_base) * sampleRate <= 1.0;

        const bool headerDurationReliable =
            stream->duration != AV_NOPTS_VALUE && format->duration_estimation_method != AVFMT_DURATION_FROM_BITRATE;

        // Containers without reliable timestamps or durations of their own
        // (raw MP3 and AAC) get an exact seek index from a demux-only pass.
        if (! headerDurationReliable || juce::String (format->iformat->name) == "mp3")
        {
            scanPackets();
            if (! rewind())
                return false;
        }
        else
        {
            resetDecoder (true);
        }

        // The first decoded sample is position 0; its timestamp anchors seeks.
        if (! decodeNextFrame())
            return false;

        if (! hasTimestamps)
            seekByTimestamp = false;

        auto length = measureDecodedLength();

        // The header can include codec delay (Opus pre-skip) and the decoder
        // can miss end trimming (Vorbis), but neither under-reports, so the
        // smaller of the two is the true length.
        if (headerDurationReliable)
            length = juce::jmin (length, av_rescale_q (stream->duration, stream->time_base, sampleTimeBase()));

        lengthInSamples = juce::jmax (juce::int64 { 0 }, length);
        return lengthInSamples > 0 && rewind();
    }

    /** Opens the container and decoder on `io`, from its current position. */
    bool openDemuxer()
    {
        format = avformat_alloc_context();
        if (format == nullptr)
            return false;

        format->pb = io;
        format->flags |= AVFMT_FLAG_CUSTOM_IO;

        // The file name only helps FFmpeg guess the container; I/O goes through `io`.
        juce::String name;
        if (auto* fileStream = dynamic_cast<juce::FileInputStream*> (input))
            name = fileStream->getFile().getFileName();

        // The MP3 Xing TOC only approximates positions in VBR files; seek
        // through the exact index built by scanPackets() instead.
        AVDictionary* options = nullptr;
        av_dict_set (&options, "usetoc", "0", 0);
        const auto openResult = avformat_open_input (&format, name.toRawUTF8(), nullptr, &options);
        av_dict_free (&options);

        if (openResult < 0 || avformat_find_stream_info (format, nullptr) < 0)
            return false; // A failed avformat_open_input has already freed `format`.

        streamIndex = av_find_best_stream (format, AVMEDIA_TYPE_AUDIO, -1, -1, nullptr, 0);
        if (streamIndex < 0)
            return false;

        // Skip attached cover art and any other streams while demuxing.
        for (unsigned int i = 0; i < format->nb_streams; ++i)
            if (static_cast<int> (i) != streamIndex)
                format->streams[i]->discard = AVDISCARD_ALL;

        stream = format->streams[streamIndex];
        const auto* parameters = stream->codecpar;
        const auto* decoder = avcodec_find_decoder (parameters->codec_id);

        if (decoder == nullptr || parameters->sample_rate <= 0 || parameters->ch_layout.nb_channels <= 0)
            return false;

        codec = avcodec_alloc_context3 (decoder);
        if (codec == nullptr || avcodec_parameters_to_context (codec, parameters) < 0)
            return false;

        codec->pkt_timebase = stream->time_base;
        if (avcodec_open2 (codec, decoder, nullptr) < 0)
            return false;

        for (const auto& entry : scannedIndex)
            av_add_index_entry (stream, entry.position, entry.timestamp, entry.size, 0, AVINDEX_KEYFRAME);

        return true;
    }

    void closeDemuxer()
    {
        avcodec_free_context (&codec);
        avformat_close_input (&format);
        stream = nullptr;
    }

    /** Reads every packet once, recording an exact seek index that survives rewind(). */
    void scanPackets()
    {
        scannedIndex.clear();

        while (av_read_frame (format, packet) >= 0)
        {
            if (packet->stream_index == streamIndex && packet->pos >= 0 && packet->pts != AV_NOPTS_VALUE)
                scannedIndex.push_back ({ packet->pos, packet->pts, packet->size });
            av_packet_unref (packet);
        }
    }

    /** Decodes from near the end to EOF and returns where the output ends. */
    juce::int64 measureDecodedLength()
    {
        juce::int64 end = frameStart + frameLength;
        const auto margin = static_cast<juce::int64> (endMeasurementSeconds * sampleRate);

        if (seekByTimestamp && stream->duration != AV_NOPTS_VALUE)
        {
            const auto estimate = av_rescale_q (stream->duration, stream->time_base, sampleTimeBase());
            if (estimate > 2 * margin && seekNear (estimate - margin))
                end = frameStart + frameLength;
        }

        while (decodeNextFrame())
            end = frameStart + frameLength;

        return end;
    }

    //==============================================================================
    AVRational sampleTimeBase() const { return { 1, static_cast<int> (sampleRate) }; }

    bool bufferHolds (juce::int64 position) const
    {
        return frameLength > 0 && position >= frameStart && position < frameStart + frameLength;
    }

    /** Leaves the frame containing `position` in frameBuffer. */
    bool decodeUpTo (juce::int64 position)
    {
        if (format == nullptr)
            return false; // A failed rewind() left nothing to decode.

        const auto decodedEnd = frameStart + frameLength;
        const bool reachableByDecoding = frameLength > 0 && position >= decodedEnd
                                         && position - decodedEnd < juce::jmax (preroll, juce::int64 { 8192 });

        if (! reachableByDecoding && ! seekNear (position))
            return false;

        while (! bufferHolds (position))
        {
            if (frameLength > 0 && frameStart > position)
                return false; // Landed past the target and cannot go back.

            if (! decodeNextFrame())
                return false;
        }

        return true;
    }

    /** Positions the decoder so the current frame starts at or before
        `position`, leaving that frame decoded. */
    bool seekNear (juce::int64 position)
    {
        if (seekByTimestamp)
        {
            const auto margin = static_cast<juce::int64> (seekMarginSeconds * sampleRate);
            // lengthInSamples is still 0 while open() measures the length.
            const auto maximumTarget =
                lengthInSamples > 0 ? lengthInSamples - margin : std::numeric_limits<juce::int64>::max();

            for (int attempt = 0; attempt < 4; ++attempt)
            {
                // Retries back off further, even for lossless codecs (no preroll).
                const auto backoff = attempt == 0 ? preroll : juce::jmax (preroll, juce::int64 { 4096 }) << attempt;
                const auto target = juce::jmin (position - backoff, maximumTarget);
                if (target < margin)
                    break;

                const auto timestamp = av_rescale_q (firstTimestamp + target, sampleTimeBase(), stream->time_base);
                if (av_seek_frame (format, streamIndex, timestamp, AVSEEK_FLAG_BACKWARD) < 0)
                    break;

                resetDecoder (true);

                if (! decodeNextFrame() || ! positionKnown)
                    break;

                if (frameStart <= position)
                    return true;
            }
        }

        return rewind() && decodeNextFrame();
    }

    /** Returns to the start by reopening the container and decoder. Seeking
        to the start is not equivalent: decoders that need the previous packet
        (AAC, Vorbis) and start trimming (MP3, Opus) behave differently there. */
    bool rewind()
    {
        closeDemuxer();

        if (avio_seek (io, 0, SEEK_SET) < 0 || ! openDemuxer())
            return false;

        const auto* parameters = stream->codecpar;
        if (parameters->sample_rate != static_cast<int> (sampleRate)
            || parameters->ch_layout.nb_channels != static_cast<int> (numChannels))
            return false;

        // Before anchoring, the first frame's timestamp becomes firstTimestamp.
        resetDecoder (seekByTimestamp);
        if (! seekByTimestamp)
            nextPosition = 0;
        return true;
    }

    /** Flushes the decoder after a seek. With `fromTimestamp`, the next
        frame's timestamp sets the position; otherwise counting continues. */
    void resetDecoder (bool fromTimestamp)
    {
        if (codec != nullptr)
            avcodec_flush_buffers (codec);
        demuxEnded = false;
        frameLength = 0;
        positionKnown = ! fromTimestamp;
        awaitingTimestamp = fromTimestamp;
    }

    //==============================================================================
    /** Decodes the next non-empty frame into frameBuffer. */
    bool decodeNextFrame()
    {
        if (codec == nullptr)
            return false;

        for (;;)
        {
            const auto received = avcodec_receive_frame (codec, frame);

            if (received == 0)
            {
                const bool converted = frame->nb_samples > 0 && convertFrame();
                av_frame_unref (frame);
                if (converted)
                    return true;
                continue;
            }

            if (received != AVERROR (EAGAIN))
                return false; // AVERROR_EOF after the final flush, or a decoder error.

            if (demuxEnded)
                return false;

            if (av_read_frame (format, packet) < 0)
            {
                demuxEnded = true;
                avcodec_send_packet (codec, nullptr); // Flush the decoder's delayed frames.
                continue;
            }

            if (packet->stream_index == streamIndex)
                avcodec_send_packet (codec, packet); // A corrupt packet is skipped, not fatal.

            av_packet_unref (packet);
        }
    }

    bool convertFrame()
    {
        if (awaitingTimestamp)
        {
            awaitingTimestamp = false;
            const auto timestamp = frame->best_effort_timestamp;

            if (timestamp != AV_NOPTS_VALUE)
            {
                const auto samples = av_rescale_q (timestamp, stream->time_base, sampleTimeBase());

                if (! anchored)
                {
                    firstTimestamp = samples;
                    hasTimestamps = true;
                    anchored = true;
                }

                nextPosition = samples - firstTimestamp;
                positionKnown = true;
            }
            else if (! anchored)
            {
                // No timestamps: this is the first frame, so count from 0.
                anchored = true;
                nextPosition = 0;
                positionKnown = true;
            }
        }
        else if (! anchored)
        {
            anchored = true;
            nextPosition = 0;
            positionKnown = true;
        }

        if (! prepareResampler())
            return false;

        const auto samples = frame->nb_samples;
        if (frameBuffer.getNumSamples() < samples)
            frameBuffer.setSize (static_cast<int> (numChannels), samples, false, false, true);

        auto* const* output = reinterpret_cast<uint8_t* const*> (frameBuffer.getArrayOfWritePointers());
        const auto converted = swr_convert (resampler, output, samples, frame->extended_data, samples);
        if (converted < 0)
            return false;

        frameStart = nextPosition;
        frameLength = converted;
        nextPosition += converted;
        return converted > 0;
    }

    /** Converts every frame to planar float in the file's channel layout. The
        sample rate is unchanged; JUCE resamples to the device later. */
    bool prepareResampler()
    {
        if (resampler != nullptr && frame->format == resamplerInputFormat
            && av_channel_layout_compare (&frame->ch_layout, &resamplerInputLayout) == 0)
            return true;

        swr_free (&resampler);
        av_channel_layout_uninit (&resamplerInputLayout);

        const auto rate = static_cast<int> (sampleRate);
        if (swr_alloc_set_opts2 (&resampler, &outputLayout, AV_SAMPLE_FMT_FLTP, rate, &frame->ch_layout,
                                 static_cast<AVSampleFormat> (frame->format), rate, 0, nullptr)
                < 0
            || swr_init (resampler) < 0 || av_channel_layout_copy (&resamplerInputLayout, &frame->ch_layout) < 0)
        {
            swr_free (&resampler);
            return false;
        }

        resamplerInputFormat = frame->format;
        return true;
    }

    //==============================================================================
    AVIOContext* io = nullptr;
    AVFormatContext* format = nullptr;
    AVCodecContext* codec = nullptr;
    AVStream* stream = nullptr;
    AVPacket* packet = nullptr;
    AVFrame* frame = nullptr;
    SwrContext* resampler = nullptr;
    AVChannelLayout outputLayout {};
    AVChannelLayout resamplerInputLayout {};
    int resamplerInputFormat = -1;
    int streamIndex = -1;

    bool opened = false;
    bool seekByTimestamp = true;
    bool demuxEnded = false;
    juce::int64 preroll = 0;

    struct IndexEntry
    {
        int64_t position;
        int64_t timestamp;
        int size;
    };
    std::vector<IndexEntry> scannedIndex;

    // Positions are in samples from the first decoded sample.
    bool anchored = false;          // The first frame has set position 0.
    bool hasTimestamps = false;     // The first frame had a timestamp.
    bool awaitingTimestamp = false; // The next frame's timestamp sets the position.
    bool positionKnown = false;     // nextPosition is exact, not a guess after a failed seek.
    juce::int64 firstTimestamp = 0; // Timestamp of the first decoded sample, in samples.
    juce::int64 nextPosition = 0;   // Position of the next frame to be decoded.

    juce::AudioBuffer<float> frameBuffer;
    juce::int64 frameStart = 0; // Position of frameBuffer's first sample.
    int frameLength = 0;        // Valid samples in frameBuffer; 0 if none.

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (FFmpegAudioFormatReader)
};
} // namespace

//==============================================================================
FFmpegAudioFormat::FFmpegAudioFormat()
    : juce::AudioFormat ("FFmpeg",
                         { ".mp3", ".flac", ".wav", ".aif", ".aiff", ".aifc", ".ogg", ".oga", ".opus", ".m4a", ".m4b",
                           ".mp4", ".aac", ".wma" })
{
    av_log_set_level (AV_LOG_ERROR);
}

FFmpegAudioFormat::~FFmpegAudioFormat() = default;

juce::Array<int> FFmpegAudioFormat::getPossibleSampleRates() { return {}; }
juce::Array<int> FFmpegAudioFormat::getPossibleBitDepths() { return {}; }
bool FFmpegAudioFormat::canDoStereo() { return true; }
bool FFmpegAudioFormat::canDoMono() { return true; }
bool FFmpegAudioFormat::isCompressed() { return true; }

juce::AudioFormatReader* FFmpegAudioFormat::createReaderFor (juce::InputStream* sourceStream,
                                                             bool deleteStreamIfOpeningFails)
{
    if (sourceStream == nullptr)
        return nullptr;

    auto reader = std::make_unique<FFmpegAudioFormatReader> (sourceStream);

    if (reader->isOpen())
        return reader.release();

    if (! deleteStreamIfOpeningFails)
        reader->input = nullptr; // The base class would otherwise delete it.

    return nullptr;
}

std::unique_ptr<juce::AudioFormatWriter> FFmpegAudioFormat::createWriterFor (std::unique_ptr<juce::OutputStream>&,
                                                                             const juce::AudioFormatWriterOptions&)
{
    return nullptr;
}

void FFmpegAudioFormat::silenceLog() { av_log_set_level (AV_LOG_QUIET); }

juce::String FFmpegAudioFormat::readChapters (const juce::File& file, juce::Array<Chapter>& chapters)
{
    chapters.clear();
    juce::FileInputStream input (file);
    if (! input.openedOk())
        return "Cannot open file: " + file.getFullPathName();

    auto* buffer = static_cast<uint8_t*> (av_malloc (ioBufferSize));
    auto* io = avio_alloc_context (buffer, ioBufferSize, 0, &input, readPacket, nullptr, seekStream);
    if (io == nullptr)
    {
        av_free (buffer);
        return "Out of memory";
    }
    auto* format = avformat_alloc_context();
    if (format == nullptr)
    {
        av_freep (&io->buffer);
        avio_context_free (&io);
        return "Out of memory";
    }
    format->pb = io;
    format->flags |= AVFMT_FLAG_CUSTOM_IO;

    juce::String error;
    // Chapters come from the header, so the streams needn't be probed.
    if (avformat_open_input (&format, file.getFileName().toRawUTF8(), nullptr, nullptr) < 0)
    {
        error = "Unsupported or unreadable file: " + file.getFullPathName();
    }
    else
    {
        const auto total =
            format->duration != AV_NOPTS_VALUE ? static_cast<double> (format->duration) / AV_TIME_BASE : -1.0;
        for (unsigned int i = 0; i < format->nb_chapters; ++i)
        {
            const auto* chapter = format->chapters[i];
            Chapter entry;
            entry.start = static_cast<double> (chapter->start) * av_q2d (chapter->time_base);
            entry.end = chapter->end != AV_NOPTS_VALUE && chapter->end > chapter->start
                            ? static_cast<double> (chapter->end) * av_q2d (chapter->time_base)
                            : -1.0;
            if (const auto* title = av_dict_get (chapter->metadata, "title", nullptr, 0))
                entry.title = juce::String::fromUTF8 (title->value).trim();
            chapters.add (entry);
        }
        // Chapters without an end run to the next one, or the end of the file.
        for (int i = 0; i < chapters.size(); ++i)
            if (chapters.getReference (i).end < 0.0)
                chapters.getReference (i).end = i + 1 < chapters.size() ? chapters[i + 1].start : total;
        avformat_close_input (&format);
    }

    if (format != nullptr)
        avformat_free_context (format);
    av_freep (&io->buffer);
    avio_context_free (&io);
    return error;
}
} // namespace anomp
