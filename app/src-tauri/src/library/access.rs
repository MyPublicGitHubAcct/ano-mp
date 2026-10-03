//! Keeps library folders readable across launches, and says why one isn't.
//!
//! Under the macOS App Sandbox (and on iOS) the app may read a folder the
//! user picked only until it quits, so `add_folder` saves a security-scoped
//! bookmark in `folders.bookmark`, and the folder is resolved from it
//! before each scan and each time a track in it is opened for playback. A
//! bookmark follows its folder when it is moved on its volume, and the
//! stored path follows it, except into the Trash (PLAN.md H22): a folder
//! found there is unavailable until the user locates or removes it.
//!
//! `check_folder` classifies a folder (`FolderState`) without scanning it,
//! for the launch check and the sidebar. Bookmarks go through the
//! `Bookmarks` trait, so tests can stand in for the OS (`System`).

use std::any::Any;
use std::path::{Component, Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::{check_overlap, Error};
use crate::anomp::{self, FolderAccess};

/// Whether a library folder can be read now, and if not, why (PLAN.md H22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum FolderState {
    Available,
    /// Its bookmark doesn't resolve and its path is gone: a drive that
    /// isn't connected, a share that isn't mounted, a folder deleted.
    Missing,
    /// It can be read but holds nothing, though it had tracks: often the
    /// mount point of a drive or share that isn't mounted.
    Empty,
    /// A scan would have removed most of its tracks at once; they are kept
    /// until the user says to remove them.
    MostlyGone,
    /// It was moved to the Trash.
    InTrash,
    /// It is there, but the app may not read it: permissions, or a
    /// bookmark another build of the app made (`rebookmark`).
    NoPermission,
}

impl FolderState {
    /// The reason code in a `folderUnavailable` error.
    pub fn code(self) -> &'static str {
        match self {
            FolderState::Available => "available",
            FolderState::Missing => "missing",
            FolderState::Empty => "empty",
            FolderState::MostlyGone => "mostlyGone",
            FolderState::InTrash => "inTrash",
            FolderState::NoPermission => "noPermission",
        }
    }

    fn from_code(code: &str) -> Option<FolderState> {
        [
            FolderState::Available,
            FolderState::Missing,
            FolderState::Empty,
            FolderState::MostlyGone,
            FolderState::InTrash,
            FolderState::NoPermission,
        ]
        .into_iter()
        .find(|state| state.code() == code)
    }

    /// The state a `folderUnavailable` error names, if `error` is one.
    pub fn of_error(error: &impl std::fmt::Display) -> Option<FolderState> {
        let value: serde_json::Value = serde_json::from_str(&error.to_string()).ok()?;
        if value["code"] != "folderUnavailable" {
            return None;
        }
        Some(
            value["params"]["reason"]
                .as_str()
                .and_then(FolderState::from_code)
                .unwrap_or(FolderState::Missing),
        )
    }
}

/// A folder's state, with the system's words for it when there are any.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    pub state: FolderState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl FolderStatus {
    pub const AVAILABLE: FolderStatus = FolderStatus {
        state: FolderState::Available,
        detail: None,
    };

    fn new(state: FolderState, detail: Option<String>) -> FolderStatus {
        FolderStatus { state, detail }
    }

    /// The status a `folderUnavailable` error gives, if `error` is one.
    pub fn of_error(error: &impl std::fmt::Display) -> Option<FolderStatus> {
        let state = FolderState::of_error(error)?;
        let value: serde_json::Value = serde_json::from_str(&error.to_string()).ok()?;
        let detail = value["params"]["detail"].as_str().map(str::to_owned);
        Some(FolderStatus::new(state, detail))
    }
}

/// A folder the bookmark resolved to, readable while `guard` lives.
pub struct Started {
    pub path: PathBuf,
    pub stale: bool,
    pub guard: Option<Box<dyn Any + Send + Sync>>,
}

/// Making and resolving bookmarks: the OS's (`System`), or a test's.
pub trait Bookmarks {
    fn create(&self, folder: &Path) -> Result<Vec<u8>, String>;
    /// Resolves `bookmark` and starts accessing its folder.
    fn start(&self, bookmark: &[u8]) -> Result<Started, String>;
}

/// The core's security-scoped bookmarks (`FolderAccess`).
pub struct System;

impl Bookmarks for System {
    fn create(&self, folder: &Path) -> Result<Vec<u8>, String> {
        anomp::create_bookmark(folder)
    }

