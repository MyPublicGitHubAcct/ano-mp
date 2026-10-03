//! Checking the library database at launch, and recovering a damaged one
//! (PLAN.md H10).
//!
//! `PRAGMA quick_check` runs in the background at launch. When it fails,
//! the UI offers to restore the newest copy `db::back_up` wrote before a
//! migration, or to rebuild the library by rescanning, after exporting
//! what F20 can save (`transfer`). Either is done at the next launch,
//! before anything opens the database: the choice is written to
//! `RECOVERY_FILE` and the app restarts. The damaged database is never
//! deleted; it is moved aside as `<db>.damaged-<unix seconds>`.
//!
//! A rebuild starts from an empty database with the old one's folders and
//! settings (when they can still be read), scans every folder, then
//! imports the export.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::db::{self, Copy};
use super::{unix_now, Error};

/// The recovery the user chose, waiting for the next launch.
pub const RECOVERY_FILE: &str = "library.recovery.json";

/// At most this many problems are reported.
const MAX_PROBLEMS: usize = 20;

/// What the launch check found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum DbCheck {
    Running,
    Ok,
    Failed {
        /// SQLite's report, in English, for the details.
        problems: Vec<String>,
        /// The copy a restore would use, if there is one.
        copy: Option<CopyInfo>,
    },
}

/// A copy of the database, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CopyInfo {
    pub file_name: String,
    /// Unix seconds, when the copy was written.
    pub written_at: Option<i64>,
}

impl CopyInfo {
    fn of(copy: &Copy) -> CopyInfo {
        let written_at = std::fs::metadata(&copy.path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|elapsed| elapsed.as_secs() as i64);
        CopyInfo {
            file_name: file_name(&copy.path),
            written_at,
        }
    }
}

/// Runs `PRAGMA quick_check` over `conn`: the problems it finds, none when
/// the database is sound. A check that can't run at all is a problem too.
pub fn quick_check(conn: &Connection) -> Vec<String> {
    let rows = conn
        .prepare(&format!("PRAGMA quick_check({MAX_PROBLEMS})"))
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
        });
    match rows {
        Ok(rows) if rows == ["ok"] => Vec::new(),
        Ok(rows) => rows,
        Err(error) => vec![error.to_string()],
    }
}

/// Checks the database at `db_path` on a connection of its own.
pub fn check(db_path: &Path) -> DbCheck {
    let problems = match db::open(db_path) {
        Ok(conn) => quick_check(&conn),
        Err(error) => vec![error.to_string()],
    };
    if problems.is_empty() {
        return DbCheck::Ok;
    }
    failed(db_path, problems)
}

/// The database at `db_path` failed with `problems`.
pub fn failed(db_path: &Path, problems: Vec<String>) -> DbCheck {
    DbCheck::Failed {
        problems,
        copy: db::copies(db_path).first().map(CopyInfo::of),
    }
}

/// A recovery waiting for the next launch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "action")]
pub enum Recovery {
    /// Put the copy with this file name (next to the database) in its place.
    Restore { copy: String },
    /// Start again from the folders and settings, then import `export` (a
    /// file F20 wrote; `None` if nothing could be exported).
    Rebuild { export: Option<PathBuf> },
}

/// Records `recovery` for the next launch.
pub fn schedule(db_path: &Path, recovery: &Recovery) -> Result<(), Error> {
    let json = serde_json::to_vec(recovery).map_err(|e| Error::Invalid(e.to_string()))?;
    std::fs::write(recovery_path(db_path), json)
        .map_err(|e| Error::Invalid(format!("Cannot save the recovery: {e}")))
}

/// What `apply_scheduled` did.
#[derive(Debug, Clone, PartialEq)]
pub enum Recovered {
    Restored,
    /// Rebuilt: scan every folder, then import this export, if any.
    Rebuilt {
        export: Option<PathBuf>,
    },
}

/// Carries out a recovery scheduled before the last restart, if any. Call
/// before anything opens the database. The request is removed first, so a
/// recovery that fails isn't tried at every launch.
pub fn apply_scheduled(db_path: &Path) -> Result<Option<Recovered>, Error> {
    let request = recovery_path(db_path);
    let Ok(bytes) = std::fs::read(&request) else {
        return Ok(None);
    };
    let _ = std::fs::remove_file(&request);
    let recovery: Recovery = serde_json::from_slice(&bytes)
        .map_err(|e| Error::Invalid(format!("Unreadable recovery request: {e}")))?;
    log::warn!("recovering the database: {recovery:?}");
    match recovery {
        Recovery::Restore { copy } => {
            let copy = sibling(db_path, &copy)?;
            if !copy.is_file() {
                return Err(Error::Invalid(format!(
                    "The copy {} is gone",
                    copy.display()
                )));
            }
            let damaged = move_aside(db_path)?;
            std::fs::copy(&copy, db_path)
                .map_err(|e| Error::Invalid(format!("Cannot restore {}: {e}", copy.display())))?;
            log::warn!(
                "restored {}; the damaged database is {}",
                copy.display(),
                damaged.display()
            );
            Ok(Some(Recovered::Restored))
        }
        Recovery::Rebuild { export } => {
            let damaged = move_aside(db_path)?;
            let conn = db::open(db_path)?;
            if let Err(error) = carry_over(&conn, &damaged) {
                log::warn!("cannot carry the folders and settings over: {error}");
            }
            log::warn!(
                "started a new database; the damaged one is {}",
                damaged.display()
            );
            Ok(Some(Recovered::Rebuilt { export }))
        }
    }
}

