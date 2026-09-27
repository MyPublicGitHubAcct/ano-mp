#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "anomp/anomp.h"

#include <juce_core/juce_core.h>

#include <fileref.h>
#include <tpropertymap.h>

#include <cmath>
#include <cstring>
#include <memory>
#include <string>
#include <string_view>

namespace
{
juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }

struct TagsDeleter
{
    void operator() (anomp_tags* tags) const { anomp_tags_free (tags); }
};

using Tags = std::unique_ptr<anomp_tags, TagsDeleter>;

Tags readTags (const juce::File& file, int flags = ANOMP_TAGS_PICTURE)
{
    char error[512] = "unchanged";
    Tags tags (anomp_read_tags (file.getFullPathName().toRawUTF8(), flags, error, sizeof (error)));
    INFO (error);
    REQUIRE (tags != nullptr);
    CHECK (std::string_view (error).empty());
    return tags;
}

std::string readError (const char* path)
{
    char error[512] = "";
    Tags tags (anomp_read_tags (path, ANOMP_TAGS_PICTURE, error, sizeof (error)));
    CHECK (tags == nullptr);
    return error;
}

bool isPng (const anomp_tags& tags)
{
    return tags.picture != nullptr && tags.picture_size >= 8 && std::memcmp (tags.picture, "\x89PNG\r\n\x1a\n", 8) == 0;
}

// Must match scripts/make-test-fixtures.py.
constexpr size_t coverSize = 463;
constexpr auto fixtureSeconds = 22371 / 44100.0;

/** A copy of a fixture in the temp directory, deleted afterwards. */
struct TempCopy
{
    explicit TempCopy (const char* fixture) : temp ("." + fixtureFile (fixture).getFileExtension().substring (1))
    {
        REQUIRE (fixtureFile (fixture).copyFileTo (temp.getFile()));
    }

    juce::File file() const { return temp.getFile(); }

    juce::TemporaryFile temp;
};

TagLib::ByteVector fakeImage (const char* magic, size_t magicSize, size_t size)
{
    TagLib::ByteVector data (static_cast<unsigned int> (size), 'x');
    std::memcpy (data.data(), magic, magicSize);
    return data;
}
} // namespace

TEST_CASE ("Tags from an ID3v2.3 MP3 written by FFmpeg", "[tags]")
{
    const auto tags = readTags (fixtureFile ("tagged-id3v23.mp3"));

    CHECK (std::string_view (tags->title) == "Café Déjà Vu");
    CHECK (std::string_view (tags->artist) == "Ano Artist");
    CHECK (std::string_view (tags->album) == "東京 Sessions");
    CHECK (std::string_view (tags->album_artist) == "Various Artists");
    CHECK (std::string_view (tags->genre) == "Electronic");
    CHECK (tags->track_number == 3);
    CHECK (tags->track_total == 12);
    CHECK (tags->disc_number == 1);
    CHECK (tags->disc_total == 2);
    CHECK (tags->year == 2004);

    // TagLib's MP3 length includes the encoder delay and padding.
    CHECK (tags->duration == Catch::Approx (fixtureSeconds).margin (0.05));
    CHECK (tags->sample_rate == 44100);
    CHECK (tags->channels == 2);
    CHECK (tags->bitrate_kbps > 0);

    // ID3v2 keeps the recording ID in a UFID frame, which FFmpeg can't write.
    CHECK (std::string_view (tags->musicbrainz_recording_id).empty());
    CHECK (std::string_view (tags->musicbrainz_release_id) == "a1b2c3d4-0000-4000-8000-000000000001");
    CHECK (std::string_view (tags->musicbrainz_release_group_id) == "a1b2c3d4-0000-4000-8000-000000000002");
    CHECK (std::string_view (tags->musicbrainz_release_track_id) == "a1b2c3d4-0000-4000-8000-000000000003");
    CHECK (std::string_view (tags->musicbrainz_artist_id) == "a1b2c3d4-0000-4000-8000-000000000004");
    CHECK (std::string_view (tags->musicbrainz_album_artist_id) == "89ad4ac3-39f7-470e-963a-56509c546377");

    CHECK (isPng (*tags));
    CHECK (tags->picture_size == coverSize);
    CHECK (std::string_view (tags->picture_mime_type) == "image/png");
}

