#pragma once

#include <juce_core/juce_core.h>

#include <memory>

namespace anomp
{
/** Durable access to a folder the user picked.

    A sandboxed app (the macOS App Sandbox, iOS) may read a folder the user
    picked only until it quits, unless it saves a security-scoped bookmark
    and resolves that in later sessions. A bookmark also follows the folder
    when it is moved or renamed on its volume. Where there is no sandbox, a
    bookmark just holds the path.

    One implementation per platform: FolderAccess_apple.mm and
    FolderAccess_unsandboxed.cpp. May be used from any thread.
*/
class FolderAccess
{
public:
    /** Creates a bookmark for `folder`, which the app must be able to read
        now (the user just picked it, or a FolderAccess for it is open):
        read-only, or `writable` for a folder the app writes to (recordings,
        PLAN.md X6). Returns an error message, or an empty string on success. */
    static juce::String createBookmark (const juce::File& folder, juce::MemoryBlock& bookmark, bool writable = false);

    /** Resolves `bookmark` and starts accessing its folder until the result
        is destroyed. Returns null and sets `error` on failure, e.g. when the
        folder was deleted or its volume is not mounted. */
    static std::unique_ptr<FolderAccess> start (const juce::MemoryBlock& bookmark, juce::String& error);

    ~FolderAccess();

    /** Where the folder is now, which may differ from where it was when the
        bookmark was made. */
    const juce::File& getFolder() const noexcept { return folder; }

    /** True if the bookmark should be replaced: create a new one for
        getFolder() while this access is open, and save it instead. */
    bool isStale() const noexcept { return stale; }

private:
    FolderAccess (juce::File folderIn, bool staleIn, void* scopeIn) noexcept
        : folder (std::move (folderIn)),
          stale (staleIn),
          scope (scopeIn)
    {
    }

    juce::File folder;
    bool stale = false;
    void* scope = nullptr; /**< Platform handle to release, or null. */

    JUCE_DECLARE_NON_COPYABLE (FolderAccess)
};
} // namespace anomp
