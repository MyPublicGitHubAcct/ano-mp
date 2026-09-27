//! The mini player (PLAN.md F7): a small second window with the playing bar,
//! optionally above other windows. Its capability (capabilities/mini.json)
//! gives it only the commands it needs.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "mini";

/// Opens the mini player, or closes it if it's open.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return window.close().map_err(|e| e.to_string());
    }
    let on_top = crate::settings::current(app).window.mini_player_on_top;
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("mini".into()))
        .title("ano-mp")
        .inner_size(460.0, 140.0)
        .min_inner_size(320.0, 120.0)
        .resizable(true)
        .maximizable(false)
        .always_on_top(on_top)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn set_on_top<R: Runtime>(app: &AppHandle<R>, on: bool) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_always_on_top(on);
    }
}
