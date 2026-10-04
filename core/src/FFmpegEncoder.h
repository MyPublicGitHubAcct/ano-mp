#pragma once

#include "Recorder.h"

#include <memory>

namespace anomp
{
/** Opens `file` (replacing it) as a recording in `format`, encoded by
    FFmpeg (PLAN.md X6), from stereo samples at `sampleRate`. A lossy
    format whose encoder can't take that rate (MP3 above 48 kHz, AAC above
    96 kHz) is resampled to the highest it can. Returns null and sets
    `error` on failure. Recorder's EncoderFactory in the app. */
std::unique_ptr<RecordingEncoder> openFFmpegEncoder (const juce::File& file,
                                                     const RecordingFormat& format,
                                                     double sampleRate,
                                                     RecordingError& error);

/** Whether this FFmpeg build has `kind`'s encoder and muxer
    (scripts/build-ffmpeg.sh; MP3 needs LAME). */
bool canEncode (RecordingFormat::Kind kind);
} // namespace anomp
