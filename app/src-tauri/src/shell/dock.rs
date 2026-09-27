//! The Dock icon's menu (PLAN.md F6), through the core's `DockMenu`: the
//! current track, then play or pause, next and previous. Main thread only,
//! like the media controls.

use std::cell::RefCell;

use tauri::{AppHandle, Runtime};

use crate::anomp::{DockMenu, MenuItem};

thread_local! {
    static DOCK: RefCell<Option<DockMenu>> = const { RefCell::new(None) };
}

const CURRENT: i32 = 1;
const PLAY: i32 = 2;
const NEXT: i32 = 3;
const PREVIOUS: i32 = 4;

pub fn init<R: Runtime>(app: &AppHandle<R>) {
    if !DockMenu::supported() {
        return;
    }
    let handler_app = app.clone();
    let menu = DockMenu::new(move |id| {
        let item = match id {
            PLAY => "play",
            NEXT => "next",
            PREVIOUS => "previous",
            _ => return,
        };
        super::menu::handle(&handler_app, item);
    });
    DOCK.with_borrow_mut(|slot| *slot = menu);
    update();
}

pub fn shutdown() {
    DOCK.with_borrow_mut(|slot| *slot = None);
}

/// Shows what `super::now` says.
pub fn update() {
    let now = super::now();
    let item = |id, title: &str, enabled| MenuItem {
        id,
        title: title.into(),
        enabled,
        checked: false,
    };
    let mut items = Vec::new();
    if let Some(title) = &now.title {
        let text = match &now.artist {
            Some(artist) => format!("{title} — {artist}"),
            None => title.clone(),
        };
        items.push(item(CURRENT, &text, false));
        items.push(item(0, "", false));
    }
    items.push(item(
        PLAY,
        if now.playing { "Pause" } else { "Play" },
        now.title.is_some(),
    ));
    items.push(item(NEXT, "Next", now.has_next));
    items.push(item(PREVIOUS, "Previous", now.title.is_some()));
    DOCK.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut() {
            if let Some(menu) = slot.as_mut() {
                menu.set_items(&items);
            }
        }
    });
}
