//! The Help window (PLAN.md Phase 7c D1): the user guide, bundled with the
//! app (`app/src/routes/help`), opened from Help › ano-mp Help. Its
//! capability (capabilities/help.json) allows only reading the settings,
//! for the theme, and opening web links in the browser.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "help";

/// Opens the Help window, or brings it to the front if it's open.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        return window.set_focus().map_err(|e| e.to_string());
    }
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("help".into()))
        .title("ano-mp Help")
        .inner_size(960.0, 720.0)
        .min_inner_size(480.0, 360.0)
        .resizable(true)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
