#pragma once

#include <juce_core/juce_core.h>

namespace anomp
{
/** What the file system says about a file without reading it (PLAN.md H12).

    With "Optimize Mac Storage", iCloud Drive (and other file providers)
    keep dataless placeholders: the file is listed with its full size, but
    its contents are downloaded only when something reads it. Reading tags
    from a whole library of them would download it all.

    One implementation per platform: FileStatus_apple.cpp and
    FileStatus_none.cpp. May be used from any thread.
*/
struct FileStatus
{
    enum class Dataless
    {
        no,     ///< Its contents are on this machine (or the platform has no placeholders).
        yes,    ///< A placeholder: reading it downloads it first.
        unknown ///< It couldn't be checked (it doesn't exist, or can't be reached).
    };

    /** Checks `file` with stat(), which doesn't download it. */
    static Dataless isDataless (const juce::File& file);
};
} // namespace anomp