/// Copies the folders (with their bookmarks) and the settings from the
/// damaged database, as far as they can be read. The folders' tracks
/// come back with a scan.
fn carry_over(conn: &Connection, damaged: &Path) -> Result<(), Error> {
    conn.execute("ATTACH DATABASE ?1 AS damaged", [damaged.to_string_lossy()])?;
    let copied = conn.execute_batch(
        "INSERT OR IGNORE INTO main.folders (id, path, bookmark, added_at)
             SELECT id, path, bookmark, added_at FROM damaged.folders;
         INSERT OR IGNORE INTO main.settings (key, value)
             SELECT key, value FROM damaged.settings;",
    );
    conn.execute("DETACH DATABASE damaged", [])?;
    Ok(copied?)
}

/// Moves the database at `db_path` and its WAL files aside, to
/// `<db>.damaged-<unix seconds>`; returns the new path.
fn move_aside(db_path: &Path) -> Result<PathBuf, Error> {
    let damaged = db::with_suffix(db_path, &format!(".damaged-{}", unix_now()));
    for suffix in ["", "-wal", "-shm"] {
        let from = db::with_suffix(db_path, suffix);
        if from.exists() {
            std::fs::rename(&from, db::with_suffix(&damaged, suffix)).map_err(|e| {
                Error::Invalid(format!("Cannot move {} aside: {e}", from.display()))
            })?;
        }
    }
    Ok(damaged)
}

fn recovery_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(RECOVERY_FILE)
}

/// `name`, a bare file name, next to the database.
fn sibling(db_path: &Path, name: &str) -> Result<PathBuf, Error> {
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." {
        return Err(Error::Invalid(format!("Not a copy's name: {name}")));
    }
    Ok(db_path.with_file_name(name))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The newest copy of the database at `db_path`, for a restore.
pub fn newest_copy(db_path: &Path) -> Option<String> {
    db::copies(db_path)
        .first()
        .map(|copy| file_name(&copy.path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library(dir: &Path) -> PathBuf {
        let path = dir.join("library.sqlite3");
        let conn = db::open(&path).unwrap();
        conn.execute_batch(
            "INSERT INTO folders (path, bookmark, added_at, last_scan_at)
                 VALUES ('/Music', x'0102', 5, 6);
             INSERT INTO settings (key, value) VALUES ('volume', '0.5');
             INSERT INTO playlists (name, created_at, updated_at) VALUES ('Mine', 1, 1);",
        )
        .unwrap();
        path
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn a_sound_database_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = library(dir.path());
        assert_eq!(check(&path), DbCheck::Ok);
    }

    #[test]
    fn a_damaged_database_fails_and_names_the_newest_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        {
            let conn = db::open(&path).unwrap();
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE;")
                .unwrap();
            let tx = conn.unchecked_transaction().unwrap();
            for i in 0..2000 {
                tx.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                    [format!("key {i}"), "x".repeat(200)],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        std::fs::write(db::with_suffix(&path, ".pre-9"), "a copy").unwrap();
        // Garbage over the pages after the schema's.
        let mut bytes = std::fs::read(&path).unwrap();
        let page = 4096;
        let start = bytes.len() / 2 / page * page;
        bytes[start..start + 4 * page].fill(0xA5);
        std::fs::write(&path, bytes).unwrap();
        match check(&path) {
            DbCheck::Failed { problems, copy } => {
                assert!(!problems.is_empty());
                assert_eq!(copy.unwrap().file_name, "library.sqlite3.pre-9");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nothing_scheduled_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = library(dir.path());
        assert_eq!(apply_scheduled(&path).unwrap(), None);
        assert_eq!(count(&db::open(&path).unwrap(), "playlists"), 1);
    }

    #[test]
    fn restores_a_copy_and_keeps_the_damaged_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = library(dir.path());
        let copy = db::back_up(&db::open(&path).unwrap(), &path, 10).unwrap();
        db::open(&path)
            .unwrap()
            .execute("DELETE FROM playlists", [])
            .unwrap();
        schedule(
            &path,
            &Recovery::Restore {
                copy: newest_copy(&path).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(apply_scheduled(&path).unwrap(), Some(Recovered::Restored));
        assert_eq!(count(&db::open(&path).unwrap(), "playlists"), 1);
        assert!(copy.path.exists(), "the copy is kept");
        let damaged: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.starts_with("library.sqlite3.damaged-") && !name.contains("-wal"))
            .filter(|name| !name.ends_with("-shm"))
            .collect();
        assert_eq!(damaged.len(), 1, "{damaged:?}");
        // Done once only.
        assert!(!dir.path().join(RECOVERY_FILE).exists());
        assert_eq!(apply_scheduled(&path).unwrap(), None);
    }

    #[test]
    fn a_rebuild_keeps_folders_and_settings_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = library(dir.path());
        let export = dir.path().join("rescue.json");
        schedule(
            &path,
            &Recovery::Rebuild {
                export: Some(export.clone()),
            },
        )
        .unwrap();
        assert_eq!(
            apply_scheduled(&path).unwrap(),
            Some(Recovered::Rebuilt {
                export: Some(export)
            })
        );
        let conn = db::open(&path).unwrap();
        let folder: (String, Vec<u8>, Option<i64>) = conn
            .query_row(
                "SELECT path, bookmark, last_scan_at FROM folders",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(folder, ("/Music".into(), vec![1, 2], None));
        assert_eq!(count(&conn, "settings"), 1);
        assert_eq!(count(&conn, "playlists"), 0);
    }

    #[test]
    fn a_restore_names_a_file_next_to_the_database_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = library(dir.path());
        for copy in ["../elsewhere", "/etc/passwd", ""] {
            schedule(&path, &Recovery::Restore { copy: copy.into() }).unwrap();
            assert!(apply_scheduled(&path).is_err(), "{copy}");
        }
        assert_eq!(count(&db::open(&path).unwrap(), "playlists"), 1);
    }
}
