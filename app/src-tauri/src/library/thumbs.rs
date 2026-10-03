//! Cover thumbnails (PLAN.md H17): an album's or a track's picture scaled
//! down for lists (`Size::List`) and for the album header, grids and Now
//! Playing (`Size::Header`), so the webview and the OS get small images and
//! the in-memory cache holds small ones. Full size is served only where a
//! picture is shown full size (the Now Playing view, "Choose cover").
//!
//! A thumbnail is a file in the image cache (`metadata::images`) named by
//! the SHA-256 of the picture it was made from, so two albums with the same
//! cover share it, and a picture that changes gets a new one. The
//! `art_thumbs` table (migration 012) says which picture an album shows and
//! where it came from, with a stamp of that source: a launch checks the
//! stamp (a `stat`, which never downloads a cloud placeholder, H12) and
//! serves the thumbnail without reading the picture again.
//!
//! Thumbnails are made only when the UI asks for one, never in the
//! background; `art::lookup` already passes over cloud placeholders.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader, Limits};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::access;
use super::art::{self, Art, ArtKey, Origin};
use super::commands::LibraryState;
use super::{track_path, Error};

/// JPEG quality of the thumbnails.
const QUALITY: u8 = 85;

/// Decoding stops past this much memory: a picture claiming to be huge
/// (a corrupt or hostile file) is served as it is instead.
const DECODE_LIMIT: u64 = 256 << 20;

/// The sizes the UI asks for, by their longer side in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Size {
    /// Queue, browser and search rows, the now-playing bar: up to 64 CSS
    /// pixels, twice that on a Retina screen.
    List,
    /// The album header and grids, the visualizer, the OS's Now Playing.
    Header,
}

impl Size {
    pub fn pixels(self) -> u32 {
        match self {
            Size::List => 128,
            Size::Header => 512,
        }
    }

    pub fn from_query(value: &str) -> Option<Size> {
        match value {
            "list" => Some(Size::List),
            "header" => Some(Size::Header),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Size::List => "list",
            Size::Header => "header",
        }
    }
}

/// `data` scaled to fit `pixels` square, as JPEG (PNG if it has
/// transparency); `None` if it is that small already or can't be decoded
/// (a GIF, a damaged file), when the picture is served as it is.
pub fn make(data: &[u8], pixels: u32) -> Option<Vec<u8>> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(DECODE_LIMIT);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    if image.width() <= pixels && image.height() <= pixels {
        return None;
    }
    let thumb = image.thumbnail(pixels, pixels);
    let mut out = Vec::new();
    if thumb.color().has_alpha() {
        thumb
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .ok()?;
    } else {
        let rgb = DynamicImage::ImageRgb8(thumb.to_rgb8());
        rgb.write_with_encoder(JpegEncoder::new_with_quality(&mut out, QUALITY))
            .ok()?;
    }
    Some(out)
}

/// The SHA-256 of `data`, in hex.
fn hash(data: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, data)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The image cache's key for a thumbnail. The cache names files by the
/// SHA-256 of their key, as it does downloads by their URL.
fn cache_key(size: Size, hash: &str) -> String {
    format!("thumbnail:{}:{hash}", size.name())
}

fn index_key(key: ArtKey) -> String {
    match key {
        ArtKey::Album(id) => format!("album-{id}"),
        ArtKey::Track(id) => format!("track-{id}"),
    }
}

/// What a source looks like now, to compare with the index's stamp; `None`
/// when it can't be told (gone, or its folder out of reach).
fn stamp(library: &LibraryState, origin: &Origin) -> Option<String> {
    match origin {
        Origin::File {
            folder_id,
            relative,
            ..
        } => {
            let folder = access::open_folder(&library.conn(), *folder_id).ok()?;
            let path = track_path(&folder.path, relative);
            let file = file_stamp(&path)?;
            let dir = file_stamp(path.parent()?)?;
            Some(format!("{file}|{dir}"))
        }
        Origin::Download { url } => library.images.contains(url).then(|| url.clone()),
    }
}

/// A file's or folder's size and modification time, from `stat`.
fn file_stamp(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!("{}:{}", metadata.len(), modified.as_nanos()))
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Row {
    origin: Origin,
    stamp: String,
    hash: String,
}

