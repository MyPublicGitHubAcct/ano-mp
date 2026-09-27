// DockMenu on macOS: AppKit asks the app delegate for the Dock menu
// (applicationDockMenu:). The delegate belongs to the host's windowing
// library (tao, under Tauri), which doesn't answer it, so the method is added
// to the delegate's class at run time, once, returning a menu built from the
// items of the DockMenu showing. On iOS there is no Dock: nothing is shown.
// Compiled with ARC (core/CMakeLists.txt).

#include "DockMenu.h"

#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#import <objc/runtime.h>
#endif

#if TARGET_OS_OSX
/** Receives the menu's choices and passes them to the owner, if it is
    still showing. */
@interface AnompDockMenuTarget : NSObject
@property (nonatomic, assign) anomp::DockMenu* owner;
- (void)choose:(NSMenuItem*)item;
@end

@implementation AnompDockMenuTarget
- (void)choose:(NSMenuItem*)item
{
    if (self.owner != nullptr)
        self.owner->choose (static_cast<int> (item.tag));
}
@end
#endif

namespace anomp
{
namespace
{
#if TARGET_OS_OSX
/** The target of the DockMenu showing; nil when none is. */
AnompDockMenuTarget* showing = nil;

NSMenu* buildMenu()
{
    auto* owner = showing != nil ? showing.owner : nullptr;
    if (owner == nullptr || owner->getItems().empty())
        return nil;

    NSMenu* menu = [[NSMenu alloc] initWithTitle:@""];
    menu.autoenablesItems = NO;
    for (const auto& item : owner->getItems())
    {
        if (item.title.isEmpty())
        {
            [menu addItem:[NSMenuItem separatorItem]];
            continue;
        }
        NSString* title = [NSString stringWithUTF8String:item.title.toRawUTF8()];
        if (title == nil)
            continue;
        NSMenuItem* entry = [[NSMenuItem alloc] initWithTitle:(NSString* _Nonnull) title
                                                       action:@selector (choose:)
                                                keyEquivalent:@""];
        entry.target = showing;
        entry.tag = item.id;
        entry.enabled = item.enabled;
        entry.state = item.checked ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:entry];
    }
    return menu;
}

NSMenu* dockMenu (id, SEL, NSApplication*) { return buildMenu(); }

/** Adds applicationDockMenu: to the app delegate's class, once there is a
    delegate. Returns whether it is there. */
bool installOnDelegate()
{
    static bool installed = false;
    if (installed || NSApp == nil)
        return installed;
    id delegate = [NSApp delegate];
    if (delegate == nil)
        return false;
    const auto selector = @selector (applicationDockMenu:);
    Class delegateClass = object_getClass (delegate);
    // A delegate that answers it already keeps its own menu.
    if ([delegate respondsToSelector:selector])
        return false;
    installed = class_addMethod (delegateClass, selector, reinterpret_cast<IMP> (dockMenu), "@@:@");
    return installed;
}

class AppleBackend final : public DockMenu::Backend
{
public:
    ~AppleBackend() override { detach(); }

    void attach (DockMenu& owner) override
    {
        target = [AnompDockMenuTarget new];
        target.owner = &owner;
        showing = target;
        installOnDelegate();
    }

    void detach() override
    {
        if (target != nil)
        {
            target.owner = nullptr;
            if (showing == target)
                showing = nil;
            target = nil;
        }
    }

    // The menu is built when AppKit asks for it, so it's always current;
    // the delegate may only have been set up since attach().
    void update() override { installOnDelegate(); }

private:
    AnompDockMenuTarget* target = nil;
};
#endif
} // namespace

std::unique_ptr<DockMenu::Backend> DockMenu::createPlatformBackend()
{
#if TARGET_OS_OSX
    return std::make_unique<AppleBackend>();
#else
    return std::make_unique<Backend>();
#endif
}

bool DockMenu::isSupported()
{
#if TARGET_OS_OSX
    return true;
#else
    return false;
#endif
}
} // namespace anomp
