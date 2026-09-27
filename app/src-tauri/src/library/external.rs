//! Files opened from outside the library (PLAN.md F5): "Open With" in the
//! Finder, the Dock icon, or a drop on the window. Such a file plays
//! without being added. The queue knows it by a negative track id, good for
//! this session only: the macOS sandbox lets the app open it until it
//! quits, so the saved queue leaves it out.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension};

use super::art::Art;
use super::Error;
use crate::anomp::{self, ReplayGain};
use crate::metadata::settings::SourceId;
use crate::queue::model::TrackInfo;

/// A file known by a negative id.
#[derive(Debug, Clone)]
pub struct ExternalFile {
    pub path: PathBuf,
    pub replay_gain: ReplayGain,
    pub sample_rate: u32,
}

static FILES: Mutex<Option<HashMap<i64, ExternalFile>>> = Mutex::new(None);
static NEXT_ID: AtomicI64 = AtomicI64::new(-1);

fn files() -> MutexGuard<'static, Option<HashMap<i64, ExternalFile>>> {
    FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reads the file at `path` and gives it an id; any thread. Fails for a
/// file the core can't read.
pub fn register(path: &Path) -> Result<TrackInfo, String> {
    let tags = anomp::read_tags(path, false)?;
    let id = NEXT_ID.fetch_sub(1, Ordering::Relaxed);
    let title = tags.title.clone().unwrap_or_else(|| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    files().get_or_insert_with(HashMap::new).insert(
        id,
        ExternalFile {
            path: path.to_path_buf(),
            replay_gain: tags.replay_gain,
            sample_rate: tags.sample_rate,
        },
    );
    Ok(TrackInfo {
        track_id: id,
        title,
        artist: tags.artist,
        album: tags.album,
        duration: tags.duration,
        external: true,
        ..TrackInfo::default()
    })
}

/// The file behind an external track id.
pub fn file(track_id: i64) -> Option<ExternalFile> {
    files().as_ref()?.get(&track_id).cloned()
}

/// The embedded picture of an external track, for Now Playing and the UI.
pub fn art(track_id: i64) -> Option<Arc<Art>> {
    let file = file(track_id)?;
    let picture = anomp::read_tags(&file.path, true).ok()?.picture?;
    Some(Arc::new(Art {
        mime_type: picture.mime_type.unwrap_or_default(),
        data: picture.data,
        source: SourceId::Embedded,
        chosen: false,
    }))
}

/// The library track of the file at `path` (its first part, for a file
/// split into parts), if it is in a library folder and scanned.
pub fn library_track(conn: &Connection, path: &Path) -> Result<Option<i64>, Error> {
    let folders: Vec<(i64, String)> = conn
        .prepare_cached("SELECT id, path FROM folders")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (folder_id, folder) in folders {
        let Ok(rest) = path.strip_prefix(&folder) else {
            continue;
        };
        let names: Option<Vec<&str>> = rest
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect();
        let Some(names) = names else { continue };
        return Ok(conn
            .prepare_cached(
                "SELECT id FROM tracks WHERE folder_id = ?1 AND relative_path = ?2
                 ORDER BY range_start LIMIT 1",
            )?
            .query_row(rusqlite::params![folder_id, names.join("/")], |row| {
                row.get(0)
            })
            .optional()?);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_files_with_negative_ids() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/tests/fixtures/tagged-id3v23.mp3");
        let track = register(&fixture).unwrap();
        assert!(track.track_id < 0);
        assert!(track.external);
        assert_eq!(track.title, "Café Déjà Vu");
        assert_eq!(file(track.track_id).unwrap().path, fixture);
        assert!(art(track.track_id).is_some());
        let other = register(&fixture).unwrap();
        assert_ne!(other.track_id, track.track_id);
        assert!(register(Path::new("/no/such/file.mp3")).is_err());
        assert!(file(1).is_none());
    }

    #[test]
    fn finds_library_tracks_by_path() {
        let library =
            crate::library::test_library::Library::new([crate::library::test_library::track(
                "a/b.flac",
            )
            .title("B")]);
        let id = library_track(&library.conn, Path::new("/Music/a/b.flac")).unwrap();
        assert!(id.is_some());
        assert_eq!(
            library_track(&library.conn, Path::new("/Music/a/c.flac")).unwrap(),
            None
        );
        assert_eq!(
            library_track(&library.conn, Path::new("/Elsewhere/b.flac")).unwrap(),
            None
        );
    }
}
