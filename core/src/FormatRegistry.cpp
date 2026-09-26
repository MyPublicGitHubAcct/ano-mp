#include "FormatRegistry.h"
#include "FFmpegAudioFormat.h"

namespace anomp
{
FormatRegistry::FormatRegistry()
{
    // FFmpeg decodes every format on every platform (PLAN.md §4.3).
    formats.registerFormat (new FFmpegAudioFormat(), true);
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
