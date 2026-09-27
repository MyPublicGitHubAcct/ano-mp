#include "OutputRoute.h"

#include <TargetConditionals.h>

#if TARGET_OS_IPHONE
#import <AVFoundation/AVFoundation.h>
#else
#include <CoreAudio/CoreAudio.h>

#include <vector>
#endif

namespace anomp
{
#if TARGET_OS_IPHONE

OutputRoute::Headphones OutputRoute::headphones (const juce::String&)
{
    for (AVAudioSessionPortDescription* output in AVAudioSession.sharedInstance.currentRoute.outputs)
    {
        NSString* port = output.portType;
        if ([port isEqualToString:AVAudioSessionPortHeadphones] ||
            [port isEqualToString:AVAudioSessionPortBluetoothA2DP] ||
            [port isEqualToString:AVAudioSessionPortBluetoothLE])
            return Headphones::yes;
    }
    return Headphones::no;
}

#else

namespace
{
template <typename T>
bool getProperty (AudioObjectID object, AudioObjectPropertySelector selector, AudioObjectPropertyScope scope, T& value)
{
    const AudioObjectPropertyAddress address { selector, scope, kAudioObjectPropertyElementMain };
    UInt32 size = sizeof (T);
    return AudioObjectGetPropertyData (object, &address, 0, nullptr, &size, &value) == noErr;
}

juce::String deviceName (AudioObjectID device)
{
    CFStringRef name = nullptr;
    if (! getProperty (device, kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, name) || name == nullptr)
        return {};
    const auto text = juce::String::fromCFString (name);
    CFRelease (name);
    return text;
}

std::vector<AudioObjectID> devices()
{
    const AudioObjectPropertyAddress address { kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal,
                                               kAudioObjectPropertyElementMain };
    UInt32 size = 0;
    if (AudioObjectGetPropertyDataSize (kAudioObjectSystemObject, &address, 0, nullptr, &size) != noErr)
        return {};
    std::vector<AudioObjectID> result (size / sizeof (AudioObjectID));
    if (AudioObjectGetPropertyData (kAudioObjectSystemObject, &address, 0, nullptr, &size, result.data()) != noErr)
        return {};
    result.resize (size / sizeof (AudioObjectID));
    return result;
}
} // namespace

OutputRoute::Headphones OutputRoute::headphones (const juce::String& name)
{
    for (const auto device : devices())
    {
        if (deviceName (device) != name)
            continue;

        UInt32 transport = 0;
        if (! getProperty (device, kAudioDevicePropertyTransportType, kAudioObjectPropertyScopeGlobal, transport))
            return Headphones::unknown;

        // The built-in output says which jack it plays through.
        if (transport == kAudioDeviceTransportTypeBuiltIn)
        {
            UInt32 source = 0;
            if (! getProperty (device, kAudioDevicePropertyDataSource, kAudioDevicePropertyScopeOutput, source))
                return Headphones::no; // A built-in output without sources, e.g. a Mac mini's speaker.
            return source == 'hdpn' ? Headphones::yes : Headphones::no;
        }

        // Bluetooth devices are headphones as often as speakers; the OS
        // doesn't say which.
        return Headphones::unknown;
    }
    return Headphones::unknown;
}

#endif
} // namespace anomp
