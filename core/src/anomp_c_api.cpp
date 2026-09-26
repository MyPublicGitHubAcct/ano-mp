#include "anomp/anomp.h"
#include "AudioEngine.h"
#include "FormatRegistry.h"

#include <cstring>

struct anomp_engine
{
    anomp::AudioEngine engine;
    anomp_event_callback callback = nullptr;
    void* userData = nullptr;
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
} // namespace

extern "C" const char* anomp_version (void)
{
    return "0.1.0";
}

extern "C" int anomp_can_decode_extension (const char* extension)
{
    if (extension == nullptr)
        return 0;
    return registry().canDecodeExtension (juce::String::fromUTF8 (extension)) ? 1 : 0;
}

extern "C" anomp_engine* anomp_engine_create (void)
{
    try
    {
        auto* handle = new anomp_engine();
        handle->engine.onDeviceChanged = [handle]
        {
            if (handle->callback != nullptr)
            {
                const anomp_event event { ANOMP_EVENT_DEVICE_CHANGED };
                handle->callback (&event, handle->userData);
            }
        };
        return handle;
    }
    catch (...)
    {
        return nullptr;
    }
}

extern "C" void anomp_engine_destroy (anomp_engine* engine)
{
    delete engine;
}

extern "C" void anomp_engine_set_event_callback (anomp_engine* engine,
                                                 anomp_event_callback callback,
                                                 void* userData)
{
    if (engine == nullptr)
        return;
    engine->callback = callback;
    engine->userData = userData;
}

extern "C" int anomp_engine_open_default_device (anomp_engine* engine, char* error, size_t errorSize)
{
    const auto message = engine != nullptr ? engine->engine.openDefaultDevice()
                                           : juce::String ("Null engine");
    copyUtf8 (message, error, errorSize);
    return message.isEmpty() ? 1 : 0;
}

extern "C" size_t anomp_engine_device_name (anomp_engine* engine, char* buffer, size_t bufferSize)
{
    return copyUtf8 (engine != nullptr ? engine->engine.currentDeviceName() : juce::String(),
                     buffer,
                     bufferSize);
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
