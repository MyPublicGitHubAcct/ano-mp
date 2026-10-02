// An excerpt in the shape of core/include/anomp/anomp.h.
#pragma once

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Returns the version. Mentions anomp_engine_play(engine) in a comment. */
const char* anomp_version(void);

typedef struct anomp_engine anomp_engine;

typedef struct anomp_event
{
    int kind;
    double position;
} anomp_event;

typedef void (*anomp_event_callback)(const anomp_event* event, void* user_data);

void anomp_engine_set_event_callback(anomp_engine* engine, anomp_event_callback callback, void* user_data);

int anomp_engine_load(anomp_engine* engine,
                      const char* path,
                      double gain,
                      char* error,
                      size_t error_size);

void anomp_engine_play(anomp_engine* engine);

#ifdef __cplusplus
}
#endif