    fn start(&self, bookmark: &[u8]) -> Result<Started, String> {
        let access = FolderAccess::start(bookmark)?;
        Ok(Started {
            path: access.path().to_path_buf(),
            stale: access.is_stale(),
            guard: Some(Box::new(access)),
        })
    }
}

/// A library folder, readable while this is alive.
pub struct OpenFolder {
    /// Where the folder is now.
    pub path: PathBuf,
    /// `None` for a folder without a bookmark, which is readable only if the
    /// app isn't sandboxed.
    _access: Option<Box<dyn Any + Send + Sync>>,
}

/// Resolves library folder `folder_id` and starts accessing it. If the folder
/// moved, its stored path is updated (and its tracks, stored relative to it,
/// follow), unless it moved into the Trash; a stale bookmark is replaced,
/// and so is one that no longer resolves if the folder can still be read
/// (`rebookmark`). A folder added before bookmarks were saved gets one now
/// if it can be read. Fails with a `folderUnavailable` error naming the
/// reason (`FolderState::of_error`) if the folder can't be resolved, e.g. on
/// an unmounted drive.
pub fn open_folder(conn: &Connection, folder_id: i64) -> Result<OpenFolder, Error> {
    open_folder_with(conn, folder_id, &System)
}

pub fn open_folder_with(
    conn: &Connection,
    folder_id: i64,
    bookmarks: &impl Bookmarks,
) -> Result<OpenFolder, Error> {
    let (stored, bookmark): (String, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT path, bookmark FROM folders WHERE id = ?1",
            [folder_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| Error::Invalid(crate::coded::no_folder(folder_id)))?;
    let unavailable = |state: FolderState, detail: Option<&str>| {
        Error::Invalid(crate::coded::folder_unavailable(
            &stored,
            state.code(),
            detail,
        ))
    };

    let Some(bookmark) = bookmark else {
        if let Ok(bookmark) = bookmarks.create(Path::new(&stored)) {
            save_bookmark(conn, folder_id, &bookmark)?;
        }
        return Ok(OpenFolder {
            path: stored.into(),
            _access: None,
        });
    };

    let started = match bookmarks.start(&bookmark) {
        Ok(started) => started,
        Err(error) => match rebookmark(conn, folder_id, &stored, bookmarks) {
            Some(open) => return Ok(open),
            None => return Err(unavailable(unresolved(&stored, &error), Some(&error))),
        },
    };
    let path = started.path;
    if in_trash(&path) {
        return Err(unavailable(
            FolderState::InTrash,
            Some(&path.to_string_lossy()),
        ));
    }
    if path != Path::new(&stored) {
        let text = path.to_str().ok_or_else(|| {
            Error::Invalid(format!("Path is not valid UTF-8: {}", path.display()))
        })?;
        check_overlap(conn, &path, Some(folder_id))?;
        conn.execute(
            "UPDATE folders SET path = ?1 WHERE id = ?2",
            params![text, folder_id],
        )?;
        log::info!("folder {folder_id} moved");
        log::debug!("folder {folder_id} is now {}", path.display());
    }
    if started.stale {
        // Keep the old bookmark if this fails; it still resolved.
        if let Ok(bookmark) = bookmarks.create(&path) {
            save_bookmark(conn, folder_id, &bookmark)?;
        }
    }
    Ok(OpenFolder {
        path,
        _access: started.guard,
    })
}

/// Why a folder whose bookmark doesn't resolve can't be opened, going by
/// its stored path first: nothing there is missing; a folder there, or a
/// path the sandbox won't let the app look at, is no permission. Only
/// otherwise does the bookmark's error decide: "isn't in the correct
/// format" is a bookmark made by another build. Under the sandbox a deleted
/// folder's bookmark fails with those words too.
fn unresolved(stored: &str, error: &str) -> FolderState {
    match std::fs::metadata(stored) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => FolderState::Missing,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            FolderState::NoPermission
        }
        Ok(metadata) if metadata.is_dir() => FolderState::NoPermission,
        _ if error.contains("isn’t in the correct format")
            || error.contains("isn't in the correct format") =>
        {
            FolderState::NoPermission
        }
        _ => FolderState::Missing,
    }
}

