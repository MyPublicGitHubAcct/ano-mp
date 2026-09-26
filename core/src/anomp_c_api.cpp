#include "anomp/anomp.h"
#include "AudioEngine.h"
#include "FolderAccess.h"
#include "FormatRegistry.h"
#include "TagReader.h"

#include <cstring>
#include <string>

struct anomp_engine
{
    anomp::AudioEngine engine;
    anomp_event_callback callback = nullptr;
    void* userData = nullptr;
};

/** Owns the storage behind the public struct's pointers. */
struct TagsHandle : anomp_tags
{
    struct
    {
        std::string title, artist, album, albumArtist, genre;
        std::string recordingId, releaseId, releaseGroupId, releaseTrackId, artistId, albumArtistId;
        juce::MemoryBlock picture;
        std::string pictureMimeType;
    } owned;
};

/** Owns the bytes behind the public struct's pointer. */
struct BookmarkHandle : anomp_bookmark
{
    juce::MemoryBlock owned;
};

struct anomp_folder_access
{
    std::unique_ptr<anomp::FolderAccess> access;
    std::string path;
};

namespace
{
anomp::FormatRegistry& registry()
{
    static anomp::FormatRegistry instance;
    return instance;
}

/** snprintf-style copy: truncates to fit and returns the full UTF-8 length. */
size_t copyUtf8 (const juce::String& text, char* buffer, size_t bufferSize)
{
    const auto utf8 = text.toStdString();

    if (buffer != nullptr && bufferSize > 0)
    {
        const auto count = std::min (utf8.size(), bufferSize - 1);
        std::memcpy (buffer, utf8.data(), count);
        buffer[count] = '\0';
    }

    return utf8.size();
}

void emit (anomp_engine* handle, const anomp_event& event)
{
    if (handle->callback != nullptr)
        handle->callback (&event, handle->userData);
}

int toCState (anomp::PlayerEngine::State state)
{
    switch (state)
    {
        case anomp::PlayerEngine::State::empty:   return ANOMP_STATE_EMPTY;
        case anomp::PlayerEngine::State::stopped: return ANOMP_STATE_STOPPED;
        case anomp::PlayerEngine::State::playing: return ANOMP_STATE_PLAYING;
        case anomp::PlayerEngine::State::paused:  return ANOMP_STATE_PAUSED;
    }
    return ANOMP_STATE_EMPTY;
}

/** Runs a load-style command on an absolute UTF-8 path; returns 1 on success. */
template <typename Command>
int withPath (anomp_engine* engine, const char* path, char* error, size_t errorSize, Command&& command)
{
    juce::String message;

    if (engine == nullptr)
        message = "Null engine";
    else if (path == nullptr)
        message = "Null path";
    else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
        message = "Path is not absolute: " + text;
    else
        message = command (engine->engine.player(), juce::File (text));

    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? 1 : 0;
}
} // namespace

extern "C" const char* anomp_version (void) { return "0.1.0"; }

extern "C" int anomp_can_decode_extension (const char* extension)
{
    if (extension == nullptr)
        return 0;
    return registry().canDecodeExtension (juce::String::fromUTF8 (extension)) ? 1 : 0;
}

