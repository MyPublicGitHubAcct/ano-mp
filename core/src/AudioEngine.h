#pragma once

#include <juce_audio_devices/juce_audio_devices.h>

#include <functional>

namespace anomp
{
/** Owns JUCE's runtime and the audio output device.

    Phase 0 only plays a test tone; Phase 1 grows this into the player.
    Must be created, used and destroyed on the main thread: the constructor
    starts JUCE's message loop integration there, and device-change
    notifications are delivered on that thread.
*/
class AudioEngine final : private juce::ChangeListener
{
public:
    AudioEngine();
    ~AudioEngine() override;

    /** Opens the default output device. Returns an empty string on success,
        otherwise the error. */
    juce::String openDefaultDevice();

    /** Name of the open output device, or empty if none is open. */
    juce::String currentDeviceName() const;

    /** Starts a sine tone at the given frequency on the open device.
        Returns false if no device is open or the frequency is not positive. */
    bool playTestTone (double frequencyHz);

    void stopTestTone();

    /** Called on the main thread when the device list or the open device changes. */
    std::function<void()> onDeviceChanged;

private:
    void changeListenerCallback (juce::ChangeBroadcaster* source) override;

    // Declared first so JUCE is initialised before, and shut down after,
    // everything else here.
    juce::ScopedJuceInitialiser_GUI juceInitialiser;
    juce::AudioDeviceManager deviceManager;
    juce::AudioSourcePlayer sourcePlayer;
    juce::ToneGeneratorAudioSource tone;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (AudioEngine)
};
} // namespace anomp
