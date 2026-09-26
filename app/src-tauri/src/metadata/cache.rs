//! The response cache: JSON documents from every service, keyed by URL, in
//! the `mb_cache` table (named when MusicBrainz was the only service
//! planned). How long a copy stays fresh is up to the caller; stale copies
//! are kept as a fallback for when the service can't be reached, until
//! `prune` removes them.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

/// The cached body for `url` and when it was fetched (Unix seconds).
pub fn lookup(conn: &Connection, url: &str) -> rusqlite::Result<Option<(String, i64)>> {
    conn.prepare_cached("SELECT response, fetched_at FROM mb_cache WHERE request = ?1")?
        .query_row([url], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
}

pub fn store(conn: &Connection, url: &str, body: &str, fetched_at: i64) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO mb_cache (request, response, fetched_at) VALUES (?1, ?2, ?3)
         ON CONFLICT (request) DO UPDATE SET
             response = excluded.response, fetched_at = excluded.fetched_at",
    )?
    .execute(params![url, body, fetched_at])?;
    Ok(())
}

/// Removes copies fetched before `before` (Unix seconds); returns how many.
#[allow(dead_code)] // Nothing prunes the cache yet (PLAN.md 4.5, known limits).
pub fn prune(conn: &Connection, before: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM mb_cache WHERE fetched_at < ?1", [before])
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    #[test]
    fn stores_replaces_and_prunes() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(lookup(&conn, "https://a/1").unwrap(), None);
        store(&conn, "https://a/1", "one", 100).unwrap();
        store(&conn, "https://a/2", "two", 200).unwrap();
        store(&conn, "https://a/1", "uno", 300).unwrap();
        assert_eq!(
            lookup(&conn, "https://a/1").unwrap(),
            Some(("uno".into(), 300))
        );
        assert_eq!(prune(&conn, 250).unwrap(), 1);
        assert_eq!(lookup(&conn, "https://a/2").unwrap(), None);
        assert!(lookup(&conn, "https://a/1").unwrap().is_some());
    }
}
