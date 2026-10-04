// VolumeWatcher's NSWorkspace path on macOS: the C API tests call notify
// directly, which skips the notification blocks. Compiled with ARC
// (core/tests/CMakeLists.txt).

#include <catch2/catch_test_macros.hpp>

#include "VolumeWatcher.h"

#import <AppKit/AppKit.h>

#include <string>
#include <utility>
#include <vector>

namespace
{
void post (NSNotificationName name, NSString* path)
{
    @autoreleasepool
    {
        [NSWorkspace.sharedWorkspace.notificationCenter
            postNotificationName:name
                          object:NSWorkspace.sharedWorkspace
                        userInfo:@ { NSWorkspaceVolumeURLKey : [NSURL fileURLWithPath:path isDirectory:YES] }];
    }
}

/** Overwrites the stack the watcher's constructor used, as the run loop
    does before a real volume mounts. */
[[gnu::noinline]] void clobberStack()
{
    volatile unsigned char bytes[4096];
    for (auto& byte : bytes)
        byte = 0;
}
} // namespace

TEST_CASE ("VolumeWatcher reports NSWorkspace's mount notifications after its constructor returns", "[volumes]")
{
    std::vector<std::pair<bool, std::string>> seen;
    {
        anomp::VolumeWatcher watcher ([&seen] (bool mounted, const juce::String& path)
                                      { seen.emplace_back (mounted, path.toStdString()); });
        clobberStack();

        // Posted on the main thread to observers on the main queue, so the
        // blocks run before post returns.
        post (NSWorkspaceDidMountNotification, @"/Volumes/My Shared Files");
        post (NSWorkspaceDidUnmountNotification, @"/Volumes/My Shared Files");
    }
    post (NSWorkspaceDidMountNotification, @"/Volumes/After");

    CHECK (seen
           == std::vector<std::pair<bool, std::string>> { { true, "/Volumes/My Shared Files" },
                                                          { false, "/Volumes/My Shared Files" } });
}
