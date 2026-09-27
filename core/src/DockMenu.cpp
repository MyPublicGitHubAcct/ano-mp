#include "DockMenu.h"

namespace anomp
{
DockMenu::DockMenu (Handler onChoice, std::unique_ptr<Backend> platform)
    : handler (std::move (onChoice)),
      backend (platform != nullptr ? std::move (platform) : std::make_unique<Backend>())
{
    backend->attach (*this);
}

DockMenu::~DockMenu() { backend->detach(); }

void DockMenu::setItems (std::vector<Item> newItems)
{
    items = std::move (newItems);
    backend->update();
}

bool DockMenu::choose (int id)
{
    for (const auto& item : items)
    {
        if (item.id == id && item.enabled && item.title.isNotEmpty())
        {
            if (handler)
                handler (id);
            return handler != nullptr;
        }
    }
    return false;
}
} // namespace anomp
