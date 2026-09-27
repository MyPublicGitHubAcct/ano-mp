#pragma once

#include <juce_core/juce_core.h>

namespace anomp
{
/** Where the sound goes, as far as the OS says (PLAN.md O11): whether the
    output device plays through headphones, so crossfeed can turn itself on.
    One implementation per OS: OutputRoute_apple.mm (Core Audio's data
    source on macOS, the AVAudioSession route on iOS) and OutputRoute_none.cpp.
*/
struct OutputRoute
{
    enum class Headphones
    {
        no,
        yes,
        unknown
    };

    /** Whether the output device called `deviceName` (as the audio device
        type names it) plays through headphones. */
    static Headphones headphones (const juce::String& deviceName);
};
} // namespace anomp
