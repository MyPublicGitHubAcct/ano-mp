#include "anomp/anomp.h"
#include "AudioEngine.h"
#include "DockMenu.h"
#include "FileAnalyser.h"
#include "FileStatus.h"
#include "FolderAccess.h"
#include "FormatRegistry.h"
#include "Log.h"
#include "MediaControls.h"
#include "TagReader.h"
#include "VolumeWatcher.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstring>
#include <string>
#include <vector>

struct anomp_engine
{
    anomp::AudioEngine engine;
    anomp_event_callback callback = nullptr;
    void* userData = nullptr;
    juce::StringArray outputDevices; // As found by anomp_engine_output_device_count.
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
        std::string work, movementName, composer, conductor, date, originalDate;
        std::string lyrics, syncedLyrics, cueSheet;
        std::vector<std::string> chapterTitles;
        std::vector<anomp_chapter> chapters;
        std::string artists;
    } owned;
};

/** Owns the storage behind anomp_file_info's pointers. */
struct FileInfoHandle : anomp_file_info
{
    struct
    {
        std::vector<std::pair<std::string, std::string>> fieldText;
        std::vector<anomp_tag_field> fields;
        struct PictureText
        {
            std::string type, mimeType, description;
            juce::MemoryBlock data;
        };
        std::vector<PictureText> pictureData;
        std::vector<anomp_picture> pictures;
        std::string tagTypes;
    } owned;
};

struct anomp_dock_menu
{
    anomp_menu_callback callback = nullptr;
    void* userData = nullptr;
    std::unique_ptr<anomp::DockMenu> menu; // Declared last, so destroyed first.
};

