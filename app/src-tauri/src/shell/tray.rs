//! The optional menu-bar controls (PLAN.md F7): an icon in the menu bar
//! whose menu shows the current track, the transport, the mini player and
//! the main window. Its items go through `menu::handle` like the menu bar's.

use std::sync::Mutex;

use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager, Runtime};

use crate::settings::WindowSettings;

pub struct TrayState<R: Runtime>(Mutex<Option<TrayIcon<R>>>);

/// Shows or removes the tray as the settings say.
pub fn configure<R: Runtime>(app: &AppHandle<R>, window: &WindowSettings) {
    if app.try_state::<TrayState<R>>().is_none() {
        app.manage(TrayState::<R>(Mutex::new(None)));
    }
    let state = app.state::<TrayState<R>>();
    let mut tray = state.0.lock().unwrap_or_else(|e| e.into_inner());
    match (window.menu_bar_controls, tray.is_some()) {
        (true, false) => match build(app) {
            Ok(built) => *tray = Some(built),
            Err(error) => eprintln!("[tray] {error}"),
        },
        (false, true) => {
            if let Some(tray) = tray.take() {
                let _ = app.remove_tray_by_id(tray.id());
            }
        }
        _ => {}
    }
}

fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    let now = super::now();
    let title = match (&now.title, &now.artist) {
        (Some(title), Some(artist)) => format!("{title} — {artist}"),
        (Some(title), None) => title.clone(),
        (None, _) => "Not playing".into(),
    };
    MenuBuilder::new(app)
        .item(
            &MenuItemBuilder::with_id("tray-current", title)
                .enabled(false)
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("play", if now.playing { "Pause" } else { "Play" })
                .enabled(now.title.is_some())
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("next", "Next")
                .enabled(now.has_next)
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("previous", "Previous")
                .enabled(now.title.is_some())
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("mini-player", "Mini Player").build(app)?)
        .item(&MenuItemBuilder::with_id("show-main", "Show ano-mp").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()
}

fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<TrayIcon<R>> {
    let mut builder = TrayIconBuilder::with_id("controls")
        .tooltip("ano-mp")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

/// Shows the current track and transport as `super::now` says.
pub fn update<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<TrayState<R>>() else {
        return;
    };
    let tray = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(tray) = tray.as_ref() {
        if let Ok(menu) = menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
        let now = super::now();
        let tooltip = now
            .title
            .map_or("ano-mp".to_owned(), |title| format!("ano-mp: {title}"));
        let _ = tray.set_tooltip(Some(tooltip));
    }
}
