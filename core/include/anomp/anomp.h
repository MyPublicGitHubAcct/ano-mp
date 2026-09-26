// Public C API for the ano-mp audio core.
// This is the only surface the Tauri (Rust) layer links against, so it must
// stay plain C: no C++ types, no exceptions across the boundary.
#pragma once

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Returns the core library version as "major.minor.patch". Never null. */
const char* anomp_version(void);

/** Returns 1 if the core can decode files with the given extension
    (e.g. "flac" or ".mp3", case-insensitive), otherwise 0. */
int anomp_can_decode_extension(const char* extension);

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
void anomp_engine_set_event_callback(anomp_engine* engine,
                                     anomp_event_callback callback,
                                     void* user_data);

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
    start. Returns 1 on success; on failure returns 0 and nothing changes. */
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

/* ---- Test tone ---------------------------------------------------------- */

/** Pauses the player and plays a sine test tone on the open device. Returns 1
    on success, 0 if no device is open or the frequency is not positive. */
int anomp_engine_play_test_tone(anomp_engine* engine, double frequency_hz);

/** Stops the tone and reconnects the player. */
void anomp_engine_stop_test_tone(anomp_engine* engine);

#ifdef __cplusplus
}
#endif
