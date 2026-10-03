#include "TagReader.h"
#include "FFmpegAudioFormat.h"

#include <aifffile.h>
#include <apefile.h>
#include <asffile.h>
#include <fileref.h>
#include <flacfile.h>
#include <id3v2tag.h>
#include <mp4file.h>
#include <mpegfile.h>
#include <oggfile.h>
#include <popularimeterframe.h>
#include <synchronizedlyricsframe.h>
#include <tfilestream.h>
#include <tpropertymap.h>
#include <wavfile.h>
#include <wavpackfile.h>

#include <cmath>
#include <cstring>
#include <limits>

namespace anomp
{
namespace
{
juce::String toJuce (const TagLib::String& text) { return juce::String::fromUTF8 (text.to8Bit (true).c_str()); }

/** All values of a property, joined with "; ". */
juce::String joined (const TagLib::PropertyMap& properties, const char* key)
{
    juce::StringArray values;
    for (const auto& value : properties.value (key))
        if (const auto text = toJuce (value).trim(); text.isNotEmpty())
            values.add (text);
    return values.joinIntoString ("; ");
}

/** A property's first value, or an empty string. */
juce::String first (const TagLib::PropertyMap& properties, const char* key)
{
    const auto values = properties.value (key);
    return values.isEmpty() ? juce::String() : toJuce (values.front()).trim();
}

/** Reads "3/12"-style numbers from `key`, taking the total from one of
    `totalKeys` when the value doesn't carry it. */
void numberAndTotal (const TagLib::PropertyMap& properties,
                     const char* key,
                     std::initializer_list<const char*> totalKeys,
                     int& numberOut,
                     int& totalOut)
{
    const auto text = first (properties, key);
    numberOut = juce::jmax (0, text.upToFirstOccurrenceOf ("/", false, false).getIntValue());
    totalOut = juce::jmax (0, text.fromFirstOccurrenceOf ("/", false, false).getIntValue());

    for (auto* totalKey : totalKeys)
        if (totalOut == 0)
            totalOut = juce::jmax (0, first (properties, totalKey).getIntValue());
}

/** The year from a DATE such as "2004", "2004-05-01" or "2004-05-01T12:00". */
int year (const TagLib::PropertyMap& properties)
{
    const auto digits = first (properties, "DATE").substring (0, 4);
    return digits.length() == 4 && digits.containsOnly ("0123456789") ? digits.getIntValue() : 0;
}

/** A date such as "2004-05-01T12:00" cut to "2004-05-01", "2004-05" or
    "2004", whichever it has; empty unless it starts with a year. */
juce::String dateOf (const TagLib::PropertyMap& properties, const char* key)
{
    const auto text = first (properties, key).upToFirstOccurrenceOf ("T", false, false).trim();
    const auto digits = [&text] (int from, int count)
    {
        const auto part = text.substring (from, from + count);
        return part.length() == count && part.containsOnly ("0123456789");
    };
    if (! digits (0, 4))
        return {};
    if (text[4] != '-' || ! digits (5, 2))
        return text.substring (0, 4);
    if (text[7] != '-' || ! digits (8, 2))
        return text.substring (0, 7);
    return text.substring (0, 10);
}

/** The lyrics tag: LYRICS (ID3v2 USLT, MP4 ©lyr, Vorbis), else one with a
    description (LYRICS:…), else Vorbis's UNSYNCEDLYRICS. */
juce::String lyricsOf (const TagLib::PropertyMap& properties)
{
    if (const auto text = first (properties, "LYRICS"); text.isNotEmpty())
        return text;
    for (const auto& [key, values] : properties)
        if (key.startsWith ("LYRICS:") && ! values.isEmpty())
            return toJuce (values.front()).trim();
    return first (properties, "UNSYNCEDLYRICS");
}

/** The file's ID3v2 tag, if it has one. */
TagLib::ID3v2::Tag* id3v2TagOf (TagLib::File* file)
{
    if (auto* mpeg = dynamic_cast<TagLib::MPEG::File*> (file))
        return mpeg->hasID3v2Tag() ? mpeg->ID3v2Tag() : nullptr;
    if (auto* aiff = dynamic_cast<TagLib::RIFF::AIFF::File*> (file))
        return aiff->hasID3v2Tag() ? aiff->tag() : nullptr;
    if (auto* wav = dynamic_cast<TagLib::RIFF::WAV::File*> (file))
        return wav->hasID3v2Tag() ? wav->ID3v2Tag() : nullptr;
    if (auto* flac = dynamic_cast<TagLib::FLAC::File*> (file))
        return flac->hasID3v2Tag() ? flac->ID3v2Tag() : nullptr;
    return nullptr;
}

/** The first ID3v2 SYLT lyrics with millisecond times, as LRC text. */
juce::String syncedLyricsOf (TagLib::File* file)
{
    auto* tag = id3v2TagOf (file);
    if (tag == nullptr)
        return {};

    using Frame = TagLib::ID3v2::SynchronizedLyricsFrame;
    for (auto* frame : tag->frameList ("SYLT"))
    {
        const auto* sylt = dynamic_cast<const Frame*> (frame);
        if (sylt == nullptr || sylt->timestampFormat() != Frame::AbsoluteMilliseconds
            || (sylt->type() != Frame::Lyrics && sylt->type() != Frame::Other))
            continue;
        juce::String lrc;
        for (const auto& line : sylt->synchedText())
        {
            const auto ms = static_cast<int> (line.time);
            lrc << "[" << juce::String (ms / 60000).paddedLeft ('0', 2) << ":"
                << juce::String ((ms / 1000) % 60).paddedLeft ('0', 2) << "."
                << juce::String ((ms % 1000) / 10).paddedLeft ('0', 2) << "]"
                << toJuce (line.text).trim().replaceCharacters ("\r\n", "  ") << "\n";
        }
        if (lrc.isNotEmpty())
            return lrc;
    }
    return {};
}

/** Whole stars (1 to 5) as a rating out of 100. */
int starsToRating (int stars) { return juce::jlimit (0, 5, stars) * 20; }

/** An ID3v2 POPM rating (1 to 255; 0 is "unrated") as whole stars, by the
    thresholds Windows Media Player and most taggers write (1, 64, 128, 196,
    255). */
int popmToRating (int popm)
{
    if (popm <= 0)
        return 0;
    const auto stars = popm < 32 ? 1 : popm < 96 ? 2 : popm < 160 ? 3 : popm < 224 ? 4 : 5;
    return starsToRating (stars);
}

/** A RATING value: a fraction up to 1 ("0.8"), else 1 to 5 stars, else a
    percentage (6 to 100), else POPM's scale (up to 255). 0 if unusable. */
int ratingFromText (const juce::String& text)
{
    const auto trimmed = text.trim();
    if (trimmed.isEmpty() || ! trimmed.containsOnly ("0123456789.") || ! trimmed.containsAnyOf ("0123456789"))
        return 0;
    const auto value = trimmed.getDoubleValue();
    if (trimmed.containsChar ('.') && value <= 1.0)
        return juce::jlimit (0, 100, juce::roundToInt (value * 100.0));
    if (value <= 5.0)
        return starsToRating (juce::roundToInt (value));
    if (value <= 100.0)
        return juce::roundToInt (value);
    if (value <= 255.0)
        return popmToRating (juce::roundToInt (value));
    return 0;
}

/** The rating the tags give, 1 to 100, or 0 (see TrackTags::rating). */
int ratingOf (TagLib::File* file, const TagLib::PropertyMap& properties)
{
    if (auto* tag = id3v2TagOf (file))
        for (auto* frame : tag->frameList ("POPM"))
            if (const auto* popm = dynamic_cast<const TagLib::ID3v2::PopularimeterFrame*> (frame))
                if (const auto rating = popmToRating (popm->rating()); rating > 0)
                    return rating;

    if (const auto fmps = first (properties, "FMPS_RATING"); fmps.isNotEmpty())
    {
        const auto value = fmps.getDoubleValue();
        if (fmps.containsOnly ("0123456789.") && value > 0.0 && value <= 1.0)
            return juce::jlimit (1, 100, juce::roundToInt (value * 100.0));
    }

    if (const auto rating = ratingFromText (first (properties, "RATING")); rating > 0)
        return rating;

    if (auto* mp4 = dynamic_cast<TagLib::MP4::File*> (file); mp4 != nullptr && mp4->hasMP4Tag())
    {
        const auto item = mp4->tag()->item ("rate");
        if (item.isValid())
        {
            const auto strings = item.toStringList();
            const auto rating = strings.isEmpty() ? item.toInt() : ratingFromText (toJuce (strings.front()));
            return juce::jlimit (0, 100, rating);
        }
    }
    return 0;
}

/** Whether a flag tag says yes: "1", "true", "yes" or a positive number. */
bool isSet (const juce::String& value)
{
    const auto text = value.trim().toLowerCase();
    return text == "true" || text == "yes" || (text.containsOnly ("0123456789") && text.getIntValue() > 0);
}

void readWork (const TagLib::PropertyMap& properties, TrackTags& result)
{
    result.work = first (properties, "WORK");
    result.movementName = first (properties, "MOVEMENTNAME");
    result.composer = joined (properties, "COMPOSER");
    result.conductor = joined (properties, "CONDUCTOR");
    numberAndTotal (properties, "MOVEMENTNUMBER", { "MOVEMENTCOUNT", "MOVEMENTTOTAL" }, result.movementNumber,
                    result.movementTotal);

    // Some Vorbis taggers write MOVEMENT: a number, or else the name.
    if (const auto movement = first (properties, "MOVEMENT"); movement.isNotEmpty())
    {
        if (result.movementNumber == 0 && movement.containsOnly ("0123456789/"))
            numberAndTotal (properties, "MOVEMENT", { "MOVEMENTCOUNT", "MOVEMENTTOTAL" }, result.movementNumber,
                            result.movementTotal);
        else if (result.movementName.isEmpty() && ! movement.containsOnly ("0123456789/"))
            result.movementName = movement;
    }
}

/** A ReplayGain value such as "-6.54 dB" or "0.98765", or NaN if `key` is
    missing or isn't a number within `limit` of 0. */
double replayGainValue (const TagLib::PropertyMap& properties, const char* key, double limit)
{
    const auto text = first (properties, key).upToFirstOccurrenceOf ("dB", false, true).trim();
    const auto valid = text.isNotEmpty() && text.containsOnly ("+-.0123456789") && text.containsAnyOf ("0123456789");
    const auto value = valid ? text.getDoubleValue() : 0.0;
    return valid && std::abs (value) <= limit ? value : std::numeric_limits<double>::quiet_NaN();
}

/** An Opus R128 gain (a Q7.8 integer, relative to -23 LUFS) as a ReplayGain
    gain in dB (relative to about -18 LUFS), or NaN. */
double r128Gain (const TagLib::PropertyMap& properties, const char* key)
{
    const auto text = first (properties, key);
    const auto digits = text.startsWithChar ('-') || text.startsWithChar ('+') ? text.substring (1) : text;
    if (digits.isEmpty() || digits.length() > 5 || ! digits.containsOnly ("0123456789"))
        return std::numeric_limits<double>::quiet_NaN();
    return text.getIntValue() / 256.0 + 5.0;
}

void readReplayGain (const TagLib::PropertyMap& properties, TrackTags& result)
{
    // Gains beyond ±60 dB and peaks above 16 (+24 dBFS) are nonsense.
    result.trackGainDb = replayGainValue (properties, "REPLAYGAIN_TRACK_GAIN", 60.0);
    result.albumGainDb = replayGainValue (properties, "REPLAYGAIN_ALBUM_GAIN", 60.0);
    result.trackPeak = replayGainValue (properties, "REPLAYGAIN_TRACK_PEAK", 16.0);
    result.albumPeak = replayGainValue (properties, "REPLAYGAIN_ALBUM_PEAK", 16.0);

    if (std::isnan (result.trackGainDb))
        result.trackGainDb = r128Gain (properties, "R128_TRACK_GAIN");
    if (std::isnan (result.albumGainDb))
        result.albumGainDb = r128Gain (properties, "R128_ALBUM_GAIN");
    if (result.trackPeak < 0.0)
        result.trackPeak = std::numeric_limits<double>::quiet_NaN();
    if (result.albumPeak < 0.0)
        result.albumPeak = std::numeric_limits<double>::quiet_NaN();
}

juce::String sniffMimeType (const juce::MemoryBlock& data)
{
    const auto* bytes = static_cast<const juce::uint8*> (data.getData());
    if (data.getSize() >= 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff)
        return "image/jpeg";
    if (data.getSize() >= 8 && std::memcmp (bytes, "\x89PNG\r\n\x1a\n", 8) == 0)
        return "image/png";
    return {};
}

void readPicture (const TagLib::FileRef& ref, TrackTags& result)
{
    const auto pictures = ref.complexProperties ("PICTURE");
    if (pictures.isEmpty())
        return;

    auto chosen = pictures.front();
    for (const auto& picture : pictures)
    {
        if (picture.value ("pictureType").value<TagLib::String>() == "Front Cover")
        {
            chosen = picture;
            break;
        }
    }

    const auto data = chosen.value ("data").value<TagLib::ByteVector>();
    result.picture.replaceAll (data.data(), data.size());
    result.pictureMimeType = toJuce (chosen.value ("mimeType").value<TagLib::String>()).trim().toLowerCase();
    if (result.pictureMimeType.isEmpty())
        result.pictureMimeType = sniffMimeType (result.picture);
}
} // namespace

juce::String readTags (TagLib::IOStream& stream, int parts, TrackTags& result)
{
    result = {};

    try
    {
        // A stream (not a file name) keeps TagLib read-only; it detects the
        // format from the stream's name, then the content.
        const TagLib::FileRef ref (&stream, true, TagLib::AudioProperties::Average);

        if (ref.isNull())
            return stream.isOpen() ? juce::String() : juce::String ("Cannot open stream");

        const auto properties = ref.properties();

        result.title = joined (properties, "TITLE");
        result.artist = joined (properties, "ARTIST");
        result.album = joined (properties, "ALBUM");
        result.albumArtist = joined (properties, "ALBUMARTIST");
        result.genre = joined (properties, "GENRE");
        numberAndTotal (properties, "TRACKNUMBER", { "TRACKTOTAL", "TOTALTRACKS" }, result.trackNumber,
                        result.trackTotal);
        numberAndTotal (properties, "DISCNUMBER", { "DISCTOTAL", "TOTALDISCS" }, result.discNumber, result.discTotal);
        result.year = year (properties);
        result.date = dateOf (properties, "DATE");
        result.originalDate = dateOf (properties, "ORIGINALDATE");
        readReplayGain (properties, result);
        readWork (properties, result);

        result.musicBrainzRecordingId = joined (properties, "MUSICBRAINZ_TRACKID");
        result.musicBrainzReleaseId = joined (properties, "MUSICBRAINZ_ALBUMID");
        result.musicBrainzReleaseGroupId = joined (properties, "MUSICBRAINZ_RELEASEGROUPID");
        result.musicBrainzReleaseTrackId = joined (properties, "MUSICBRAINZ_RELEASETRACKID");
        result.musicBrainzArtistId = joined (properties, "MUSICBRAINZ_ARTISTID");
        result.musicBrainzAlbumArtistId = joined (properties, "MUSICBRAINZ_ALBUMARTISTID");
        result.rating = ratingOf (ref.file(), properties);
        result.compilation = isSet (first (properties, "COMPILATION"));
        result.artists = joined (properties, "ARTISTS");

        if ((parts & TagParts::picture) != 0)
            readPicture (ref, result);

        if ((parts & TagParts::lyrics) != 0)
        {
            result.lyrics = lyricsOf (properties);
            result.syncedLyrics = syncedLyricsOf (ref.file());
        }

        if ((parts & TagParts::chapters) != 0)
            result.cueSheet = first (properties, "CUESHEET");

        if (const auto* audio = ref.audioProperties())
        {
            result.durationSeconds = audio->lengthInMilliseconds() / 1000.0;
            result.sampleRate = audio->sampleRate();
            result.channels = audio->channels();
            result.bitrateKbps = audio->bitrate();
        }
    }
    catch (const std::exception& e)
    {
        return "Cannot read tags: " + juce::String (e.what());
    }

    return {};
}

juce::String readTags (const juce::File& file, int parts, juce::AudioFormatManager& formats, TrackTags& result)
{
    if (! file.existsAsFile())
        return "File not found: " + file.getFullPathName();

    result = {};
    const auto& path = file.getFullPathName();

    {
#if JUCE_WINDOWS
        TagLib::FileStream stream (path.toWideCharPointer(), true);
#else
        TagLib::FileStream stream (path.toRawUTF8(), true);
#endif
        if (! stream.isOpen())
            return "Cannot open file: " + path;
        if (const auto error = readTags (stream, parts, result); error.isNotEmpty())
            return error;
    }

    if (result.durationSeconds <= 0.0 || result.sampleRate <= 0)
    {
        const std::unique_ptr<juce::AudioFormatReader> reader (formats.createReaderFor (file));
        if (reader == nullptr)
            return "Unsupported or unreadable file: " + path;

        if (reader->sampleRate > 0.0)
            result.durationSeconds = static_cast<double> (reader->lengthInSamples) / reader->sampleRate;
        result.sampleRate = juce::roundToInt (reader->sampleRate);
        result.channels = static_cast<int> (reader->numChannels);
    }

    if ((parts & TagParts::chapters) != 0)
    {
        juce::Array<FFmpegAudioFormat::Chapter> chapters;
        // A file FFmpeg can't open has no chapters it can play either.
        if (FFmpegAudioFormat::readChapters (file, chapters).isEmpty())
            for (const auto& chapter : chapters)
                result.chapters.add ({ chapter.start, chapter.end, chapter.title });
    }

    return {};
}
namespace
{
/** The kinds of tag `file` carries, as the user would know them. */
juce::StringArray tagTypesOf (TagLib::File* file)
{
    juce::StringArray types;
    const auto id3v2 = [&types] (TagLib::ID3v2::Tag* tag)
    {
        if (tag != nullptr)
            types.add ("ID3v2." + juce::String (tag->header()->majorVersion()));
    };

    if (auto* mpeg = dynamic_cast<TagLib::MPEG::File*> (file))
    {
        if (mpeg->hasID3v2Tag())
            id3v2 (mpeg->ID3v2Tag());
        if (mpeg->hasAPETag())
            types.add ("APE");
        if (mpeg->hasID3v1Tag())
            types.add ("ID3v1");
    }
    else if (auto* flac = dynamic_cast<TagLib::FLAC::File*> (file))
    {
        if (flac->hasXiphComment())
            types.add ("Vorbis comment");
        if (flac->hasID3v2Tag())
            id3v2 (flac->ID3v2Tag());
        if (flac->hasID3v1Tag())
            types.add ("ID3v1");
    }
    else if (auto* mp4 = dynamic_cast<TagLib::MP4::File*> (file))
    {
        if (mp4->hasMP4Tag())
            types.add ("MP4");
    }
    else if (dynamic_cast<TagLib::Ogg::File*> (file) != nullptr)
    {
        types.add ("Vorbis comment");
    }
    else if (auto* wav = dynamic_cast<TagLib::RIFF::WAV::File*> (file))
    {
        if (wav->hasID3v2Tag())
            id3v2 (wav->ID3v2Tag());
        if (wav->hasInfoTag())
            types.add ("RIFF INFO");
    }
    else if (auto* aiff = dynamic_cast<TagLib::RIFF::AIFF::File*> (file))
    {
        if (aiff->hasID3v2Tag())
            id3v2 (aiff->tag());
    }
    else if (auto* ape = dynamic_cast<TagLib::APE::File*> (file))
    {
        if (ape->hasAPETag())
            types.add ("APE");
        if (ape->hasID3v1Tag())
            types.add ("ID3v1");
    }
    else if (auto* wavPack = dynamic_cast<TagLib::WavPack::File*> (file))
    {
        if (wavPack->hasAPETag())
            types.add ("APE");
        if (wavPack->hasID3v1Tag())
            types.add ("ID3v1");
    }
    else if (dynamic_cast<TagLib::ASF::File*> (file) != nullptr)
    {
        types.add ("ASF");
    }
    return types;
}
} // namespace

juce::String readFileInfo (TagLib::IOStream& stream, FileInfo& result, bool& tagged)
{
    tagged = false;

    try
    {
        const TagLib::FileRef ref (&stream, true, TagLib::AudioProperties::Average);
        if (ref.isNull())
            return stream.isOpen() ? juce::String() : juce::String ("Cannot open stream");

        tagged = true;
        const auto properties = ref.properties();
        for (const auto& [key, values] : properties)
            for (const auto& value : values)
                result.fields.emplace_back (toJuce (key), toJuce (value));

        // What TagLib found but can't map to a field (e.g. POPM with its
        // e-mail address, private frames), listed by name.
        for (const auto& unsupported : properties.unsupportedData())
            result.fields.emplace_back ("(unsupported)", toJuce (unsupported));

        if (auto* tag = id3v2TagOf (ref.file()))
            for (auto* frame : tag->frameList ("POPM"))
                if (const auto* popm = dynamic_cast<const TagLib::ID3v2::PopularimeterFrame*> (frame))
                    result.fields.emplace_back ("POPM", juce::String (popm->rating()) + "/255 " + toJuce (popm->email())
                                                            + " (played " + juce::String (popm->counter()) + " times)");

        for (const auto& picture : ref.complexProperties ("PICTURE"))
        {
            FileInfo::Picture info;
            info.type = toJuce (picture.value ("pictureType").value<TagLib::String>());
            info.mimeType = toJuce (picture.value ("mimeType").value<TagLib::String>()).trim().toLowerCase();
            info.description = toJuce (picture.value ("description").value<TagLib::String>());
            const auto data = picture.value ("data").value<TagLib::ByteVector>();
            info.data.replaceAll (data.data(), data.size());
            if (info.mimeType.isEmpty())
                info.mimeType = sniffMimeType (info.data);
            result.pictures.push_back (std::move (info));
        }

        result.tagTypes = tagTypesOf (ref.file());

        if (const auto* audio = ref.audioProperties())
        {
            result.durationSeconds = audio->lengthInMilliseconds() / 1000.0;
            result.sampleRate = audio->sampleRate();
            result.channels = audio->channels();
            result.bitrateKbps = audio->bitrate();
        }
    }
    catch (const std::exception& e)
    {
        return "Cannot read tags: " + juce::String (e.what());
    }

    return {};
}

juce::String readFileInfo (const juce::File& file, juce::AudioFormatManager& formats, FileInfo& result)
{
    if (! file.existsAsFile())
        return "File not found: " + file.getFullPathName();

    result = {};
    result.fileSize = file.getSize();
    const auto& path = file.getFullPathName();
    bool tagged = false;

    {
#if JUCE_WINDOWS
        TagLib::FileStream stream (path.toWideCharPointer(), true);
#else
        TagLib::FileStream stream (path.toRawUTF8(), true);
#endif
        if (! stream.isOpen())
            return "Cannot open file: " + path;
        if (const auto error = readFileInfo (stream, result, tagged); error.isNotEmpty())
            return error;
    }

    // The decoder's view, which the player's signal path shows too.
    if (const std::unique_ptr<juce::AudioFormatReader> reader (formats.createReaderFor (file)); reader != nullptr)
    {
        const auto& values = reader->metadataValues;
        result.codec = values[FFmpegAudioFormat::codecKey];
        result.lossless = values[FFmpegAudioFormat::losslessKey] == "1";
        result.bitsPerSample = values[FFmpegAudioFormat::bitsKey].getIntValue();
        if (const auto bitRate = values[FFmpegAudioFormat::bitRateKey].getLargeIntValue(); bitRate > 0)
            result.bitrateKbps = static_cast<int> (bitRate / 1000);
        if (reader->sampleRate > 0.0)
        {
            result.sampleRate = reader->sampleRate;
            result.durationSeconds = static_cast<double> (reader->lengthInSamples) / reader->sampleRate;
        }
        result.channels = static_cast<int> (reader->numChannels);
    }
    else if (! tagged)
    {
        return "Unsupported or unreadable file: " + path;
    }

    return {};
}
} // namespace anomp
