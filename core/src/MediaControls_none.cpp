// MediaControls where the core has no OS integration yet (Linux and Windows
// until Phases 9 and 10): the base Backend, which does nothing.

#include "MediaControls.h"

namespace anomp
{
std::unique_ptr<MediaControls::Backend> MediaControls::createPlatformBackend() { return std::make_unique<Backend>(); }

bool MediaControls::isSupported() { return false; }
} // namespace anomp
