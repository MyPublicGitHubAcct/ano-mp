//! Opening the library database and migrating its schema.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use super::Error;

/// Schema migrations in order. `PRAGMA user_version` records how many have
/// been applied. Append new ones; never edit one that has shipped.
const MIGRATIONS: &[&str] = &[include_str!("migrations/001_initial.sql")];

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

/// Per-connection settings.
fn configure(conn: &Connection) -> Result<(), Error> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<(), Error> {
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
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        version as usize
    }

    #[test]
    fn creates_the_schema() {
        let conn = open_in_memory().unwrap();
        assert_eq!(user_version(&conn), MIGRATIONS.len());
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(tables, ["albums", "artists", "folders", "mb_cache", "settings", "tracks"]);
    }

    #[test]
    fn reopening_keeps_data_and_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.sqlite3");
        {
            let conn = open(&path).unwrap();
            conn.execute("INSERT INTO settings (key, value) VALUES ('volume', '0.5')", [])
                .unwrap();
        }
        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), MIGRATIONS.len());
        let journal: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(journal, "wal");
        let volume: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'volume'", [], |row| row.get(0))
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