fn read_row(conn: &Connection, key: ArtKey) -> Result<Option<Row>, Error> {
    let row: Option<(String, String, String)> = conn
        .prepare_cached("SELECT origin, stamp, hash FROM art_thumbs WHERE key = ?1")?
        .query_row([index_key(key)], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    // An origin this version can't read is looked up again.
    Ok(row.and_then(|(origin, stamp, hash)| {
        Some(Row {
            origin: serde_json::from_str(&origin).ok()?,
            stamp,
            hash,
        })
    }))
}

fn write_row(conn: &Connection, key: ArtKey, row: &Row) -> Result<(), Error> {
    let origin = serde_json::to_string(&row.origin).expect("an origin serializes");
    conn.prepare_cached(
        "INSERT INTO art_thumbs (key, origin, stamp, hash) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (key) DO UPDATE SET origin = excluded.origin,
             stamp = excluded.stamp, hash = excluded.hash",
    )?
    .execute(params![index_key(key), origin, row.stamp, row.hash])?;
    Ok(())
}

/// Forgets which picture `key` shows: a cover was chosen or downloaded.
pub fn forget(conn: &Connection, key: ArtKey) -> Result<(), Error> {
    conn.execute("DELETE FROM art_thumbs WHERE key = ?1", [index_key(key)])?;
    Ok(())
}

/// Forgets every album's: the sources or their order changed.
pub fn forget_all(conn: &Connection) -> Result<(), Error> {
    conn.execute("DELETE FROM art_thumbs", [])?;
    Ok(())
}

/// `key`'s picture at `size`: a thumbnail, or the picture itself when it is
/// that small already or can't be scaled; `None` if it has none.
pub fn lookup(library: &LibraryState, key: ArtKey, size: Size) -> Result<Option<Arc<Art>>, Error> {
    // A file opened from outside the library (F5) has no row; its picture
    // is in memory already.
    if matches!(key, ArtKey::Track(id) if id < 0) {
        return art::lookup(library, key);
    }
    if let Some(cached) = library.art.get_sized(key, size) {
        return Ok(cached);
    }
    let found = from_index(library, key, size)?.map(Arc::new);
    let found = match found {
        Some(art) => Some(art),
        None => made(library, key, size)?,
    };
    library.art.insert_sized(key, size, found.clone());
    Ok(found)
}

/// The thumbnail the index names, if its source is as it was and the file
/// is still in the image cache.
fn from_index(library: &LibraryState, key: ArtKey, size: Size) -> Result<Option<Art>, Error> {
    let Some(row) = read_row(&library.conn(), key)? else {
        return Ok(None);
    };
    if stamp(library, &row.origin).as_deref() != Some(row.stamp.as_str()) {
        return Ok(None);
    }
    Ok(library
        .images
        .get(&cache_key(size, &row.hash))
        .map(|(mime_type, data)| Art {
            mime_type: mime_type.into(),
            data,
            source: row.origin.source(),
            chosen: false,
            origin: Some(row.origin),
        }))
}

/// Looks the picture up, and makes (or finds) its thumbnail.
fn made(library: &LibraryState, key: ArtKey, size: Size) -> Result<Option<Arc<Art>>, Error> {
    let Some(full) = art::lookup(library, key)? else {
        return Ok(None);
    };
    let hash = hash(&full.data);
    let cached = library.images.get(&cache_key(size, &hash));
    let data = match cached {
        Some((_, data)) => data,
        None => match make(&full.data, size.pixels()) {
            Some(data) => {
                if let Err(error) = library.images.store(&cache_key(size, &hash), &data) {
                    log::warn!("cannot keep a thumbnail: {error}");
                }
                data
            }
            // Small already, or not scalable: the picture itself.
            None => return Ok(Some(full)),
        },
    };
    if let Some(origin) = &full.origin {
        if let Some(stamp) = stamp(library, origin) {
            let row = Row {
                origin: origin.clone(),
                stamp,
                hash,
            };
            if let Err(error) = write_row(&library.conn(), key, &row) {
                log::warn!("cannot index a thumbnail: {error}");
            }
        }
    }
    Ok(Some(Arc::new(Art {
        mime_type: crate::metadata::images::image_type(&data)
            .unwrap_or("image/jpeg")
            .into(),
        data,
        source: full.source,
        chosen: full.chosen,
        origin: full.origin.clone(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{db, test_library};
    use crate::metadata::images::ImageCache;
    use crate::metadata::settings::{self, Kind, SourceId};
    use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

    fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        });
        let mut out = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Jpeg)
            .unwrap();
        out
    }

    fn dimensions(data: &[u8]) -> (u32, u32) {
        let image = image::load_from_memory(data).unwrap();
        (image.width(), image.height())
    }

    #[test]
    fn make_scales_large_pictures_and_leaves_small_ones() {
        let thumb = make(&jpeg(1000, 800), 128).unwrap();
        assert_eq!(
            crate::metadata::images::image_type(&thumb),
            Some("image/jpeg")
        );
        assert_eq!(dimensions(&thumb), (128, 102));
        assert_eq!(make(&jpeg(100, 100), 128), None);
        assert_eq!(make(b"not a picture", 128), None);

        // Transparency stays: PNG, not JPEG.
        let image = RgbaImage::from_pixel(600, 600, Rgba([10, 20, 30, 128]));
        let mut png = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .unwrap();
        let thumb = make(&png, 512).unwrap();
        assert_eq!(
            crate::metadata::images::image_type(&thumb),
            Some("image/png")
        );
        assert_eq!(dimensions(&thumb), (512, 512));
    }

    /// An album whose cover is a 1000 px `cover.jpg` in its folder, the
    /// folder source first; the image cache in a folder of its own.
    fn album(cover: &[u8]) -> (tempfile::TempDir, LibraryState, ArtKey, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap().join("Music");
        let album_dir = root.join("Artist/Album");
        std::fs::create_dir_all(&album_dir).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures/flac-44k.flac"),
            album_dir.join("01.flac"),
        )
        .unwrap();
        std::fs::write(album_dir.join("cover.jpg"), cover).unwrap();
        let library = test_library::Library::from_conn(db::open_in_memory().unwrap());
        let folder_id = library.add_folder(root.to_str().unwrap());
        library.add(
            folder_id,
            test_library::track("Artist/Album/01.flac")
                .artist("Artist")
                .album("Album"),
        );
        let album_id: i64 = library
            .conn
            .query_row("SELECT id FROM albums", [], |row| row.get(0))
            .unwrap();
        let mut state = LibraryState::for_tests(library.conn);
        state.images = Arc::new(ImageCache::new(dir.path().join("images")));
        {
            let conn = state.conn();
            let mut sources = settings::service_settings(&conn).unwrap();
            sources
                .order
                .insert(Kind::AlbumArt, vec![SourceId::Folder, SourceId::Embedded]);
            settings::save_service_settings(&conn, sources).unwrap();
        }
        (dir, state, ArtKey::Album(album_id), album_dir)
    }

    fn rows(library: &LibraryState) -> i64 {
        library
            .conn()
            .query_row("SELECT count(*) FROM art_thumbs", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn a_thumbnail_is_made_once_kept_on_disk_and_indexed() {
        let (_dir, library, album, album_dir) = album(&jpeg(1000, 1000));
        let list = lookup(&library, album, Size::List).unwrap().unwrap();
        assert_eq!(dimensions(&list.data), (128, 128));
        let header = lookup(&library, album, Size::Header).unwrap().unwrap();
        assert_eq!(dimensions(&header.data), (512, 512));
        let row = read_row(&library.conn(), album).unwrap().unwrap();
        assert!(
            matches!(&row.origin, Origin::File { relative, .. } if relative == "Artist/Album/cover.jpg")
        );
        assert!(library.images.contains(&cache_key(Size::List, &row.hash)));

        // After a launch (nothing in memory) the index serves it unread.
        library.art.clear();
        let found = from_index(&library, album, Size::List).unwrap().unwrap();
        assert_eq!(found.data, list.data);

        // A picture added to the folder changes its time: looked up again.
        std::fs::write(album_dir.join("back.jpg"), jpeg(10, 10)).unwrap();
        assert_eq!(from_index(&library, album, Size::List).unwrap(), None);
        library.art.clear();
        assert!(lookup(&library, album, Size::List).unwrap().is_some());
        assert!(from_index(&library, album, Size::List).unwrap().is_some());
    }

    #[test]
    fn a_small_picture_is_served_as_it_is_and_not_indexed() {
        let small = jpeg(100, 100);
        let (_dir, library, album, _) = album(&small);
        let art = lookup(&library, album, Size::Header).unwrap().unwrap();
        assert_eq!(art.data, small);
        assert_eq!(rows(&library), 0);
    }

    #[test]
    fn choosing_downloading_or_changing_the_sources_forgets_the_index() {
        let (_dir, library, album, _) = album(&jpeg(800, 800));
        lookup(&library, album, Size::List).unwrap();
        assert_eq!(rows(&library), 1);
        forget(&library.conn(), album).unwrap();
        assert_eq!(rows(&library), 0);
        library.art.clear();
        lookup(&library, album, Size::List).unwrap();
        forget_all(&library.conn()).unwrap();
        assert_eq!(rows(&library), 0);
    }

    #[test]
    fn an_album_without_a_picture_has_no_thumbnail() {
        let (_dir, library, album, album_dir) = album(b"");
        std::fs::remove_file(album_dir.join("cover.jpg")).unwrap();
        library.art.clear();
        assert_eq!(lookup(&library, album, Size::List).unwrap(), None);
    }
}
