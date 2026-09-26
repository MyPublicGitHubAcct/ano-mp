//! Cover art for the UI. The webview loads it from the `anomp-art` URI scheme
//! (`anomp-art://localhost/album-<id>` or `/track-<id>`), so an `<img>` shows
//! it with no IPC round trip, and it is cached in memory so scrolling an
//! album list doesn't re-read files.
//!
//! `lookup` is the one place art comes from: the picture the user chose for
//! the album, else the first found among the album-art sources in the order
//! set in `metadata::settings`: embedded pictures, images in the album's
//! folder, and covers already downloaded from the Cover Art Archive.
//!
//! The "Choose cover" dialog lists an album's pictures from each source
//! (`local_candidates` here, the archive's in `coverartarchive`), shows them
//! through the same scheme (`anomp-art://localhost/album-<id>/<source>?ref=…`,
//! see `Target`), and stores the pick (`choose`).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
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

/// Images in an album's folders offered in the "Choose cover" dialog.
const FOLDER_IMAGES_LISTED: usize = 24;

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
    /// Where it came from.
    pub source: SourceId,
    /// Whether it's the picture the user chose for the album.
    pub chosen: bool,
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

    /// Forgets `key`'s art, e.g. after a cover was downloaded for it.
    pub fn remove(&self, key: ArtKey) {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((art, _)) = cache.entries.remove(&key) {
            cache.bytes -= art.map_or(0, |art| art.data.len());
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
/// pictures already downloaded are used; this never goes online. Downloaded
/// pictures are shown while the online switch is off too
/// (`ServiceSettings::sources_shown`).
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
            return Ok(Some(Art {
                chosen: true,
                ..art
            }));
        }
    }
    Ok(settings
        .sources_shown(Kind::AlbumArt)
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
pub fn chosen_art(
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
                source,
                chosen: false,
            })
        }),
        SourceId::Folder => {
            let mut tried = HashSet::new();
            files.iter().find_map(|file| {
                let root = folders.root(file.folder_id)?.to_path_buf();
                let candidates = match reference {
                    Some(reference) if folder_art::is_relative_path(reference) => {
                        vec![track_path(&root, reference)]
                    }
                    Some(_) => Vec::new(),
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
                        source,
                        chosen: false,
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
                source,
                chosen: false,
            })
        }
        // Not album-art sources.
        SourceId::MusicBrainz | SourceId::Wikipedia => None,
    }
}

/// A picture the user can choose as an album's cover.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverCandidate {
    pub source: SourceId,
    /// What `album_art.reference` stores for it; `None` for the embedded
    /// picture.
    pub reference: Option<String>,
    /// What it is: a file name, or the archive's types ("Front, Booklet").
    pub label: String,
    /// More about it: the folder it's in, the archive's comment.
    pub detail: Option<String>,
    /// The reference to preview it by (`Target::Candidate`): for the
    /// archive a smaller picture, which must be downloaded first
    /// (`metadata_fetch_image`); `None` for the embedded picture.
    pub preview: Option<String>,
}

/// The pictures `source`, a local source, has for album `album_id`: the
/// embedded picture, or every image in the album's folders (as
/// `folder_art::folders_for` finds them).
pub fn local_candidates(
    library: &LibraryState,
    album_id: i64,
    source: SourceId,
) -> Result<Vec<CoverCandidate>, Error> {
    match source {
        SourceId::Embedded => {
            let files = track_files(&library.conn(), ArtKey::Album(album_id))?;
            let mut album = AlbumSources {
                files,
                covers: Vec::new(),
                folders: Folders {
                    library,
                    open: HashMap::new(),
                },
            };
            Ok(from_source(source, None, &mut album)
                .map(|_| CoverCandidate {
                    source,
                    reference: None,
                    label: "Embedded in the files".into(),
                    detail: None,
                    preview: None,
                })
                .into_iter()
                .collect())
        }
        SourceId::Folder => folder_candidates(library, album_id),
        SourceId::MusicBrainz | SourceId::CoverArtArchive | SourceId::Wikipedia => Ok(Vec::new()),
    }
}