/// Whether `path` is in a Trash: the user's (`~/.Trash`), a volume's
/// (`/Volumes/X/.Trashes/<uid>`), or a freedesktop one (`…/Trash/files`).
pub fn in_trash(path: &Path) -> bool {
    let names: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    names
        .iter()
        .any(|name| *name == ".Trash" || *name == ".Trashes")
        || names
            .windows(2)
            .any(|pair| pair[0] == "Trash" && pair[1] == "files")
}

/// Opens the library folder that holds the file at `path`, or returns `None`
/// if it is in none (e.g. a file the user picked directly).
pub fn open_folder_of(conn: &Connection, path: &Path) -> Result<Option<OpenFolder>, Error> {
    let mut statement = conn.prepare("SELECT id, path FROM folders")?;
    let folders = statement.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for folder in folders {
        let (id, folder_path) = folder?;
        if path.starts_with(&folder_path) {
            return open_folder(conn, id).map(Some);
        }
    }
    Ok(None)
}

/// Says whether library folder `folder_id` can be read now, without
/// scanning it: it resolves (`open_folder_with`), it can be listed, and it
/// isn't empty if it had tracks. A folder whose tracks a scan kept
/// (`FolderState::MostlyGone`) reads as available here; the scan says so.
pub fn check_folder(
    conn: &Connection,
    folder_id: i64,
    bookmarks: &impl Bookmarks,
) -> Result<FolderStatus, Error> {
    let open = match open_folder_with(conn, folder_id, bookmarks) {
        Ok(open) => open,
        Err(error) => return FolderStatus::of_error(&error).ok_or(error),
    };
    let had_tracks: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM tracks WHERE folder_id = ?1)",
        [folder_id],
        |row| row.get(0),
    )?;
    Ok(match std::fs::read_dir(&open.path) {
        Ok(mut entries) => {
            let empty = !entries.any(|entry| {
                entry.is_ok_and(|entry| entry.file_name().as_encoded_bytes().first() != Some(&b'.'))
            });
            if empty && had_tracks {
                FolderStatus::new(FolderState::Empty, None)
            } else {
                FolderStatus::AVAILABLE
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            FolderStatus::new(FolderState::NoPermission, Some(error.to_string()))
        }
        Err(error) => FolderStatus::new(FolderState::Missing, Some(error.to_string())),
    })
}

/// Replaces a bookmark that no longer resolves while its folder is still
/// readable at the stored path. macOS ties a security-scoped bookmark to the
/// code signature of the app that made it, and an ad-hoc signature (every
/// development build) changes with each build, so a rebuilt app can't
/// resolve its predecessor's bookmarks. Outside the sandbox the folder is
/// still readable and gets a new bookmark; inside it, it isn't, and the
/// user must pick the folder again.
fn rebookmark(
    conn: &Connection,
    folder_id: i64,
    stored: &str,
    bookmarks: &impl Bookmarks,
) -> Option<OpenFolder> {
    let path = Path::new(stored);
    std::fs::read_dir(path).ok()?;
    let bookmark = bookmarks.create(path).ok()?;
    let started = bookmarks.start(&bookmark).ok()?;
    save_bookmark(conn, folder_id, &bookmark).ok()?;
    Some(OpenFolder {
        path: started.path,
        _access: started.guard,
    })
}

fn save_bookmark(conn: &Connection, folder_id: i64, bookmark: &[u8]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE folders SET bookmark = ?1 WHERE id = ?2",
        params![bookmark, folder_id],
    )?;
    Ok(())
}

/// Bookmarks for tests: a bookmark is a path, resolved to wherever `moves`
/// says it went, or failing with `fail`'s message.
#[cfg(test)]
pub mod testing {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    use super::{Bookmarks, Started};

    #[derive(Default)]
    pub struct FakeBookmarks {
        pub moves: HashMap<PathBuf, PathBuf>,
        /// Bookmarks (paths) that don't resolve, with the error.
        pub fail: HashMap<PathBuf, String>,
    }

    impl Bookmarks for FakeBookmarks {
        fn create(&self, folder: &Path) -> Result<Vec<u8>, String> {
            Ok(folder.to_string_lossy().as_bytes().to_vec())
        }

