#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "anomp/anomp.h"
#include "FormatRegistry.h"
#include "TagReader.h"

#include <juce_core/juce_core.h>

#include <chapterframe.h>
#include <fileref.h>
#include <id3v2tag.h>
#include <mp4file.h>
#include <mpegfile.h>
#include <popularimeterframe.h>
#include <synchronizedlyricsframe.h>
#include <tbytevectorstream.h>
#include <textidentificationframe.h>
#include <tpropertymap.h>

#include <cmath>
#include <cstring>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

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

TEST_CASE ("Classical work tags and full dates", "[tags][classical]")
{
    const auto* fixture = GENERATE ("flac-44k.flac", "mp3-44k.mp3", "alac-44k.m4a");
    INFO (fixture);
    TempCopy copy (fixture);
    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (! ref.isNull());
        auto properties = ref.properties();
        properties["WORK"] = TagLib::String ("Symphony No. 5 in C minor, Op. 67");
        properties["MOVEMENTNAME"] = TagLib::String ("Allegro con brio");
        properties["MOVEMENTNUMBER"] = TagLib::String ("1");
        properties["COMPOSER"] = TagLib::String ("Ludwig van Beethoven");
        properties["CONDUCTOR"] = TagLib::String ("Carlos Kleiber");
        properties["DATE"] = TagLib::String ("1975-03-01T00:00:00");
        properties["ORIGINALDATE"] = TagLib::String ("1975-03");
        CHECK (ref.setProperties (properties).isEmpty());
        REQUIRE (ref.save());
    }

    const auto tags = readTags (copy.file(), 0);
    CHECK (std::string_view (tags->work) == "Symphony No. 5 in C minor, Op. 67");
    CHECK (std::string_view (tags->movement_name) == "Allegro con brio");
    CHECK (tags->movement_number == 1);
    CHECK (std::string_view (tags->composer) == "Ludwig van Beethoven");
    CHECK (std::string_view (tags->conductor) == "Carlos Kleiber");
    CHECK (std::string_view (tags->date) == "1975-03-01");
    CHECK (std::string_view (tags->original_date) == "1975-03");
    CHECK (tags->year == 1975);
}

TEST_CASE ("A Vorbis MOVEMENT tag is a number or a name", "[tags][classical]")
{
    TempCopy copy ("flac-44k.flac");
    const auto write = [&] (const char* movement)
    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        TagLib::PropertyMap properties;
        properties["MOVEMENT"] = TagLib::String (movement);
        properties["MOVEMENTTOTAL"] = TagLib::String ("4");
        ref.setProperties (properties);
        REQUIRE (ref.save());
    };
    write ("2");
    auto tags = readTags (copy.file(), 0);
    CHECK (tags->movement_number == 2);
    CHECK (tags->movement_total == 4);
    CHECK (std::string_view (tags->movement_name).empty());

    write ("Andante con moto");
    tags = readTags (copy.file(), 0);
    CHECK (tags->movement_number == 0);
    CHECK (std::string_view (tags->movement_name) == "Andante con moto");

    // Untagged: empty.
    const auto untagged = readTags (fixtureFile ("wav-s16-44k.wav"), 0);
    CHECK (std::string_view (untagged->work).empty());
    CHECK (std::string_view (untagged->date).empty());
}

