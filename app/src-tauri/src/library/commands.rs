//! Exposes the library as Tauri commands. The database lives in the app data
//! folder. Commands share one connection for short queries; a scan runs on a
//! blocking thread with a connection of its own, so the library stays
//! readable while it writes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use super::scanner::{self, ScanFailure, ScanReport};
use super::{db, Error, Folder, TrackSummary};

/// Frontend event with a `ScanProgress` payload.
pub const SCAN_PROGRESS_EVENT: &str = "library-scan-progress";

pub struct LibraryState {
    db_path: PathBuf,
    conn: Mutex<Connection>,
    scanning: AtomicBool,
}

impl LibraryState {
    fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-query leaves nothing half-done that SQLite hasn't
        // rolled back, so a poisoned lock is still usable.
        self.conn.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Opens (or creates) the library database and registers `LibraryState`.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let db_path = dir.join("library.sqlite3");
    let conn = db::open(&db_path).map_err(|e| e.to_string())?;
    app.manage(LibraryState {
        db_path,
        conn: Mutex::new(conn),
        scanning: AtomicBool::new(false),
    });
    Ok(())
}

#[tauri::command]
pub fn library_folders(state: State<'_, LibraryState>) -> Result<Vec<Folder>, String> {
    super::folders(&state.conn()).map_err(|e| e.to_string())
}

/// Adds a folder, unscanned; follow with `library_scan`.
#[tauri::command]
pub fn library_add_folder(state: State<'_, LibraryState>, path: PathBuf) -> Result<Folder, String> {
    super::add_folder(&state.conn(), &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_remove_folder(state: State<'_, LibraryState>, folder_id: i64) -> Result<(), String> {
    if state.scanning.load(Ordering::SeqCst) {
        return Err("Wait for the library scan to finish".into());
    }
    super::remove_folder(&mut state.conn(), folder_id).map_err(|e| e.to_string())
}

/// Every track; see `library::tracks`.
#[tauri::command]
pub fn library_tracks(state: State<'_, LibraryState>) -> Result<Vec<TrackSummary>, String> {
    super::tracks(&state.conn()).map_err(|e| e.to_string())
}

/// Scans one folder, or every folder when `folder_id` is null, emitting
/// progress events. One scan runs at a time. A folder that can't be scanned
/// (e.g. on an unmounted drive) gets a report with the reason as its only
/// failure, and is left as it was.
#[tauri::command]
pub async fn library_scan<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, LibraryState>,
    folder_id: Option<i64>,
) -> Result<Vec<ScanReport>, String> {
    if state.scanning.swap(true, Ordering::SeqCst) {
        return Err("A library scan is already running".into());
    }
    let _scanning = ClearOnDrop(&state.scanning);
    let db_path = state.db_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        scan_folders(&db_path, folder_id, |progress| {
            let _ = app.emit(SCAN_PROGRESS_EVENT, progress);
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

fn scan_folders(
    db_path: &Path,
    folder_id: Option<i64>,
    mut progress: impl FnMut(scanner::ScanProgress),
) -> Result<Vec<ScanReport>, Error> {
    let mut conn = db::open(db_path)?;
    let folders: Vec<Folder> = super::folders(&conn)?
        .into_iter()
        .filter(|folder| folder_id.is_none_or(|id| id == folder.id))
        .collect();
    if let (Some(id), true) = (folder_id, folders.is_empty()) {
        return Err(Error::Invalid(format!("No library folder with id {id}")));
    }
    let mut reports = Vec::new();
    for folder in folders {
        reports.push(match scanner::scan_folder(&mut conn, folder.id, &mut progress) {
            Ok(report) => report,
            Err(Error::Invalid(error)) => ScanReport {
                folder_id: folder.id,
                failed: vec![ScanFailure {
                    path: folder.path,
                    error,
                }],
                ..ScanReport::default()
            },
            Err(error) => return Err(error),
        });
    }
    Ok(reports)
}

struct ClearOnDrop<'a>(&'a AtomicBool);

impl Drop for ClearOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
