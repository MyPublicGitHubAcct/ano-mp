//! Opening the library database and migrating its schema.

use std::path::Path;
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
];

/// Opens (creating if needed) the library database at `path` and brings its
/// schema up to date. Each thread that needs the database opens its own
/// connection; WAL mode lets readers carry on while a scan writes.
pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update_and_check(None, "journal_mode", "wal", |row| row.get::<_, String>(0))?;
    conn.pragma_update(None, "synchronous", "normal")?;
    configure(&conn)?;
    migrate(&mut conn)?;
    Ok(conn)
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
    let applied: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let applied = usize::try_from(applied).unwrap_or(usize::MAX);
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
                 AND name NOT LIKE '%search%' ORDER BY name"
            ),
            [
                "album_analysis",
                "album_art",
                "album_links",
                "album_prefs",
                "albums",
                "artist_links",
                "artists",
                "folders",
                "listens_pending",
                "mb_cache",
                "plays",
                "remote_devices",
                "settings",
                "track_analysis",
                "track_prefs",
                "tracks"
            ]
        );
        assert_eq!(
            names(
                "SELECT name FROM sqlite_schema
                 WHERE type = 'table' AND sql LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name"
            ),
            ["albums_search", "artists_search", "tracks_search"]
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
