//! Opening the library database and migrating its schema, with a copy of
//! the database written before any migration (PLAN.md H10).

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use super::{genres, sort_key, Error};

/// Schema migrations in order. `PRAGMA user_version` records how many have
/// been applied. Append new ones; never edit one that has shipped.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/001_initial.sql"),
    include_str!("migrations/002_search.sql"),
    include_str!("migrations/003_metadata.sql"),
    include_str!("migrations/004_replay_gain.sql"),
    include_str!("migrations/005_track_parts.sql"),
    include_str!("migrations/006_features.sql"),
    include_str!("migrations/007_user_data.sql"),
    include_str!("migrations/008_credits.sql"),
    include_str!("migrations/009_substring_search.sql"),
    include_str!("migrations/010_dataless.sql"),
    include_str!("migrations/011_queue_items.sql"),
    include_str!("migrations/012_art_thumbs.sql"),
];

/// How many copies `back_up` keeps: the newest two.
const COPIES_KEPT: usize = 2;

/// Opens (creating if needed) the library database at `path` and brings its
/// schema up to date, first writing a copy (`back_up`) when there are
/// migrations to apply to an existing database. Each thread that needs the
/// database opens its own connection; WAL mode lets readers carry on while
/// a scan writes.
pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update_and_check(None, "journal_mode", "wal", |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", "normal")?;
    configure(&conn)?;
    let applied = schema_version(&conn)?;
    if applied > 0 && applied < MIGRATIONS.len() {
        back_up(&conn, path, applied + 1)?;
    }
    migrate(&mut conn)?;
    Ok(conn)
}

/// How many migrations the database has had.
fn schema_version(conn: &Connection) -> Result<usize, Error> {
    let applied: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(usize::try_from(applied).unwrap_or(usize::MAX))
}

/// A copy of the database written before migration `migration` was applied.
#[derive(Debug, Clone, PartialEq)]
pub struct Copy {
    pub path: PathBuf,
    pub migration: usize,
}

/// Writes a copy of the database at `path` to `<path>.pre-<migration>`
/// with `VACUUM INTO`, then deletes all but the newest `COPIES_KEPT`
/// copies. A copy that can't be written stops the migration, so the
/// database is left as it was.
pub fn back_up(conn: &Connection, path: &Path, migration: usize) -> Result<Copy, Error> {
    let target = copy_path(path, migration);
    let partial = with_suffix(&target, ".partial");
    let failed = |error: &dyn std::fmt::Display| {
        Error::Invalid(format!(
            "Cannot write a copy of the library database to {} before upgrading it: {error}",
            target.display()
        ))
    };
    if partial.exists() {
        std::fs::remove_file(&partial).map_err(|e| failed(&e))?;
    }
    conn.execute("VACUUM INTO ?1", [partial.to_string_lossy()])
        .map_err(|e| failed(&e))?;
    std::fs::rename(&partial, &target).map_err(|e| failed(&e))?;
    log::info!("copied the database before migration {migration}");
    for old in copies(path).into_iter().skip(COPIES_KEPT) {
        if let Err(error) = std::fs::remove_file(&old.path) {
            log::warn!(
                "cannot delete the copy from before migration {}: {error}",
                old.migration
            );
        }
    }
    Ok(Copy {
        path: target,
        migration,
    })
}

/// The copies `back_up` wrote of the database at `path`, newest first.
pub fn copies(path: &Path) -> Vec<Copy> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let prefix = format!("{}.pre-", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut copies: Vec<Copy> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let migration = file_name.to_str()?.strip_prefix(&prefix)?.parse().ok()?;
            Some(Copy {
                path: entry.path(),
                migration,
            })
        })
        .collect();
    copies.sort_by_key(|copy| std::cmp::Reverse(copy.migration));
    copies
}

fn copy_path(path: &Path, migration: usize) -> PathBuf {
    with_suffix(path, &format!(".pre-{migration}"))
}

/// `path` with `suffix` added to its file name.
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// A migrated in-memory database, for tests.
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, Error> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// An in-memory database with only the first `version` migrations applied,
/// for testing later ones.
#[cfg(test)]
pub fn open_in_memory_at(version: usize) -> Result<Connection, Error> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    for (index, sql) in MIGRATIONS.iter().take(version).enumerate() {
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", index as i64 + 1)?;
    }
    Ok(conn)
}

/// Per-connection settings and functions.
fn configure(conn: &Connection) -> Result<(), Error> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    sort_key::register(conn)?;
    genres::register(conn)?;
    Ok(())
}

