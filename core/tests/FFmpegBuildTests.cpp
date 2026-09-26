// Checks that third_party/ffmpeg was built as PLAN.md §4.3 requires:
// LGPL, every planned demuxer and decoder, and no encoders.

#include <catch2/catch_test_macros.hpp>

extern "C"
{
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libswresample/swresample.h>
}

#include <string_view>

TEST_CASE ("FFmpeg is licensed under the LGPL", "[ffmpeg]")
{
    CHECK (std::string_view (avcodec_license()).starts_with ("LGPL"));
    CHECK (std::string_view (avformat_license()).starts_with ("LGPL"));
    CHECK (std::string_view (swresample_license()).starts_with ("LGPL"));
}

TEST_CASE ("FFmpeg has a demuxer for every planned container", "[ffmpeg]")
{
    for (const auto* name : { "mp3", "flac", "wav", "aiff", "ogg", "mov", "asf", "aac" })
    {
        INFO ("demuxer " << name);
        CHECK (av_find_input_format (name) != nullptr);
    }
}

TEST_CASE ("FFmpeg has a decoder for every planned codec", "[ffmpeg]")
{
    for (const auto id : { AV_CODEC_ID_MP3, AV_CODEC_ID_FLAC, AV_CODEC_ID_VORBIS, AV_CODEC_ID_OPUS,
                           AV_CODEC_ID_AAC, AV_CODEC_ID_ALAC, AV_CODEC_ID_WMAV1, AV_CODEC_ID_WMAV2,
                           AV_CODEC_ID_WMAPRO, AV_CODEC_ID_WMALOSSLESS, AV_CODEC_ID_PCM_S16LE,
                           AV_CODEC_ID_PCM_S16BE, AV_CODEC_ID_PCM_S24LE, AV_CODEC_ID_PCM_S24BE,
                           AV_CODEC_ID_PCM_F32LE })
    {
        INFO ("decoder " << avcodec_get_name (id));
        CHECK (avcodec_find_decoder (id) != nullptr);
    }
}

TEST_CASE ("FFmpeg contains no encoders", "[ffmpeg]")
{
    void* iterator = nullptr;

    while (const auto* codec = av_codec_iterate (&iterator))
    {
        INFO ("codec " << codec->name);
        CHECK_FALSE (av_codec_is_encoder (codec));
    }
}

TEST_CASE ("FFmpeg resampler can be allocated", "[ffmpeg]")
{
    auto* context = swr_alloc();
    REQUIRE (context != nullptr);
    swr_free (&context);
    CHECK (context == nullptr);
}
