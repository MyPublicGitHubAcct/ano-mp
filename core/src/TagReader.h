#pragma once

#include <juce_audio_formats/juce_audio_formats.h>

#include <limits>
#include <utility>
#include <vector>

namespace TagLib
{
class IOStream;
}

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

    // Classical works (PLAN.md O6): the work a track is a movement of, the
    // movement's own name and number, and who composed and conducted it.
    juce::String work, movementName, composer, conductor;
    int movementNumber = 0, movementTotal = 0;

    // The release date as tagged, as precise as it is ("2004-05-01",
    // "2004-05" or "2004"), and the original release's where the file says.
    juce::String date, originalDate;

    // A rating from the tags (PLAN.md F3), 1 to 100, 0 if none: the first
    // ID3v2 POPM frame's (as whole stars), else FMPS_RATING (0 to 1), else
    // RATING (0 to 100, or 1 to 5 stars), else MP4's rate.
    int rating = 0;

    // Marked as part of a compilation (ID3v2 TCMP, MP4 cpil, Vorbis
    // COMPILATION), and the credited artists of a multi-valued ARTISTS tag
    // (PLAN.md F11), joined with "; ".
    bool compilation = false;
    juce::String artists;

    // Only with TagParts::lyrics: unsynced lyrics, and synced ones as LRC
    // text ("[mm:ss.xx]line" per line) from an ID3v2 SYLT frame.
    juce::String lyrics, syncedLyrics;

    // Only with TagParts::chapters: a cue sheet embedded as a CUESHEET tag,
    // and the chapters the container records (see FFmpegAudioFormat).
    juce::String cueSheet;
    struct Chapter
    {
        double start = 0.0, end = -1.0; // Seconds; -1 for the end of the file.
        juce::String title;
    };
    juce::Array<Chapter> chapters;
};

/** What readTags copies out besides the tags themselves, as flags. */
struct TagParts
{
    static constexpr int picture = 1, lyrics = 2, chapters = 4;
};

/** Reads `file`'s tags with TagLib, opening it read-only. The embedded
    picture, the lyrics and the chapters are read only when `parts` (a
    combination of TagParts) asks for them. If TagLib reports no duration,
    the audio properties come from a reader created by `formats` instead; a
    file with a duration from neither is an error.

    Safe to call from any thread, concurrently. Returns an error message,
    or an empty string on success (then `result` is filled). */
juce::String readTags (const juce::File& file, int parts, juce::AudioFormatManager& formats, TrackTags& result);

/** TagLib's part of readTags, over any stream (the tag fuzzer's way in,
    PLAN.md H5): the tags and TagLib's audio properties, with no fallback to
    the decoder and no chapters from the container. A stream TagLib doesn't
    recognise leaves `result` empty and isn't an error. */
juce::String readTags (TagLib::IOStream& stream, int parts, TrackTags& result);

/** Everything a file says about itself, for a "Get Info" view (PLAN.md
    F16): every tag field TagLib reads, every embedded picture, the kinds of
    tag it carries, and the format as the decoder sees it. */
struct FileInfo
{
    /** Each value of each field, in the tag's order; several values of one
        field are separate entries. */
    std::vector<std::pair<juce::String, juce::String>> fields;

    struct Picture
    {
        juce::String type, mimeType, description;
        juce::MemoryBlock data;
    };
    std::vector<Picture> pictures;

    /** e.g. "ID3v2.4", "ID3v1", "Xiph comment", "MP4". */
    juce::StringArray tagTypes;

    // As FFmpegAudioFormat's reader reports them (the same facts as the
    // player's signal path).
    juce::String codec;
    bool lossless = false;
    int bitsPerSample = 0, bitrateKbps = 0, channels = 0;
    double sampleRate = 0.0, durationSeconds = 0.0;
    juce::int64 fileSize = 0;
};

/** Reads `file`'s FileInfo read-only; any thread. Returns an error message,
    or an empty string on success. A file the decoder can't open still
    reports its tags. */
juce::String readFileInfo (const juce::File& file, juce::AudioFormatManager& formats, FileInfo& result);

/** TagLib's part of readFileInfo, over any stream: the fields, pictures, tag
    types and TagLib's audio properties. `tagged` says whether TagLib
    recognised the stream. */
juce::String readFileInfo (TagLib::IOStream& stream, FileInfo& result, bool& tagged);
} // namespace anomp
