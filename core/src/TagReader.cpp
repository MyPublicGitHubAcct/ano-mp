#include "TagReader.h"

#include <fileref.h>
#include <tfilestream.h>
#include <tpropertymap.h>

#include <cstring>

namespace anomp
{
namespace
{
juce::String toJuce (const TagLib::String& text)
{
    return juce::String::fromUTF8 (text.to8Bit (true).c_str());
}

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

juce::String readTags (const juce::File& file,
                       bool includePicture,
                       juce::AudioFormatManager& formats,
                       TrackTags& result)
{
    if (! file.existsAsFile())
        return "File not found: " + file.getFullPathName();

    result = {};
    const auto path = file.getFullPathName();

    try
    {
       #if JUCE_WINDOWS
        TagLib::FileStream stream (path.toWideCharPointer(), true);
       #else
        TagLib::FileStream stream (path.toRawUTF8(), true);
       #endif

        // The stream (not a file name) keeps TagLib read-only; it detects the
        // format from the extension, then the content.
        const TagLib::FileRef ref (&stream, true, TagLib::AudioProperties::Average);

        if (! ref.isNull())
        {
            const auto properties = ref.properties();

            result.title = joined (properties, "TITLE");
            result.artist = joined (properties, "ARTIST");
            result.album = joined (properties, "ALBUM");
            result.albumArtist = joined (properties, "ALBUMARTIST");
            result.genre = joined (properties, "GENRE");
            numberAndTotal (properties, "TRACKNUMBER", { "TRACKTOTAL", "TOTALTRACKS" }, result.trackNumber, result.trackTotal);
            numberAndTotal (properties, "DISCNUMBER", { "DISCTOTAL", "TOTALDISCS" }, result.discNumber, result.discTotal);
            result.year = year (properties);

            result.musicBrainzRecordingId = joined (properties, "MUSICBRAINZ_TRACKID");
            result.musicBrainzReleaseId = joined (properties, "MUSICBRAINZ_ALBUMID");
            result.musicBrainzReleaseGroupId = joined (properties, "MUSICBRAINZ_RELEASEGROUPID");
            result.musicBrainzReleaseTrackId = joined (properties, "MUSICBRAINZ_RELEASETRACKID");
            result.musicBrainzArtistId = joined (properties, "MUSICBRAINZ_ARTISTID");
            result.musicBrainzAlbumArtistId = joined (properties, "MUSICBRAINZ_ALBUMARTISTID");

            if (includePicture)
                readPicture (ref, result);

            if (const auto* audio = ref.audioProperties())
            {
                result.durationSeconds = audio->lengthInMilliseconds() / 1000.0;
                result.sampleRate = audio->sampleRate();
                result.channels = audio->channels();
                result.bitrateKbps = audio->bitrate();
            }
        }
        else if (! stream.isOpen())
        {
            return "Cannot open file: " + path;
        }
    }
    catch (const std::exception& e)
    {
        return "Cannot read tags: " + juce::String (e.what());
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

    return {};
}
} // namespace anomp
