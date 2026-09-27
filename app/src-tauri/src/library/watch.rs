//! Keeping the library in step with the disk (PLAN.md F9): a rescan of
//! every folder in the background at launch, and, while the app runs, a
//! rescan of a folder soon after its files change.
//!
//! Folders are watched with the OS's file events (FSEvents on macOS,
//! through the `notify` crate), each only while its bookmark is open, as
//! the sandbox needs. Events are gathered until the folders have been
//! quiet for `QUIET`, or for at most `MAX_WAIT`, and then the folders they
//! came from are scanned together, so a file moved from one to another
//! keeps its track (F10). Scans skip unchanged files, so a rescan is cheap.
//! Events for hidden files (".DS_Store", "._" files) are ignored.
//!
//! Background scans run at a low priority (`lower_priority`), and never
//! alongside another scan: they wait for one in progress.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use notify::{EventKind, RecursiveMode, Watcher as _};
use tauri::{AppHandle, Manager, Runtime};

use super::access::{self, OpenFolder};
use super::commands::{run_scan, LibraryState};
use crate::settings::{self, LibrarySettings};

/// How long the folders must be quiet before they are scanned.
const QUIET: Duration = Duration::from_secs(3);
/// The longest events wait for a quiet moment.
const MAX_WAIT: Duration = Duration::from_secs(30);

/// The folders being watched, if any.
#[derive(Default)]
pub struct WatchState(Mutex<Option<Watching>>);

struct Watching {
    _watcher: notify::RecommendedWatcher,
    /// Each watched folder, readable while it is watched.
    _open: Vec<OpenFolder>,
}

/// Starts watching as the settings say, and rescans every folder scanned
/// before in the background if they say so. Call after the library.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(WatchState::default());
    let library = settings::current(app).library;
    if library.rescan_at_launch {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Only folders that have been scanned: a new one is scanned
            // when it's added.
            let scanned: Option<Vec<i64>> = app.try_state::<LibraryState>().and_then(|library| {
                super::folders(&library.conn()).ok().map(|folders| {
                    folders
                        .into_iter()
                        .filter(|folder| folder.last_scan_at.is_some())
                        .map(|folder| folder.id)
                        .collect()
                })
            });
            if let Some(ids) = scanned.filter(|ids| !ids.is_empty()) {
                scan_when_free(&app, ids).await;
            }
        });
    }
    configure(app, &library);
}

/// The settings changed: starts or stops watching.
pub fn configure<R: Runtime>(app: &AppHandle<R>, settings: &LibrarySettings) {
    let Some(state) = app.try_state::<WatchState>() else {
        return;
    };
    // Dropping the watcher stops it, and its debouncing thread with it.
    *state.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
    if settings.watch_folders {
        match start(app) {
            Ok(watching) => *state.0.lock().unwrap_or_else(|e| e.into_inner()) = watching,
            Err(error) => eprintln!("[library] cannot watch the folders: {error}"),
        }
    }
}

/// The library's folders changed (added, removed, found again): watches
/// the new set.
pub fn folders_changed<R: Runtime>(app: &AppHandle<R>) {
    configure(app, &settings::current(app).library);
}

fn start<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Watching>, String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let (roots, open) = {
        let conn = library.conn();
        let folders = super::folders(&conn).map_err(|e| e.to_string())?;
        let mut roots = Vec::new();
        let mut open = Vec::new();
        for folder in folders {
            // A folder that isn't there now (an unplugged drive) isn't watched.
            if let Ok(folder_open) = access::open_folder(&conn, folder.id) {
                roots.push((folder.id, folder_open.path.clone()));
                open.push(folder_open);
            }
        }
        (roots, open)
    };
    if roots.is_empty() {
        return Ok(None);
    }
    let (sender, receiver) = mpsc::channel::<i64>();
    let watched = roots.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else {
            return;
        };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for path in &event.paths {
            if let Some(folder_id) = folder_of(&watched, path) {
                let _ = sender.send(folder_id);
            }
        }
    })
    .map_err(|e| e.to_string())?;
    for (_, root) in &roots {
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| format!("{}: {e}", root.display()))?;
    }
    let app = app.clone();
    std::thread::Builder::new()
        .name("library watcher".into())
        .spawn(move || debounce(&app, &receiver))
        .map_err(|e| e.to_string())?;
    Ok(Some(Watching {
        _watcher: watcher,
        _open: open,
    }))
}

/// The watched folder `path` is in, unless it's hidden there.
fn folder_of(roots: &[(i64, PathBuf)], path: &Path) -> Option<i64> {
    roots.iter().find_map(|(id, root)| {
        let rest = path.strip_prefix(root).ok()?;
        let hidden = rest
            .components()
            .any(|part| part.as_os_str().as_encoded_bytes().first() == Some(&b'.'));
        (!hidden).then_some(*id)
    })
}

/// Gathers the folders events came from until they are quiet, then scans
/// them; ends when the watcher is dropped.
fn debounce<R: Runtime>(app: &AppHandle<R>, receiver: &mpsc::Receiver<i64>) {
    loop {
        let Ok(first) = receiver.recv() else {
            return;
        };
        let mut folders = BTreeSet::from([first]);
        let started = Instant::now();
        loop {
            match receiver.recv_timeout(QUIET) {
                Ok(folder_id) => {
                    folders.insert(folder_id);
                    if started.elapsed() > MAX_WAIT {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        tauri::async_runtime::block_on(scan_when_free(app, folders.into_iter().collect()));
    }
}

/// Scans `folder_ids` in the background, after any scan in progress.
async fn scan_when_free<R: Runtime>(app: &AppHandle<R>, folder_ids: Vec<i64>) {
    loop {
        match run_scan(app, Some(folder_ids.clone()), true).await {
            Err(error) if error.contains("already running") => {
                let _ = tauri::async_runtime::spawn_blocking(|| {
                    std::thread::sleep(Duration::from_secs(1))
                })
                .await;
            }
            Err(error) => {
                eprintln!("[library] background scan: {error}");
                return;
            }
            Ok(_) => return,
        }
    }
}

/// Runs the calling thread at a low priority: a background scan shouldn't
/// slow playback or the UI.
pub fn lower_priority() {
    #[cfg(target_vendor = "apple")]
    // SAFETY: sets the calling thread's own QoS class; no pointers.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
    #[cfg(all(unix, not(target_vendor = "apple")))]
    // SAFETY: changes the calling thread's own niceness; no pointers.
    unsafe {
        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_map_to_their_folder_unless_hidden() {
        let roots = vec![(1, PathBuf::from("/Music")), (2, PathBuf::from("/Other"))];
        assert_eq!(folder_of(&roots, Path::new("/Music/A/b.flac")), Some(1));
        assert_eq!(folder_of(&roots, Path::new("/Other")), Some(2));
        assert_eq!(folder_of(&roots, Path::new("/Music/A/.DS_Store")), None);
        assert_eq!(folder_of(&roots, Path::new("/Music/.hidden/b.flac")), None);
        assert_eq!(folder_of(&roots, Path::new("/Elsewhere/b.flac")), None);
    }
}
