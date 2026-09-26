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

/** Event kinds passed to anomp_event_callback. Phase 1 adds playback events. */
enum
{
    ANOMP_EVENT_DEVICE_CHANGED = 1 /**< Device list or open output device changed. */
};

typedef struct anomp_event
{
    int type; /**< One of the ANOMP_EVENT_* values. */
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

/** Plays a sine test tone on the open device. Returns 1 on success, 0 if no
    device is open or the frequency is not positive. */
int anomp_engine_play_test_tone(anomp_engine* engine, double frequency_hz);

void anomp_engine_stop_test_tone(anomp_engine* engine);

#ifdef __cplusplus
}
#endif
