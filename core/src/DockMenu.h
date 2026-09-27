#pragma once

#include <juce_core/juce_core.h>

#include <functional>
#include <memory>
#include <vector>

namespace anomp
{
/** The menu the OS shows for the app's Dock icon (PLAN.md F6): transport
    and the current track, as the host lists them.

    Platform-neutral like MediaControls: this class keeps the items and
    gates the choices that come back; a Backend per OS shows them
    (DockMenu_apple.mm, which answers AppKit's applicationDockMenu: on the
    app delegate, and DockMenu_none.cpp). One menu exists at a time.

    Main thread only; choices reach the handler on the main thread, never
    from inside a call to this class. */
class DockMenu
{
public:
    struct Item
    {
        int id = 0;
        juce::String title; ///< Empty for a separator.
        bool enabled = true, checked = false;
    };

    struct Backend
    {
        virtual ~Backend() = default;
        /** Starts showing `owner`'s items and sending choices to `owner.choose`. */
        virtual void attach (DockMenu& owner) { juce::ignoreUnused (owner); }
        virtual void detach() {}
        /** The items changed. */
        virtual void update() {}
    };

    using Handler = std::function<void (int id)>;

    static std::unique_ptr<Backend> createPlatformBackend();
    static bool isSupported();

    explicit DockMenu (Handler handler, std::unique_ptr<Backend> backend = createPlatformBackend());
    ~DockMenu();

    void setItems (std::vector<Item> newItems);
    const std::vector<Item>& getItems() const noexcept { return items; }

    /** An item was chosen: calls the handler if `id` is an enabled item.
        Returns whether it did. */
    bool choose (int id);

private:
    Handler handler;
    std::unique_ptr<Backend> backend;
    std::vector<Item> items;

    JUCE_DECLARE_NON_COPYABLE (DockMenu)
};
} // namespace anomp
