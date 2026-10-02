//! Which library folders can be read now (PLAN.md H22), kept up to date
//! while the app runs.
//!
//! Every folder is checked (`access::check_folder`) off the main thread at
//! launch, before the launch rescan, which then scans only the folders
//! that are there. The states are kept here (`FolderStates`), not in the
//! database: they are found again at each launch. A scan updates them too:
//! a folder it held (`FolderState::Empty` or `MostlyGone`, see `scanner`)
//! stays so until a scan succeeds or the user removes the tracks.
//!
//! Folders are checked again when a volume is mounted or unmounted (the
//! core's `VolumeWatcher`, hosted on the main thread) and whenever the UI
//! lists them. A folder that comes back is rescanned and watched, and the
//! queue tries its tracks again; one that goes is no longer watched, and
//! the queue passes over its tracks. Each change is logged and announced
//! with `FOLDERS_EVENT`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::access::{self, Bookmarks, FolderState, FolderStatus};
use super::commands::LibraryState;
use super::{Error, Folder};
use crate::anomp::VolumeWatcher;

/// Frontend event, without a payload: a folder's state changed.
pub const FOLDERS_EVENT: &str = "library-folders";

/// Each folder's state, by id. A folder not checked yet counts as there.
#[derive(Default)]
pub struct FolderStates(Mutex<HashMap<i64, FolderStatus>>);

/// Folders whose state changed: those that can be read again, and those
/// that can't any more.
#[derive(Debug, Default, PartialEq)]
pub struct Changes {
    pub returned: Vec<i64>,
    pub lost: Vec<i64>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.returned.is_empty() && self.lost.is_empty()
    }
}

impl FolderStates {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, FolderStatus>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, folder_id: i64) -> FolderStatus {
        self.lock()
            .get(&folder_id)
            .cloned()
            .unwrap_or(FolderStatus::AVAILABLE)
    }

    /// The folders none of whose files can be opened now: unavailable ones,
    /// except those a scan held, whose remaining files still play.
    pub fn unreadable(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = self
            .lock()
            .iter()
            .filter(|(_, status)| {
                !matches!(
                    status.state,
                    FolderState::Available | FolderState::MostlyGone
                )
            })
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Records `status` for folder `folder_id`, logging a change and adding
    /// it to `changes`.
    pub fn set(&self, folder_id: i64, status: FolderStatus, changes: &mut Changes) {
        let mut states = self.lock();
        let before = states
            .get(&folder_id)
            .map_or(FolderState::Available, |status| status.state);
        if before != status.state {
            match &status.detail {
                Some(detail) => log::warn!(
                    "folder {folder_id} is now {} ({detail})",
                    status.state.code()
                ),
                None => log::warn!("folder {folder_id} is now {}", status.state.code()),
            }
            if status.state == FolderState::Available {
                changes.returned.push(folder_id);
            } else if before == FolderState::Available {
                changes.lost.push(folder_id);
            }
        }
        states.insert(folder_id, status);
    }

    pub fn forget(&self, folder_id: i64) {
        self.lock().remove(&folder_id);
    }

    /// Checks every folder in `folders` and records what it finds. A folder
    /// a scan held stays held while it reads as there: only a scan, or the
    /// user, changes that.
    pub fn check(
        &self,
        conn: &Connection,
        folders: &[Folder],
        bookmarks: &impl Bookmarks,
    ) -> Result<Changes, Error> {
        let mut changes = Changes::default();
        for folder in folders {
            let mut status = access::check_folder(conn, folder.id, bookmarks)?;
            if status.state == FolderState::Available
                && self.get(folder.id).state == FolderState::MostlyGone
            {
                continue;
            }
            if status.state == FolderState::Available {
                status.detail = None;
            }
            self.set(folder.id, status, &mut changes);
        }
        Ok(changes)
    }
}

/// The library's folders, each with its state as last found.
pub fn folders_with_states(conn: &Connection, states: &FolderStates) -> Result<Vec<Folder>, Error> {
    let mut folders = super::folders(conn)?;
    for folder in &mut folders {
        let status = states.get(folder.id);
        folder.available = Some(status.state == FolderState::Available);
        folder.status = Some(status);
    }
    Ok(folders)
}

