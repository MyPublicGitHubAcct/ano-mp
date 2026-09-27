// DockMenu where the OS has no Dock menu the core fills (Linux and Windows
// until Phases 9 and 10; a taskbar jump list may follow): the base Backend,
// which shows nothing.

#include "DockMenu.h"

namespace anomp
{
std::unique_ptr<DockMenu::Backend> DockMenu::createPlatformBackend() { return std::make_unique<Backend>(); }

bool DockMenu::isSupported() { return false; }
} // namespace anomp
