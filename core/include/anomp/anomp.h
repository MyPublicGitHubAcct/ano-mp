// Public C API for the ano-mp audio core.
// This is the only surface the Tauri (Rust) layer links against, so it must
// stay plain C: no C++ types, no exceptions across the boundary.
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Returns the core library version as "major.minor.patch". Never null. */
const char* anomp_version(void);

/** Returns 1 if the core can decode files with the given extension
    (e.g. "flac" or ".mp3", case-insensitive), otherwise 0. */
int anomp_can_decode_extension(const char* extension);

/* ---- Tags ----------------------------------------------------------------
   Unlike the engine, these functions may be called from any thread,
   concurrently (e.g. by a library scanner). */

/** A file's tags and audio properties. Strings are NUL-terminated UTF-8 and
    never null: "" when the file doesn't have the field. Several values of one
    field (e.g. two ARTIST comments) are joined with "; ". Numbers are 0 when
    absent. */
typedef struct anomp_tags
{
    const char* title;
    const char* artist;
    const char* album;
    const char* album_artist;
    const char* genre;
    int track_number;
    int track_total;
    int disc_number;
    int disc_total;
    int year;

    double duration; /**< Seconds, from the file's headers: lossy files can be
                          off by tens of milliseconds (encoder delay and
                          padding). The player measures exactly on load. */
    int sample_rate; /**< Hz. */
    int channels;
    int bitrate_kbps; /**< Average or nominal; 0 if unknown. */

    /* MusicBrainz IDs, named after what they identify (Picard's "track id"
       is the recording, its "album id" the release). */
    const char* musicbrainz_recording_id;
    const char* musicbrainz_release_id;
    const char* musicbrainz_release_group_id;
    const char* musicbrainz_release_track_id;
    const char* musicbrainz_artist_id;
    const char* musicbrainz_album_artist_id;

    /** Embedded front cover (or else the first picture), only when read with
        ANOMP_TAGS_PICTURE: `picture_size` bytes, null when there is none. */
    const unsigned char* picture;
    size_t picture_size;
    const char* picture_mime_type; /**< e.g. "image/jpeg"; "" if unknown. */
} anomp_tags;

/** Flags for anomp_read_tags. */
enum
{
    ANOMP_TAGS_PICTURE = 1 /**< Also copy out the embedded picture. */
};

/** Reads the tags of the file at `path` (absolute, UTF-8) without modifying
    it. Fails for files that neither the tag reader nor the decoder can
    read. Returns null on failure and writes the error message to `error` (see
    anomp_engine_device_name for the buffer rules; `error` may be null).
    Free the result with anomp_tags_free. */
anomp_tags* anomp_read_tags(const char* path, int flags, char* error, size_t error_size);

/** Frees tags returned by anomp_read_tags. Null is ignored. */
void anomp_tags_free(anomp_tags* tags);

/* ---- Folder access -------------------------------------------------------
   A sandboxed app (the macOS App Sandbox, iOS) may read a folder the user
   picked only until it quits, unless it saves a security-scoped bookmark and
   resolves it in later sessions. A bookmark also follows its folder when it
   is moved or renamed on the same volume. On platforms without a sandbox a
   bookmark holds the path. These functions may be called from any thread. */

/** Opaque bookmark bytes, to store as is. */
typedef struct anomp_bookmark
{
    const unsigned char* data;
    size_t size;
} anomp_bookmark;

/** Creates a bookmark for the folder at `path` (absolute, UTF-8), which the
    app must be able to read now: the user just picked it, or an
    anomp_folder_access for it is open. Returns null on failure and writes the
    error message to `error` (buffer rules of anomp_engine_device_name;
    `error` may be null). Free the result with anomp_bookmark_free. */
anomp_bookmark* anomp_bookmark_create(const char* path, char* error, size_t error_size);

/** Frees a bookmark returned by anomp_bookmark_create. Null is ignored. */
void anomp_bookmark_free(anomp_bookmark* bookmark);

typedef struct anomp_folder_access anomp_folder_access;

/** Resolves a bookmark and starts accessing its folder; the app can read the
    folder until anomp_folder_access_stop. Returns null on failure (e.g. the
    folder was deleted or its volume isn't mounted) and writes the error
    message as anomp_bookmark_create does. */
anomp_folder_access* anomp_folder_access_start(const unsigned char* bookmark,
                                               size_t bookmark_size,
                                               char* error,
                                               size_t error_size);

/** Where the folder is now (absolute, UTF-8), which may differ from where it
    was when the bookmark was made. Valid until anomp_folder_access_stop;
    "" for a null access. */
const char* anomp_folder_access_path(const anomp_folder_access* access);

/** Returns 1 if the bookmark is stale: create a new one for the path above
    while this access is open, and store it in place of the old one. */
int anomp_folder_access_is_stale(const anomp_folder_access* access);

/** Stops accessing the folder and frees `access`. Null is ignored. */
void anomp_folder_access_stop(anomp_folder_access* access);

/* ---- Engine ------------------------------------------------------------
   Every engine function must be called on the process's main thread, and
   event callbacks are delivered on it. The host must run the platform's main
   run loop (Tauri does). Passing a null engine is safe and does nothing. */

typedef struct anomp_engine anomp_engine;