TEST_CASE ("Tags from a FLAC file written by FFmpeg", "[tags]")
{
    const auto tags = readTags (fixtureFile ("tagged-vorbis.flac"));

    CHECK (std::string_view (tags->title) == "Café Déjà Vu");
    CHECK (std::string_view (tags->artist) == "Ano Artist");
    CHECK (std::string_view (tags->album) == "東京 Sessions");
    CHECK (std::string_view (tags->album_artist) == "Various Artists");
    CHECK (std::string_view (tags->genre) == "Electronic");
    // Numbers and totals in separate comments this time, and a full date.
    CHECK (tags->track_number == 3);
    CHECK (tags->track_total == 12);
    CHECK (tags->disc_number == 1);
    CHECK (tags->disc_total == 2);
    CHECK (tags->year == 2004);

    CHECK (tags->duration == Catch::Approx (fixtureSeconds).margin (0.001));
    CHECK (tags->sample_rate == 44100);
    CHECK (tags->channels == 2);

    CHECK (std::string_view (tags->musicbrainz_recording_id) == "a1b2c3d4-0000-4000-8000-000000000000");
    CHECK (std::string_view (tags->musicbrainz_release_id) == "a1b2c3d4-0000-4000-8000-000000000001");
    CHECK (std::string_view (tags->musicbrainz_album_artist_id) == "89ad4ac3-39f7-470e-963a-56509c546377");

    CHECK (isPng (*tags));
    CHECK (tags->picture_size == coverSize);
    CHECK (std::string_view (tags->picture_mime_type) == "image/png");
}

TEST_CASE ("Tags leave out the picture unless asked", "[tags]")
{
    const auto tags = readTags (fixtureFile ("tagged-vorbis.flac"), 0);

    CHECK (std::string_view (tags->title) == "Café Déjà Vu");
    CHECK (tags->picture == nullptr);
    CHECK (tags->picture_size == 0);
    CHECK (std::string_view (tags->picture_mime_type).empty());
}

TEST_CASE ("Reading tags doesn't modify the file", "[tags]")
{
    const auto file = fixtureFile ("tagged-id3v23.mp3");
    juce::MemoryBlock before;
    REQUIRE (file.loadFileAsData (before));
    const auto modified = file.getLastModificationTime();

    readTags (file);

    juce::MemoryBlock after;
    REQUIRE (file.loadFileAsData (after));
    CHECK (after == before);
    CHECK (file.getLastModificationTime() == modified);
}

TEST_CASE ("Untagged files still report their audio properties", "[tags]")
{
    struct Untagged
    {
        const char* file;
        int sampleRate;
        int channels;
        double seconds;
    };

    const auto untagged = GENERATE (values<Untagged> ({
        { "wav-s16-44k.wav", 44100, 2, 22371 / 44100.0 },
        { "wav-mono-48k.wav", 48000, 1, 24321 / 48000.0 },
        { "aiff-s16-44k.aiff", 44100, 2, 22371 / 44100.0 },
        { "flac-44k.flac", 44100, 2, 22371 / 44100.0 },
        { "alac-44k.m4a", 44100, 2, 22371 / 44100.0 },
        { "mp3-44k.mp3", 44100, 2, 22371 / 44100.0 },
        { "mp3-vbr-44k.mp3", 44100, 2, 22371 / 44100.0 },
        { "mp3-noheader-44k.mp3", 44100, 2, 24192 / 44100.0 },
        { "aac-44k.m4a", 44100, 2, 22371 / 44100.0 },
        { "aac-adts-44k.aac", 44100, 2, 23552 / 44100.0 },
        { "vorbis-44k.ogg", 44100, 2, 22371 / 44100.0 },
        { "opus-48k.opus", 48000, 2, 24321 / 48000.0 },
        { "wma-44k.wma", 44100, 2, 20480 / 44100.0 },
        { "flac-long-48k-mono.flac", 48000, 1, 192321 / 48000.0 },
    }));

    INFO (untagged.file);
    const auto tags = readTags (fixtureFile (untagged.file));

    CHECK (std::string_view (tags->title).empty());
    CHECK (std::string_view (tags->artist).empty());
    CHECK (std::string_view (tags->album).empty());
    CHECK (tags->track_number == 0);
    CHECK (tags->year == 0);
    CHECK (tags->picture == nullptr);
    CHECK (tags->sample_rate == untagged.sampleRate);
    CHECK (tags->channels == untagged.channels);
    // Lossy headers are approximate; the player measures exactly on load.
    CHECK (tags->duration == Catch::Approx (untagged.seconds).margin (0.05));
}

