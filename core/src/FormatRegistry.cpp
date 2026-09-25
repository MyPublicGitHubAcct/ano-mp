#include "FormatRegistry.h"

namespace anomp
{
FormatRegistry::FormatRegistry()
{
    // WAV, AIFF, FLAC, Ogg Vorbis, MP3, plus CoreAudio (AAC/ALAC/M4A) on Apple.
    formats.registerBasicFormats();
}

bool FormatRegistry::canDecodeExtension (juce::String extension) const
{
    extension = extension.trim().toLowerCase();
    if (extension.isEmpty())
        return false;
    if (! extension.startsWithChar ('.'))
        extension = "." + extension;

    return supportedExtensions().contains (extension);
}

juce::StringArray FormatRegistry::supportedExtensions() const
{
    juce::StringArray result;
    for (auto* format : formats)
        for (auto& ext : format->getFileExtensions())
            result.addIfNotAlreadyThere (ext.toLowerCase());
    return result;
}
} // namespace anomp
