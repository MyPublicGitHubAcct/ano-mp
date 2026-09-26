//! The music library: the SQLite database, the folders in it, the
//! incremental folder scanner that fills it from the core's tag reader, and
//! browsing it under configurable sort rules.

pub mod browse;
pub mod commands;
pub mod db;
pub mod genres;
pub mod rules;
pub mod scanner;
pub mod sort_key;

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::Serialize;

#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    /// A problem to show the user as is, e.g. an unusable folder.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(error) => write!(f, "Library database error: {error}"),
            Error::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Error::Db(error)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub track_count: u32,
    /// Unix seconds; `None` until the first scan.
    pub last_scan_at: Option<i64>,
}

pub fn folders(conn: &Connection) -> Result<Vec<Folder>, Error> {
    let mut statement = conn.prepare(
        "SELECT id, path, last_scan_at,
                (SELECT count(*) FROM tracks WHERE tracks.folder_id = folders.id)
         FROM folders ORDER BY path",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Folder {
            id: row.get(0)?,
            path: row.get(1)?,
            last_scan_at: row.get(2)?,
            track_count: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Adds the folder at the absolute `path` (resolving symlinks and `..`),
/// unscanned. Refuses a folder that is inside, or contains, one already in
/// the library, so no file is listed twice.
pub fn add_folder(conn: &Connection, path: &Path) -> Result<Folder, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid(format!("Path is not absolute: {}", path.display())));
    }
    let path = std::fs::canonicalize(path)
        .map_err(|error| Error::Invalid(format!("Cannot open {}: {error}", path.display())))?;
    if !path.is_dir() {
        return Err(Error::Invalid(format!("Not a folder: {}", path.display())));
    }
    let text = path
        .to_str()
        .ok_or_else(|| Error::Invalid(format!("Path is not valid UTF-8: {}", path.display())))?;
    for existing in folders(conn)? {
        let other = Path::new(&existing.path);
        if path.starts_with(other) || other.starts_with(&path) {
            return Err(Error::Invalid(if path == other {
                format!("{text} is already in the library")
            } else {
                format!("{text} overlaps the library folder {}", existing.path)
            }));
        }
    }
    conn.execute(
        "INSERT INTO folders (path, added_at) VALUES (?1, ?2)",
        params![text, unix_now()],
    )?;
    Ok(Folder {
        id: conn.last_insert_rowid(),
        path: text.to_owned(),
        track_count: 0,
        last_scan_at: None,
    })
}

/// Removes a folder and its tracks from the library (not from disk).
pub fn remove_folder(conn: &mut Connection, folder_id: i64) -> Result<(), Error> {
    let tx = conn.transaction()?;
    if tx.execute("DELETE FROM folders WHERE id = ?1", [folder_id])? == 0 {
        return Err(Error::Invalid(format!("No library folder with id {folder_id}")));
    }
    remove_orphans(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Deletes albums and artists that no track refers to any more.
fn remove_orphans(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DELETE FROM albums WHERE id NOT IN
             (SELECT album_id FROM tracks WHERE album_id IS NOT NULL);
         DELETE FROM artists WHERE id NOT IN
             (SELECT artist_id FROM tracks WHERE artist_id IS NOT NULL
              UNION SELECT album_artist_id FROM tracks WHERE album_artist_id IS NOT NULL
              UNION SELECT artist_id FROM albums WHERE artist_id IS NOT NULL);",
    )
}

/// A track as the library lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSummary {
    pub id: i64,
    /// Absolute path of the file.
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration: f64,
}

/// The columns `track_from_row` reads, from `TRACKS_FROM`.
const TRACK_COLUMNS: &str = "t.id, f.path, t.relative_path, t.title, artist.name, album.title,
     album_artist.name, t.genre, t.year, t.disc_number, t.track_number, t.duration";

/// Tracks `t` with their folder `f`, `artist`, `album` and `album_artist`.
const TRACKS_FROM: &str = "FROM tracks t
     JOIN folders f ON f.id = t.folder_id
     LEFT JOIN artists artist ON artist.id = t.artist_id
     LEFT JOIN albums album ON album.id = t.album_id
     LEFT JOIN artists album_artist ON album_artist.id = t.album_artist_id";

fn track_from_row(row: &rusqlite::Row) -> rusqlite::Result<TrackSummary> {
    let folder: String = row.get(1)?;
    let relative: String = row.get(2)?;
    Ok(TrackSummary {
        id: row.get(0)?,
        path: track_path(Path::new(&folder), &relative)
            .to_string_lossy()
            .into_owned(),
        title: row.get(3)?,
        artist: row.get(4)?,
        album: row.get(5)?,
        album_artist: row.get(6)?,
        genre: row.get(7)?,
        year: row.get(8)?,
        disc_number: row.get(9)?,
        track_number: row.get(10)?,
        duration: row.get(11)?,
    })
}

/// Every track, by folder and then path within it (bytewise), for tests.
/// The app browses with `browse`.
#[cfg(test)]
pub fn tracks(conn: &Connection) -> Result<Vec<TrackSummary>, Error> {
    let mut statement = conn.prepare(&format!(
        "SELECT {TRACK_COLUMNS} {TRACKS_FROM} ORDER BY f.path, t.relative_path"
    ))?;
    let rows = statement.query_map([], track_from_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The file for a track stored as `relative` ('/'-separated) in `folder`.
pub fn track_path(folder: &Path, relative: &str) -> PathBuf {
    let mut path = folder.to_path_buf();
    path.extend(relative.split('/'));
    path
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_and_lists_folders() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Música");
        std::fs::create_dir(&music).unwrap();

        let folder = add_folder(&conn, &music).unwrap();
        // canonicalize resolves e.g. macOS's /var -> /private/var.
        assert_eq!(Path::new(&folder.path), std::fs::canonicalize(&music).unwrap());
        assert_eq!(folders(&conn).unwrap(), [folder]);
    }

    #[test]
    fn refuses_bad_and_overlapping_folders() {
        let conn = db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("Music");
        std::fs::create_dir_all(music.join("Jazz")).unwrap();
        std::fs::write(music.join("notes.txt"), "").unwrap();
        add_folder(&conn, &music).unwrap();

        let error = |path: &Path| add_folder(&conn, path).unwrap_err().to_string();
        assert!(error(Path::new("Music")).starts_with("Path is not absolute"));
        assert!(error(&dir.path().join("missing")).starts_with("Cannot open"));
        assert!(error(&music.join("notes.txt")).starts_with("Not a folder"));
        assert!(error(&music).ends_with("is already in the library"));
        assert!(error(&music.join("Jazz/..")).ends_with("is already in the library"));
        assert!(error(&music.join("Jazz")).contains("overlaps"));
        assert!(error(dir.path()).contains("overlaps"));
        // A sibling whose name merely starts the same way is fine.
        std::fs::create_dir(dir.path().join("Music 2")).unwrap();
        add_folder(&conn, &dir.path().join("Music 2")).unwrap();
    }

    #[test]
    fn joins_relative_paths_with_native_separators() {
        let path = track_path(Path::new("/Music"), "Artist/Album/01 Song.flac");
        let expected: PathBuf = ["/Music", "Artist", "Album", "01 Song.flac"].iter().collect();
        assert_eq!(path, expected);
    }
}