extern "C" anomp_tags* anomp_read_tags (const char* path, int flags, char* error, size_t errorSize)
{
    try
    {
        juce::String message;
        anomp::TrackTags tags;

        if (path == nullptr)
            message = "Null path";
        else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
            message = "Path is not absolute: " + text;
        else
            message =
                anomp::readTags (juce::File (text), (flags & ANOMP_TAGS_PICTURE) != 0, registry().manager(), tags);

        copyUtf8 (message, error, errorSize);
        if (message.isNotEmpty())
            return nullptr;

        auto handle = std::make_unique<TagsHandle>();
        handle->owned.title = tags.title.toStdString();
        handle->owned.artist = tags.artist.toStdString();
        handle->owned.album = tags.album.toStdString();
        handle->owned.albumArtist = tags.albumArtist.toStdString();
        handle->owned.genre = tags.genre.toStdString();
        handle->owned.recordingId = tags.musicBrainzRecordingId.toStdString();
        handle->owned.releaseId = tags.musicBrainzReleaseId.toStdString();
        handle->owned.releaseGroupId = tags.musicBrainzReleaseGroupId.toStdString();
        handle->owned.releaseTrackId = tags.musicBrainzReleaseTrackId.toStdString();
        handle->owned.artistId = tags.musicBrainzArtistId.toStdString();
        handle->owned.albumArtistId = tags.musicBrainzAlbumArtistId.toStdString();
        handle->owned.picture = std::move (tags.picture);
        handle->owned.pictureMimeType = tags.pictureMimeType.toStdString();

        // The strings are final now, so pointers into them stay valid until
        // anomp_tags_free.
        handle->title = handle->owned.title.c_str();
        handle->artist = handle->owned.artist.c_str();
        handle->album = handle->owned.album.c_str();
        handle->album_artist = handle->owned.albumArtist.c_str();
        handle->genre = handle->owned.genre.c_str();
        handle->track_number = tags.trackNumber;
        handle->track_total = tags.trackTotal;
        handle->disc_number = tags.discNumber;
        handle->disc_total = tags.discTotal;
        handle->year = tags.year;
        handle->duration = tags.durationSeconds;
        handle->sample_rate = tags.sampleRate;
        handle->channels = tags.channels;
        handle->bitrate_kbps = tags.bitrateKbps;
        handle->musicbrainz_recording_id = handle->owned.recordingId.c_str();
        handle->musicbrainz_release_id = handle->owned.releaseId.c_str();
        handle->musicbrainz_release_group_id = handle->owned.releaseGroupId.c_str();
        handle->musicbrainz_release_track_id = handle->owned.releaseTrackId.c_str();
        handle->musicbrainz_artist_id = handle->owned.artistId.c_str();
        handle->musicbrainz_album_artist_id = handle->owned.albumArtistId.c_str();
        handle->picture = handle->owned.picture.isEmpty()
                              ? nullptr
                              : static_cast<const unsigned char*> (handle->owned.picture.getData());
        handle->picture_size = handle->owned.picture.getSize();
        handle->picture_mime_type = handle->owned.pictureMimeType.c_str();
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot read tags", error, errorSize);
        return nullptr;
    }
}

extern "C" void anomp_tags_free (anomp_tags* tags) { delete static_cast<TagsHandle*> (tags); }

extern "C" anomp_bookmark* anomp_bookmark_create (const char* path, char* error, size_t errorSize)
{
    try
    {
        juce::String message;
        juce::MemoryBlock bookmark;

        if (path == nullptr)
            message = "Null path";
        else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
            message = "Path is not absolute: " + text;
        else
            message = anomp::FolderAccess::createBookmark (juce::File (text), bookmark);

        copyUtf8 (message, error, errorSize);
        if (message.isNotEmpty())
            return nullptr;

        auto handle = std::make_unique<BookmarkHandle>();
        handle->owned = std::move (bookmark);
        handle->data = static_cast<const unsigned char*> (handle->owned.getData());
        handle->size = handle->owned.getSize();
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot create a bookmark", error, errorSize);
        return nullptr;
    }
}

extern "C" void anomp_bookmark_free (anomp_bookmark* bookmark) { delete static_cast<BookmarkHandle*> (bookmark); }

extern "C" anomp_folder_access* anomp_folder_access_start (const unsigned char* bookmark,
                                                           size_t bookmarkSize,
                                                           char* error,
                                                           size_t errorSize)
{
    try
    {
        juce::String message;
        std::unique_ptr<anomp::FolderAccess> access;

        if (bookmark == nullptr && bookmarkSize > 0)
            message = "Null bookmark";
        else
            access = anomp::FolderAccess::start (juce::MemoryBlock (bookmark, bookmarkSize), message);

        copyUtf8 (message, error, errorSize);
        if (access == nullptr)
            return nullptr;

        auto handle = std::make_unique<anomp_folder_access>();
        handle->path = access->getFolder().getFullPathName().toStdString();
        handle->access = std::move (access);
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot resolve the bookmark", error, errorSize);
        return nullptr;
    }
}

extern "C" const char* anomp_folder_access_path (const anomp_folder_access* access)
{
    return access != nullptr ? access->path.c_str() : "";
}

extern "C" int anomp_folder_access_is_stale (const anomp_folder_access* access)
{
    return access != nullptr && access->access->isStale() ? 1 : 0;
}

extern "C" void anomp_folder_access_stop (anomp_folder_access* access) { delete access; }