/** Event kinds passed to anomp_event_callback. Player events are dispatched
    about every 50 ms, never from inside an engine call. */
enum
{
    ANOMP_EVENT_DEVICE_CHANGED = 1, /**< Device list or open output device changed. */
    ANOMP_EVENT_STATE_CHANGED = 2,  /**< Player state changed; see `state`. */
    ANOMP_EVENT_POSITION = 3,       /**< Position or duration changed; see `position`, `duration`. */
    ANOMP_EVENT_TRACK_ENDED = 4     /**< The current track played to its end; see `advanced`. */
};

/** Player states. */
enum
{
    ANOMP_STATE_EMPTY = 0,   /**< No track loaded. */
    ANOMP_STATE_STOPPED = 1, /**< Track loaded, at the start. */
    ANOMP_STATE_PLAYING = 2,
    ANOMP_STATE_PAUSED = 3
};

typedef struct anomp_event
{
    int type;        /**< One of the ANOMP_EVENT_* values. */
    int state;       /**< STATE_CHANGED: the new ANOMP_STATE_* value. */
    int advanced;    /**< TRACK_ENDED: 1 if the next track took over gaplessly,
                          0 if playback stopped (a STATE_CHANGED follows). */
    double position; /**< POSITION: seconds into the current track. */
    double duration; /**< POSITION: length of the current track in seconds. */
} anomp_event;

/** `event` is only valid for the duration of the call. */
typedef void (*anomp_event_callback)(const anomp_event* event, void* user_data);

/** Starts the audio runtime. Returns null on failure. Destroy with
    anomp_engine_destroy. */
anomp_engine* anomp_engine_create(void);

void anomp_engine_destroy(anomp_engine* engine);

/** Sets (or, with a null callback, clears) the event callback. */
void anomp_engine_set_event_callback(anomp_engine* engine, anomp_event_callback callback, void* user_data);

/** Opens the default output device. Returns 1 on success, otherwise 0 and
    writes the error message as UTF-8 to `error` (see anomp_engine_device_name
    for the buffer rules; `error` may be null). */
int anomp_engine_open_default_device(anomp_engine* engine, char* error, size_t error_size);

/** Writes the open output device's name as NUL-terminated UTF-8 into
    `buffer` (truncated to fit; `buffer` may be null when `buffer_size` is 0).
    Returns the full length in bytes excluding the NUL, or 0 if no device is
    open. */
size_t anomp_engine_device_name(anomp_engine* engine, char* buffer, size_t buffer_size);

/* ---- Player -------------------------------------------------------------
   One current track plus an optional next track that follows it gaplessly.
   The host owns the queue: on ANOMP_EVENT_TRACK_ENDED with `advanced` set,
   the next track has become current, and the host sets a new next track.
   Paths are absolute, UTF-8. Functions that take `error` write the message
   there on failure, with the buffer rules of anomp_engine_device_name. */

/** Opens `path` as the current track (clearing the next one) and stops at its
    start. A TRACK_ENDED without `advanced` for the replaced track that has not
    been dispatched yet is dropped. Returns 1 on success; on failure returns 0
    and nothing changes. */
int anomp_engine_load(anomp_engine* engine, const char* path, char* error, size_t error_size);

/** Opens `path` as the track after the current one; a null `path` clears
    it. Returns 1 on success, 0 on failure (the previous next track stays). */
int anomp_engine_set_next(anomp_engine* engine, const char* path, char* error, size_t error_size);

/** Starts or resumes playback. Returns 0 if no track is loaded. */
int anomp_engine_play(anomp_engine* engine);

/** Pauses (with a short fade). Does nothing unless playing. */
void anomp_engine_pause(anomp_engine* engine);

/** Stops and rewinds to the start of the current track. */
void anomp_engine_stop(anomp_engine* engine);

/** Seeks to `seconds` into the current track, clamped to its length.
    Returns 0 if no track is loaded or `seconds` is not finite. */
int anomp_engine_seek(anomp_engine* engine, double seconds);

/** Sets the linear output gain, clamped to 0..1. */
void anomp_engine_set_volume(anomp_engine* engine, double gain);

double anomp_engine_volume(anomp_engine* engine);

/** Returns an ANOMP_STATE_* value. */
int anomp_engine_state(anomp_engine* engine);

/** Seconds into the current track, or 0 if none is loaded. */
double anomp_engine_position(anomp_engine* engine);

/** Length of the current track in seconds, or 0 if none is loaded. */
double anomp_engine_duration(anomp_engine* engine);

/** How many times a next track has taken over gaplessly since the engine was
    created. TRACK_ENDED reports a hand-off up to 50 ms late; this counts it
    as it happens, so before changing the next track (or loading) the host
    can tell whether the one it set has already become current. 0 for a null
    engine. */
int64_t anomp_engine_advance_count(anomp_engine* engine);

/* ---- Test tone ---------------------------------------------------------- */

/** Pauses the player and plays a sine test tone on the open device. Returns 1
    on success, 0 if no device is open or the frequency is not positive. */
int anomp_engine_play_test_tone(anomp_engine* engine, double frequency_hz);

/** Stops the tone and reconnects the player. */
void anomp_engine_stop_test_tone(anomp_engine* engine);

#ifdef __cplusplus
}
#endif