/// The images in album `album_id`'s folders, those in its first track's
/// folders first, at most `FOLDER_IMAGES_LISTED`.
fn folder_candidates(library: &LibraryState, album_id: i64) -> Result<Vec<CoverCandidate>, Error> {
    let dirs: Vec<(i64, String)> = {
        let conn = library.conn();
        let mut statement = conn.prepare_cached(
            "SELECT folder_id, relative_path FROM tracks WHERE album_id = ?1
             ORDER BY IFNULL(disc_number, 1), track_number NULLS LAST, relative_path",
        )?;
        let rows = statement.query_map([album_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut seen = HashSet::new();
        let mut dirs = Vec::new();
        for row in rows {
            let (folder_id, relative) = row?;
            for dir in folder_art::relative_folders(&relative) {
                if seen.insert((folder_id, dir.clone())) {
                    dirs.push((folder_id, dir));
                }
            }
        }
        dirs
    };
    let mut folders = Folders {
        library,
        open: HashMap::new(),
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for (folder_id, dir) in dirs {
        let Some(root) = folders.root(folder_id) else {
            continue;
        };
        let path = match dir.as_str() {
            "" => root.to_path_buf(),
            dir => track_path(root, dir),
        };
        for name in folder_art::images(&path) {
            let reference = match dir.as_str() {
                "" => name.clone(),
                dir => format!("{dir}/{name}"),
            };
            if !seen.insert(reference.clone()) {
                continue;
            }
            candidates.push(CoverCandidate {
                source: SourceId::Folder,
                reference: Some(reference.clone()),
                label: name,
                detail: (!dir.is_empty()).then(|| dir.clone()),
                preview: Some(reference),
            });
            if candidates.len() == FOLDER_IMAGES_LISTED {
                return Ok(candidates);
            }
        }
    }
    Ok(candidates)
}

/// Makes picture `reference` of `source` album `album_id`'s cover, ahead of
/// the source order, or with `None` goes back to that order. A folder
/// picture must be one of the album's images (`local_candidates`), and an
/// archive picture one of the archive's URLs, shown once it's downloaded.
pub fn choose(
    library: &LibraryState,
    album_id: i64,
    choice: Option<(SourceId, Option<&str>)>,
) -> Result<(), Error> {
    let exists: Option<i64> = library
        .conn()
        .query_row("SELECT id FROM albums WHERE id = ?1", [album_id], |row| {
            row.get(0)
        })
        .optional()?;
    if exists.is_none() {
        return Err(Error::Invalid(
            "The album is no longer in the library".into(),
        ));
    }
    let invalid = |message: &str| Err(Error::Invalid(message.into()));
    match choice {
        None => {
            library
                .conn()
                .execute("DELETE FROM album_art WHERE album_id = ?1", [album_id])?;
        }
        Some((source, reference)) => {
            match (source, reference) {
                (SourceId::Embedded, None) => {}
                (SourceId::Folder, Some(reference)) => {
                    let listed = folder_candidates(library, album_id)?
                        .iter()
                        .any(|candidate| candidate.reference.as_deref() == Some(reference));
                    if !listed {
                        return invalid("That picture is not in the album's folders");
                    }
                }
                (SourceId::CoverArtArchive, Some(url)) if coverartarchive::is_archive_url(url) => {}
                (SourceId::Embedded | SourceId::Folder | SourceId::CoverArtArchive, _) => {
                    return invalid("Not a picture that source has");
                }
                (SourceId::MusicBrainz | SourceId::Wikipedia, _) => {
                    return Err(Error::Invalid(format!(
                        "{} doesn't supply album art",
                        source.info().name
                    )));
                }
            }
            library.conn().execute(
                "INSERT INTO album_art (album_id, source, reference) VALUES (?1, ?2, ?3)
                 ON CONFLICT (album_id) DO UPDATE SET
                     source = excluded.source, reference = excluded.reference",
                params![album_id, source.as_str(), reference],
            )?;
        }
    }
    library.art.remove(ArtKey::Album(album_id));
    Ok(())
}

/// What an `anomp-art` URI asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// "/album-12" or "/track-7".
    Art(ArtKey),
    /// "/album-12/folder?ref=Scans%2Fback.jpg": a picture the "Choose
    /// cover" dialog offers, by its source and `CoverCandidate::preview`.
    Candidate {
        album_id: i64,
        source: SourceId,
        reference: Option<String>,
    },
}

impl Target {
    /// Parses a URI's path and query.
    pub fn parse(path: &str, query: Option<&str>) -> Option<Target> {
        if let Some(key) = ArtKey::from_path(path) {
            return Some(Target::Art(key));
        }
        let (album, source) = path.trim_start_matches('/').split_once('/')?;
        let Some(ArtKey::Album(album_id)) = ArtKey::from_path(album) else {
            return None;
        };
        let source = SourceId::from_str(source)?;
        let reference = match query {
            Some(query) => tauri::Url::parse(&format!("{SCHEME}://localhost/?{query}"))
                .ok()?
                .query_pairs()
                .find(|(name, _)| name == "ref")
                .map(|(_, value)| value.into_owned()),
            None => None,
        };
        Some(Target::Candidate {
            album_id,
            source,
            reference,
        })
    }
}

/// The candidate picture `reference` of `source` for album `album_id`, as
/// `from_source` finds it: never online, and a folder picture only by a
/// path inside the library folder.
fn candidate(
    library: &LibraryState,
    album_id: i64,
    source: SourceId,
    reference: Option<&str>,
) -> Result<Option<Art>, Error> {
    let files = track_files(&library.conn(), ArtKey::Album(album_id))?;
    let mut album = AlbumSources {
        files,
        covers: Vec::new(),
        folders: Folders {
            library,
            open: HashMap::new(),
        },
    };
    Ok(from_source(source, reference, &mut album))
}

/// The response to a request for `anomp-art://localhost<path>?<query>`.
pub fn respond(
    library: Option<&LibraryState>,
    path: &str,
    query: Option<&str>,
) -> Response<Vec<u8>> {
    let status = |status: StatusCode| {
        Response::builder()
            .status(status)
            .header(header::CACHE_CONTROL, "no-store")
            .body(Vec::new())
            .expect("a valid response")
    };
    let (Some(library), Some(target)) = (library, Target::parse(path, query)) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let (found, cache_control) = match target {
        // The UI adds the library's scan count and the album's count of
        // `metadata-changed` events to the URL, so a rescan or a download
        // that changed the art gets a new one.
        Target::Art(key) => (lookup(library, key), "max-age=86400"),
        // Only while the dialog is open; the file may change.
        Target::Candidate {
            album_id,
            source,
            reference,
        } => (
            candidate(library, album_id, source, reference.as_deref()).map(|art| art.map(Arc::new)),
            "no-store",
        ),
    };
    match found {
        Ok(Some(art)) => Response::builder()
            .header(header::CONTENT_TYPE, art.mime_type.as_str())
            .header(header::CACHE_CONTROL, cache_control)
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
                source: SourceId::Embedded,
                chosen: false,
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
        cache.remove(ArtKey::Album(1));
        assert!(cache.get(ArtKey::Album(1)).is_none());
        assert!(cache.get(ArtKey::Album(4)).is_some(), "only that key");
        assert_eq!(cache.0.lock().unwrap().bytes, 1);
        cache.clear();
        assert!(cache.get(ArtKey::Album(4)).is_none());
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
        library.images = Arc::new(ImageCache::new(images.path().to_path_buf()));
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

        // Still shown with the online switch off, but not with the archive
        // turned off.
        let set = |change: &dyn Fn(&mut settings::ServiceSettings)| {
            let conn = library.conn();
            let mut settings = settings::service_settings(&conn).unwrap();
            change(&mut settings);
            settings::save_service_settings(&conn, settings).unwrap();
        };
        set(&|settings| settings.online = false);
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
        library.conn().execute("DELETE FROM album_art", []).unwrap();
        link("matched");
        assert_eq!(mime(&library, album).as_deref(), Some("image/jpeg"));
        set(&|settings| settings.sources[3].enabled = false);
        assert_eq!(mime(&library, album).as_deref(), Some("image/png"));
    }

    #[test]
    fn parses_targets() {
        assert_eq!(
            Target::parse("/album-3", Some("g=1.0")),
            Some(Target::Art(ArtKey::Album(3)))
        );
        assert_eq!(
            Target::parse("/album-3/folder", Some("ref=Scans%2Fback%20cover.jpg&g=2")),
            Some(Target::Candidate {
                album_id: 3,
                source: SourceId::Folder,
                reference: Some("Scans/back cover.jpg".into()),
            })
        );
        assert_eq!(
            Target::parse("/album-3/embedded", None),
            Some(Target::Candidate {
                album_id: 3,
                source: SourceId::Embedded,
                reference: None,
            })
        );
        for (path, query) in [
            ("/track-3/folder", None),
            ("/album-3/lastfm", None),
            ("/album-x/folder", None),
            ("/album-3/folder/x", None),
        ] {
            assert_eq!(Target::parse(path, query), None, "{path}");
        }
    }

    #[test]
    fn lists_serves_and_chooses_candidates() {
        let (dir, library, album_id) = album_with_two_pictures();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("Artist/Album/CD1/back.png"), b"png bytes").unwrap();
        std::fs::write(root.join("Artist/Album/notes.txt"), b"text").unwrap();

        let embedded = local_candidates(&library, album_id, SourceId::Embedded).unwrap();
        assert_eq!(embedded.len(), 1);
        assert_eq!(embedded[0].reference, None);
        let folder = local_candidates(&library, album_id, SourceId::Folder).unwrap();
        let references: Vec<Option<&str>> = folder.iter().map(|c| c.reference.as_deref()).collect();
        assert_eq!(
            references,
            [
                Some("Artist/Album/CD1/back.png"),
                Some("Artist/Album/cover.jpg")
            ]
        );
        assert_eq!(folder[1].label, "cover.jpg");
        assert_eq!(folder[1].detail.as_deref(), Some("Artist/Album"));
        assert!(
            local_candidates(&library, album_id, SourceId::CoverArtArchive)
                .unwrap()
                .is_empty()
        );

        // Previews, through the scheme's handler.
        let get = |path: &str, query: Option<&str>| {
            let response = respond(Some(&library), path, query);
            (response.status(), response.into_body())
        };
        let album = format!("/album-{album_id}");
        assert_eq!(
            get(
                &format!("{album}/folder"),
                Some("ref=Artist%2FAlbum%2FCD1%2Fback.png")
            ),
            (StatusCode::OK, b"png bytes".to_vec())
        );
        assert_eq!(get(&format!("{album}/embedded"), None).0, StatusCode::OK);
        for escape in ["..%2F..%2Fetc%2Fhosts.png", "%2Fetc%2Fcover.jpg"] {
            assert_eq!(
                get(&format!("{album}/folder"), Some(&format!("ref={escape}"))).0,
                StatusCode::NOT_FOUND,
                "{escape}"
            );
        }

        // What's shown, and where from.
        let shown = |library: &LibraryState| {
            let art = lookup(library, ArtKey::Album(album_id)).unwrap().unwrap();
            (art.source, art.chosen, art.data.clone())
        };
        assert_eq!(shown(&library).0, SourceId::Embedded);
        choose(
            &library,
            album_id,
            Some((SourceId::Folder, Some("Artist/Album/CD1/back.png"))),
        )
        .unwrap();
        assert_eq!(
            shown(&library),
            (SourceId::Folder, true, b"png bytes".to_vec())
        );
        for bad in [
            (SourceId::Folder, Some("Artist/Album/notes.txt")),
            (SourceId::Folder, Some("../elsewhere/cover.jpg")),
            (SourceId::Folder, None),
            (SourceId::Embedded, Some("x")),
            (
                SourceId::CoverArtArchive,
                Some("https://example.com/cover.jpg"),
            ),
            (SourceId::MusicBrainz, None),
        ] {
            assert!(choose(&library, album_id, Some(bad)).is_err(), "{bad:?}");
        }
        assert!(choose(&library, 99, None).is_err(), "no such album");
        assert_eq!(shown(&library).0, SourceId::Folder, "unchanged");
        choose(&library, album_id, None).unwrap();
        assert_eq!(shown(&library).1, false);
        assert_eq!(chosen_art(&library.conn(), album_id).unwrap(), None);
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
