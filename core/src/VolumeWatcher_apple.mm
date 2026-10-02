// VolumeWatcher on macOS: NSWorkspace posts a notification on the main
// thread for each volume mounted or unmounted, including network shares and
// disk images. iOS mounts no volumes, so nothing is reported there.
// Compiled with ARC (core/CMakeLists.txt).

#include "VolumeWatcher.h"

#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#endif

#include <vector>

namespace anomp
{
struct VolumeWatcher::Platform
{
#if TARGET_OS_OSX
    std::vector<id> observers;
#endif
    /** Cleared on destruction, so a notification already queued for the
        main thread does nothing. Main thread only, like the blocks. */
    std::shared_ptr<bool> alive = std::make_shared<bool> (true);
};

VolumeWatcher::VolumeWatcher (Handler handlerIn)
    : handler (std::move (handlerIn)),
      platform (std::make_unique<Platform>())
{
#if TARGET_OS_OSX
    @autoreleasepool
    {
        NSNotificationCenter* center = NSWorkspace.sharedWorkspace.notificationCenter;
        auto alive = platform->alive;
        auto* owner = this;
        auto observe = [&] (NSNotificationName name, bool mounted)
        {
            id token = [center
                addObserverForName:name
                            object:nil
                             queue:NSOperationQueue.mainQueue
                        usingBlock:^(NSNotification* note) {
                          if (! *alive)
                              return;
                          NSURL* url = note.userInfo[NSWorkspaceVolumeURLKey];
                          const char* path = url != nil ? url.path.UTF8String : nullptr;
                          owner->notify (mounted, path != nullptr ? juce::String::fromUTF8 (path) : juce::String());
                        }];
            platform->observers.push_back (token);
        };
        observe (NSWorkspaceDidMountNotification, true);
        observe (NSWorkspaceDidUnmountNotification, false);
    }
#endif
}

VolumeWatcher::~VolumeWatcher()
{
    *platform->alive = false;
#if TARGET_OS_OSX
    @autoreleasepool
    {
        NSNotificationCenter* center = NSWorkspace.sharedWorkspace.notificationCenter;
        for (id token : platform->observers)
            [center removeObserver:token];
    }
#endif
}

bool VolumeWatcher::isSupported()
{
#if TARGET_OS_OSX
    return true;
#else
    return false;
#endif
}
} // namespace anomp
