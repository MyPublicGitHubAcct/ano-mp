//! Cover art for the UI. The webview loads it from the `anomp-art` URI scheme
//! (`anomp-art://localhost/album-<id>` or `/track-<id>`), so an `<img>` shows
//! it with no IPC round trip, and it is cached in memory so scrolling an
//! album list doesn't re-read files.
//!
//! `lookup` is the one place art comes from: the picture the user chose for
//! the album, else the first found among the album-art sources in the order
//! set in `metadata::settings`: embedded pictures, images in the album's
//! folder, and covers already downloaded from the Cover Art Archive.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::http::{header, Response, StatusCode};

use super::access::{self, OpenFolder};
use super::commands::LibraryState;
use super::{track_path, Error};
use crate::anomp;
use crate::metadata::settings::{self, Kind, SourceId};
use crate::metadata::{coverartarchive, folder_art};

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
    let art = find(library, key)?.map(Arc::new);
    library.art.insert(key, art.clone());
    Ok(art)
}

/// A file of the album or track: its library folder and path within it.
struct TrackFile {
    folder_id: i64,
    relative: String,
}

/// The library folders of an album's files, opened as they're needed and
/// held until the lookup ends. A folder that can't be opened (e.g. on an
/// unmounted drive) is skipped.
struct Folders<'a> {
    library: &'a LibraryState,
    open: HashMap<i64, Option<OpenFolder>>,
}

impl Folders<'_> {
    fn root(&mut self, folder_id: i64) -> Option<&Path> {
        let library = self.library;
        self.open
            .entry(folder_id)
            .or_insert_with(|| access::open_folder(&library.conn(), folder_id).ok())
            .as_ref()
            .map(|folder| folder.path.as_path())
    }
}

/// The user's choice of picture, if any, then the first picture from the
/// album-art sources in the configured order. Only local sources and
/// pictures already downloaded are used; this never goes online.
fn find(library: &LibraryState, key: ArtKey) -> Result<Option<Art>, Error> {
    let (settings, choice, files, covers) = {
        let conn = library.conn();
        let (choice, covers) = match key {
            ArtKey::Album(id) => (
                chosen_art(&conn, id)?,
                coverartarchive::cover_urls(&conn, id)?,
            ),
            ArtKey::Track(_) => (None, Vec::new()),
        };
        (
            settings::service_settings(&conn)?,
            choice,
            track_files(&conn, key)?,
            covers,
        )
    };
    let mut album = AlbumSources {
        files,
        covers,
        folders: Folders {
            library,
            open: HashMap::new(),
        },
    };
    if let Some((source, reference)) = choice {
        // A choice whose picture has gone falls back to the order.
        if let Some(art) = from_source(source, reference.as_deref(), &mut album) {
            return Ok(Some(art));
        }
    }
    Ok(settings
        .sources_for(Kind::AlbumArt)
        .into_iter()
        .find_map(|source| from_source(source, None, &mut album)))
}

/// Where the sources look for an album's (or a track's) picture.
struct AlbumSources<'a> {
    files: Vec<TrackFile>,
    /// Cover Art Archive URLs that may hold its cover, best first.
    covers: Vec<String>,
    folders: Folders<'a>,
}

