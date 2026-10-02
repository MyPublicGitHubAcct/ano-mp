//! The /dev page's commands (`routes/dev`): the core's version, the output
//! device's name, a test tone, and files loaded straight into the engine by
//! path, outside the queue. Debug builds only (PLAN.md H2): in a release
//! build `player_load` would hand any path the page names to FFmpeg, which
//! nothing but the sandbox would stop, and Linux and Windows have none.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, Runtime};

use crate::audio::with_engine;
use crate::library::access::OpenFolder;
use crate::library::commands::LibraryState;
use crate::queue;

#[tauri::command]
pub fn core_version() -> String {
    crate::anomp::version()
}

#[tauri::command]
pub fn audio_device_name<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    with_engine(&app, |engine| engine.device_name())
}

#[tauri::command]
pub fn play_test_tone<R: Runtime>(app: AppHandle<R>, frequency: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.play_test_tone(frequency))?
        .then_some(())
        .ok_or_else(|| "No output device is open".to_string())
}

#[tauri::command]
pub fn stop_test_tone<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.stop_test_tone())
}

/// Opens the library folder holding `path`, if any: under the sandbox the
/// engine can open a library track only while its folder's bookmark is
/// resolved (`library::access`). A file stays readable once it is open.
fn open_library_folder<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
) -> Result<Option<OpenFolder>, String> {
    app.try_state::<LibraryState>()
        .map_or(Ok(None), |library| library.open_folder_of(path))
}

/// Loads a file directly, for the dev page; the queue (`queue_*`) stops
/// following the engine until it next starts a track itself.
#[tauri::command]
pub fn player_load<R: Runtime>(app: AppHandle<R>, path: PathBuf) -> Result<(), String> {
    let _folder = open_library_folder(&app, &path)?;
    with_engine(&app, move |engine| engine.load(&path, 1.0))??;
    queue::detach(&app)
}

/// Sets the file that follows the current track gaplessly (`null` clears
/// it), for the dev page; detaches the queue like `player_load`.
#[tauri::command]
pub fn player_set_next<R: Runtime>(app: AppHandle<R>, path: Option<PathBuf>) -> Result<(), String> {
    let _folder = path
        .as_deref()
        .map(|path| open_library_folder(&app, path))
        .transpose()?;
    with_engine(&app, move |engine| {
        engine.set_next(path.as_deref().map(|path| (path, 1.0)))
    })??;
    queue::detach(&app)
}

#[tauri::command]
pub fn player_play<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.play())?
        .then_some(())
        .ok_or_else(|| "No track is loaded".to_string())
}

#[tauri::command]
pub fn player_pause<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.pause())
}

#[tauri::command]
pub fn player_stop<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.stop())
}

#[tauri::command]
pub fn player_seek<R: Runtime>(app: AppHandle<R>, seconds: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.seek(seconds))?
        .then_some(())
        .ok_or_else(|| "No track is loaded".to_string())
}
