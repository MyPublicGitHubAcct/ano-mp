#include "anomp/anomp.h"
#include "AudioEngine.h"
#include "FolderAccess.h"
#include "FormatRegistry.h"
#include "MediaControls.h"
#include "TagReader.h"

#include <cstring>
#include <string>

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
