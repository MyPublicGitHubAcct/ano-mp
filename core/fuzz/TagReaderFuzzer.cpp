// libFuzzer target for the tag reader (PLAN.md H5): TagLib's part of
// readTags and readFileInfo over the input, held in memory as a
// TagLib::ByteVectorStream instead of a file.

#include "TagReader.h"

#include <tbytevectorstream.h>

#include <cstddef>
#include <cstdint>

extern "C" int LLVMFuzzerTestOneInput (const uint8_t* data, size_t size)
{
    const TagLib::ByteVector bytes (reinterpret_cast<const char*> (data), static_cast<unsigned int> (size));

    {
        TagLib::ByteVectorStream stream (bytes);
        anomp::TrackTags tags;
        anomp::readTags (stream, anomp::TagParts::picture | anomp::TagParts::lyrics | anomp::TagParts::chapters, tags);
    }
    {
        TagLib::ByteVectorStream stream (bytes);
        anomp::FileInfo info;
        bool tagged = false;
        anomp::readFileInfo (stream, info, tagged);
    }
    return 0;
}
