#pragma once

#include <juce_audio_formats/juce_audio_formats.h>

#include <limits>

namespace anomp
{
/** A file's tags and audio properties. Strings are empty and numbers 0 when
    the file doesn't say; fields with several values join them with "; ". */
struct TrackTags
{
    juce::String title, artist, album, albumArtist, genre;
    int trackNumber = 0, trackTotal = 0, discNumber = 0, discTotal = 0, year = 0;

    // From the headers, so lossy files can be off by tens of milliseconds
    // (e.g. MP3 encoder delay and padding); the player measures exactly on load.
    double durationSeconds = 0.0;
    int sampleRate = 0, channels = 0, bitrateKbps = 0;

    // ReplayGain: gains in dB relative to ReplayGain's reference level
    // (Opus R128 gains are converted), peaks as linear sample values; NaN
    // when the file doesn't say.
    double trackGainDb = std::numeric_limits<double>::quiet_NaN(), trackPeak = std::numeric_limits<double>::quiet_NaN(),
           albumGainDb = std::numeric_limits<double>::quiet_NaN(), albumPeak = std::numeric_limits<double>::quiet_NaN();

    // MusicBrainz identifiers, named after the entity they identify (Picard's
    // "track id" is the recording, its "album id" the release).
    juce::String musicBrainzRecordingId, musicBrainzReleaseId, musicBrainzReleaseGroupId, musicBrainzReleaseTrackId,
        musicBrainzArtistId, musicBrainzAlbumArtistId;

    juce::MemoryBlock picture; // Front cover if marked, else the first picture.
    juce::String pictureMimeType;
};

/** Reads `file`'s tags with TagLib, opening it read-only. The embedded
    picture is copied only if `includePicture`. If TagLib reports no duration,
    the audio properties come from a reader created by `formats` instead; a
    file with a duration from neither is an error.

    Safe to call from any thread, concurrently. Returns an error message,
    or an empty string on success (then `result` is filled). */
juce::String readTags (const juce::File& file,
                       bool includePicture,
                       juce::AudioFormatManager& formats,
                       TrackTags& result);
} // namespace anomp
