//! The response cache: JSON documents from every service, keyed by URL, in
//! the `mb_cache` table (named when MusicBrainz was the only service
//! planned). How long a copy stays fresh is up to the caller; stale copies
//! are kept as a fallback for when the service can't be reached, until
//! `prune` removes them: at launch, those older than `MAX_AGE`, then the
//! oldest until the rest fit in `MAX_BYTES` (PLAN.md H10).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// How long a copy is kept as a fallback: well past the longest time any
/// service's copy stays fresh (30 days).
pub const MAX_AGE: Duration = Duration::from_secs(180 * 86400);
/// The most the cache keeps, counting its URLs and bodies.
pub const MAX_BYTES: i64 = 64 * 1024 * 1024;

/// Removes copies fetched before `now - max_age`, then the oldest of the
/// rest until they fit in `max_bytes`; returns how many went.
pub fn prune(
    conn: &Connection,
    now: i64,
    max_age: Duration,
    max_bytes: i64,
) -> rusqlite::Result<usize> {
    let before = now.saturating_sub(max_age.as_secs() as i64);
    let old = conn.execute("DELETE FROM mb_cache WHERE fetched_at < ?1", [before])?;
    let over = conn.execute(
        "DELETE FROM mb_cache WHERE request IN (
             SELECT request FROM (
                 SELECT request,
                        sum(length(CAST(request AS BLOB)) + length(CAST(response AS BLOB)))
                            OVER (ORDER BY fetched_at DESC, request) AS total
                 FROM mb_cache)
             WHERE total > ?1)",
        [max_bytes],
    )?;
    Ok(old + over)
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
        assert_eq!(
            prune(&conn, 350, Duration::from_secs(100), MAX_BYTES).unwrap(),
            1
        );
        assert_eq!(lookup(&conn, "https://a/2").unwrap(), None);
        assert!(lookup(&conn, "https://a/1").unwrap().is_some());
    }

    #[test]
    fn prunes_the_oldest_past_the_size_limit() {
        let conn = db::open_in_memory().unwrap();
        // Each row is 11 bytes of URL and 89 of body: 100 in all.
        for (i, fetched_at) in [(1, 500), (2, 400), (3, 300), (4, 200)] {
            store(
                &conn,
                &format!("https://a/{i}"),
                &"x".repeat(89),
                fetched_at,
            )
            .unwrap();
        }
        let day = Duration::from_secs(86400);
        assert_eq!(prune(&conn, 500, day, 400).unwrap(), 0);
        assert_eq!(prune(&conn, 500, day, 250).unwrap(), 2);
        let left: Vec<String> = conn
            .prepare("SELECT request FROM mb_cache ORDER BY request")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(left, ["https://a/1", "https://a/2"]);
    }
}