/// Checks every folder now (a blocking call: not on the main thread, and
/// not holding the shared connection), then acts on what changed
/// (`changed`). Returns the folders with their states.
pub fn refresh<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<Folder>, String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let states = app
        .try_state::<FolderStates>()
        .ok_or("The library is not available")?;
    // A connection of its own: resolving a folder on a sleeping drive can
    // take seconds, and the shared one opens tracks for the main thread.
    let (folders, changes) = {
        let conn = super::db::open(library.db_path()).map_err(|e| e.to_string())?;
        let folders = super::folders(&conn).map_err(|e| e.to_string())?;
        let changes = states
            .check(&conn, &folders, &access::System)
            .map_err(|e| e.to_string())?;
        (
            folders_with_states(&conn, &states).map_err(|e| e.to_string())?,
            changes,
        )
    };
    changed(app, changes, true);
    Ok(folders)
}

/// Acts on folders that came back or went: rescans (if `rescan`) and
/// watches what came back, stops watching what went, has the queue try or
/// pass over their tracks, and tells the UI.
pub fn changed<R: Runtime>(app: &AppHandle<R>, changes: Changes, rescan: bool) {
    if changes.is_empty() {
        return;
    }
    let _ = app.emit(FOLDERS_EVENT, ());
    super::watch::folders_changed(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::queue::folders_changed(&app).await;
        if rescan && !changes.returned.is_empty() {
            super::watch::scan_when_free(&app, changes.returned).await;
        }
    });
}

/// The folders none of whose files can be opened now (`unreadable`), or
/// none before the library is set up. Lists that pick tracks to play
/// (radio, smart playlists' "play", Home's suggestions) leave their tracks
/// out (H22b).
pub fn unreadable<R: Runtime>(app: &AppHandle<R>) -> Vec<i64> {
    app.try_state::<FolderStates>()
        .map(|states| states.unreadable())
        .unwrap_or_default()
}

/// `ids` as a JSON array, to bind where a query reads it with `json_each`
/// (never formatted into the SQL).
pub fn json_ids(ids: &[i64]) -> String {
    serde_json::to_string(ids).unwrap_or_else(|_| "[]".into())
}

/// Which of `track_ids` are in the folders `folder_ids`.
pub fn tracks_in(
    conn: &Connection,
    track_ids: &[i64],
    folder_ids: &[i64],
) -> Result<HashSet<i64>, Error> {
    if folder_ids.is_empty() || track_ids.is_empty() {
        return Ok(HashSet::new());
    }
    let mut statement = conn.prepare_cached(
        "SELECT id FROM tracks
         WHERE id IN (SELECT value FROM json_each(?1))
           AND folder_id IN (SELECT value FROM json_each(?2))",
    )?;
    let ids = statement
        .query_map([json_ids(track_ids), json_ids(folder_ids)], |row| {
            row.get(0)
        })?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

thread_local! {
    /// The volume watcher, on the main thread.
    static VOLUMES: RefCell<Option<VolumeWatcher>> = const { RefCell::new(None) };
}

/// Starts checking the folders again whenever a volume is mounted or
/// unmounted. Main thread.
pub fn watch_volumes<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let watcher = VolumeWatcher::new(move |mounted, path| {
        log::info!(
            "a volume was {}",
            if mounted { "mounted" } else { "unmounted" }
        );
        log::debug!("the volume is {}", path.display());
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(error) = refresh(&app) {
                log::warn!("cannot check the folders: {error}");
            }
        });
    });
    VOLUMES.with(|slot| *slot.borrow_mut() = watcher);
}

