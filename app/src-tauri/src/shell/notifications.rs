//! Track-change notifications (PLAN.md F21): opt-in, only while no window
//! of the app is in front, with the cover. Shown through `notify-rust`,
//! which tauri-plugin-notification uses on desktop, since the plugin leaves
//! out a notification's image there.

use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{AppHandle, Manager, Runtime};

use crate::library::art::{self, ArtKey};
use crate::library::commands::LibraryState;
use crate::queue::model::QueueState;

/// The queue item last seen current, so each track notifies once.
static LAST: AtomicU64 = AtomicU64::new(0);

pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let Some(item) = state.current_item.as_ref().filter(|_| state.loaded) else {
        return;
    };
    if LAST.swap(item.uid, Ordering::Relaxed) == item.uid {
        return;
    }
    if !crate::settings::current(app).window.track_notifications {
        return;
    }
    let focused = app
        .webview_windows()
        .values()
        .any(|window| window.is_focused().unwrap_or(false));
    if focused {
        return;
    }
    let title = item.track.title.clone();
    let body = [item.track.artist.clone(), item.track.album.clone()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" — ");
    let key = item
        .track
        .album_id
        .map_or(ArtKey::Track(item.track.track_id), ArtKey::Album);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || show(&app, &title, &body, key));
}

fn show<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str, key: ArtKey) {
    #[cfg(target_os = "macos")]
    {
        use std::sync::Once;
        static APPLICATION: Once = Once::new();
        let identifier = app.config().identifier.clone();
        APPLICATION.call_once(|| {
            let _ = notify_rust::set_application(&identifier);
        });
    }
    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(body);
    // The cover, from a file the notification reads: one, overwritten.
    let cover = app.try_state::<LibraryState>().and_then(|library| {
        let art = art::lookup(&library, key).ok().flatten()?;
        let extension = if art.mime_type == "image/png" {
            "png"
        } else {
            "jpg"
        };
        let path = app
            .path()
            .app_cache_dir()
            .ok()?
            .join(format!("notification-cover.{extension}"));
        std::fs::create_dir_all(path.parent()?).ok()?;
        std::fs::write(&path, &art.data).ok()?;
        Some(path)
    });
    if let Some(path) = cover.as_ref().and_then(|path| path.to_str()) {
        notification.image_path(path);
    }
    if let Err(error) = notification.show() {
        eprintln!("[notifications] {error}");
    }
}
