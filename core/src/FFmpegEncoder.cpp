#include "FFmpegEncoder.h"

extern "C"
{
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/audio_fifo.h>
#include <libavutil/opt.h>
#include <libswresample/swresample.h>
}

#include <cerrno>
#include <cmath>

namespace anomp
{
namespace
{
/** The muxer, encoder and sample format each format is written with. */
struct Spec
{
    const char* muxer;
    const char* encoder;
    AVSampleFormat sampleFormat;
    int rawBits; ///< bits_per_raw_sample, or 0.
};

Spec specFor (const RecordingFormat& format)
{
    using Kind = RecordingFormat::Kind;
    const auto deep = format.bits >= 24;
    switch (format.kind)
    {
        case Kind::wav:
            if (format.bits == 32)
                return { "wav", "pcm_f32le", AV_SAMPLE_FMT_FLT, 0 };
            return deep ? Spec { "wav", "pcm_s24le", AV_SAMPLE_FMT_S32, 24 }
                        : Spec { "wav", "pcm_s16le", AV_SAMPLE_FMT_S16, 0 };
        case Kind::aiff:
            return deep ? Spec { "aiff", "pcm_s24be", AV_SAMPLE_FMT_S32, 24 }
                        : Spec { "aiff", "pcm_s16be", AV_SAMPLE_FMT_S16, 0 };
        case Kind::flac:
            return deep ? Spec { "flac", "flac", AV_SAMPLE_FMT_S32, 24 }
                        : Spec { "flac", "flac", AV_SAMPLE_FMT_S16, 16 };
        case Kind::alac:
            return deep ? Spec { "ipod", "alac", AV_SAMPLE_FMT_S32P, 24 }
                        : Spec { "ipod", "alac", AV_SAMPLE_FMT_S16P, 16 };
        case Kind::aac: return { "ipod", "aac", AV_SAMPLE_FMT_FLTP, 0 };
        case Kind::mp3: return { "mp3", "libmp3lame", AV_SAMPLE_FMT_FLTP, 0 };
    }
    return { "wav", "pcm_f32le", AV_SAMPLE_FMT_FLT, 0 };
}

const char* formatName (RecordingFormat::Kind kind)
{
    using Kind = RecordingFormat::Kind;
    switch (kind)
    {
        case Kind::wav:  return "WAV";
        case Kind::aiff: return "AIFF";
        case Kind::flac: return "FLAC";
        case Kind::alac: return "Apple Lossless";
        case Kind::aac:  return "AAC";
        case Kind::mp3:  return "MP3";
    }
    return "WAV";
}

RecordingError failure (const juce::String& what, int code)
{
    char text[AV_ERROR_MAX_STRING_SIZE] = {};
    av_strerror (code, text, sizeof (text));
    return { what + ": " + juce::String::fromUTF8 (text), code == AVERROR (ENOSPC) };
}

/** The rate to encode `rate` at: itself if the codec takes it, otherwise
    the highest it takes below it, preferring one that divides it. */
int encoderRate (const AVCodec* codec, int rate)
{
    const void* configs = nullptr;
    int count = 0;
    if (avcodec_get_supported_config (nullptr, codec, AV_CODEC_CONFIG_SAMPLE_RATE, 0, &configs, &count) < 0
        || configs == nullptr || count <= 0)
        return rate;

    const auto* rates = static_cast<const int*> (configs);
    int below = 0, dividing = 0, highest = 0;
    for (int i = 0; i < count; ++i)
    {
        const auto candidate = rates[i];
        if (candidate == rate)
            return rate;
        highest = juce::jmax (highest, candidate);
        if (candidate < rate)
        {
            below = juce::jmax (below, candidate);
            if (rate % candidate == 0)
                dividing = juce::jmax (dividing, candidate);
        }
    }
    return dividing > 0 ? dividing : below > 0 ? below : highest;
}

//==============================================================================
class FFmpegEncoder final : public RecordingEncoder
{
public:
    ~FFmpegEncoder() override
    {
        if (headerWritten && ! finished)
            finish();
        av_packet_free (&packet);
        av_frame_free (&frame);
        if (fifo != nullptr)
            av_audio_fifo_free (fifo);
        if (converted != nullptr)
            av_freep (&converted[0]);
        av_freep (&converted);
        swr_free (&resampler);
        avcodec_free_context (&codec);
        if (format != nullptr)
            avio_closep (&format->pb);
        avformat_free_context (format);
    }