/// The picture the user chose for album `album_id`: its source and reference
/// (see migration 003's `album_art`).
fn chosen_art(
    conn: &Connection,
    album_id: i64,
) -> Result<Option<(SourceId, Option<String>)>, Error> {
    let row: Option<(String, Option<String>)> = conn
        .prepare_cached("SELECT source, reference FROM album_art WHERE album_id = ?1")?
        .query_row([album_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    // A source this version doesn't know is ignored.
    Ok(row.and_then(|(source, reference)| Some((SourceId::from_str(&source)?, reference))))
}

/// The track's file, or the first few of an album's in track order.
fn track_files(conn: &Connection, key: ArtKey) -> Result<Vec<TrackFile>, Error> {
    let (sql, id) = match key {
        ArtKey::Album(id) => (
            "SELECT folder_id, relative_path FROM tracks WHERE album_id = ?1
             ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST, relative_path
             LIMIT ?2",
            id,
        ),
        ArtKey::Track(id) => (
            "SELECT folder_id, relative_path FROM tracks WHERE id = ?1 LIMIT ?2",
            id,
        ),
    };
    let mut statement = conn.prepare_cached(sql)?;
    let rows = statement.query_map(params![id, ALBUM_FILES_TRIED], |row| {
        Ok(TrackFile {
            folder_id: row.get(0)?,
            relative: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// A picture from `source`: the one `reference` names, or the best it has.
fn from_source(source: SourceId, reference: Option<&str>, album: &mut AlbumSources) -> Option<Art> {
    let AlbumSources {
        files,
        covers,
        folders,
    } = album;
    match source {
        SourceId::Embedded => files.iter().find_map(|file| {
            let path = track_path(folders.root(file.folder_id)?, &file.relative);
            let picture = anomp::read_tags(&path, true).ok()?.picture?;
            Some(Art {
                mime_type: picture
                    .mime_type
                    .unwrap_or_else(|| "application/octet-stream".into()),
                data: picture.data,
            })
        }),
        SourceId::Folder => {
            let mut tried = HashSet::new();
            files.iter().find_map(|file| {
                let root = folders.root(file.folder_id)?.to_path_buf();
                let candidates = match reference {
                    Some(reference) => vec![track_path(&root, reference)],
                    None => folder_art::folders_for(&root, &file.relative)
                        .into_iter()
                        .filter(|dir| tried.insert(dir.clone()))
                        .filter_map(|dir| folder_art::find(&dir))
                        .collect(),
                };
                candidates.iter().find_map(|path| {
                    let (mime_type, data) = folder_art::read(path)?;
                    Some(Art {
                        mime_type: mime_type.into(),
                        data,
                    })
                })
            })
        }
        // Only pictures already downloaded; the metadata worker fetches them.
        SourceId::CoverArtArchive => {
            let images = &folders.library.images;
            let (mime_type, data) = match reference {
                Some(url) => images.get(url),
                None => covers.iter().find_map(|url| images.get(url)),
            }?;
            Some(Art {
                mime_type: mime_type.into(),
                data,
            })
        }
        // Supplies no art.
        SourceId::MusicBrainz => None,
    }
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
    use crate::library::{db, test_library};

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

    /// A library folder holding one album, "Artist/Album/CD1/01.flac" with
    /// an embedded PNG, and "Artist/Album/cover.jpg" above its disc folder.
    fn album_with_two_pictures() -> (tempfile::TempDir, LibraryState, i64) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let disc = root.join("Artist/Album/CD1");
        std::fs::create_dir_all(&disc).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../core/tests/fixtures/tagged-vorbis.flac"),
            disc.join("01.flac"),
        )
        .unwrap();
        std::fs::write(root.join("Artist/Album/cover.jpg"), b"jpeg bytes").unwrap();

        let library = test_library::Library::from_conn(db::open_in_memory().unwrap());
        let folder_id = library.add_folder(root.to_str().unwrap());
        library.add(
            folder_id,
            test_library::track("Artist/Album/CD1/01.flac")
                .artist("Artist")
                .album("Album"),
        );
        let album_id = library
            .conn
            .query_row("SELECT id FROM albums", [], |row| row.get(0))
            .unwrap();
        (dir, LibraryState::for_tests(library.conn), album_id)
    }

    fn mime(library: &LibraryState, key: ArtKey) -> Option<String> {
        library.art.clear();
        lookup(library, key)
            .unwrap()
            .map(|art| art.mime_type.clone())
    }

    #[test]
    fn follows_the_source_order_and_the_users_choice() {
        let (_dir, library, album) = album_with_two_pictures();
        let album = ArtKey::Album(album);
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));

        let set_order = |order: Vec<SourceId>, enabled: bool| {
            let conn = library.conn();
            let mut settings = settings::service_settings(&conn).unwrap();
            settings.order.insert(Kind::AlbumArt, order);
            for source in &mut settings.sources {
                source.enabled = enabled;
            }
            settings::save_service_settings(&conn, settings).unwrap();
        };
        set_order(vec![SourceId::Folder, SourceId::Embedded], true);
        let art = {
            library.art.clear();
            lookup(&library, album).unwrap().unwrap()
        };
        assert_eq!(art.mime_type, "image/jpeg");
        assert_eq!(art.data, b"jpeg bytes");

        // The user's choice beats the order.
        let choose = |source: &str, reference: Option<&str>| {
            library
                .conn()
                .execute(
                    "INSERT OR REPLACE INTO album_art (album_id, source, reference)
                     VALUES (?1, ?2, ?3)",
                    params![1, source, reference],
                )
                .unwrap();
        };
        choose("embedded", None);
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));
        // A chosen picture that has gone, or from an unknown source, falls
        // back to the order.
        choose("folder", Some("Artist/Album/gone.jpg"));
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
        choose("lastfm", Some("https://example.com/a.jpg"));
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
        choose("folder", Some("Artist/Album/cover.jpg"));
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));

        // With every source off, only the choice is left.
        set_order(vec![], false);
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
        library.conn().execute("DELETE FROM album_art", []).unwrap();
        assert_eq!(mime(&library, album), None);
    }

    #[test]
    fn serves_downloaded_covers_from_the_image_cache() {
        use crate::metadata::coverartarchive::{release_front_url, release_group_front_url};
        use crate::metadata::images::{testing::JPEG, ImageCache};
        use crate::metadata::musicbrainz::fixtures::{RELEASES, RELEASE_GROUP};

        let (_dir, mut library, album_id) = album_with_two_pictures();
        let images = tempfile::tempdir().unwrap();
        library.images = ImageCache::new(images.path().to_path_buf());
        let album = ArtKey::Album(album_id);
        let (release_id, details) = RELEASES[0];
        let link = |status: &str| {
            let details = crate::metadata::musicbrainz::parse_release(details).unwrap();
            library
                .conn()
                .execute(
                    "INSERT OR REPLACE INTO album_links
                         (album_id, source, status, external_id, score, chosen_by, details,
                          checked_at)
                     VALUES (?1, 'musicbrainz', ?2, ?3, 1.0, 'auto', ?4, 0)",
                    params![
                        album_id,
                        status,
                        release_id,
                        serde_json::to_string(&details).unwrap()
                    ],
                )
                .unwrap();
        };
        link("matched");
        {
            let conn = library.conn();
            let mut settings = settings::service_settings(&conn).unwrap();
            settings.order.insert(
                Kind::AlbumArt,
                vec![SourceId::CoverArtArchive, SourceId::Embedded],
            );
            settings::save_service_settings(&conn, settings).unwrap();
        }
        // Nothing downloaded yet: the next source.
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));

        // The release group's cover, then the release's, which is better.
        library
            .images
            .store(
                &release_group_front_url(RELEASE_GROUP),
                &JPEG[..JPEG.len() - 1],
            )
            .unwrap();
        library.art.clear();
        assert_eq!(
            lookup(&library, album).unwrap().unwrap().data.len(),
            JPEG.len() - 1
        );
        library
            .images
            .store(&release_front_url(release_id), JPEG)
            .unwrap();
        library.art.clear();
        let art = lookup(&library, album).unwrap().unwrap();
        assert_eq!(
            (art.mime_type.as_str(), &art.data[..]),
            ("image/jpeg", JPEG)
        );

        // Not for a match that awaits review.
        link("review");
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));

        // The user's pick of an archive image, by URL, once it's downloaded.
        let chosen = "https://coverartarchive.org/release/x/1-500.jpg";
        library
            .conn()
            .execute(
                "INSERT INTO album_art (album_id, source, reference)
                 VALUES (?1, 'cover-art-archive', ?2)",
                params![album_id, chosen],
            )
            .unwrap();
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));
        library.images.store(chosen, JPEG).unwrap();
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
    }

    #[test]
    fn a_track_uses_its_own_file_and_folder() {
        let (_dir, library, _album) = album_with_two_pictures();
        assert_eq!(
            mime(&library, ArtKey::Track(1)).as_deref(),
            Some("image/png")
        );
        assert_eq!(mime(&library, ArtKey::Track(99)), None);
    }
}