extern "C" anomp_engine* anomp_engine_create (void)
{
    try
    {
        auto* handle = new anomp_engine();
        auto& player = handle->engine.player();

        handle->engine.onDeviceChanged = [handle]
        {
            emit (handle, anomp_event { ANOMP_EVENT_DEVICE_CHANGED, 0, 0, 0.0, 0.0 });
        };
        player.onStateChanged = [handle] (anomp::PlayerEngine::State state)
        {
            emit (handle, anomp_event { ANOMP_EVENT_STATE_CHANGED, toCState (state), 0, 0.0, 0.0 });
        };
        player.onPositionChanged = [handle] (double position, double duration)
        {
            emit (handle, anomp_event { ANOMP_EVENT_POSITION, 0, 0, position, duration });
        };
        player.onTrackEnded = [handle] (bool advanced)
        {
            emit (handle, anomp_event { ANOMP_EVENT_TRACK_ENDED, 0, advanced ? 1 : 0, 0.0, 0.0 });
        };
        return handle;
    }
    catch (...)
    {
        return nullptr;
    }
}

extern "C" void anomp_engine_destroy (anomp_engine* engine) { delete engine; }

extern "C" void anomp_engine_set_event_callback (anomp_engine* engine, anomp_event_callback callback, void* userData)
{
    if (engine == nullptr)
        return;
    engine->callback = callback;
    engine->userData = userData;
}

extern "C" int anomp_engine_open_default_device (anomp_engine* engine, char* error, size_t errorSize)
{
    const auto message = engine != nullptr ? engine->engine.openDefaultDevice() : juce::String ("Null engine");
    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? 1 : 0;
}

extern "C" size_t anomp_engine_device_name (anomp_engine* engine, char* buffer, size_t bufferSize)
{
    return copyUtf8 (engine != nullptr ? engine->engine.currentDeviceName() : juce::String(), buffer, bufferSize);
}

extern "C" int anomp_engine_load (anomp_engine* engine, const char* path, char* error, size_t errorSize)
{
    return withPath (engine, path, error, errorSize,
                     [] (anomp::PlayerEngine& player, const juce::File& file) { return player.load (file); });
}

extern "C" int anomp_engine_set_next (anomp_engine* engine, const char* path, char* error, size_t errorSize)
{
    if (engine != nullptr && path == nullptr)
    {
        engine->engine.player().clearNext();
        copyUtf8 ({}, error, errorSize);
        return 1;
    }

    return withPath (engine, path, error, errorSize,
                     [] (anomp::PlayerEngine& player, const juce::File& file) { return player.setNext (file); });
}

extern "C" int anomp_engine_play (anomp_engine* engine)
{
    return engine != nullptr && engine->engine.player().play() ? 1 : 0;
}

extern "C" void anomp_engine_pause (anomp_engine* engine)
{
    if (engine != nullptr)
        engine->engine.player().pause();
}

extern "C" void anomp_engine_stop (anomp_engine* engine)
{
    if (engine != nullptr)
        engine->engine.player().stop();
}

extern "C" int anomp_engine_seek (anomp_engine* engine, double seconds)
{
    return engine != nullptr && engine->engine.player().seek (seconds) ? 1 : 0;
}

extern "C" void anomp_engine_set_volume (anomp_engine* engine, double gain)
{
    if (engine != nullptr)
        engine->engine.player().setVolume (static_cast<float> (gain));
}

extern "C" double anomp_engine_volume (anomp_engine* engine)
{
    return engine != nullptr ? static_cast<double> (engine->engine.player().getVolume()) : 0.0;
}

extern "C" int anomp_engine_state (anomp_engine* engine)
{
    return engine != nullptr ? toCState (engine->engine.player().getState()) : ANOMP_STATE_EMPTY;
}

extern "C" double anomp_engine_position (anomp_engine* engine)
{
    return engine != nullptr ? engine->engine.player().getPositionSeconds() : 0.0;
}

extern "C" double anomp_engine_duration (anomp_engine* engine)
{
    return engine != nullptr ? engine->engine.player().getDurationSeconds() : 0.0;
}

extern "C" int64_t anomp_engine_advance_count (anomp_engine* engine)
{
    return engine != nullptr ? engine->engine.player().getAdvanceCount() : 0;
}

extern "C" int anomp_engine_play_test_tone (anomp_engine* engine, double frequencyHz)
{
    return engine != nullptr && engine->engine.playTestTone (frequencyHz) ? 1 : 0;
}

extern "C" void anomp_engine_stop_test_tone (anomp_engine* engine)
{
    if (engine != nullptr)
        engine->engine.stopTestTone();
}