    RecordingError open (const juce::File& file, const RecordingFormat& recording, double sampleRate)
    {
        const auto spec = specFor (recording);
        const auto name = juce::String (formatName (recording.kind));
        const auto* encoder = avcodec_find_encoder_by_name (spec.encoder);
        if (encoder == nullptr)
            return { "This build can't record " + name };

        const auto& path = file.getFullPathName();
        const auto inputRate = static_cast<int> (std::lround (sampleRate));
        if (const auto code = avformat_alloc_output_context2 (&format, nullptr, spec.muxer, path.toRawUTF8());
            code < 0 || format == nullptr)
            return failure ("This build can't write " + name + " files", code < 0 ? code : AVERROR_MUXER_NOT_FOUND);

        stream = avformat_new_stream (format, nullptr);
        codec = avcodec_alloc_context3 (encoder);
        packet = av_packet_alloc();
        frame = av_frame_alloc();
        if (stream == nullptr || codec == nullptr || packet == nullptr || frame == nullptr)
            return { "Out of memory" };

        const auto rate = recording.isLossy() ? encoderRate (encoder, inputRate) : inputRate;
        const AVChannelLayout stereo = AV_CHANNEL_LAYOUT_STEREO;
        codec->sample_rate = rate;
        av_channel_layout_copy (&codec->ch_layout, &stereo);
        codec->sample_fmt = spec.sampleFormat;
        if (spec.rawBits > 0)
            codec->bits_per_raw_sample = spec.rawBits;
        if (recording.isLossy())
            codec->bit_rate = static_cast<int64_t> (recording.bitrateKbps) * 1000;
        codec->time_base = { 1, rate };
        if ((format->oformat->flags & AVFMT_GLOBALHEADER) != 0)
            codec->flags |= AV_CODEC_FLAG_GLOBAL_HEADER;

        if (const auto code = avcodec_open2 (codec, encoder, nullptr); code < 0)
            return failure ("Cannot record " + name + " at " + juce::String (rate) + " Hz", code);
        if (const auto code = avcodec_parameters_from_context (stream->codecpar, codec); code < 0)
            return failure ("Cannot record " + name, code);
        stream->time_base = codec->time_base;

        frameSize = (encoder->capabilities & AV_CODEC_CAP_VARIABLE_FRAME_SIZE) != 0 || codec->frame_size <= 0
                        ? 4096
                        : codec->frame_size;

        // Sample format (and, for a lossy format, rate) conversion; 16-bit
        // samples are dithered.
        if (const auto code = swr_alloc_set_opts2 (&resampler, &codec->ch_layout, codec->sample_fmt, rate, &stereo,
                                                   AV_SAMPLE_FMT_FLTP, inputRate, 0, nullptr);
            code < 0)
            return failure ("Cannot convert the samples", code);
        if (spec.sampleFormat == AV_SAMPLE_FMT_S16 || spec.sampleFormat == AV_SAMPLE_FMT_S16P)
            av_opt_set_int (resampler, "dither_method", SWR_DITHER_TRIANGULAR, 0);
        if (const auto code = swr_init (resampler); code < 0)
            return failure ("Cannot convert the samples", code);

        fifo = av_audio_fifo_alloc (codec->sample_fmt, 2, frameSize * 2);
        frame->nb_samples = frameSize;
        frame->format = codec->sample_fmt;
        frame->sample_rate = rate;
        av_channel_layout_copy (&frame->ch_layout, &codec->ch_layout);
        if (fifo == nullptr || av_frame_get_buffer (frame, 0) < 0)
            return { "Out of memory" };

        if (const auto code = avio_open (&format->pb, path.toRawUTF8(), AVIO_FLAG_WRITE); code < 0)
            return failure ("Cannot create " + file.getFileName(), code);

        AVDictionary* options = nullptr;
        if (recording.kind == RecordingFormat::Kind::wav)
            av_dict_set (&options, "rf64", "auto", 0); // Past 4 GB.
        const auto code = avformat_write_header (format, &options);
        av_dict_free (&options);
        if (code < 0)
            return failure ("Cannot write " + file.getFileName(), code);
        headerWritten = true;
        return {};
    }

    RecordingError write (const float* left, const float* right, int numSamples) override
    {
        const uint8_t* input[] = { reinterpret_cast<const uint8_t*> (left), reinterpret_cast<const uint8_t*> (right) };
        if (auto error = convert (input, numSamples); error.failed())
            return error;
        return encodeFrames (false);
    }