        fn start(&self, bookmark: &[u8]) -> Result<Started, String> {
            let path = PathBuf::from(String::from_utf8_lossy(bookmark).into_owned());
            if let Some(error) = self.fail.get(&path) {
                return Err(error.clone());
            }
            Ok(Started {
                path: self.moves.get(&path).cloned().unwrap_or(path),
                stale: false,
                guard: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{add_folder, db, folders};

    fn bookmark_of(conn: &Connection, folder_id: i64) -> Option<Vec<u8>> {
        conn.query_row(
            "SELECT bookmark FROM folders WHERE id = ?1",
            [folder_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn adding_a_folder_saves_a_bookmark_that_resolves() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let folder = add_folder(&conn, dir.path()).unwrap();
        assert!(bookmark_of(&conn, folder.id).is_some_and(|b| !b.is_empty()));

        let open = open_folder(&conn, folder.id).unwrap();
        assert_eq!(open.path, Path::new(&folder.path));
        assert_eq!(folders(&conn).unwrap()[0].path, folder.path);

        let file = open.path.join("Album").join("01.flac");
        assert_eq!(
            open_folder_of(&conn, &file).unwrap().map(|f| f.path),
            Some(open.path)
        );
        assert!(open_folder_of(&conn, Path::new("/elsewhere/01.flac"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn folders_without_a_bookmark_get_one() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let folder = add_folder(&conn, dir.path()).unwrap();
        conn.execute("UPDATE folders SET bookmark = NULL", [])
            .unwrap();

        let open = open_folder(&conn, folder.id).unwrap();
        assert_eq!(open.path, Path::new(&folder.path));
        assert!(bookmark_of(&conn, folder.id).is_some());
    }

    #[cfg(target_vendor = "apple")]
    #[test]
    fn a_moved_folder_keeps_its_place_in_the_library() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let before = dir.path().join("Music");
        std::fs::create_dir(&before).unwrap();
        let folder = add_folder(&conn, &before).unwrap();

        let after = std::fs::canonicalize(dir.path()).unwrap().join("Música");
        std::fs::rename(&before, &after).unwrap();
        let open = open_folder(&conn, folder.id).unwrap();
        assert_eq!(open.path, after);
        assert_eq!(Path::new(&folders(&conn).unwrap()[0].path), after);
    }

    #[test]
    fn unresolvable_bookmarks_of_readable_folders_are_replaced() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let folder = add_folder(&conn, dir.path()).unwrap();
        // As a rebuilt, ad-hoc signed app sees its predecessor's bookmark.
        conn.execute("UPDATE folders SET bookmark = x'00'", [])
            .unwrap();

        let open = open_folder(&conn, folder.id).unwrap();
        assert_eq!(open.path, Path::new(&folder.path));
        let bookmark = bookmark_of(&conn, folder.id).unwrap();
        assert_ne!(bookmark, [0]);
        assert!(FolderAccess::start(&bookmark).is_ok());
    }

    #[test]
    fn unavailable_folders_are_reported() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Music");
        std::fs::create_dir(&music).unwrap();
        let folder = add_folder(&conn, &music).unwrap();
        std::fs::remove_dir(&music).unwrap();

        let error = open_folder(&conn, folder.id).err().unwrap();
        assert!(crate::coded::is(&error, "folderUnavailable"), "{error}");
        let error = open_folder(&conn, folder.id + 1).err().unwrap();
        assert!(crate::coded::is(&error, "noFolder"), "{error}");
        // The stored path and bookmark are left for when it comes back.
        assert_eq!(folders(&conn).unwrap()[0].path, folder.path);
        assert!(bookmark_of(&conn, folder.id).is_some());
    }

    // ---- Folder states (PLAN.md H22), with fake bookmarks ----

    use super::testing::FakeBookmarks;

    /// A library folder at `path` with a fake bookmark, and `tracks` tracks.
    fn fake_folder(conn: &Connection, path: &Path, tracks: usize) -> i64 {
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

    fn state(conn: &Connection, folder_id: i64, bookmarks: &FakeBookmarks) -> FolderState {
        check_folder(conn, folder_id, bookmarks).unwrap().state
    }

    fn stored_path(conn: &Connection, folder_id: i64) -> String {
        conn.query_row(
            "SELECT path FROM folders WHERE id = ?1",
            [folder_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn a_readable_folder_is_available() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.flac"), "").unwrap();
        let id = fake_folder(&conn, dir.path(), 3);
        assert_eq!(
            state(&conn, id, &FakeBookmarks::default()),
            FolderState::Available
        );
    }

    #[test]
    fn a_folder_that_is_gone_is_missing() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Music");
        let id = fake_folder(&conn, &music, 3);
        let bookmarks = FakeBookmarks {
            fail: [(music.clone(), "The file couldn’t be opened".into())].into(),
            ..FakeBookmarks::default()
        };
        let status = check_folder(&conn, id, &bookmarks).unwrap();
        assert_eq!(status.state, FolderState::Missing);
        assert_eq!(
            status.detail.as_deref(),
            Some("The file couldn’t be opened")
        );
        // A bookmark that resolves to a path that has gone since.
        assert_eq!(
            state(&conn, id, &FakeBookmarks::default()),
            FolderState::Missing
        );
        assert_eq!(stored_path(&conn, id), music.to_str().unwrap());
    }

    #[test]
    fn an_empty_folder_that_had_tracks_is_empty() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        // A mount point left behind holds nothing but hidden files.
        std::fs::write(dir.path().join(".DS_Store"), "").unwrap();
        let had = fake_folder(&conn, dir.path(), 2);
        assert_eq!(
            state(&conn, had, &FakeBookmarks::default()),
            FolderState::Empty
        );

        let other = tempfile::tempdir().unwrap();
        let new = fake_folder(&conn, other.path(), 0);
        assert_eq!(
            state(&conn, new, &FakeBookmarks::default()),
            FolderState::Available
        );
    }

    #[test]
    fn a_folder_in_the_trash_is_not_followed_there() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Music");
        let trashed = dir.path().join(".Trash").join("Music");
        std::fs::create_dir_all(&trashed).unwrap();
        std::fs::write(trashed.join("a.flac"), "").unwrap();
        let id = fake_folder(&conn, &music, 1);
        let bookmarks = FakeBookmarks {
            moves: [(music.clone(), trashed)].into(),
            ..FakeBookmarks::default()
        };
        assert_eq!(state(&conn, id, &bookmarks), FolderState::InTrash);
        let error = open_folder_with(&conn, id, &bookmarks).err().unwrap();
        assert_eq!(FolderState::of_error(&error), Some(FolderState::InTrash));
        assert_eq!(stored_path(&conn, id), music.to_str().unwrap());
    }

    #[test]
    fn trash_paths() {
        assert!(in_trash(Path::new("/Users/me/.Trash/Music")));
        assert!(in_trash(Path::new("/Volumes/Disk/.Trashes/501/Music")));
        assert!(in_trash(Path::new(
            "/home/me/.local/share/Trash/files/Music"
        )));
        assert!(!in_trash(Path::new("/Users/me/Music/Trash")));
        assert!(!in_trash(Path::new("/Users/me/Music/.Trashy")));
    }

    #[test]
    fn a_folder_the_app_may_not_read_has_no_permission() {
        let conn = db::open_in_memory().unwrap();
        // The bookmark of another build of the app, over a folder still there.
        let dir = tempfile::tempdir().unwrap();
        let id = fake_folder(&conn, dir.path(), 1);
        let bookmarks = FakeBookmarks {
            fail: [(
                dir.path().to_path_buf(),
                "The file couldn’t be opened because it isn’t in the correct format.".into(),
            )]
            .into(),
            ..FakeBookmarks::default()
        };
        assert_eq!(state(&conn, id, &bookmarks), FolderState::NoPermission);

        // A deleted folder's bookmark fails with the same words under the
        // sandbox (seen in a bundle, Step 4): nothing at its path is
        // missing, whatever the bookmark says.
        let gone = dir.path().join("Gone");
        let gone_id = fake_folder(&conn, &gone, 1);
        let bookmarks = FakeBookmarks {
            fail: [(
                gone.clone(),
                "The file couldn’t be opened because it isn’t in the correct format.".into(),
            )]
            .into(),
            ..FakeBookmarks::default()
        };
        assert_eq!(state(&conn, gone_id, &bookmarks), FolderState::Missing);

        // A folder that can't be listed.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let locked = tempfile::tempdir().unwrap();
            let id = fake_folder(&conn, locked.path(), 1);
            std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o000))
                .unwrap();
            let found = state(&conn, id, &FakeBookmarks::default());
            std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o700))
                .unwrap();
            assert_eq!(found, FolderState::NoPermission);
        }
    }

    #[test]
    fn states_round_trip_through_errors() {
        for state in [
            FolderState::Missing,
            FolderState::Empty,
            FolderState::MostlyGone,
            FolderState::InTrash,
            FolderState::NoPermission,
        ] {
            let error = crate::coded::folder_unavailable("/Music", state.code(), Some("why"));
            assert_eq!(FolderState::of_error(&error), Some(state));
        }
        assert_eq!(FolderState::of_error(&crate::coded::scan_running()), None);
    }
}