/// Stops watching volumes. Main thread.
pub fn shutdown() {
    VOLUMES.with(|slot| slot.borrow_mut().take());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::access::testing::FakeBookmarks;
    use crate::library::db;
    use rusqlite::params;
    use std::path::Path;

    fn folder(conn: &Connection, path: &Path, tracks: usize) -> i64 {
        conn.execute(
            "INSERT INTO folders (path, bookmark, added_at) VALUES (?1, ?2, 0)",
            params![path.to_str().unwrap(), path.to_str().unwrap().as_bytes()],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        for i in 0..tracks {
            conn.execute(
                "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                     sample_rate, channels, scanned_at)
                 VALUES (?1, ?2, 1, 1, 1.0, 44100, 2, 0)",
                params![id, format!("{i}.flac")],
            )
            .unwrap();
        }
        id
    }

    #[test]
    fn records_folders_that_go_and_come_back() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let (here, away) = (dir.path().join("Here"), dir.path().join("Away"));
        for path in [&here, &away] {
            std::fs::create_dir(path).unwrap();
            std::fs::write(path.join("a.flac"), "").unwrap();
        }
        let here_id = folder(&conn, &here, 1);
        let away_id = folder(&conn, &away, 1);
        let states = FolderStates::default();
        let bookmarks = FakeBookmarks::default();
        let folders = super::super::folders(&conn).unwrap();

        // Both there: nothing changed.
        assert!(states
            .check(&conn, &folders, &bookmarks)
            .unwrap()
            .is_empty());

        // A drive is unplugged.
        std::fs::remove_dir_all(&away).unwrap();
        let changes = states.check(&conn, &folders, &bookmarks).unwrap();
        assert_eq!(
            changes,
            Changes {
                returned: vec![],
                lost: vec![away_id]
            }
        );
        assert_eq!(states.unreadable(), [away_id]);
        let listed = folders_with_states(&conn, &states).unwrap();
        let away_listed = listed.iter().find(|f| f.id == away_id).unwrap();
        assert_eq!(away_listed.available, Some(false));
        assert_eq!(
            away_listed.status.as_ref().unwrap().state,
            FolderState::Missing
        );
        // Checking again changes nothing.
        assert!(states
            .check(&conn, &folders, &bookmarks)
            .unwrap()
            .is_empty());

        // It comes back, empty at first (the mount point), then whole.
        std::fs::create_dir(&away).unwrap();
        let changes = states.check(&conn, &folders, &bookmarks).unwrap();
        assert!(changes.is_empty(), "missing → empty is still unavailable");
        assert_eq!(states.get(away_id).state, FolderState::Empty);
        std::fs::write(away.join("a.flac"), "").unwrap();
        let changes = states.check(&conn, &folders, &bookmarks).unwrap();
        assert_eq!(
            changes,
            Changes {
                returned: vec![away_id],
                lost: vec![]
            }
        );
        assert_eq!(states.get(here_id), FolderStatus::AVAILABLE);
        assert!(states.unreadable().is_empty());
    }

    #[test]
    fn finds_the_tracks_of_folders() {
        let conn = db::open_in_memory().unwrap();
        let a = folder(&conn, Path::new("/A"), 2);
        let b = folder(&conn, Path::new("/B"), 2);
        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM tracks ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            tracks_in(&conn, &ids, &[b]).unwrap(),
            HashSet::from([ids[2], ids[3]])
        );
        assert_eq!(
            tracks_in(&conn, &ids[..3], &[a, b]).unwrap(),
            HashSet::from([ids[0], ids[1], ids[2]])
        );
        assert!(tracks_in(&conn, &ids, &[]).unwrap().is_empty());
    }

    #[test]
    fn a_held_folder_stays_held_until_a_scan_says_otherwise() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.flac"), "").unwrap();
        let id = folder(&conn, dir.path(), 30);
        let states = FolderStates::default();
        let mut changes = Changes::default();
        states.set(
            id,
            FolderStatus {
                state: FolderState::MostlyGone,
                detail: Some("29 tracks not found".into()),
            },
            &mut changes,
        );
        assert_eq!(changes.lost, [id]);
        let folders = super::super::folders(&conn).unwrap();
        assert!(states
            .check(&conn, &folders, &FakeBookmarks::default())
            .unwrap()
            .is_empty());
        assert_eq!(states.get(id).state, FolderState::MostlyGone);

        // Its other files still play.
        assert!(states.unreadable().is_empty());

        // A scan that succeeds clears it.
        let mut changes = Changes::default();
        states.set(id, FolderStatus::AVAILABLE, &mut changes);
        assert_eq!(changes.returned, [id]);
    }
}
