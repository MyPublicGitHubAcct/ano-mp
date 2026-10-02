#pragma once

#include "AnalysisThread.h"
#include "FormatRegistry.h"
#include "OutputRoute.h"
#include "PlayerEngine.h"

#include <juce_audio_devices/juce_audio_devices.h>

#include <functional>

namespace anomp
{
/** JUCE's runtime, as AudioEngine's first base: bases are destroyed after
    members and in reverse order, so it is shut down after everything else
    in the engine, the Timer and ChangeListener bases included. */
struct JuceRuntime
{
    juce::ScopedJuceInitialiser_GUI juceInitialiser;
};

/** Owns JUCE's runtime, the audio output device and the player.

    Must be created, used and destroyed on the main thread: the constructor
    starts JUCE's message loop integration there, and device-change and
    player notifications are delivered on that thread.
*/
class AudioEngine final : private JuceRuntime,
                          private juce::ChangeListener,
                          private juce::Timer,
                          private juce::AsyncUpdater
{
public:
    AudioEngine();
    ~AudioEngine() override;

    /** Opens the default output device. Returns an empty string on success,
        otherwise the error. */
    juce::String openDefaultDevice();

    /** Name of the open output device, or empty if none is open. */
    juce::String currentDeviceName() const;

    /** Looks for output devices of the default device type (Core Audio on
        Apple platforms) and returns their names. */
    juce::StringArray outputDeviceNames();

    /** Opens the output device `name` (empty for the system default) with
        `bufferSize` samples per block (0, or a size the device doesn't
        offer, for its default), at the device's current sample rate. The
        player carries on through it. A name that isn't in
        outputDeviceNames() fails without touching the open device; a
        device that fails to open may leave none open. Returns an empty
        string on success, otherwise the error. */
    juce::String openDevice (const juce::String& name, int bufferSize);

    struct DeviceInfo
    {
        int bufferSize = 0, defaultBufferSize = 0;
        juce::Array<int> bufferSizes; // Smallest first.
        double sampleRate = 0.0;
        double outputLatencySeconds = 0.0;
    };

    /** The open device's settings; false if none is open. */
    bool getDeviceInfo (DeviceInfo& info) const;

    /** Switches the open device to `sampleRate` if it offers it (PLAN.md
        O10), reopening it. Returns true if it runs at that rate afterwards. */
    bool setSampleRate (double sampleRate);

    /** Whether the open device plays through headphones (OutputRoute). */
    OutputRoute::Headphones outputIsHeadphones() const;

    /** The player's events are dispatched from a timer on the main thread,
        and at once when a file opened asynchronously is ready. */
    PlayerEngine& player() noexcept { return playerEngine; }

    /** Pauses the player and plays a sine tone at the given frequency on the
        open device instead. Returns false if no device is open or the
        frequency is not positive. */
    bool playTestTone (double frequencyHz);

    /** Stops the tone and reconnects the player. */
    void stopTestTone();

    /** Called on the main thread when the device list or the open device changes. */
    std::function<void()> onDeviceChanged;

    /** Starts analysing what the player plays for the visualizer, calling
        `callback` on an analysis thread (see AnalysisThread), or stops with
        a null callback. Replaces a running analysis, first waiting for its
        callback to return, so once this returns the old callback is never
        called again. */
    void setAnalysisCallback (AnalysisThread::Callback callback,
                              int numBands,
                              int waveformLength,
                              double framesPerSecond);

private:
    juce::AudioIODeviceType* outputDeviceType();
    void connectPlayer();

    void changeListenerCallback (juce::ChangeBroadcaster* source) override;
    void timerCallback() override;
    void handleAsyncUpdate() override;

    FormatRegistry formats;
    juce::TimeSliceThread readAheadThread { "anomp read-ahead" };
    PlayerEngine playerEngine { formats.manager(), &readAheadThread }; // Freed before the thread stops.
    juce::AudioDeviceManager deviceManager;
    juce::AudioSourcePlayer sourcePlayer;
    juce::ToneGeneratorAudioSource tone;
    std::unique_ptr<AnalysisThread> analysis; // Reads the player's tap; stopped first.

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (AudioEngine)
};
} // namespace anomp
