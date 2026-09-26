#include "AudioEngine.h"

namespace anomp
{
namespace
{
constexpr float testToneAmplitude = 0.1f;
constexpr int playerEventIntervalMs = 50;
}

AudioEngine::AudioEngine()
{
    readAheadThread.startThread (juce::Thread::Priority::high);
    sourcePlayer.setSource (&playerEngine);
    deviceManager.addChangeListener (this);
    startTimer (playerEventIntervalMs);
}

AudioEngine::~AudioEngine()
{
    stopTimer();
    deviceManager.removeChangeListener (this);
    sourcePlayer.setSource (nullptr);
    deviceManager.removeAudioCallback (&sourcePlayer);
    deviceManager.closeAudioDevice();
}

juce::String AudioEngine::openDefaultDevice()
{
    auto error = deviceManager.initialiseWithDefaultDevices (0, 2);

    if (error.isEmpty() && deviceManager.getCurrentAudioDevice() == nullptr)
        error = "No audio output device available";

    if (error.isEmpty())
    {
        deviceManager.removeAudioCallback (&sourcePlayer);
        deviceManager.addAudioCallback (&sourcePlayer);
    }

    return error;
}

juce::String AudioEngine::currentDeviceName() const
{
    if (auto* device = deviceManager.getCurrentAudioDevice())
        return device->getName();

    return {};
}

bool AudioEngine::playTestTone (double frequencyHz)
{
    if (deviceManager.getCurrentAudioDevice() == nullptr || ! (frequencyHz > 0.0))
        return false;

    // Detach first so the audio thread never reads the tone while it changes.
    playerEngine.pause();
    sourcePlayer.setSource (nullptr);
    tone.setFrequency (frequencyHz);
    tone.setAmplitude (testToneAmplitude);
    sourcePlayer.setSource (&tone);
    return true;
}

void AudioEngine::stopTestTone()
{
    sourcePlayer.setSource (&playerEngine);
}

void AudioEngine::changeListenerCallback (juce::ChangeBroadcaster*)
{
    if (onDeviceChanged)
        onDeviceChanged();
}

void AudioEngine::timerCallback()
{
    playerEngine.dispatchEvents();
}
} // namespace anomp
