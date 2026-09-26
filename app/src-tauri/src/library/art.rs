//! Cover art for the UI. The webview loads it from the `anomp-art` URI scheme
//! (`anomp-art://localhost/album-<id>` or `/track-<id>`), so an `<img>` shows
//! it with no IPC round trip, and it is cached in memory so scrolling an
//! album list doesn't re-read files.
//!
//! `lookup` is the one place art comes from. Its only source for now is the
//! picture embedded in the files; Phase 4 adds the Cover Art Archive.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::params;
use tauri::http::{header, Response, StatusCode};

use super::commands::LibraryState;
use super::{track_path, Error};
use crate::anomp;

/// The URI scheme the webview loads art from.
pub const SCHEME: &str = "anomp-art";

/// Cached art is dropped, least recently used first, beyond this many bytes.
const CACHE_BUDGET: usize = 64 << 20;

/// Files of an album tried for an embedded picture, in track order.
const ALBUM_FILES_TRIED: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtKey {
    Album(i64),
    /// A track's own art; for tracks without an album.
    Track(i64),
}

impl ArtKey {
    /// Parses a URI path: "/album-12" or "/track-7".
    pub fn from_path(path: &str) -> Option<ArtKey> {
        let (kind, id) = path.trim_start_matches('/').split_once('-')?;
        let id = id.parse().ok()?;
        match kind {
            "album" => Some(ArtKey::Album(id)),
            "track" => Some(ArtKey::Track(id)),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Art {
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Art looked up so far, including what has none.
#[derive(Default)]
pub struct ArtCache(Mutex<CacheEntries>);

#[derive(Default)]
struct CacheEntries {
    entries: HashMap<ArtKey, (Option<Arc<Art>>, u64)>,
    bytes: usize,
    clock: u64,
}

impl ArtCache {
    fn get(&self, key: ArtKey) -> Option<Option<Arc<Art>>> {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.clock += 1;
        let clock = cache.clock;
        cache.entries.get_mut(&key).map(|(art, used)| {
            *used = clock;
            art.clone()
        })
    }

    fn insert(&self, key: ArtKey, art: Option<Arc<Art>>) {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.clock += 1;
        let clock = cache.clock;
        cache.bytes += art.as_ref().map_or(0, |art| art.data.len());
        if let Some((old, _)) = cache.entries.insert(key, (art, clock)) {
            cache.bytes -= old.map_or(0, |art| art.data.len());
        }
        while cache.bytes > CACHE_BUDGET && cache.entries.len() > 1 {
            let oldest = cache
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| *key);
            if let Some((art, _)) = oldest.and_then(|key| cache.entries.remove(&key)) {
                cache.bytes -= art.map_or(0, |art| art.data.len());
            }
        }
    }

    /// Forgets everything, e.g. after a scan may have changed the files.
    pub fn clear(&self) {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        cache.entries.clear();
        cache.bytes = 0;
    }
}

/// The art for `key`, or `None` if there is none.
pub fn lookup(library: &LibraryState, key: ArtKey) -> Result<Option<Arc<Art>>, Error> {
    if let Some(cached) = library.art.get(key) {
        return Ok(cached);
    }
    let art = embedded(library, key)?.map(Arc::new);
    library.art.insert(key, art.clone());
    Ok(art)
}

/// The picture embedded in the track, or in the first of an album's tracks
/// that has one.
fn embedded(library: &LibraryState, key: ArtKey) -> Result<Option<Art>, Error> {
    let files: Vec<PathBuf> = {
        let conn = library.conn();
        let (sql, id) = match key {
            ArtKey::Album(id) => (
                "SELECT f.path, t.relative_path FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.album_id = ?1
                 ORDER BY IFNULL(t.disc_number, 1), t.track_number NULLS LAST, t.relative_path
                 LIMIT ?2",
                id,
            ),
            ArtKey::Track(id) => (
                "SELECT f.path, t.relative_path FROM tracks t JOIN folders f ON f.id = t.folder_id
                 WHERE t.id = ?1 LIMIT ?2",
                id,
            ),
        };
        let mut statement = conn.prepare_cached(sql)?;
        let rows = statement.query_map(params![id, ALBUM_FILES_TRIED], |row| {
            Ok(track_path(
                PathBuf::from(row.get::<_, String>(0)?).as_path(),
                &row.get::<_, String>(1)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    for path in files {
        // Under the sandbox the file is readable only while its folder is open.
        let _folder = library.open_folder_of(&path).map_err(Error::Invalid)?;
        if let Ok(tags) = anomp::read_tags(&path, true) {
            if let Some(picture) = tags.picture {
                return Ok(Some(Art {
                    mime_type: picture
                        .mime_type
                        .unwrap_or_else(|| "application/octet-stream".into()),
                    data: picture.data,
                }));
            }
        }
    }
    Ok(None)
}

/// The response to a request for `anomp-art://localhost<path>`.
pub fn respond(library: Option<&LibraryState>, path: &str) -> Response<Vec<u8>> {
    let status = |status: StatusCode| {
        Response::builder()
            .status(status)
            .header(header::CACHE_CONTROL, "no-store")
            .body(Vec::new())
            .expect("a valid response")
    };
    let (Some(library), Some(key)) = (library, ArtKey::from_path(path)) else {
        return status(StatusCode::BAD_REQUEST);
    };
    match lookup(library, key) {
        Ok(Some(art)) => Response::builder()
            .header(header::CONTENT_TYPE, art.mime_type.as_str())
            // The UI adds the library's scan count to the URL, so a rescan
            // that changed the art gets a new one.
            .header(header::CACHE_CONTROL, "max-age=86400")
            .body(art.data.clone())
            .expect("a valid response"),
        Ok(None) => status(StatusCode::NOT_FOUND),
        Err(error) => {
            eprintln!("[art] {path}: {error}");
            status(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keys() {
        assert_eq!(ArtKey::from_path("/album-12"), Some(ArtKey::Album(12)));
        assert_eq!(ArtKey::from_path("track-7"), Some(ArtKey::Track(7)));
        for bad in [
            "/album",
            "/album-",
            "/album-x",
            "/genre-1",
            "",
            "/album-1/2",
        ] {
            assert_eq!(ArtKey::from_path(bad), None, "{bad}");
        }
    }

    #[test]
    fn cache_drops_the_least_recently_used_beyond_its_budget() {
        let cache = ArtCache::default();
        let art = |size: usize| {
            Some(Arc::new(Art {
                mime_type: "image/png".into(),
                data: vec![0; size],
            }))
        };
        cache.insert(ArtKey::Album(1), art(CACHE_BUDGET / 2));
        cache.insert(ArtKey::Album(3), art(CACHE_BUDGET / 2));
        assert!(cache.get(ArtKey::Album(1)).is_some()); // Now the most recent.
        cache.insert(ArtKey::Album(4), art(1));
        assert!(cache.get(ArtKey::Album(3)).is_none(), "evicted");
        assert!(cache.get(ArtKey::Album(1)).unwrap().is_some());
        cache.insert(ArtKey::Album(2), None);
        assert_eq!(
            cache.get(ArtKey::Album(2)),
            Some(None),
            "no art is remembered"
        );
        cache.clear();
        assert!(cache.get(ArtKey::Album(1)).is_none());
    }
}
