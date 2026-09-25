#pragma once

#include <juce_audio_formats/juce_audio_formats.h>

namespace anomp
{
/** Owns the AudioFormatManager with every format the player supports. */
class FormatRegistry
{
public:
    FormatRegistry();

    juce::AudioFormatManager& manager() noexcept { return formats; }

    /** True if a registered format handles this extension ("flac" or ".flac"). */
    bool canDecodeExtension (juce::String extension) const;

    /** Lower-case extensions with a leading dot, e.g. ".flac". */
    juce::StringArray supportedExtensions() const;

private:
    juce::AudioFormatManager formats;
};
} // namespace anomp
