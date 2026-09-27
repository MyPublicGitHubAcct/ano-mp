//! Exposes the library as Tauri commands. The database lives in the app data
//! folder. Commands share one connection for short queries; a scan runs on a
//! blocking thread with a connection of its own, so the library stays
//! readable while it writes. Queries that can take tens of milliseconds
//! (browsing, search) run on a blocking thread too, off the main thread
//! that the engine and the queue use.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use super::access::{self, OpenFolder};
use super::art::ArtCache;
use super::artists::{self, ArtistPage};
use super::browse::{self, BrowsePage, GroupKey};
use super::covers::{self, CoverBasis, CoverWall};
use super::playback::{self, TrackPlay};
use super::rules::{self, SortRule, SortSettings};
use super::scanner::{self, ScanFailure, ScanOptions, ScanReport};
use super::search::{self, SearchKind, SearchResults};
use super::{db, Error, Folder};
use crate::metadata::images::ImageCache;
use crate::settings::FeatureSettings;

/// Frontend event with a `ScanProgress` payload.
pub const SCAN_PROGRESS_EVENT: &str = "library-scan-progress";

pub struct LibraryState {
    db_path: PathBuf,
    conn: Mutex<Connection>,
    scanning: AtomicBool,
    pub art: ArtCache,
    /// Pictures downloaded from online sources; the metadata worker stores
    /// them.
    pub images: Arc<ImageCache>,
}

impl LibraryState {
    /// The shared connection. Hold it briefly, and never while waiting for
    /// the main thread, which takes it for the queue.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic mid-query leaves nothing half-done that SQLite hasn't
        // rolled back, so a poisoned lock is still usable.
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The database file, for threads that open their own connection.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Opens the library folder holding the file at `path`, if any; files in
    /// it can be opened while the result is alive. See `library::access`.
    pub fn open_folder_of(&self, path: &Path) -> Result<Option<OpenFolder>, String> {
        access::open_folder_of(&self.conn(), path).map_err(|e| e.to_string())
    }

    /// How to play track `track_id` under `features` (`library::playback`).
    pub fn track_play(
        &self,
        track_id: i64,
        features: &FeatureSettings,
    ) -> Result<TrackPlay, String> {
        playback::track_play(&self.conn(), track_id, features)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "The track is no longer in the library".to_string())
    }
}

#[cfg(test)]
impl LibraryState {
    /// A library over `conn`, for tests; scans aren't possible.
    pub fn for_tests(conn: Connection) -> LibraryState {
        LibraryState {
            db_path: PathBuf::new(),
            conn: Mutex::new(conn),
            scanning: AtomicBool::new(false),
            art: ArtCache::default(),
            // Empty and never written to; a test that needs pictures sets
            // its own.
            images: Arc::new(ImageCache::new(
                std::env::temp_dir().join("ano-mp-tests-no-images"),
            )),
        }
    }
}

/// Runs `f` with the library on a blocking thread.
pub async fn on_library<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&LibraryState) -> Result<T, Error> + Send + 'static,
) -> Result<T, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let library = app
            .try_state::<LibraryState>()
            .ok_or("The library is not available")?;
        f(&library).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Opens (or creates) the library database and registers `LibraryState`.
