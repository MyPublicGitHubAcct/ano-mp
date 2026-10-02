#pragma once

#include <juce_core/juce_core.h>

#include <functional>
#include <memory>

namespace anomp
{
/** Tells the host when a volume (a drive, a disk image, a network share)
    is mounted or unmounted, so it can check library folders on it again
    (PLAN.md H22): a folder whose drive comes back is rescanned without a
    restart.

    One implementation per platform, next to FolderAccess:
    VolumeWatcher_apple.mm (NSWorkspace's mount notifications on macOS;
    nothing on iOS, which mounts no volumes) and VolumeWatcher_none.cpp.

    Main thread only: create and destroy it there, and the handler is
    called there, never from inside a call to this class. */
class VolumeWatcher
{
public:
    /** `path` is the volume's mount point (UTF-8), empty if unknown. */
    using Handler = std::function<void (bool mounted, const juce::String& path)>;

    explicit VolumeWatcher (Handler handler);
    ~VolumeWatcher();

    /** Whether this platform reports volumes at all. */
    static bool isSupported();

    /** Reports a volume as the platform would; for the platform code and
        for tests. */
    void notify (bool mounted, const juce::String& path)
    {
        if (handler)
            handler (mounted, path);
    }

private:
    struct Platform;

    Handler handler;
    std::unique_ptr<Platform> platform;

    JUCE_DECLARE_NON_COPYABLE (VolumeWatcher)
};
} // namespace anomp