TEST_CASE ("Lyrics are read only when asked, synced ones as LRC", "[tags][lyrics]")
{
    TempCopy copy ("mp3-44k.mp3");
    {
        TagLib::MPEG::File file (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (file.isValid());
        auto* tag = file.ID3v2Tag (true);
        auto properties = tag->properties();
        properties["LYRICS"] = TagLib::String ("First line\nSecond line");
        tag->setProperties (properties);

        auto* synced = new TagLib::ID3v2::SynchronizedLyricsFrame (TagLib::String::UTF8);
        synced->setTimestampFormat (TagLib::ID3v2::SynchronizedLyricsFrame::AbsoluteMilliseconds);
        synced->setType (TagLib::ID3v2::SynchronizedLyricsFrame::Lyrics);
        synced->setSynchedText ({ { 1500, "First line" }, { 65250, "Second line" } });
        tag->addFrame (synced);
        REQUIRE (file.save());
    }

    const auto without = readTags (copy.file(), 0);
    CHECK (std::string_view (without->lyrics).empty());
    CHECK (std::string_view (without->synced_lyrics).empty());

    const auto tags = readTags (copy.file(), ANOMP_TAGS_LYRICS);
    CHECK (std::string_view (tags->lyrics) == "First line\nSecond line");
    CHECK (std::string_view (tags->synced_lyrics) == "[00:01.50]First line\n[01:05.25]Second line\n");
}

TEST_CASE ("Chapters and cue sheets are read only when asked", "[tags][chapters]")
{
    TempCopy copy ("mp3-44k.mp3");
    {
        TagLib::MPEG::File file (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (file.isValid());
        auto* tag = file.ID3v2Tag (true);
        const auto chapter = [] (const char* id, unsigned int start, unsigned int end, const char* title)
        {
            auto* frame = new TagLib::ID3v2::ChapterFrame (TagLib::ByteVector (id), start, end, 0xffffffff, 0xffffffff);
            auto* text = new TagLib::ID3v2::TextIdentificationFrame ("TIT2", TagLib::String::UTF8);
            text->setText (title);
            frame->addEmbeddedFrame (text);
            return frame;
        };
        tag->addFrame (chapter ("ch0", 0, 200, "Intro"));
        tag->addFrame (chapter ("ch1", 200, 500, "Main theme"));
        REQUIRE (file.save());
    }

    CHECK (readTags (copy.file(), 0)->chapter_count == 0);

    const auto tags = readTags (copy.file(), ANOMP_TAGS_CHAPTERS);
    REQUIRE (tags->chapter_count == 2);
    CHECK (tags->chapters[0].start == Catch::Approx (0.0));
    CHECK (tags->chapters[0].end == Catch::Approx (0.2));
    CHECK (std::string_view (tags->chapters[0].title) == "Intro");
    CHECK (tags->chapters[1].start == Catch::Approx (0.2));
    CHECK (tags->chapters[1].end == Catch::Approx (0.5));
    CHECK (std::string_view (tags->chapters[1].title) == "Main theme");

    // A cue sheet in a Vorbis comment comes back as text.
    TempCopy flac ("flac-44k.flac");
    {
        TagLib::FileRef ref (flac.file().getFullPathName().toRawUTF8());
        TagLib::PropertyMap properties;
        properties["CUESHEET"] = TagLib::String ("FILE \"a.flac\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n");
        ref.setProperties (properties);
        REQUIRE (ref.save());
    }
    const auto cued = readTags (flac.file(), ANOMP_TAGS_CHAPTERS);
    CHECK (std::string_view (cued->cuesheet).starts_with ("FILE \"a.flac\" WAVE"));
    CHECK (cued->chapter_count == 0);
    CHECK (cued->chapters == nullptr);
}

TEST_CASE ("Ratings from POPM, FMPS_RATING, RATING and MP4's rate", "[tags][ratings]")
{
    SECTION ("ID3v2 POPM, as whole stars")
    {
        TempCopy copy ("mp3-44k.mp3");
        const auto rate = [&] (int popm)
        {
            {
                TagLib::MPEG::File file (copy.file().getFullPathName().toRawUTF8());
                REQUIRE (file.isValid());
                auto* tag = file.ID3v2Tag (true);
                tag->removeFrames ("POPM");
                auto* frame = new TagLib::ID3v2::PopularimeterFrame();
                frame->setEmail ("someone@example.com");
                frame->setRating (popm);
                tag->addFrame (frame);
                REQUIRE (file.save());
            }
            return readTags (copy.file(), 0)->rating;
        };
        CHECK (rate (1) == 20);
        CHECK (rate (64) == 40);
        CHECK (rate (128) == 60);
        CHECK (rate (196) == 80);
        CHECK (rate (255) == 100);
        CHECK (rate (0) == 0);
    }

    SECTION ("Vorbis comments")
    {
        TempCopy copy ("flac-44k.flac");
        const auto rate = [&] (const char* key, const char* value)
        {
            {
                TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
                TagLib::PropertyMap properties;
                properties[key] = TagLib::String (value);
                ref.setProperties (properties);
                REQUIRE (ref.save());
            }
            return readTags (copy.file(), 0)->rating;
        };
        CHECK (rate ("FMPS_RATING", "0.6") == 60);
        CHECK (rate ("RATING", "4") == 80);
        CHECK (rate ("RATING", "85") == 85);
        CHECK (rate ("RATING", "0.5") == 50);
        CHECK (rate ("RATING", "192") == 80);
        CHECK (rate ("RATING", "lots") == 0);
    }

    SECTION ("MP4's rate atom")
    {
        TempCopy copy ("aac-44k.m4a");
        {
            TagLib::MP4::File file (copy.file().getFullPathName().toRawUTF8());
            REQUIRE (file.isValid());
            file.tag()->setItem ("rate", TagLib::MP4::Item (TagLib::StringList ("60")));
            REQUIRE (file.save());
        }
        CHECK (readTags (copy.file(), 0)->rating == 60);
    }

    CHECK (readTags (fixtureFile ("wav-s16-44k.wav"), 0)->rating == 0);
}

TEST_CASE ("Compilation flags and credited artists", "[tags][artists]")
{
    const auto* fixture = GENERATE ("flac-44k.flac", "mp3-44k.mp3", "aac-44k.m4a", "vorbis-44k.ogg");
    INFO (fixture);
    TempCopy copy (fixture);
    {
        TagLib::FileRef ref (copy.file().getFullPathName().toRawUTF8());
        REQUIRE (! ref.isNull());
        auto properties = ref.properties();
        properties["ARTIST"] = TagLib::String ("Singer feat. Rapper");
        properties["ARTISTS"] = TagLib::StringList ({ "Singer", "Rapper" });
        properties["COMPILATION"] = TagLib::String ("1");
        CHECK (ref.setProperties (properties).isEmpty());
        REQUIRE (ref.save());
    }
    const auto tags = readTags (copy.file(), 0);
    CHECK (tags->compilation == 1);
    CHECK (std::string_view (tags->artists) == "Singer; Rapper");
    CHECK (std::string_view (tags->artist) == "Singer feat. Rapper");

    const auto untagged = readTags (fixtureFile ("wav-s16-44k.wav"), 0);
    CHECK (untagged->compilation == 0);
    CHECK (std::string_view (untagged->artists).empty());
}

TEST_CASE ("File info lists every field, the pictures and the format", "[tags][info]")
{
    struct InfoDeleter
    {
        void operator() (anomp_file_info* info) const { anomp_file_info_free (info); }
    };
    using Info = std::unique_ptr<anomp_file_info, InfoDeleter>;
    const auto read = [] (const juce::File& file)
    {
        char error[512] = "unchanged";
        Info info (anomp_read_file_info (file.getFullPathName().toRawUTF8(), error, sizeof (error)));
        INFO (error);
        REQUIRE (info != nullptr);
        CHECK (std::string_view (error).empty());
        return info;
    };
    const auto field = [] (const anomp_file_info& info, std::string_view key)
    {
        std::vector<std::string> values;
        for (int i = 0; i < info.field_count; ++i)
            if (key == info.fields[i].key)
                values.emplace_back (info.fields[i].value);
        return values;
    };

    const auto mp3 = read (fixtureFile ("tagged-id3v23.mp3"));
    CHECK (field (*mp3, "TITLE") == std::vector<std::string> { "Café Déjà Vu" });
    CHECK (std::string_view (mp3->tag_types).find ("ID3v2.3") != std::string_view::npos);
    REQUIRE (mp3->picture_count >= 1);
    CHECK (mp3->pictures[0].size == coverSize);
    CHECK (std::string_view (mp3->codec) == "mp3");
    CHECK (mp3->lossless == 0);
    CHECK (mp3->sample_rate == 44100.0);
    CHECK (mp3->bitrate_kbps > 0);
    CHECK (mp3->file_size == fixtureFile ("tagged-id3v23.mp3").getSize());

    const auto flac = read (fixtureFile ("tagged-vorbis.flac"));
    CHECK (std::string_view (flac->codec) == "flac");
    CHECK (flac->lossless == 1);
    CHECK (flac->bits_per_sample == 16);
    CHECK (flac->duration == Catch::Approx (fixtureSeconds).margin (0.001));
    CHECK (std::string_view (flac->tag_types).find ("Vorbis comment") != std::string_view::npos);
    CHECK (field (*flac, "ARTIST") == std::vector<std::string> { "Ano Artist" });

    // Untagged: the format alone.
    const auto wav = read (fixtureFile ("wav-s16-44k.wav"));
    CHECK (wav->field_count == 0);
    CHECK (wav->fields == nullptr);
    CHECK (wav->picture_count == 0);
    CHECK (std::string_view (wav->codec).starts_with ("pcm"));

    char error[256] = "";
    CHECK (anomp_read_file_info ("/no/such/file.flac", error, sizeof (error)) == nullptr);
    CHECK (std::string_view (error).starts_with ("File not found"));
    CHECK (anomp_read_file_info ("relative.flac", error, sizeof (error)) == nullptr);
    anomp_file_info_free (nullptr);
}

TEST_CASE ("Tags from a stream in memory match the file's", "[tags][fuzz]")
{
    // The fuzzer reads tags this way (PLAN.md H5), so it must be the path
    // the file goes through.
    const auto name = GENERATE ("tagged-id3v23.mp3", "tagged-vorbis.flac", "aac-44k.m4a", "opus-48k.opus");
    CAPTURE (name);
    const auto file = fixtureFile (name);
    anomp::FormatRegistry registry;

    anomp::TrackTags fromFile, fromMemory;
    const auto parts = anomp::TagParts::picture | anomp::TagParts::lyrics;
    REQUIRE (anomp::readTags (file, parts, registry.manager(), fromFile).isEmpty());

    juce::MemoryBlock data;
    REQUIRE (file.loadFileAsData (data));
    TagLib::ByteVectorStream stream (
        TagLib::ByteVector (static_cast<const char*> (data.getData()), static_cast<unsigned int> (data.getSize())));
    REQUIRE (anomp::readTags (stream, parts, fromMemory).isEmpty());

    CHECK (fromMemory.title == fromFile.title);
    CHECK (fromMemory.artist == fromFile.artist);
    CHECK (fromMemory.album == fromFile.album);
    CHECK (fromMemory.trackNumber == fromFile.trackNumber);
    CHECK (fromMemory.picture == fromFile.picture);
    CHECK (fromMemory.sampleRate == fromFile.sampleRate);
    CHECK (fromMemory.durationSeconds == Catch::Approx (fromFile.durationSeconds).margin (0.05));

    anomp::FileInfo info;
    bool tagged = false;
    TagLib::ByteVectorStream again (
        TagLib::ByteVector (static_cast<const char*> (data.getData()), static_cast<unsigned int> (data.getSize())));
    REQUIRE (anomp::readFileInfo (again, info, tagged).isEmpty());
    CHECK (tagged);

    // Nothing TagLib knows: no tags, and no error.
    TagLib::ByteVectorStream junk (TagLib::ByteVector ("not audio at all"));
    anomp::TrackTags none;
    CHECK (anomp::readTags (junk, parts, none).isEmpty());
    CHECK (none.title.isEmpty());
}

TEST_CASE ("Inputs the fuzzer found stay harmless", "[tags][fuzz]")
{
    // Each file in fixtures/fuzz is an input that once crashed or tripped a
    // sanitizer in the tag fuzzer (PLAN.md H5); the asan preset runs this
    // with UBSan errors fatal.
    // - tags-shorten-shift.bin: a Shorten header whose Rice code shifted
    //   a signed int too far in TagLib's Shorten reader. TagLib is now built
    //   without the formats FFmpeg can't play (cmake/TagLib.cmake), so it
    //   isn't parsed at all.
    const auto name = GENERATE ("fuzz/tags-shorten-shift.bin");
    CAPTURE (name);
    juce::MemoryBlock data;
    REQUIRE (fixtureFile (name).loadFileAsData (data));
    const TagLib::ByteVector bytes (static_cast<const char*> (data.getData()),
                                    static_cast<unsigned int> (data.getSize()));

    TagLib::ByteVectorStream stream (bytes);
    anomp::TrackTags tags;
    CHECK (anomp::readTags (stream, anomp::TagParts::picture | anomp::TagParts::lyrics, tags).isEmpty());
    CHECK (tags.title.isEmpty());
    CHECK (tags.sampleRate == 0);

    TagLib::ByteVectorStream again (bytes);
    anomp::FileInfo info;
    bool tagged = true;
    anomp::readFileInfo (again, info, tagged);
    CHECK_FALSE (tagged);
}
