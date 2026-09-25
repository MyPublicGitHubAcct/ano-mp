#include "anomp/anomp.h"
#include "FormatRegistry.h"

namespace
{
anomp::FormatRegistry& registry()
{
    static anomp::FormatRegistry instance;
    return instance;
}
} // namespace

extern "C" const char* anomp_version (void)
{
    return "0.1.0";
}

extern "C" int anomp_can_decode_extension (const char* extension)
{
    if (extension == nullptr)
        return 0;
    return registry().canDecodeExtension (juce::String::fromUTF8 (extension)) ? 1 : 0;
}
