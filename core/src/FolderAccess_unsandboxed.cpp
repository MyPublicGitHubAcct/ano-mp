// FolderAccess where there is no sandbox (Linux, Windows): a bookmark is the
// folder's absolute path in UTF-8, and access is always open.

#include "FolderAccess.h"

namespace anomp
{
juce::String FolderAccess::createBookmark (const juce::File& folder, juce::MemoryBlock& bookmark, bool)
{
    if (! folder.isDirectory())
        return "Cannot create a bookmark for " + folder.getFullPathName() + ": not a folder";

    const auto path = folder.getFullPathName().toStdString();
    bookmark.replaceAll (path.data(), path.size());
    return {};
}

std::unique_ptr<FolderAccess> FolderAccess::start (const juce::MemoryBlock& bookmark, juce::String& error)
{
    if (bookmark.isEmpty())
    {
        error = "Empty bookmark";
        return nullptr;
    }

    const auto path =
        juce::String::fromUTF8 (static_cast<const char*> (bookmark.getData()), static_cast<int> (bookmark.getSize()));
    if (! juce::File::isAbsolutePath (path))
    {
        error = "Cannot resolve the bookmark: not an absolute path";
        return nullptr;
    }

    const juce::File folder (path);
    if (! folder.isDirectory())
    {
        error = "Cannot resolve the bookmark: " + path + " is not an available folder";
        return nullptr;
    }

    error.clear();
    return std::unique_ptr<FolderAccess> (new FolderAccess (folder, false, nullptr));
}

FolderAccess::~FolderAccess()
{
    juce::ignoreUnused (scope); // Always null: there is nothing to release.
}
} // namespace anomp
