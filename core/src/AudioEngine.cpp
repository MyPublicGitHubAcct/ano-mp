#include "AudioEngine.h"

namespace anomp
{
namespace
{
constexpr float testToneAmplitude = 0.1f;
constexpr int playerEventIntervalMs = 50;
} // namespace

AudioEngine::AudioEngine()
{
    readAheadThread.startThread (juce::Thread::Priority::high);
    sourcePlayer.setSource (&playerEngine);
    deviceManager.addChangeListener (this);
    startTimer (playerEventIntervalMs);
}

AudioEngine::~AudioEngine()
{
    analysis.reset();
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
        connectPlayer();

    return error;
}

void AudioEngine::connectPlayer()
{
    deviceManager.removeAudioCallback (&sourcePlayer);
    deviceManager.addAudioCallback (&sourcePlayer);
}

juce::AudioIODeviceType* AudioEngine::outputDeviceType()
{
    // Creates the device types on first use.
    deviceManager.getAvailableDeviceTypes();
    return deviceManager.getCurrentDeviceTypeObject();
}

juce::StringArray AudioEngine::outputDeviceNames()
{
    auto* type = outputDeviceType();
    if (type == nullptr)
        return {};

    type->scanForDevices();
    return type->getDeviceNames (false);
}

juce::String AudioEngine::openDevice (const juce::String& name, int bufferSize)
{
    auto* type = outputDeviceType();
    if (type == nullptr)
        return "No audio output device available";

    const auto names = outputDeviceNames();
    auto deviceName = name;

    if (deviceName.isEmpty())
    {
        const auto index = type->getDefaultDeviceIndex (false);
        if (! juce::isPositiveAndBelow (index, names.size()))
            return "No audio output device available";
        deviceName = names[index];
    }
    else if (! names.contains (deviceName))
    {
        return "No output device called \"" + deviceName + "\"";
    }

    auto setup = deviceManager.getAudioDeviceSetup();
    setup.outputDeviceName = deviceName;
    setup.inputDeviceName = {};
    setup.useDefaultOutputChannels = true;
    setup.useDefaultInputChannels = false;
    setup.inputChannels.clear();
    setup.bufferSize = juce::jmax (0, bufferSize); // 0: the device's default.
    setup.sampleRate = 0.0;                        // The device's current rate.

    auto error = deviceManager.setAudioDeviceSetup (setup, true);

    if (error.isEmpty() && deviceManager.getCurrentAudioDevice() == nullptr)
        error = "Cannot open " + deviceName;

    if (error.isEmpty())
        connectPlayer();

    return error;
}

bool AudioEngine::getDeviceInfo (DeviceInfo& info) const
{
    auto* device = deviceManager.getCurrentAudioDevice();
    if (device == nullptr)
        return false;

    info.bufferSize = device->getCurrentBufferSizeSamples();
    info.defaultBufferSize = device->getDefaultBufferSize();
    info.bufferSizes = device->getAvailableBufferSizes();
    info.bufferSizes.sort();
    info.sampleRate = device->getCurrentSampleRate();
    info.outputLatencySeconds =
        info.sampleRate > 0.0 ? (device->getOutputLatencyInSamples() + info.bufferSize) / info.sampleRate : 0.0;
    return true;
}

bool AudioEngine::setSampleRate (double sampleRate)
{
    auto* device = deviceManager.getCurrentAudioDevice();
    if (device == nullptr || ! (sampleRate > 0.0))
        return false;
    if (juce::approximatelyEqual (device->getCurrentSampleRate(), sampleRate))
        return true;
    if (! device->getAvailableSampleRates().contains (sampleRate))
        return false;

    auto setup = deviceManager.getAudioDeviceSetup();
    setup.sampleRate = sampleRate;
    if (deviceManager.setAudioDeviceSetup (setup, true).isNotEmpty())
        return false;
    connectPlayer();

    device = deviceManager.getCurrentAudioDevice();
    return device != nullptr && juce::approximatelyEqual (device->getCurrentSampleRate(), sampleRate);
}

OutputRoute::Headphones AudioEngine::outputIsHeadphones() const
{
    auto* device = deviceManager.getCurrentAudioDevice();
    return device != nullptr ? OutputRoute::headphones (device->getName()) : OutputRoute::Headphones::unknown;
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

void AudioEngine::stopTestTone() { sourcePlayer.setSource (&playerEngine); }

void AudioEngine::setAnalysisCallback (AnalysisThread::Callback callback,
                                       int numBands,
                                       int waveformLength,
                                       double framesPerSecond)
{
    analysis.reset();
    if (callback)
        analysis = std::make_unique<AnalysisThread> (playerEngine.getTap(), numBands, waveformLength, framesPerSecond,
                                                     std::move (callback));
}

void AudioEngine::changeListenerCallback (juce::ChangeBroadcaster*)
{
    if (onDeviceChanged)
        onDeviceChanged();
}

void AudioEngine::timerCallback() { playerEngine.dispatchEvents(); }
} // namespace anomp
