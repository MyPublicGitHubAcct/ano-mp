// FolderAccess on macOS and iOS: security-scoped bookmarks. Compiled with ARC
// (core/CMakeLists.txt).

#include "FolderAccess.h"

#import <Foundation/Foundation.h>

#include <climits>
#include <cstdlib>

namespace anomp
{
namespace
{
#if TARGET_OS_OSX
// Read-only is all a player needs; the recordings' folder is written too.
constexpr NSURLBookmarkCreationOptions creationOptions =
    NSURLBookmarkCreationWithSecurityScope | NSURLBookmarkCreationSecurityScopeAllowOnlyReadAccess;
constexpr NSURLBookmarkCreationOptions writableCreationOptions = NSURLBookmarkCreationWithSecurityScope;
constexpr NSURLBookmarkResolutionOptions resolutionOptions = NSURLBookmarkResolutionWithSecurityScope
                                                             | NSURLBookmarkResolutionWithoutUI
                                                             | NSURLBookmarkResolutionWithoutMounting;
#else
// On iOS every bookmark carries its scope implicitly.
constexpr NSURLBookmarkCreationOptions creationOptions = 0;
constexpr NSURLBookmarkCreationOptions writableCreationOptions = 0;
constexpr NSURLBookmarkResolutionOptions resolutionOptions =
    NSURLBookmarkResolutionWithoutUI | NSURLBookmarkResolutionWithoutMounting;
#endif

juce::String describe (NSError* error)
{
    return error != nil ? juce::String::fromUTF8 (error.localizedDescription.UTF8String)
                        : juce::String ("Unknown error");
}

/** The path of a resolved URL as the file system spells it. NSURL gives
    names decomposed (NFD) even when they are stored precomposed; realpath
    gives the stored names, as the host's own path canonicalization does. */
juce::File realPath (NSURL* url)
{
    char buffer[PATH_MAX];
    if (realpath (url.fileSystemRepresentation, buffer) != nullptr)
        return juce::File (juce::String::fromUTF8 (buffer));
    return juce::File (juce::String::fromUTF8 (url.path.UTF8String));
}
} // namespace

juce::String FolderAccess::createBookmark (const juce::File& folder, juce::MemoryBlock& bookmark, bool writable)
{
    @autoreleasepool
    {
        NSString* path = [NSString stringWithUTF8String:folder.getFullPathName().toRawUTF8()];
        NSURL* url = [NSURL fileURLWithPath:path isDirectory:YES];
        NSError* error = nil;
        NSData* data = [url bookmarkDataWithOptions:writable ? writableCreationOptions : creationOptions
                     includingResourceValuesForKeys:nil
                                      relativeToURL:nil
                                              error:&error];
        if (data == nil)
            return "Cannot create a bookmark for " + folder.getFullPathName() + ": " + describe (error);

        bookmark.replaceAll (data.bytes, data.length);
        return {};
    }
}

std::unique_ptr<FolderAccess> FolderAccess::start (const juce::MemoryBlock& bookmark, juce::String& error)
{
    @autoreleasepool
    {
        if (bookmark.isEmpty())
        {
            error = "Empty bookmark";
            return nullptr;
        }

        NSData* data = [NSData dataWithBytes:bookmark.getData() length:bookmark.getSize()];
        BOOL stale = NO;
        NSError* resolveError = nil;
        NSURL* url = [NSURL URLByResolvingBookmarkData:data
                                               options:resolutionOptions
                                         relativeToURL:nil
                                   bookmarkDataIsStale:&stale
                                                 error:&resolveError];
        if (url == nil)
        {
            error = "Cannot resolve the bookmark: " + describe (resolveError);
            return nullptr;
        }

        // Returns NO when there is no scope to enter (e.g. outside the
        // sandbox); reading may still work, and fails clearly if not.
        void* scope = [url startAccessingSecurityScopedResource] ? (void*) CFBridgingRetain (url) : nullptr;
        error.clear();
        return std::unique_ptr<FolderAccess> (new FolderAccess (realPath (url), stale == YES, scope));
    }
}

FolderAccess::~FolderAccess()
{
    if (scope == nullptr)
        return;

    @autoreleasepool
    {
        NSURL* url = (NSURL*) CFBridgingRelease (scope);
        [url stopAccessingSecurityScopedResource];
    }
}
} // namespace anomp
