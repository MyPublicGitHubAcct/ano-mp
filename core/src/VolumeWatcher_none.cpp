// VolumeWatcher where nothing reports volumes yet (Linux and Windows until
// Phases 9 and 10): it is created and destroyed, and reports nothing.

#include "VolumeWatcher.h"

namespace anomp
{
struct VolumeWatcher::Platform
{
};

VolumeWatcher::VolumeWatcher (Handler handlerIn)
    : handler (std::move (handlerIn)),
      platform (std::make_unique<Platform>())
{
}

VolumeWatcher::~VolumeWatcher() = default;

bool VolumeWatcher::isSupported() { return false; }
} // namespace anomp