/** Owns the arrays behind the public struct's pointers. */
struct FileAnalysisHandle : anomp_file_analysis
{
    anomp::FileAnalysis owned;
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

struct anomp_volume_watcher
{
    anomp_volume_callback callback = nullptr;
    void* userData = nullptr;
    std::unique_ptr<anomp::VolumeWatcher> watcher; // Declared last, so destroyed first.
};

struct anomp_media_controls
{
    anomp_media_command_callback callback = nullptr;
    void* userData = nullptr;
    std::unique_ptr<anomp::MediaControls> controls; // Declared last, so destroyed first.
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

bool toMediaCommand (int type, anomp::MediaControls::Command& command)
{
    using Command = anomp::MediaControls::Command;

    switch (type)
    {
        case ANOMP_MEDIA_PLAY:     command = Command::play; return true;
        case ANOMP_MEDIA_PAUSE:    command = Command::pause; return true;
        case ANOMP_MEDIA_TOGGLE:   command = Command::toggle; return true;
        case ANOMP_MEDIA_NEXT:     command = Command::next; return true;
        case ANOMP_MEDIA_PREVIOUS: command = Command::previous; return true;
        case ANOMP_MEDIA_SEEK:     command = Command::seek; return true;
        default:                   return false;
    }
}

int toCMediaCommand (anomp::MediaControls::Command command)
{
    using Command = anomp::MediaControls::Command;

    switch (command)
    {
        case Command::play:     return ANOMP_MEDIA_PLAY;
        case Command::pause:    return ANOMP_MEDIA_PAUSE;
        case Command::toggle:   return ANOMP_MEDIA_TOGGLE;
        case Command::next:     return ANOMP_MEDIA_NEXT;
        case Command::previous: return ANOMP_MEDIA_PREVIOUS;
        case Command::seek:     return ANOMP_MEDIA_SEEK;
    }
    return 0;
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

#ifndef ANOMP_VERSION
#error "ANOMP_VERSION comes from CMake's project(VERSION) (core/CMakeLists.txt)"
#endif

extern "C" const char* anomp_version (void) { return ANOMP_VERSION; }

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
        {
            static_assert (ANOMP_TAGS_PICTURE == anomp::TagParts::picture
                           && ANOMP_TAGS_LYRICS == anomp::TagParts::lyrics
                           && ANOMP_TAGS_CHAPTERS == anomp::TagParts::chapters);
            message = anomp::readTags (juce::File (text), flags, registry().manager(), tags);
        }

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
        handle->owned.work = tags.work.toStdString();
        handle->owned.movementName = tags.movementName.toStdString();
        handle->owned.composer = tags.composer.toStdString();
        handle->owned.conductor = tags.conductor.toStdString();
        handle->owned.date = tags.date.toStdString();
        handle->owned.originalDate = tags.originalDate.toStdString();
        handle->owned.lyrics = tags.lyrics.toStdString();
        handle->owned.syncedLyrics = tags.syncedLyrics.toStdString();
        handle->owned.cueSheet = tags.cueSheet.toStdString();
        handle->owned.artists = tags.artists.toStdString();
        for (const auto& chapter : tags.chapters)
            handle->owned.chapterTitles.push_back (chapter.title.toStdString());

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
        handle->replaygain_track_gain = tags.trackGainDb;
        handle->replaygain_track_peak = tags.trackPeak;
        handle->replaygain_album_gain = tags.albumGainDb;
        handle->replaygain_album_peak = tags.albumPeak;
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
        handle->work = handle->owned.work.c_str();
        handle->movement_name = handle->owned.movementName.c_str();
        handle->movement_number = tags.movementNumber;
        handle->movement_total = tags.movementTotal;
        handle->composer = handle->owned.composer.c_str();
        handle->conductor = handle->owned.conductor.c_str();
        handle->date = handle->owned.date.c_str();
        handle->original_date = handle->owned.originalDate.c_str();
        handle->lyrics = handle->owned.lyrics.c_str();
        handle->synced_lyrics = handle->owned.syncedLyrics.c_str();
        handle->cuesheet = handle->owned.cueSheet.c_str();
        for (int i = 0; i < tags.chapters.size(); ++i)
            handle->owned.chapters.push_back ({ tags.chapters[i].start, tags.chapters[i].end,
                                                handle->owned.chapterTitles[static_cast<size_t> (i)].c_str() });
        handle->chapter_count = static_cast<int> (handle->owned.chapters.size());
        handle->chapters = handle->owned.chapters.empty() ? nullptr : handle->owned.chapters.data();
        handle->rating = juce::jlimit (0, 100, tags.rating);
        handle->compilation = tags.compilation ? 1 : 0;
        handle->artists = handle->owned.artists.c_str();
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot read tags", error, errorSize);
        return nullptr;
    }
}

extern "C" void anomp_tags_free (anomp_tags* tags) { delete static_cast<TagsHandle*> (tags); }

extern "C" anomp_file_info* anomp_read_file_info (const char* path, char* error, size_t errorSize)
{
    try
    {
        juce::String message;
        anomp::FileInfo info;

        if (path == nullptr)
            message = "Null path";
        else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
            message = "Path is not absolute: " + text;
        else
            message = anomp::readFileInfo (juce::File (text), registry().manager(), info);

        copyUtf8 (message, error, errorSize);
        if (message.isNotEmpty())
            return nullptr;

        auto handle = std::make_unique<FileInfoHandle>();
        auto& owned = handle->owned;
        for (const auto& [key, value] : info.fields)
            owned.fieldText.emplace_back (key.toStdString(), value.toStdString());
        for (auto& picture : info.pictures)
            owned.pictureData.push_back ({ picture.type.toStdString(), picture.mimeType.toStdString(),
                                           picture.description.toStdString(), std::move (picture.data) });
        owned.tagTypes = info.tagTypes.joinIntoString (", ").toStdString();

        // Final now, so pointers into them stay valid until anomp_file_info_free.
        for (const auto& [key, value] : owned.fieldText)
            owned.fields.push_back ({ key.c_str(), value.c_str() });
        for (const auto& picture : owned.pictureData)
            owned.pictures.push_back (
                { picture.type.c_str(), picture.mimeType.c_str(), picture.description.c_str(),
                  picture.data.isEmpty() ? nullptr : static_cast<const unsigned char*> (picture.data.getData()),
                  picture.data.getSize() });

        handle->field_count = static_cast<int> (owned.fields.size());
        handle->fields = owned.fields.empty() ? nullptr : owned.fields.data();
        handle->picture_count = static_cast<int> (owned.pictures.size());
        handle->pictures = owned.pictures.empty() ? nullptr : owned.pictures.data();
        handle->tag_types = owned.tagTypes.c_str();
        std::memset (handle->codec, 0, sizeof (handle->codec));
        copyUtf8 (info.codec, handle->codec, sizeof (handle->codec));
        handle->lossless = info.lossless ? 1 : 0;
        handle->bits_per_sample = info.bitsPerSample;
        handle->bitrate_kbps = info.bitrateKbps;
        handle->sample_rate = info.sampleRate;
        handle->channels = info.channels;
        handle->duration = info.durationSeconds;
        handle->file_size = info.fileSize;
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot read the file", error, errorSize);
        return nullptr;
    }
}

extern "C" void anomp_file_info_free (anomp_file_info* info) { delete static_cast<FileInfoHandle*> (info); }

extern "C" anomp_file_analysis* anomp_analyse_file (const char* path,
                                                    double start,
                                                    double end,
                                                    anomp_analysis_progress progress,
                                                    void* userData,
                                                    char* error,
                                                    size_t errorSize)
{
    try
    {
        juce::String message;
        auto handle = std::make_unique<FileAnalysisHandle>();
        auto& result = handle->owned;

        if (path == nullptr)
            message = "Null path";
        else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
            message = "Path is not absolute: " + text;
        else
            message = anomp::analyseFile (
                juce::File (text), registry().manager(), start, end, [progress, userData] (double fraction)
                { return progress == nullptr || progress (fraction, userData) != 0; }, result);

        copyUtf8 (message, error, errorSize);
        if (message.isNotEmpty())
            return nullptr;

        handle->duration =
            result.sampleRate > 0.0 ? static_cast<double> (result.decodedSamples) / result.sampleRate : 0.0;
        handle->sample_rate = result.sampleRate;
        handle->channels = result.channels;
        handle->integrated_lufs = result.integratedLufs;
        handle->sample_peak = result.samplePeak;
        handle->true_peak = result.truePeak;
        handle->histogram_count = static_cast<int> (result.histogram.size());
        handle->histogram = result.histogram.data();
        handle->histogram_floor = anomp::FileAnalysis::histogramFloor;
        handle->histogram_step = anomp::FileAnalysis::histogramStep;
        handle->leading_silence = result.leadingSilence;
        handle->trailing_silence = result.trailingSilence;
        handle->gap_start = result.gapStart;
        handle->gap_length = result.gapLength;
        handle->start_level_db = result.startLevelDb;
        handle->end_level_db = result.endLevelDb;
        handle->cutoff_hz = result.cutoffHz;
        handle->envelope_length = static_cast<int> (result.envelopeMin.size());
        handle->envelope_min = result.envelopeMin.data();
        handle->envelope_max = result.envelopeMax.data();
        return handle.release();
    }
    catch (...)
    {
        copyUtf8 ("Cannot analyse the file", error, errorSize);
        return nullptr;
    }
}

extern "C" void anomp_file_analysis_free (anomp_file_analysis* analysis)
{
    delete static_cast<FileAnalysisHandle*> (analysis);
}

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

extern "C" int anomp_file_is_dataless (const char* path)
{
    if (path == nullptr)
        return -1;
    const auto text = juce::String::fromUTF8 (path);
    if (! juce::File::isAbsolutePath (text))
        return -1;
    switch (anomp::FileStatus::isDataless (juce::File (text)))
    {
        case anomp::FileStatus::Dataless::yes:     return 1;
        case anomp::FileStatus::Dataless::no:      return 0;
        case anomp::FileStatus::Dataless::unknown: break;
    }
    return -1;
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

extern "C" void anomp_set_log_callback (anomp_log_callback callback, void* userData)
{
    anomp::log::setCallback (callback, userData);
}

extern "C" void anomp_log_write (int level, const char* message)
{
    const auto clamped = juce::jlimit<int> (ANOMP_LOG_ERROR, ANOMP_LOG_DEBUG, level);
    anomp::log::write (static_cast<anomp::log::Level> (clamped),
                       juce::String::fromUTF8 (message != nullptr ? message : ""));
}

extern "C" int anomp_volume_watcher_supported (void) { return anomp::VolumeWatcher::isSupported() ? 1 : 0; }

extern "C" anomp_volume_watcher* anomp_volume_watcher_start (anomp_volume_callback callback, void* userData)
{
    if (callback == nullptr)
        return nullptr;
    try
    {
        auto handle = std::make_unique<anomp_volume_watcher>();
        handle->callback = callback;
        handle->userData = userData;
        auto* raw = handle.get();
        handle->watcher = std::make_unique<anomp::VolumeWatcher> (
            [raw] (bool mounted, const juce::String& path)
            { raw->callback (mounted ? 1 : 0, path.toRawUTF8(), raw->userData); });
        return handle.release();
    }
    catch (...)
    {
        return nullptr;
    }
}

extern "C" void anomp_volume_watcher_stop (anomp_volume_watcher* watcher) { delete watcher; }

extern "C" void anomp_volume_watcher_notify (anomp_volume_watcher* watcher, int mounted, const char* path)
{
    if (watcher == nullptr || watcher->watcher == nullptr)
        return;
    watcher->watcher->notify (mounted != 0, juce::String::fromUTF8 (path != nullptr ? path : ""));
}

extern "C" anomp_engine* anomp_engine_create (void)
{
    try
    {
        auto* handle = new anomp_engine();
        auto& player = handle->engine.player();

        handle->engine.onDeviceChanged = [handle]
        {
            emit (handle, anomp_event { ANOMP_EVENT_DEVICE_CHANGED, 0, 0, 0.0, 0.0, 0, 0, "" });
        };
        player.onStateChanged = [handle] (anomp::PlayerEngine::State state)
        {
            emit (handle, anomp_event { ANOMP_EVENT_STATE_CHANGED, toCState (state), 0, 0.0, 0.0, 0, 0, "" });
        };
        player.onPositionChanged = [handle] (double position, double duration)
        {
            emit (handle, anomp_event { ANOMP_EVENT_POSITION, 0, 0, position, duration, 0, 0, "" });
        };
        player.onTrackEnded = [handle] (bool advanced)
        {
            emit (handle, anomp_event { ANOMP_EVENT_TRACK_ENDED, 0, advanced ? 1 : 0, 0.0, 0.0, 0, 0, "" });
        };
        player.onLoadFinished =
            [handle] (anomp::PlayerEngine::LoadId id, anomp::PlayerEngine::LoadResult result, const juce::String& error)
        {
            const auto code = result == anomp::PlayerEngine::LoadResult::loaded   ? ANOMP_LOAD_LOADED
                              : result == anomp::PlayerEngine::LoadResult::failed ? ANOMP_LOAD_FAILED
                                                                                  : ANOMP_LOAD_CANCELLED;
            emit (handle, anomp_event { ANOMP_EVENT_LOAD_FINISHED, 0, 0, 0.0, 0.0, id, code, error.toRawUTF8() });
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

extern "C" int anomp_engine_output_device_count (anomp_engine* engine)
{
    if (engine == nullptr)
        return 0;
    engine->outputDevices = engine->engine.outputDeviceNames();
    return engine->outputDevices.size();
}

extern "C" size_t anomp_engine_output_device_name (anomp_engine* engine, int index, char* buffer, size_t bufferSize)
{
    const auto valid = engine != nullptr && juce::isPositiveAndBelow (index, engine->outputDevices.size());
    return copyUtf8 (valid ? engine->outputDevices[index] : juce::String(), buffer, bufferSize);
}

extern "C" int anomp_engine_open_device (anomp_engine* engine,
                                         const char* name,
                                         int bufferSize,
                                         char* error,
                                         size_t errorSize)
{
    const auto message =
        engine != nullptr
            ? engine->engine.openDevice (name != nullptr ? juce::String::fromUTF8 (name) : juce::String(), bufferSize)
            : juce::String ("Null engine");
    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? 1 : 0;
}

extern "C" int anomp_engine_device_info (anomp_engine* engine, anomp_device_info* info)
{
    anomp::AudioEngine::DeviceInfo device;
    if (engine == nullptr || info == nullptr || ! engine->engine.getDeviceInfo (device))
        return 0;

    info->buffer_size = device.bufferSize;
    info->default_buffer_size = device.defaultBufferSize;
    info->sample_rate = device.sampleRate;
    info->output_latency = device.outputLatencySeconds;
    return 1;
}

extern "C" int anomp_engine_buffer_sizes (anomp_engine* engine, int* sizes, int capacity)
{
    anomp::AudioEngine::DeviceInfo device;
    if (engine == nullptr || ! engine->engine.getDeviceInfo (device))
        return 0;

    for (int i = 0; sizes != nullptr && i < juce::jmin (capacity, device.bufferSizes.size()); ++i)
        sizes[i] = device.bufferSizes[i];
    return device.bufferSizes.size();
}

extern "C" int anomp_engine_load (anomp_engine* engine, const char* path, double gain, char* error, size_t errorSize)
{
    return withPath (engine, path, error, errorSize, [gain] (anomp::PlayerEngine& player, const juce::File& file)
                     { return player.load (file, static_cast<float> (gain)); });
}

extern "C" int anomp_engine_set_next (anomp_engine* engine,
                                      const char* path,
                                      double gain,
                                      char* error,
                                      size_t errorSize)
{
    if (engine != nullptr && path == nullptr)
    {
        engine->engine.player().clearNext();
        copyUtf8 ({}, error, errorSize);
        return 1;
    }

    return withPath (engine, path, error, errorSize, [gain] (anomp::PlayerEngine& player, const juce::File& file)
                     { return player.setNext (file, static_cast<float> (gain)); });
}

extern "C" int anomp_engine_set_track_gain (anomp_engine* engine, const char* path, double gain)
{
    if (engine == nullptr || path == nullptr)
        return 0;

    const auto text = juce::String::fromUTF8 (path);
    if (! juce::File::isAbsolutePath (text))
        return 0;

    return engine->engine.player().setTrackGain (juce::File (text), static_cast<float> (gain));
}

extern "C" anomp_track_options anomp_track_options_default (double gain)
{
    return anomp_track_options { gain, 0.0, 0.0, -1.0, -1.0, 0.0 };
}

namespace
{
anomp::PlayerEngine::TrackOptions toTrackOptions (const anomp_track_options* options)
{
    if (options == nullptr)
        return {};
    return { static_cast<float> (options->gain),
             options->start,
             options->end,
             options->skip_from,
             options->skip_to,
             std::isfinite (options->crossfade) ? juce::jlimit (0.0, ANOMP_MAX_CROSSFADE, options->crossfade) : 0.0 };
}
} // namespace

extern "C" int anomp_engine_load_track (anomp_engine* engine,
                                        const char* path,
                                        const anomp_track_options* options,
                                        char* error,
                                        size_t errorSize)
{
    const auto trackOptions = toTrackOptions (options);
    return withPath (engine, path, error, errorSize,
                     [&trackOptions] (anomp::PlayerEngine& player, const juce::File& file)
                     { return player.load (file, trackOptions); });
}

extern "C" int anomp_engine_set_next_track (anomp_engine* engine,
                                            const char* path,
                                            const anomp_track_options* options,
                                            char* error,
                                            size_t errorSize)
{
    if (engine != nullptr && path == nullptr)
    {
        engine->engine.player().clearNext();
        copyUtf8 ({}, error, errorSize);
        return 1;
    }

    const auto trackOptions = toTrackOptions (options);
    return withPath (engine, path, error, errorSize,
                     [&trackOptions] (anomp::PlayerEngine& player, const juce::File& file)
                     { return player.setNext (file, trackOptions); });
}

namespace
{
/** Starts an asynchronous request for an absolute `path`; 0 and the error if it can't. */
template <typename Request>
long long withAsyncPath (anomp_engine* engine, const char* path, char* error, size_t errorSize, Request&& request)
{
    juce::String message;
    if (engine == nullptr)
        message = "Null engine";
    else if (path == nullptr)
        message = "Null path";
    else if (const auto text = juce::String::fromUTF8 (path); ! juce::File::isAbsolutePath (text))
        message = "Path is not absolute: " + text;

    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? request (engine->engine.player(), juce::File (juce::String::fromUTF8 (path))) : 0;
}
} // namespace

extern "C" long long anomp_engine_load_track_async (anomp_engine* engine,
                                                    const char* path,
                                                    const anomp_track_options* options,
                                                    char* error,
                                                    size_t errorSize)
{
    const auto trackOptions = toTrackOptions (options);
    return withAsyncPath (engine, path, error, errorSize,
                          [&trackOptions] (anomp::PlayerEngine& player, const juce::File& file)
                          { return player.loadAsync (file, trackOptions); });
}

extern "C" long long anomp_engine_set_next_track_async (anomp_engine* engine,
                                                        const char* path,
                                                        const anomp_track_options* options,
                                                        char* error,
                                                        size_t errorSize)
{
    const auto trackOptions = toTrackOptions (options);
    return withAsyncPath (engine, path, error, errorSize,
                          [&trackOptions] (anomp::PlayerEngine& player, const juce::File& file)
                          { return player.setNextAsync (file, trackOptions); });
}

extern "C" void anomp_engine_cancel_load (anomp_engine* engine, long long request)
{
    if (engine != nullptr)
        engine->engine.player().cancelLoad (request);
}

extern "C" int anomp_engine_set_track_gain_at (anomp_engine* engine, const char* path, double start, double gain)
{
    if (engine == nullptr || path == nullptr)
        return 0;

    const auto text = juce::String::fromUTF8 (path);
    if (! juce::File::isAbsolutePath (text))
        return 0;

    return engine->engine.player().setTrackGainAt (juce::File (text), start, static_cast<float> (gain));
}

extern "C" int anomp_engine_set_loop (anomp_engine* engine, double start, double end, char* error, size_t errorSize)
{
    juce::String message;
    if (engine == nullptr)
        message = "Null engine";
    else if (start < 0.0)
        engine->engine.player().clearLoop();
    else
        message = engine->engine.player().setLoop (start, end);
    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? 1 : 0;
}

extern "C" int anomp_engine_loop (anomp_engine* engine, double* start, double* end)
{
    double a = 0.0, b = 0.0;
    if (engine == nullptr || ! engine->engine.player().getLoop (a, b))
        return 0;
    if (start != nullptr)
        *start = a;
    if (end != nullptr)
        *end = b;
    return 1;
}

extern "C" int anomp_engine_set_tempo (anomp_engine* engine, double rate, double semitones)
{
    return engine != nullptr && engine->engine.player().setTempo (rate, semitones) ? 1 : 0;
}

extern "C" int anomp_engine_set_crossfeed (anomp_engine* engine, int level)
{
    if (engine == nullptr || level < ANOMP_CROSSFEED_OFF || level > ANOMP_CROSSFEED_STRONG)
        return 0;
    engine->engine.player().setCrossfeed (level);
    return 1;
}

extern "C" int anomp_engine_set_equaliser (anomp_engine* engine, const double* gainsDb, double preampDb)
{
    static_assert (ANOMP_EQ_BANDS == anomp::Equaliser::numBands);
    if (engine == nullptr)
        return 0;

    anomp::Equaliser::Gains gains {};
    if (gainsDb != nullptr)
    {
        const auto inRange = [] (double db)
        {
            return std::isfinite (db) && std::abs (db) <= ANOMP_EQ_MAX_GAIN;
        };
        if (! inRange (preampDb))
            return 0;
        for (size_t band = 0; band < gains.size(); ++band)
        {
            if (! inRange (gainsDb[band]))
                return 0;
            gains[band] = gainsDb[band];
        }
    }
    engine->engine.player().setEqualiser (gainsDb != nullptr, gains, gainsDb != nullptr ? preampDb : 0.0);
    return 1;
}

namespace
{
static_assert (ANOMP_EFFECT_COUNT == anomp::fx::effectCount && ANOMP_EFFECT_MAX_PARAMS == anomp::fx::maxParams);
static_assert (ANOMP_EFFECT_LOFI == static_cast<int> (anomp::fx::EffectType::lofi));

bool toEffect (int effect, anomp::fx::EffectType& type)
{
    if (effect < 0 || effect >= ANOMP_EFFECT_COUNT)
        return false;
    type = static_cast<anomp::fx::EffectType> (effect);
    return true;
}
} // namespace

extern "C" int anomp_effect_describe (int effect, anomp_effect_info* info)
{
    anomp::fx::EffectType type {};
    if (info == nullptr || ! toEffect (effect, type))
        return 0;

    const auto& described = anomp::fx::describe (type);
    *info = {};
    copyUtf8 (described.id, info->id, sizeof (info->id));
    const auto& order = anomp::fx::chainOrder;
    info->position = static_cast<int> (std::find (order.begin(), order.end(), type) - order.begin());
    info->default_mix = described.defaultMix;
    info->param_count = static_cast<int> (described.numParams);
    for (size_t i = 0; i < described.numParams; ++i)
    {
        const auto& param = described.params[i];
        auto& out = info->params[i];
        copyUtf8 (param.id, out.id, sizeof (out.id));
        out.unit = static_cast<int> (param.unit);
        out.min = param.min;
        out.max = param.max;
        out.default_value = param.defaultValue;
        out.logarithmic = param.logarithmic ? 1 : 0;
    }
    return 1;
}

extern "C" int anomp_engine_set_effect (anomp_engine* engine,
                                        int effect,
                                        int enabled,
                                        double mix,
                                        const double* params,
                                        int paramCount)
{
    anomp::fx::EffectType type {};
    if (engine == nullptr || ! toEffect (effect, type) || ! (mix >= 0.0 && mix <= 1.0))
        return 0;
    const auto& described = anomp::fx::describe (type);
    if (paramCount < 0 || static_cast<size_t> (paramCount) > described.numParams
        || (paramCount > 0 && params == nullptr))
        return 0;
    // Compared as the floats the ranges are, so a bound written as a
    // double (0.05) is within them.
    for (size_t i = 0; i < static_cast<size_t> (paramCount); ++i)
    {
        const auto& param = described.params[i];
        const auto value = static_cast<float> (params[i]);
        if (! (value >= param.min && value <= param.max))
            return 0;
    }

    auto& chain = engine->engine.player().getEffects();
    for (size_t i = 0; i < static_cast<size_t> (paramCount); ++i)
        chain.setParameter (type, i, static_cast<float> (params[i]));
    chain.setMix (type, static_cast<float> (mix));
    chain.setEnabled (type, enabled != 0);
    return 1;
}

extern "C" int anomp_engine_set_freeze (anomp_engine* engine, int hold)
{
    return engine != nullptr && engine->engine.player().getEffects().setFreezeHeld (hold != 0) ? 1 : 0;
}

extern "C" int anomp_engine_freeze_held (anomp_engine* engine)
{
    return engine != nullptr && engine->engine.player().getEffects().isFreezeHeld() ? 1 : 0;
}

extern "C" int anomp_engine_output_is_headphones (anomp_engine* engine)
{
    if (engine == nullptr)
        return -1;
    switch (engine->engine.outputIsHeadphones())
    {
        case anomp::OutputRoute::Headphones::yes:     return 1;
        case anomp::OutputRoute::Headphones::no:      return 0;
        case anomp::OutputRoute::Headphones::unknown: return -1;
    }
    return -1;
}

extern "C" int anomp_engine_signal_path (anomp_engine* engine, anomp_signal_path* path)
{
    if (engine == nullptr || path == nullptr)
        return 0;

    const auto info = engine->engine.player().getSignalInfo();
    anomp::AudioEngine::DeviceInfo device;
    const auto hasDevice = engine->engine.getDeviceInfo (device);

    *path = {};
    path->loaded = info.loaded ? 1 : 0;
    copyUtf8 (info.codec, path->codec, sizeof (path->codec));
    path->lossless = info.lossless ? 1 : 0;
    path->bits_per_sample = info.bitsPerSample;
    path->bitrate_kbps = info.bitrateKbps;
    path->file_sample_rate = info.fileSampleRate;
    path->file_channels = info.channels;
    path->track_gain = info.gain;
    path->tempo = info.tempo;
    path->semitones = info.semitones;
    path->resampling = info.loaded && info.deviceSampleRate > 0.0
                               && ! juce::approximatelyEqual (info.fileSampleRate, info.deviceSampleRate)
                           ? 1
                           : 0;
    path->crossfeed = info.crossfeed;
    path->volume = engine->engine.player().getVolume();
    path->device_sample_rate = hasDevice ? device.sampleRate : 0.0;
    path->device_buffer_size = hasDevice ? device.bufferSize : 0;
    path->equaliser = info.equaliser ? 1 : 0;
    path->crossfade = info.crossfade;
    for (size_t i = 0; i < info.effects.size(); ++i)
        if (info.effects[i])
            path->effects |= 1 << i;
    path->freeze_held = info.freezeHeld ? 1 : 0;
    return 1;
}

extern "C" int anomp_engine_set_device_sample_rate (anomp_engine* engine, double sampleRate)
{
    return engine != nullptr && engine->engine.setSampleRate (sampleRate) ? 1 : 0;
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

extern "C" int anomp_engine_set_analysis_callback (anomp_engine* engine,
                                                   const anomp_analysis_config* config,
                                                   anomp_analysis_callback callback,
                                                   void* userData)
{
    if (engine == nullptr)
        return 0;

    if (callback == nullptr)
    {
        engine->engine.setAnalysisCallback (nullptr, 0, 0, 0.0);
        return 1;
    }

    using Analyser = anomp::SpectrumAnalyser;
    using Thread = anomp::AnalysisThread;

    if (config == nullptr || config->band_count < Analyser::minBands || config->band_count > Analyser::maxBands
        || config->waveform_length < Analyser::minWaveformLength
        || config->waveform_length > Analyser::maxWaveformLength
        || ! (config->frames_per_second >= Thread::minFramesPerSecond
              && config->frames_per_second <= Thread::maxFramesPerSecond))
        return 0;

    try
    {
        engine->engine.setAnalysisCallback (
            [callback, userData] (const anomp::AnalysisFrame& frame)
            {
                const anomp_analysis_frame event {
                    frame.silent ? 1 : 0,
                    static_cast<int> (frame.bands.size()),
                    frame.bands.data(),
                    frame.lowestHz,
                    frame.highestHz,
                    frame.chroma.data(),
                    frame.peak[0],
                    frame.peak[1],
                    frame.rms[0],
                    frame.rms[1],
                    static_cast<int> (frame.left.size()),
                    frame.left.data(),
                    frame.right.data(),
                    frame.onset,
                    frame.beat ? 1 : 0,
                    anomp::AnalysisFrame::noteCount,
                    anomp::AnalysisFrame::lowestNote,
                    frame.notes.data(),
                    frame.balance.data(),
                };
                callback (&event, userData);
            },
            config->band_count, config->waveform_length, config->frames_per_second);
        return 1;
    }
    catch (...)
    {
        return 0;
    }
}

extern "C" int anomp_dock_menu_supported (void) { return anomp::DockMenu::isSupported() ? 1 : 0; }

namespace
{
/** One Dock menu at a time: the OS has one per app. */
std::atomic<bool> dockMenuExists { false };
} // namespace

extern "C" anomp_dock_menu* anomp_dock_menu_create (anomp_menu_callback callback, void* userData)
{
    if (dockMenuExists.exchange (true))
        return nullptr;
    try
    {
        auto handle = std::make_unique<anomp_dock_menu>();
        handle->callback = callback;
        handle->userData = userData;
        auto* raw = handle.get();
        handle->menu = std::make_unique<anomp::DockMenu> (
            [raw] (int id)
            {
                if (raw->callback != nullptr)
                    raw->callback (id, raw->userData);
            });
        return handle.release();
    }
    catch (...)
    {
        dockMenuExists = false;
        return nullptr;
    }
}

extern "C" void anomp_dock_menu_destroy (anomp_dock_menu* menu)
{
    if (menu == nullptr)
        return;
    delete menu;
    dockMenuExists = false;
}

extern "C" void anomp_dock_menu_set_items (anomp_dock_menu* menu, const anomp_menu_item* items, int count)
{
    if (menu == nullptr)
        return;
    std::vector<anomp::DockMenu::Item> copied;
    for (int i = 0; items != nullptr && i < count; ++i)
        copied.push_back ({ items[i].id,
                            items[i].title != nullptr ? juce::String::fromUTF8 (items[i].title) : juce::String(),
                            items[i].enabled != 0, items[i].checked != 0 });
    menu->menu->setItems (std::move (copied));
}

extern "C" int anomp_dock_menu_perform (anomp_dock_menu* menu, int id)
{
    return menu != nullptr && menu->menu->choose (id) ? 1 : 0;
}

extern "C" int anomp_media_controls_supported (void) { return anomp::MediaControls::isSupported() ? 1 : 0; }

extern "C" anomp_media_controls* anomp_media_controls_create (anomp_media_command_callback callback, void* userData)
{
    try
    {
        auto handle = std::make_unique<anomp_media_controls>();
        handle->callback = callback;
        handle->userData = userData;
        auto* raw = handle.get();
        handle->controls = std::make_unique<anomp::MediaControls> (
            [raw] (anomp::MediaControls::Command command, double position)
            {
                if (raw->callback != nullptr)
                {
                    const anomp_media_command event { toCMediaCommand (command), position };
                    raw->callback (&event, raw->userData);
                }
            });
        return handle.release();
    }
    catch (...)
    {
        return nullptr;
    }
}

extern "C" void anomp_media_controls_destroy (anomp_media_controls* controls) { delete controls; }

extern "C" int anomp_media_controls_set_track (anomp_media_controls* controls, const anomp_media_track* track)
{
    if (controls == nullptr || track == nullptr)
        return 0;

    const auto text = [] (const char* utf8)
    {
        return utf8 != nullptr ? juce::String::fromUTF8 (utf8) : juce::String();
    };

    try
    {
        controls->controls->setTrack (text (track->title), text (track->artist), text (track->album));
        return 1;
    }
    catch (...)
    {
        return 0;
    }
}

extern "C" int anomp_media_controls_set_playback (anomp_media_controls* controls,
                                                  int state,
                                                  double elapsed,
                                                  double duration)
{
    if (controls == nullptr || state < ANOMP_STATE_EMPTY || state > ANOMP_STATE_PAUSED)
        return 0;

    const auto playback =
        state == ANOMP_STATE_PLAYING ? anomp::MediaControls::Playback::playing : anomp::MediaControls::Playback::paused;
    return controls->controls->setPlayback (playback, elapsed, duration) ? 1 : 0;
}

extern "C" int anomp_media_controls_set_artwork (anomp_media_controls* controls, const unsigned char* data, size_t size)
{
    if (controls == nullptr)
        return 0;

    try
    {
        if (data == nullptr && size > 0)
        {
            controls->controls->setArtwork ({});
            return 0;
        }

        return controls->controls->setArtwork (juce::MemoryBlock (data, size)) ? 1 : 0;
    }
    catch (...)
    {
        return 0;
    }
}

extern "C" void anomp_media_controls_set_navigation (anomp_media_controls* controls, int hasNext, int hasPrevious)
{
    if (controls != nullptr)
        controls->controls->setNavigation (hasNext != 0, hasPrevious != 0);
}

extern "C" void anomp_media_controls_clear (anomp_media_controls* controls)
{
    if (controls != nullptr)
        controls->controls->clear();
}

extern "C" int anomp_media_controls_perform (anomp_media_controls* controls, const anomp_media_command* command)
{
    anomp::MediaControls::Command kind;

    if (controls == nullptr || command == nullptr || controls->callback == nullptr
        || ! toMediaCommand (command->type, kind))
        return 0;

    return controls->controls->handleCommand (kind, command->position) ? 1 : 0;
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
