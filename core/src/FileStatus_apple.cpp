// FileStatus on Apple platforms: a dataless placeholder has SF_DATALESS set
// in its stat flags (sys/stat.h; macOS 10.15, iOS 13).

#include "FileStatus.h"

#include <sys/stat.h>

namespace anomp
{
FileStatus::Dataless FileStatus::isDataless (const juce::File& file)
{
    struct stat status {};
    if (stat (file.getFullPathName().toRawUTF8(), &status) != 0)
        return Dataless::unknown;
    return (status.st_flags & SF_DATALESS) != 0 ? Dataless::yes : Dataless::no;
}
} // namespace anomp
