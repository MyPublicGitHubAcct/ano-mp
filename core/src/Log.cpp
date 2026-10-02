#include "Log.h"

#include <mutex>

namespace anomp::log
{
namespace
{
std::mutex mutex;
Callback currentCallback = nullptr;
void* currentUserData = nullptr;

/** JUCE's Logger, forwarded: failed assertions as errors, the rest as
    information. */
class Forwarder final : public juce::Logger
{
public:
    // Destroyed with the process's statics, possibly while still JUCE's
    // current logger (a host needn't clear its callback): JUCE's destructor
    // would assert, and log the failure through this half-destroyed logger.
    ~Forwarder() override
    {
        if (juce::Logger::getCurrentLogger() == this)
            juce::Logger::setCurrentLogger (nullptr);
    }

    void logMessage (const juce::String& message) override
    {
        write (message.startsWith ("JUCE Assertion failure") ? Level::error : Level::info, message);
    }
};

Forwarder forwarder;
} // namespace

void setCallback (Callback callback, void* userData)
{
    {
        const std::lock_guard<std::mutex> lock (mutex);
        currentCallback = callback;
        currentUserData = userData;
    }
    juce::Logger::setCurrentLogger (callback != nullptr ? &forwarder : nullptr);
}

void write (Level level, const juce::String& message)
{
    const std::lock_guard<std::mutex> lock (mutex);
    if (currentCallback != nullptr)
        currentCallback (static_cast<int> (level), message.toRawUTF8(), currentUserData);
}
} // namespace anomp::log