    RecordingError finish() override
    {
        finished = true;
        auto error = convert (nullptr, 0); // What the resampler holds back.
        if (! error.failed())
            error = encodeFrames (true);
        if (! error.failed())
        {
            avcodec_send_frame (codec, nullptr);
            error = writePackets();
        }
        // The trailer goes on whatever happened, so what was written plays.
        if (const auto code = av_write_trailer (format); code < 0 && ! error.failed())
            error = failure ("Cannot finish the file", code);
        if (const auto code = avio_closep (&format->pb); code < 0 && ! error.failed())
            error = failure ("Cannot finish the file", code);
        return error;
    }

private:
    /** Converts `numSamples` input frames (null input flushes) into the FIFO. */
    RecordingError convert (const uint8_t* const* input, int numSamples)
    {
        const auto capacity = swr_get_out_samples (resampler, numSamples);
        if (capacity <= 0)
            return {};
        if (capacity > convertedCapacity)
        {
            if (converted != nullptr)
                av_freep (&converted[0]);
            av_freep (&converted);
            if (av_samples_alloc_array_and_samples (&converted, nullptr, 2, capacity, codec->sample_fmt, 0) < 0)
                return { "Out of memory" };
            convertedCapacity = capacity;
        }
        const auto count = swr_convert (resampler, converted, capacity, input, numSamples);
        if (count < 0)
            return failure ("Cannot convert the samples", count);
        if (count > 0 && av_audio_fifo_write (fifo, reinterpret_cast<void**> (converted), count) < count)
            return { "Out of memory" };
        return {};
    }

    /** Encodes whole frames from the FIFO, and with `last` what remains. */
    RecordingError encodeFrames (bool last)
    {
        while (av_audio_fifo_size (fifo) >= frameSize || (last && av_audio_fifo_size (fifo) > 0))
        {
            if (const auto code = av_frame_make_writable (frame); code < 0)
                return failure ("Cannot encode", code);
            const auto count = av_audio_fifo_read (fifo, reinterpret_cast<void**> (frame->data), frameSize);
            frame->nb_samples = count;
            frame->pts = pts;
            pts += count;
            if (const auto code = avcodec_send_frame (codec, frame); code < 0)
                return failure ("Cannot encode", code);
            if (auto error = writePackets(); error.failed())
                return error;
        }
        return {};
    }

    RecordingError writePackets()
    {
        for (;;)
        {
            const auto received = avcodec_receive_packet (codec, packet);
            if (received == AVERROR (EAGAIN) || received == AVERROR_EOF)
                return {};
            if (received < 0)
                return failure ("Cannot encode", received);
            av_packet_rescale_ts (packet, codec->time_base, stream->time_base);
            packet->stream_index = stream->index;
            const auto code = av_interleaved_write_frame (format, packet);
            if (code < 0)
                return failure ("Cannot write the file", code);
            if (format->pb->error < 0)
                return failure ("Cannot write the file", format->pb->error);
        }
    }

    AVFormatContext* format = nullptr;
    AVStream* stream = nullptr;
    AVCodecContext* codec = nullptr;
    SwrContext* resampler = nullptr;
    AVAudioFifo* fifo = nullptr;
    AVFrame* frame = nullptr;
    AVPacket* packet = nullptr;
    uint8_t** converted = nullptr;
    int convertedCapacity = 0;
    int frameSize = 0;
    int64_t pts = 0;
    bool headerWritten = false, finished = false;
};
} // namespace

std::unique_ptr<RecordingEncoder> openFFmpegEncoder (const juce::File& file,
                                                     const RecordingFormat& format,
                                                     double sampleRate,
                                                     RecordingError& error)
{
    auto encoder = std::make_unique<FFmpegEncoder>();
    error = encoder->open (file, format, sampleRate);
    if (! error.failed())
        return encoder;
    encoder = nullptr; // Closes and removes what it began.
    file.deleteFile();
    return nullptr;
}

bool canEncode (RecordingFormat::Kind kind)
{
    RecordingFormat format;
    format.kind = kind;
    const auto spec = specFor (format);
    return avcodec_find_encoder_by_name (spec.encoder) != nullptr
           && av_guess_format (spec.muxer, nullptr, nullptr) != nullptr;
}
} // namespace anomp