pub(super) fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let applied = schema_version(conn)?;
    if applied > MIGRATIONS.len() {
        return Err(Error::Invalid(format!(
            "The library database is from a newer version of the app (schema {applied}, \
             this version knows {})",
            MIGRATIONS.len()
        )));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
        log::info!("applied migration {}", index + 1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> usize {
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        version as usize
    }

    #[test]
    fn creates_the_schema() {
        let conn = open_in_memory().unwrap();
        assert_eq!(user_version(&conn), MIGRATIONS.len());
        let names = |sql: &str| -> Vec<String> {
            conn.prepare(sql)
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        // Leaving out FTS5's own tables behind the search indexes.
        assert_eq!(
            names(
                "SELECT name FROM sqlite_schema WHERE type = 'table'
                 AND name NOT LIKE '%search%' AND name NOT LIKE '%trigram%' ORDER BY name"
            ),
            [
                "album_analysis",
                "album_art",
                "album_favourites",
                "album_links",
                "album_prefs",
                "albums",
                "art_thumbs",
                "artist_favourites",
                "artist_links",
                "artists",
                "folders",
                "kept_albums",
                "kept_artists",
                "listens_pending",
                "mb_cache",
                "playlist_items",
                "playlists",
                "plays",
                "queue_items",
                "remote_devices",
                "settings",
                "track_analysis",
                "track_artists",
                "track_favourites",
                "track_positions",
                "track_prefs",
                "track_ratings",
                "tracks"
            ]
        );
        assert_eq!(
            names(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name"
            ),
            [
                "albums_search",
                "albums_trigram",
                "artists_search",
                "artists_trigram",
                "tracks_search",
                "tracks_trigram"
            ]
        );
    }

    #[test]
    fn reopening_keeps_data_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        {
            let conn = open(&path).unwrap();
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('volume', '0.5')",
                [],
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), MIGRATIONS.len());
        let journal: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal, "wal");
        let volume: String = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'volume'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(volume, "0.5");
    }

    #[test]
    fn refuses_a_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
            .unwrap();
        let error = open(&path).unwrap_err().to_string();
        assert!(error.contains("newer version"), "{error}");
    }

    #[test]
    fn replay_gain_migration_marks_every_file_for_reading() {
        let mut conn = open_in_memory_at(3).unwrap();
        conn.execute_batch(
            "INSERT INTO folders (path, added_at) VALUES ('/Music', 0);
             INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                 sample_rate, channels, scanned_at)
             VALUES (1, 'a.flac', 10, 1234, 1.0, 44100, 2, 0);",
        )
        .unwrap();
        migrate(&mut conn).unwrap();
        let (mtime, gain): (i64, Option<f64>) = conn
            .query_row(
                "SELECT file_mtime_ns, replaygain_track_gain FROM tracks",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((mtime, gain), (-1, None));
    }

    #[test]
    fn track_parts_migration_keeps_ids_and_dates_arrivals() {
        let mut conn = open_in_memory_at(4).unwrap();
        conn.execute_batch(
            "INSERT INTO folders (path, added_at) VALUES ('/Music', 0);
             INSERT INTO artists (name) VALUES ('Band');
             INSERT INTO albums (title, artist_id) VALUES ('Record', 1);
             INSERT INTO tracks (id, folder_id, relative_path, file_size, file_mtime_ns, title,
                                 artist_id, album_id, album_artist_id, duration, sample_rate,
                                 channels, scanned_at)
             VALUES (7, 1, 'a/one.flac', 10, 1700000000123456789, 'One', 1, 1, 1, 1.0, 44100, 2,
                     1800000000),
                    (9, 1, 'a/two.flac', 10, -1, NULL, 1, 1, 1, 1.0, 44100, 2, 1800000001);",
        )
        .unwrap();
        migrate(&mut conn).unwrap();
        let rows: Vec<(i64, i64, f64, Option<f64>, i64)> = conn
            .prepare(
                "SELECT id, added_at, range_start, range_end, file_mtime_ns FROM tracks ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            rows,
            [
                (7, 1700000000, 0.0, None, -1),
                (9, 1800000001, 0.0, None, -1)
            ]
        );
        // The search index still finds both, the untitled one by its file name.
        let found: i64 = conn
            .query_row(
                "SELECT count(*) FROM tracks_search WHERE tracks_search MATCH 'one OR two'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(found, 2);
        // Two parts of one file are two tracks.
        conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, range_start, file_size, file_mtime_ns,
                                 duration, sample_rate, channels, scanned_at)
             VALUES (1, 'a/one.flac', 30.5, 10, 1, 1.0, 44100, 2, 0)",
            [],
        )
        .unwrap();
    }

    /// A database file at `path` with only the first `version` migrations.
    fn file_at(path: &Path, version: usize) {
        let conn = Connection::open(path).unwrap();
        configure(&conn).unwrap();
        for (index, sql) in MIGRATIONS.iter().take(version).enumerate() {
            conn.execute_batch(sql).unwrap();
            conn.pragma_update(None, "user_version", index as i64 + 1)
                .unwrap();
        }
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('volume', '0.5')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn the_saved_queue_moves_from_its_setting_to_rows() {
        // Migration 011 (PLAN.md H16), on a file, so the copy is written.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        file_at(&path, 10);
        let old = r#"{"tracks":[11,12,13],"original":[2,0,1],"current":1,
                      "position":42.5,"repeat":"all","volume":0.3}"#;
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('player.queue', ?1)",
                [old],
            )
            .unwrap();
        let conn = open(&path).unwrap();
        let rows: Vec<(i64, i64, f64, Option<f64>)> = conn
            .prepare("SELECT uid, track_id, ord, original FROM queue_items ORDER BY ord")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        // Before shuffling, the order was the third item, the first, the second.
        assert_eq!(
            rows,
            [
                (1, 11, 0.0, Some(1.0)),
                (2, 12, 1.0, Some(2.0)),
                (3, 13, 2.0, Some(0.0))
            ]
        );
        let setting: serde_json::Value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'player.queue'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|json| serde_json::from_str(&json).unwrap())
            .unwrap();
        assert_eq!(
            setting,
            serde_json::json!({"current": 1, "position": 42.5, "repeat": "all",
                               "volume": 0.3, "shuffled": true})
        );
        // The copy still has the list in the setting.
        let copy = Connection::open(dir.path().join("library.sqlite3.pre-11")).unwrap();
        assert_eq!(user_version(&copy), 10);
        let value: String = copy
            .query_row(
                "SELECT value FROM settings WHERE key = 'player.queue'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(value.contains("\"tracks\":[11,12,13]"));
    }

    #[test]
    fn copies_the_database_before_migrating_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        file_at(&path, 7);
        open(&path).unwrap();
        let copies = copies(&path);
        assert_eq!(
            copies,
            [Copy {
                path: dir.path().join("library.sqlite3.pre-8"),
                migration: 8
            }]
        );
        // The copy is the database as it was before migration 8.
        let copy = Connection::open(&copies[0].path).unwrap();
        assert_eq!(user_version(&copy), 7);
        let volume: String = copy
            .query_row(
                "SELECT value FROM settings WHERE key = 'volume'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(volume, "0.5");
        // Opening it again, with nothing to migrate, writes no copy.
        open(&path).unwrap();
        assert_eq!(super::copies(&path).len(), 1);
    }

    #[test]
    fn a_new_database_gets_no_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        open(&path).unwrap();
        assert!(copies(&path).is_empty());
    }

    #[test]
    fn keeps_the_newest_two_copies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        for migration in [3, 5, 6] {
            std::fs::write(copy_path(&path, migration), "old").unwrap();
        }
        // Another file whose name only starts the same way is left alone.
        std::fs::write(dir.path().join("library.sqlite3.pre-x"), "").unwrap();
        file_at(&path, 7);
        open(&path).unwrap();
        let kept: Vec<usize> = copies(&path).iter().map(|copy| copy.migration).collect();
        assert_eq!(kept, [8, 6]);
        assert!(dir.path().join("library.sqlite3.pre-x").exists());
        assert!(!with_suffix(&copy_path(&path, 8), ".partial").exists());
    }

    #[test]
    fn a_copy_that_cannot_be_written_stops_the_migration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        file_at(&path, 7);
        // A directory where the copy would go.
        std::fs::create_dir(copy_path(&path, 8)).unwrap();
        let error = open(&path).unwrap_err().to_string();
        assert!(error.contains("Cannot write a copy"), "{error}");
        assert_eq!(user_version(&Connection::open(&path).unwrap()), 7);
    }

    #[test]
    fn enforces_foreign_keys() {
        let conn = open_in_memory().unwrap();
        let orphan = conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                 sample_rate, channels, scanned_at)
             VALUES (42, 'a.flac', 1, 1, 1.0, 44100, 2, 0)",
            [],
        );
        assert!(orphan.is_err());
    }

    #[test]
    fn albums_are_unique_per_artist_even_without_one() {
        let conn = open_in_memory().unwrap();
        let insert = "INSERT INTO albums (title, artist_id) VALUES ('Untitled', NULL)";
        conn.execute(insert, []).unwrap();
        assert!(conn.execute(insert, []).is_err());
    }
}
