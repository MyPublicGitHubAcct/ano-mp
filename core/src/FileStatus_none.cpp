// FileStatus where the platform keeps no placeholders the app knows of
// (Linux, Windows): every file that exists is here.

#include "FileStatus.h"

namespace anomp
{
FileStatus::Dataless FileStatus::isDataless (const juce::File& file)
{
    return file.exists() ? Dataless::no : Dataless::unknown;
}
} // namespace anomp