/// Downloaded pictures go in the app cache dir, which the OS may clear.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    let db_path = dir.join("library.sqlite3");
    let conn = db::open(&db_path).map_err(|e| e.to_string())?;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    app.manage(LibraryState {
        db_path,
        conn: Mutex::new(conn),
        scanning: AtomicBool::new(false),
        art: ArtCache::default(),
        images: Arc::new(ImageCache::new(cache_dir.join("images"))),
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

/// One page of the children of the node at `path` under the sort rule
/// `rule_id`; see `library::browse::browse`.
#[tauri::command]
pub async fn library_browse<R: Runtime>(
    app: AppHandle<R>,
    rule_id: String,
    path: Vec<Option<GroupKey>>,
    offset: u32,
    limit: u32,
) -> Result<BrowsePage, String> {
    on_library(&app, move |library| {
        browse::browse_rule(&library.conn(), &rule_id, &path, offset, limit)
    })
    .await
}

/// Artists, albums and tracks matching `query`; see `library::search`. All
/// three kinds unless `kinds` says otherwise.
#[tauri::command]
pub async fn library_search<R: Runtime>(
    app: AppHandle<R>,
    query: String,
    kinds: Option<Vec<SearchKind>>,
    offset: u32,
    limit: u32,
) -> Result<SearchResults, String> {
    let kinds = kinds.unwrap_or_else(|| search::ALL_KINDS.to_vec());
    on_library(&app, move |library| {
        search::search(&library.conn(), &query, &kinds, offset, limit)
    })
    .await
}

/// The page for artist `artist_id`: their albums, the albums they appear
/// on, and what the metadata sources know about them; see
/// `library::artists`. The metadata worker is asked to look the artist up
/// ahead of background work, as far as the settings and the retry waits
/// allow, and names them in `metadata-changed` if anything changed.
#[tauri::command]
pub async fn library_artist<R: Runtime>(
    app: AppHandle<R>,
    artist_id: i64,
) -> Result<ArtistPage, String> {
    let page = on_library(&app, move |library| {
        artists::artist_page(&library.conn(), artist_id)?
            .ok_or_else(|| Error::Invalid("The artist is no longer in the library".into()))
    })
    .await?;
    crate::metadata::worker::viewing_artist(&app, artist_id);
    Ok(page)
}

/// The albums the visualizer's cover wall shows around track `track_id`;
/// see `library::covers`. Null if the track has no year (or artist).
#[tauri::command]
pub async fn library_cover_wall<R: Runtime>(
    app: AppHandle<R>,
    track_id: i64,
    basis: CoverBasis,
) -> Result<Option<CoverWall>, String> {
    on_library(&app, move |library| {
        covers::cover_wall(&library.conn(), track_id, basis)
    })
    .await
}

/// The sort rules and ignored articles.
#[tauri::command]
pub fn library_sort_settings(state: State<'_, LibraryState>) -> Result<SortSettings, String> {
    rules::sort_settings(&state.conn()).map_err(|e| e.to_string())
}

/// Adds a rule, or replaces the one with its id.
#[tauri::command]
pub fn library_save_sort_rule(
    state: State<'_, LibraryState>,
    rule: SortRule,
) -> Result<SortSettings, String> {
    rules::save_sort_rule(&state.conn(), rule).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_remove_sort_rule(
    state: State<'_, LibraryState>,
    rule_id: String,
) -> Result<SortSettings, String> {
    rules::remove_sort_rule(&state.conn(), &rule_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn library_set_ignored_articles(
    state: State<'_, LibraryState>,
    articles: Vec<String>,
) -> Result<SortSettings, String> {
    rules::set_ignored_articles(&state.conn(), articles).map_err(|e| e.to_string())
}

/// Back to the built-in rules and articles.
#[tauri::command]
pub fn library_reset_sort_settings(state: State<'_, LibraryState>) -> Result<SortSettings, String> {
    rules::reset_sort_settings(&state.conn()).map_err(|e| e.to_string())
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
    let scan_app = app.clone();
    let options = ScanOptions {
        parts: crate::settings::current(&app).features.cue_sheets,
    };
    let reports = tauri::async_runtime::spawn_blocking(move || {
        scan_folders(&db_path, folder_id, options, |progress| {
            let _ = scan_app.emit(SCAN_PROGRESS_EVENT, progress);
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    // Files may have new art or tags.
    state.art.clear();
    crate::queue::refresh_tracks(&app).await;
    // New albums to look up, if the settings say so, and tracks to analyse.
    crate::metadata::worker::enrich_library(&app);
    super::analysis::library_changed(&app);
    Ok(reports)
}

/// Marks every file to be read again and rescans the library, e.g. when
/// cue sheets are turned on or off. Waits for a scan in progress.
pub fn reread_all<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<LibraryState>() else {
            return;
        };
        while state.scanning.load(Ordering::SeqCst) {
            tokio_sleep().await;
        }
        if let Err(error) = state
            .conn()
            .execute("UPDATE tracks SET file_mtime_ns = -1", [])
        {
            eprintln!("[library] {error}");
            return;
        }
        if let Err(error) = library_scan(app.clone(), state, None).await {
            eprintln!("[library] {error}");
        }
    });
}

async fn tokio_sleep() {
    let _ = tauri::async_runtime::spawn_blocking(|| {
        std::thread::sleep(std::time::Duration::from_millis(500))
    })
    .await;
}

fn scan_folders(
    db_path: &Path,
    folder_id: Option<i64>,
    options: ScanOptions,
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
        reports.push(
            match scanner::scan_folder(&mut conn, folder.id, options, &mut progress) {
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
            },
        );
    }
    Ok(reports)
}

struct ClearOnDrop<'a>(&'a AtomicBool);

impl Drop for ClearOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