TEST_CASE ("Tags round-trip through every format", "[tags]")
{
    struct Format
    {
        const char* fixture;
        bool multipleArtists; // ASF keeps one author string.
        bool pictureTypes;    // MP4 cover atoms have no type: the first one wins.
    };

    const auto format = GENERATE (values<Format> ({
        { "wav-s16-44k.wav", true, true },
        { "aiff-s16-44k.aiff", true, true },
        { "flac-44k.flac", true, true },
        { "alac-44k.m4a", true, false },
        { "mp3-44k.mp3", true, true },
        { "aac-44k.m4a", true, false },
        { "aac-adts-44k.aac", true, true },
        { "vorbis-44k.ogg", true, true },
        { "opus-48k.opus", true, true },
        { "wma-44k.wma", false, true },
    }));
    const auto* fixture = format.fixture;
    INFO (fixture);
    TempCopy copy (fixture);

    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (! ref.isNull());

        TagLib::PropertyMap properties;
        properties["TITLE"] = TagLib::String ("Straße ♪", TagLib::String::UTF8);
        properties["ARTIST"] = TagLib::StringList ({ "First", "Second" });
        properties["ALBUM"] = TagLib::String ("Album");
        properties["ALBUMARTIST"] = TagLib::String ("Album Artist");
        properties["GENRE"] = TagLib::String ("Jazz");
        properties["TRACKNUMBER"] = TagLib::String ("7/9");
        properties["DISCNUMBER"] = TagLib::String ("2/3");
        properties["DATE"] = TagLib::String ("1999-12-31");
        properties["MUSICBRAINZ_TRACKID"] = TagLib::String ("00000000-0000-4000-8000-00000000000a");
        properties["MUSICBRAINZ_ALBUMID"] = TagLib::String ("00000000-0000-4000-8000-00000000000b");
        properties["MUSICBRAINZ_RELEASEGROUPID"] = TagLib::String ("00000000-0000-4000-8000-00000000000c");
        properties["MUSICBRAINZ_RELEASETRACKID"] = TagLib::String ("00000000-0000-4000-8000-00000000000d");
        properties["MUSICBRAINZ_ARTISTID"] = TagLib::String ("00000000-0000-4000-8000-00000000000e");
        properties["MUSICBRAINZ_ALBUMARTISTID"] = TagLib::String ("00000000-0000-4000-8000-00000000000f");
        const auto unsupported = ref.setProperties (properties);
        CHECK (unsupported.isEmpty());

        // A back cover first, so the reader must pick the front one.
        ref.setComplexProperties ("PICTURE", {
                                                 { { "data", fakeImage ("\xff\xd8\xff", 3, 100) },
                                                   { "mimeType", TagLib::String ("image/jpeg") },
                                                   { "pictureType", TagLib::String ("Back Cover") } },
                                                 { { "data", fakeImage ("\x89PNG\r\n\x1a\n", 8, 200) },
                                                   { "mimeType", TagLib::String ("image/png") },
                                                   { "pictureType", TagLib::String ("Front Cover") } },
                                             });
        REQUIRE (ref.save());
    }

    const auto tags = readTags (copy.file());

    CHECK (std::string_view (tags->title) == "Straße ♪");
    CHECK (std::string_view (tags->artist) == (format.multipleArtists ? "First; Second" : "First Second"));
    CHECK (std::string_view (tags->album) == "Album");
    CHECK (std::string_view (tags->album_artist) == "Album Artist");
    CHECK (std::string_view (tags->genre) == "Jazz");
    CHECK (tags->track_number == 7);
    CHECK (tags->track_total == 9);
    CHECK (tags->disc_number == 2);
    CHECK (tags->disc_total == 3);
    CHECK (tags->year == 1999);
    CHECK (std::string_view (tags->musicbrainz_recording_id) == "00000000-0000-4000-8000-00000000000a");
    CHECK (std::string_view (tags->musicbrainz_release_id) == "00000000-0000-4000-8000-00000000000b");
    CHECK (std::string_view (tags->musicbrainz_release_group_id) == "00000000-0000-4000-8000-00000000000c");
    CHECK (std::string_view (tags->musicbrainz_release_track_id) == "00000000-0000-4000-8000-00000000000d");
    CHECK (std::string_view (tags->musicbrainz_artist_id) == "00000000-0000-4000-8000-00000000000e");
    CHECK (std::string_view (tags->musicbrainz_album_artist_id) == "00000000-0000-4000-8000-00000000000f");

    if (format.pictureTypes)
    {
        CHECK (isPng (*tags));
        CHECK (tags->picture_size == 200);
        CHECK (std::string_view (tags->picture_mime_type) == "image/png");
    }
    else
    {
        CHECK (tags->picture_size == 100);
        CHECK (std::string_view (tags->picture_mime_type) == "image/jpeg");
    }
    CHECK (tags->sample_rate > 0);
}

