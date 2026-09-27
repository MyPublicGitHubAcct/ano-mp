//! Keeps library folders readable across launches. Under the macOS App
//! Sandbox (and on iOS) the app may read a folder the user picked only until
//! it quits, so `add_folder` saves a security-scoped bookmark in
//! `folders.bookmark`, and the folder is resolved from it before each scan
//! and each time a track in it is opened for playback. A bookmark follows its
//! folder when it is moved on its volume, and the stored path follows it.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use super::{check_overlap, Error};
use crate::anomp::{self, FolderAccess};

/// A library folder, readable while this is alive.
pub struct OpenFolder {
    /// Where the folder is now.
    pub path: PathBuf,
    /// `None` for a folder without a bookmark, which is readable only if the
    /// app isn't sandboxed.
    _access: Option<FolderAccess>,
}

/// Resolves library folder `folder_id` and starts accessing it. If the folder
/// moved, its stored path is updated (and its tracks, stored relative to it,
/// follow); a stale bookmark is replaced, and so is one that no longer
/// resolves if the folder can still be read (`rebookmark`). A folder added
/// before bookmarks were saved gets one now if it can be read. Fails with `Error::Invalid` if
/// the folder can't be resolved, e.g. on an unmounted drive.
pub fn open_folder(conn: &Connection, folder_id: i64) -> Result<OpenFolder, Error> {
    let (stored, bookmark): (String, Option<Vec<u8>>) = conn
        .query_row(
            "SELECT path, bookmark FROM folders WHERE id = ?1",
            [folder_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| Error::Invalid(crate::coded::no_folder(folder_id)))?;

    let Some(bookmark) = bookmark else {
        if let Ok(bookmark) = anomp::create_bookmark(Path::new(&stored)) {
            save_bookmark(conn, folder_id, &bookmark)?;
        }
        return Ok(OpenFolder {
            path: stored.into(),
            _access: None,
        });
    };

    let access = match FolderAccess::start(&bookmark) {
        Ok(access) => access,
        Err(error) => match rebookmark(conn, folder_id, &stored) {
            Some(open) => return Ok(open),
            None => {
                return Err(Error::Invalid(crate::coded::folder_unavailable(
                    &stored,
                    Some(&error.to_string()),
                )))
            }
        },
    };
    let path = access.path().to_path_buf();
    if path != Path::new(&stored) {
        let text = path.to_str().ok_or_else(|| {
            Error::Invalid(format!("Path is not valid UTF-8: {}", path.display()))
        })?;
        check_overlap(conn, &path, Some(folder_id))?;
        conn.execute(
            "UPDATE folders SET path = ?1 WHERE id = ?2",
            params![text, folder_id],
        )?;
    }
    if access.is_stale() {
        // Keep the old bookmark if this fails; it still resolved.
        if let Ok(bookmark) = anomp::create_bookmark(&path) {
            save_bookmark(conn, folder_id, &bookmark)?;
        }
    }
    Ok(OpenFolder {
        path,
        _access: Some(access),
    })
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

/// Replaces a bookmark that no longer resolves while its folder is still
/// readable at the stored path. macOS ties a security-scoped bookmark to the
/// code signature of the app that made it, and an ad-hoc signature (every
/// development build) changes with each build, so a rebuilt app can't
/// resolve its predecessor's bookmarks. Outside the sandbox the folder is
/// still readable and gets a new bookmark; inside it, it isn't, and the
/// user must pick the folder again.
fn rebookmark(conn: &Connection, folder_id: i64, stored: &str) -> Option<OpenFolder> {
    let path = Path::new(stored);
    std::fs::read_dir(path).ok()?;
    let bookmark = anomp::create_bookmark(path).ok()?;
    let access = FolderAccess::start(&bookmark).ok()?;
    save_bookmark(conn, folder_id, &bookmark).ok()?;
    Some(OpenFolder {
        path: access.path().to_path_buf(),
        _access: Some(access),
    })
}

fn save_bookmark(conn: &Connection, folder_id: i64, bookmark: &[u8]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE folders SET bookmark = ?1 WHERE id = ?2",
        params![bookmark, folder_id],
    )?;
    Ok(())
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
}
