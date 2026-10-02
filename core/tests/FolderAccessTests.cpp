#include <catch2/catch_test_macros.hpp>

#include "anomp/anomp.h"

#include <juce_core/juce_core.h>

#include <filesystem>
#include <memory>
#include <string>
#include <vector>

namespace
{
/** A new, empty folder in the temp directory, deleted at the end of the test. */
struct TempFolder
{
    TempFolder()
    {
        // Bookmarks resolve to real paths, and macOS's temp folder is under
        // the /var -> /private/var symlink.
        const auto temp = juce::File::getSpecialLocation (juce::File::tempDirectory);
        const auto real = std::filesystem::canonical (temp.getFullPathName().toStdString());
        parent = juce::File (juce::String::fromUTF8 (real.string().c_str()))
                     .getNonexistentChildFile ("anomp-folder-access", {}, false);
        REQUIRE (parent.createDirectory());
        // Precomposed, as JUCE writes it: NSURL spells resolved names
        // decomposed, and the path must still match.
        folder = parent.getChildFile ("Música");
        REQUIRE (folder.createDirectory());
    }

    ~TempFolder() { parent.deleteRecursively(); }

    juce::File parent, folder;
};

struct AccessDeleter
{
    void operator() (anomp_folder_access* access) const { anomp_folder_access_stop (access); }
};

using Access = std::unique_ptr<anomp_folder_access, AccessDeleter>;

std::vector<unsigned char> createBookmark (const juce::File& folder, std::string& error)
{
    char buffer[512] = "unchanged";
    auto* bookmark = anomp_bookmark_create (folder.getFullPathName().toRawUTF8(), buffer, sizeof (buffer));
    error = buffer;
    if (bookmark == nullptr)
        return {};

    std::vector<unsigned char> bytes (bookmark->data, bookmark->data + bookmark->size);
    anomp_bookmark_free (bookmark);
    return bytes;
}

Access startAccess (const std::vector<unsigned char>& bookmark, std::string& error)
{
    char buffer[512] = "unchanged";
    Access access (anomp_folder_access_start (bookmark.data(), bookmark.size(), buffer, sizeof (buffer)));
    error = buffer;
    return access;
}

juce::File accessedFolder (const Access& access)
{
    return juce::File (juce::String::fromUTF8 (anomp_folder_access_path (access.get())));
}
} // namespace

TEST_CASE ("Folder bookmarks resolve to their folder", "[folder-access]")
{
    TempFolder temp;
    std::string error;
    const auto bookmark = createBookmark (temp.folder, error);
    REQUIRE (error.empty());
    REQUIRE_FALSE (bookmark.empty());

    const auto access = startAccess (bookmark, error);
    REQUIRE (access != nullptr);
    CHECK (error.empty());
    CHECK (accessedFolder (access) == temp.folder);
    CHECK (anomp_folder_access_is_stale (access.get()) == 0);

    // Two accesses to one folder may be open at once.
    const auto second = startAccess (bookmark, error);
    REQUIRE (second != nullptr);
    CHECK (accessedFolder (second) == temp.folder);
}

#if JUCE_MAC || JUCE_IOS
TEST_CASE ("Folder bookmarks follow a moved folder", "[folder-access]")
{
    TempFolder temp;
    std::string error;
    const auto bookmark = createBookmark (temp.folder, error);
    REQUIRE_FALSE (bookmark.empty());

    const auto moved = temp.parent.getChildFile ("Moved");
    REQUIRE (temp.folder.moveFileTo (moved));

    const auto access = startAccess (bookmark, error);
    REQUIRE (access != nullptr);
    CHECK (accessedFolder (access) == moved);

    // A fresh bookmark for the new place, as the host makes when the old one
    // is stale, resolves there too.
    const auto refreshed = createBookmark (accessedFolder (access), error);
    REQUIRE_FALSE (refreshed.empty());
    CHECK (accessedFolder (startAccess (refreshed, error)) == moved);
}
#endif

TEST_CASE ("Folder bookmarks report errors", "[folder-access]")
{
    TempFolder temp;
    std::string error;

    CHECK (createBookmark (temp.folder.getChildFile ("missing"), error).empty());
    CHECK (error.rfind ("Cannot create a bookmark for ", 0) == 0);

    char buffer[256] = {};
    CHECK (anomp_bookmark_create ("relative/folder", buffer, sizeof (buffer)) == nullptr);
    CHECK (std::string (buffer).rfind ("Path is not absolute", 0) == 0);
    CHECK (anomp_bookmark_create (nullptr, buffer, sizeof (buffer)) == nullptr);
    CHECK (std::string (buffer) == "Null path");
    CHECK (anomp_bookmark_create (nullptr, nullptr, 0) == nullptr);

    CHECK (startAccess ({}, error) == nullptr);
    CHECK (error == "Empty bookmark");
    CHECK (startAccess ({ 'n', 'o', 't', ' ', 'a', ' ', 'b', 'o', 'o', 'k', 'm', 'a', 'r', 'k' }, error) == nullptr);
    CHECK (error.rfind ("Cannot resolve the bookmark", 0) == 0);
    CHECK (anomp_folder_access_start (nullptr, 4, buffer, sizeof (buffer)) == nullptr);
    CHECK (std::string (buffer) == "Null bookmark");

    // A bookmark whose folder was deleted no longer resolves.
    const auto bookmark = createBookmark (temp.folder, error);
    REQUIRE_FALSE (bookmark.empty());
    REQUIRE (temp.folder.deleteRecursively());
    CHECK (startAccess (bookmark, error) == nullptr);
    CHECK (error.rfind ("Cannot resolve the bookmark", 0) == 0);
}

TEST_CASE ("Folder access functions accept null", "[folder-access][c-api]")
{
    CHECK (std::string (anomp_folder_access_path (nullptr)).empty());
    CHECK (anomp_folder_access_is_stale (nullptr) == 0);
    anomp_folder_access_stop (nullptr);
    anomp_bookmark_free (nullptr);
}

TEST_CASE ("C API tells a cloud placeholder from a file that's here", "[folders][dataless]")
{
    // A real placeholder needs a file provider (iCloud Drive); the owner
    // checks one in the app (docs/step4-checklist.md, Step 5).
    const auto fixture = juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile ("flac-44k.flac");
    CHECK (anomp_file_is_dataless (fixture.getFullPathName().toRawUTF8()) == 0);
    CHECK (anomp_file_is_dataless (fixture.getSiblingFile ("missing.flac").getFullPathName().toRawUTF8()) == -1);
    CHECK (anomp_file_is_dataless ("flac-44k.flac") == -1);
    CHECK (anomp_file_is_dataless (nullptr) == -1);
}
