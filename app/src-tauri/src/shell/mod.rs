//! The app outside its main page (PLAN.md §4.7): the menu bar (F6), the
//! Dock menu (F6), the menu-bar controls and the mini player (F7), files
//! opened from the Finder or dropped on the window (F5), and track-change
//! notifications (F21). Transport items everywhere call the queue
//! functions the media keys use.
//!
//! On macOS, closing the main window hides it, and music keeps playing, as
//! in other music players; the Dock icon, the menus or the mini player
//! bring it back.

mod dock;
pub mod menu;
pub mod mini;
mod notifications;
mod tray;

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, Url, Wry};

use crate::queue::model::QueueState;
use crate::settings::{self, WindowSettings};

/// What the Dock menu and the tray show: the current track and what the
/// transport can do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowSummary {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub playing: bool,
    pub has_next: bool,
    pub has_previous: bool,
}

static NOW: Mutex<Option<NowSummary>> = Mutex::new(None);

pub fn now() -> NowSummary {
    NOW.lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}

fn update_now(change: impl FnOnce(&mut NowSummary)) -> bool {
    let mut now = NOW.lock().unwrap_or_else(|e| e.into_inner());
    let before = now.clone().unwrap_or_default();
    let mut after = before.clone();
    change(&mut after);
    *now = Some(after.clone());
    before != after
}

/// Sets up the menus, the Dock menu and the tray. Call in `setup`, on the
/// main thread, after the queue.
pub fn init(app: &AppHandle<Wry>) -> Result<(), String> {
    menu::install(app).map_err(|e| e.to_string())?;
    dock::init(app);
    tray::configure(app, &settings::current(app).window);
    Ok(())
}

/// Closes the Dock menu. Main thread.
pub fn shutdown() {
    dock::shutdown();
}

/// The queue changed. Main thread.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    menu::queue_changed(app, state);
    let item = state.current_item.as_ref();
    let changed = update_now(|now| {
        now.title = item.map(|item| item.track.title.clone());
        now.artist = item.and_then(|item| item.track.artist.clone());
        now.has_next = state.has_next;
        now.has_previous = state.has_previous;
    });
    if changed {
        dock::update();
        tray::update(app);
    }
    notifications::queue_changed(app, state);
}

/// A recording started or stopped (PLAN.md X6), or the feature was
/// switched. Main thread.
pub fn recording_changed<R: Runtime>(app: &AppHandle<R>, recording: bool) {
    let enabled = crate::settings::current(app).features.recording;
    menu::recording_changed(app, recording, enabled);
}

/// The player started or stopped playing. Main thread.
pub fn playing_changed<R: Runtime>(app: &AppHandle<R>, playing: bool) {
    menu::playing_changed(app, playing);
    if update_now(|now| now.playing = playing) {
        dock::update();
        tray::update(app);
    }
}

/// The window settings changed.
pub fn window_settings_changed<R: Runtime>(app: &AppHandle<R>, window: &WindowSettings) {
    tray::configure(app, window);
    mini::set_on_top(app, window.mini_player_on_top);
}

/// Shows the main window, in front.
pub fn show_main<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("The main window is gone")?;
    window.show().map_err(|e| e.to_string())?;
    let _ = window.unminimize();
    window.set_focus().map_err(|e| e.to_string())
}

/// Files the Finder asked the app to open ("Open With", a drop on the Dock
/// icon): audio files play (F5), and M3U playlists are imported (F1).
pub fn opened<R: Runtime>(app: &AppHandle<R>, urls: Vec<Url>) {
    let (playlists, paths): (Vec<PathBuf>, Vec<PathBuf>) = urls
        .into_iter()
        .filter_map(|url| url.to_file_path().ok())
        .partition(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("m3u") || ext.eq_ignore_ascii_case("m3u8")
                })
        });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for playlist in playlists {
            match crate::collection::playlists_import(app.clone(), playlist).await {
                Ok(report) => {
                    let _ = tauri::Emitter::emit(&app, "playlist-imported", report);
                }
                Err(error) => log::warn!("cannot import a playlist: {error}"),
            }
        }
        if !paths.is_empty() {
            if let Err(error) = crate::queue::open_files(&app, paths).await {
                log::warn!("cannot open files: {error}");
            }
        }
        let _ = show_main(&app);
    });
}

/// Paths dropped on the window, sorted: folders (to offer as library
/// folders) and files (to play).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DroppedPaths {
    pub folders: Vec<PathBuf>,
    pub files: Vec<PathBuf>,
}

/// Sorts paths dropped on the window; files the core can't play are left
/// out.
#[tauri::command]
pub fn shell_sort_dropped(paths: Vec<PathBuf>) -> DroppedPaths {
    let mut sorted = DroppedPaths::default();
    for path in paths {
        if path.is_dir() {
            sorted.folders.push(path);
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(crate::anomp::can_decode_extension)
        {
            sorted.files.push(path);
        }
    }
    sorted
}

/// The menus' shortcuts, for the shortcuts sheet: (what, keys), with
/// "CmdOrCtrl" for ⌘ on macOS and Ctrl elsewhere.
#[tauri::command]
pub fn shell_shortcuts() -> Vec<(&'static str, &'static str)> {
    menu::shortcuts()
}

#[tauri::command]
pub fn shell_show_main<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    show_main(&app)
}

#[tauri::command]
pub fn shell_toggle_mini_player<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    mini::toggle(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_dropped_paths() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("a.flac");
        let text = dir.path().join("notes.txt");
        std::fs::write(&song, "").unwrap();
        std::fs::write(&text, "").unwrap();
        let sorted = shell_sort_dropped(vec![dir.path().to_path_buf(), song.clone(), text]);
        assert_eq!(sorted.folders, [dir.path()]);
        assert_eq!(sorted.files, [song]);
    }

    #[test]
    fn keeps_what_the_dock_and_tray_show() {
        assert!(update_now(|now| now.title = Some("Song".into())));
        assert!(!update_now(|now| now.title = Some("Song".into())));
        assert_eq!(now().title.as_deref(), Some("Song"));
        assert!(!menu::shortcuts().is_empty());
    }
}
