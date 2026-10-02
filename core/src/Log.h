#pragma once

#include <juce_core/juce_core.h>

namespace anomp
{
/** The core's log (PLAN.md H9): messages go to the host's callback
    (anomp_set_log_callback), which writes them with its own. JUCE's Logger
    is routed here too, and with JUCE_LOG_ASSERTIONS (core/CMakeLists.txt)
    so are failed assertions, in every build. Without a callback, messages
    are dropped.

    May be used from any thread, the audio thread included (an assertion
    can fail there); the callback runs on the thread that logs. */
namespace log
{
enum class Level
{
    error = 1,
    warn = 2,
    info = 3,
    debug = 4
};

using Callback = void (*) (int level, const char* message, void* userData);

/** Sets where messages go; null stops them. Installs a juce::Logger
    that forwards to it while one is set. */
void setCallback (Callback callback, void* userData);

void write (Level level, const juce::String& message);

inline void error (const juce::String& message) { write (Level::error, message); }
inline void warn (const juce::String& message) { write (Level::warn, message); }
inline void info (const juce::String& message) { write (Level::info, message); }
} // namespace log
} // namespace anomp