TEST_CASE ("ReplayGain tags in every format", "[tags][replaygain]")
{
    const auto* fixture =
        GENERATE ("wav-s16-44k.wav", "aiff-s16-44k.aiff", "flac-44k.flac", "alac-44k.m4a", "mp3-44k.mp3", "aac-44k.m4a",
                  "aac-adts-44k.aac", "vorbis-44k.ogg", "opus-48k.opus", "wma-44k.wma");
    INFO (fixture);
    TempCopy copy (fixture);

    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (! ref.isNull());
        auto properties = ref.properties();
        properties["REPLAYGAIN_TRACK_GAIN"] = TagLib::String ("-6.54 dB");
        properties["REPLAYGAIN_TRACK_PEAK"] = TagLib::String ("0.988");
        properties["REPLAYGAIN_ALBUM_GAIN"] = TagLib::String ("+1.5 dB");
        properties["REPLAYGAIN_ALBUM_PEAK"] = TagLib::String ("1.25");
        CHECK (ref.setProperties (properties).isEmpty());
        REQUIRE (ref.save());
    }

    const auto tags = readTags (copy.file(), 0);
    CHECK (tags->replaygain_track_gain == Catch::Approx (-6.54));
    CHECK (tags->replaygain_track_peak == Catch::Approx (0.988));
    CHECK (tags->replaygain_album_gain == Catch::Approx (1.5));
    CHECK (tags->replaygain_album_peak == Catch::Approx (1.25));
}

TEST_CASE ("ReplayGain from Opus R128 gains, and nonsense left out", "[tags][replaygain]")
{
    TempCopy copy ("opus-48k.opus");

    const auto write = [&] (std::initializer_list<std::pair<const char*, const char*>> values)
    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (! ref.isNull());
        TagLib::PropertyMap properties;
        for (const auto& [key, value] : values)
            properties[key] = TagLib::String (value);
        CHECK (ref.setProperties (properties).isEmpty());
        REQUIRE (ref.save());
    };

    // Q7.8 relative to -23 LUFS: -1792 / 256 = -7 dB, +5 dB to ReplayGain's level.
    write ({ { "R128_TRACK_GAIN", "-1792" }, { "R128_ALBUM_GAIN", "+512" } });
    auto tags = readTags (copy.file(), 0);
    CHECK (tags->replaygain_track_gain == Catch::Approx (-2.0));
    CHECK (tags->replaygain_album_gain == Catch::Approx (7.0));
    CHECK (std::isnan (tags->replaygain_track_peak));
    CHECK (std::isnan (tags->replaygain_album_peak));

    // ReplayGain tags win over R128 ones; unreadable values are left out.
    write ({ { "R128_TRACK_GAIN", "-1792" },
             { "REPLAYGAIN_TRACK_GAIN", "-3 dB" },
             { "R128_ALBUM_GAIN", "-" },
             { "REPLAYGAIN_TRACK_PEAK", "-0.5" },
             { "REPLAYGAIN_ALBUM_PEAK", "loud" } });
    tags = readTags (copy.file(), 0);
    CHECK (tags->replaygain_track_gain == Catch::Approx (-3.0));
    CHECK (std::isnan (tags->replaygain_album_gain));
    CHECK (std::isnan (tags->replaygain_track_peak));
    CHECK (std::isnan (tags->replaygain_album_peak));

    // None at all.
    const auto untagged = readTags (fixtureFile ("flac-44k.flac"), 0);
    CHECK (std::isnan (untagged->replaygain_track_gain));
    CHECK (std::isnan (untagged->replaygain_album_gain));
}

TEST_CASE ("Tag reading errors", "[tags][c-api]")
{
    CHECK (readError (nullptr) == "Null path");
    CHECK (readError ("fixtures/flac-44k.flac").starts_with ("Path is not absolute"));
    CHECK (readError (fixtureFile ("missing.flac").getFullPathName().toRawUTF8()).starts_with ("File not found"));

    juce::TemporaryFile notAudio (".mp3");
    REQUIRE (notAudio.getFile().replaceWithText ("not an MP3 file"));
    CHECK (readError (notAudio.getFile().getFullPathName().toRawUTF8()).starts_with ("Unsupported or unreadable file"));

    // A null error buffer is allowed, and so is freeing null.
    CHECK (anomp_read_tags (nullptr, 0, nullptr, 0) == nullptr);
    anomp_tags_free (nullptr);
}
