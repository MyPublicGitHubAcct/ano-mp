//! Incremental folder scanner. Walks a library folder, reads the tags of new
//! and changed audio files with the core (in parallel), and brings the
//! folder's tracks in the database up to date. A file counts as changed when
//! its size or modification time differs from the last scan.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

use super::access::open_folder;
use super::{remove_orphans, unix_now, Error};
use crate::anomp::{self, Tags};

/// Files read and written per transaction, so a first scan of a large folder
/// fills the library (and reports progress) as it goes.
const BATCH_SIZE: usize = 256;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub folder_id: i64,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    /// Tracks left as they were, including any under a folder that couldn't
    /// be read this time.
    pub unchanged: usize,
    /// Files and folders that couldn't be read. Such files are left out of
    /// the library; tracks under such folders are kept.
    pub failed: Vec<ScanFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScanFailure {
    /// Absolute path.
    pub path: String,
    pub error: String,
}

/// Sent before the first file is read and after each batch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub folder_id: i64,
    /// New or changed files whose tags have been read so far.
    pub read: usize,
    pub to_read: usize,
}

/// A track already in the library.
struct Known {
    id: i64,
    size: i64,
    mtime_ns: i64,
}

/// A new or changed file whose tags need reading.
struct Pending {
    relative: String,
    path: PathBuf,
    size: i64,
    mtime_ns: i64,
    known: bool,
}

/// Scans one library folder, first resolving its bookmark (which updates its
/// path if it moved). Fails, changing nothing else, if the folder itself is
/// missing (e.g. on an unmounted drive).
pub fn scan_folder(
    conn: &mut Connection,
    folder_id: i64,
    mut progress: impl FnMut(ScanProgress),
) -> Result<ScanReport, Error> {
    // Readable until the scan returns.
    let folder = open_folder(conn, folder_id)?;
    let root = &folder.path;
    if !root.is_dir() {
        return Err(Error::Invalid(format!(
            "Folder not available: {}",
            root.display()
        )));
    }

    let mut report = ScanReport {
        folder_id,
        ..ScanReport::default()
    };
    let mut known = known_tracks(conn, folder_id)?;
    let (pending, unreadable) = walk(root, &mut known, &mut report);

    // What's left in `known` wasn't found, but a folder that couldn't be
    // read may still hold it.
    let under_unreadable = |relative: &str| {
        unreadable.iter().any(|dir| {
            dir.is_empty()
                || relative
                    .strip_prefix(dir.as_str())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
    };
    let (kept, missing): (Vec<_>, Vec<_>) = known
        .into_iter()
        .partition(|(relative, _)| under_unreadable(relative));
    report.unchanged += kept.len();

    let to_read = pending.len();
    progress(ScanProgress {
        folder_id,
        read: 0,
        to_read,
    });
    let mut read = 0;
    for batch in pending.chunks(BATCH_SIZE) {
        let all_tags = read_all(batch);
        let tx = conn.transaction()?;
        let now = unix_now();
        for (file, tags) in batch.iter().zip(all_tags) {
            match tags {
                Ok(tags) => {
                    write_track(&tx, folder_id, file, &tags, now)?;
                    if file.known {
                        report.updated += 1;
                    } else {
                        report.added += 1;
                    }
                }
                Err(error) => {
                    if file.known {
                        tx.prepare_cached(
                            "DELETE FROM tracks WHERE folder_id = ?1 AND relative_path = ?2",
                        )?
                        .execute(params![folder_id, file.relative])?;
                        report.removed += 1;
                    }
                    report.failed.push(ScanFailure {
                        path: file.path.to_string_lossy().into_owned(),
                        error,
                    });
                }
            }
        }
        tx.commit()?;
        read += batch.len();
        progress(ScanProgress {
            folder_id,
            read,
            to_read,
        });
    }

    let tx = conn.transaction()?;
    for (_, track) in &missing {
        tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
            .execute([track.id])?;
    }
    report.removed += missing.len();
    remove_orphans(&tx)?;
    tx.execute(
        "UPDATE folders SET last_scan_at = ?1 WHERE id = ?2",
        params![unix_now(), folder_id],
    )?;
    tx.commit()?;
    Ok(report)
}

fn known_tracks(conn: &Connection, folder_id: i64) -> rusqlite::Result<HashMap<String, Known>> {
    let mut statement = conn.prepare(
        "SELECT relative_path, id, file_size, file_mtime_ns FROM tracks WHERE folder_id = ?1",
    )?;
    let rows = statement.query_map([folder_id], |row| {
        Ok((
            row.get(0)?,
            Known {
                id: row.get(1)?,
                size: row.get(2)?,
                mtime_ns: row.get(3)?,
            },
        ))
    })?;
    rows.collect()
}

/// Walks `root`, following symlinks and skipping hidden files and folders.
/// Removes each audio file it finds from `known`, counting it as unchanged
/// or returning it as pending. Also returns the relative paths of folders
/// (or files) it couldn't read, recording them as failures.
fn walk(
    root: &Path,
    known: &mut HashMap<String, Known>,
    report: &mut ScanReport,
) -> (Vec<Pending>, Vec<String>) {
    let mut pending = Vec::new();
    let mut unreadable = Vec::new();
    let mut decodable = HashMap::new();
    let fail = |path: &Path, error: String, report: &mut ScanReport| {
        report.failed.push(ScanFailure {
            path: path.to_string_lossy().into_owned(),
            error,
        });
    };

    let entries = WalkDir::new(root)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()));
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let path = error.path().unwrap_or(root).to_path_buf();
                if let Some(relative) = relative_path(root, &path) {
                    unreadable.push(relative);
                }
                fail(&path, error.to_string(), report);
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_audio(entry.path(), &mut decodable) {
            continue;
        }
        let Some(relative) = relative_path(root, entry.path()) else {
            fail(entry.path(), "Path is not valid UTF-8".into(), report);
            continue;
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                fail(entry.path(), error.to_string(), report);
                unreadable.push(relative);
                continue;
            }
        };
        let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| {
                i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
            });

        let previous = known.remove(&relative);
        if previous
            .as_ref()
            .is_some_and(|track| track.size == size && track.mtime_ns == mtime_ns)
        {
            report.unchanged += 1;
            continue;
        }
        pending.push(Pending {
            relative,
            path: entry.into_path(),
            size,
            mtime_ns,
            known: previous.is_some(),
        });
    }
    (pending, unreadable)
}

/// Dotfiles, including macOS's "._" AppleDouble files on non-Mac volumes,
/// which carry audio extensions but no audio.
fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.as_encoded_bytes().first() == Some(&b'.')
}

/// Whether the core decodes files like `path`, cached per extension.
fn is_audio(path: &Path, decodable: &mut HashMap<String, bool>) -> bool {
    let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    let extension = extension.to_lowercase();
    *decodable
        .entry(extension)
        .or_insert_with_key(|extension| anomp::can_decode_extension(extension))
}

/// `path` relative to `root`, '/'-separated; `None` if it isn't under
/// `root` or isn't UTF-8.
fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let components = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?;
    Some(components.join("/"))
}

/// Reads the tags of `files` on one thread per core, keeping their order.
fn read_all(files: &[Pending]) -> Vec<Result<Tags, String>> {
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .min(files.len());
    let mut results: Vec<Option<Result<Tags, String>>> = files.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut read = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file) = files.get(index) else { break };
                        read.push((index, anomp::read_tags(&file.path, false)));
                    }
                    read
                })
            })
            .collect();
        for handle in handles {
            for (index, tags) in handle.join().expect("tag reader thread panicked") {
                results[index] = Some(tags);
            }
        }
    });
    results
        .into_iter()
        .map(|tags| tags.expect("every file was read"))
        .collect()
}

fn write_track(
    tx: &Transaction,
    folder_id: i64,
    file: &Pending,
    tags: &Tags,
    now: i64,
) -> rusqlite::Result<()> {
    let artist_id = artist_id(
        tx,
        tags.artist.as_deref(),
        tags.musicbrainz_artist_id.as_deref(),
    )?;
    let album_artist_id = match tags.album_artist.as_deref() {
        Some(name) => artist_id_for(tx, name, tags.musicbrainz_album_artist_id.as_deref())?.into(),
        None => artist_id,
    };
    let album_id = tags
        .album
        .as_deref()
        .map(|title| album_id(tx, title, album_artist_id, tags))
        .transpose()?;

    // An upsert rather than REPLACE, which would give the track a new id.
    tx.prepare_cached(
        "INSERT INTO tracks (
             folder_id, relative_path, file_size, file_mtime_ns, title, artist_id, album_id,
             album_artist_id, genre, track_number, track_total, disc_number, disc_total, year,
             duration, sample_rate, channels, bitrate_kbps, musicbrainz_recording_id,
             musicbrainz_release_track_id, scanned_at, replaygain_track_gain,
             replaygain_track_peak, replaygain_album_gain, replaygain_album_peak)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                 ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
         ON CONFLICT (folder_id, relative_path) DO UPDATE SET
             file_size = excluded.file_size,
             file_mtime_ns = excluded.file_mtime_ns,
             title = excluded.title,
             artist_id = excluded.artist_id,
             album_id = excluded.album_id,
             album_artist_id = excluded.album_artist_id,
             genre = excluded.genre,
             track_number = excluded.track_number,
             track_total = excluded.track_total,
             disc_number = excluded.disc_number,
             disc_total = excluded.disc_total,
             year = excluded.year,
             duration = excluded.duration,
             sample_rate = excluded.sample_rate,
             channels = excluded.channels,
             bitrate_kbps = excluded.bitrate_kbps,
             musicbrainz_recording_id = excluded.musicbrainz_recording_id,
             musicbrainz_release_track_id = excluded.musicbrainz_release_track_id,
             scanned_at = excluded.scanned_at,
             replaygain_track_gain = excluded.replaygain_track_gain,
             replaygain_track_peak = excluded.replaygain_track_peak,
             replaygain_album_gain = excluded.replaygain_album_gain,
             replaygain_album_peak = excluded.replaygain_album_peak",
    )?
    .execute(params![
        folder_id,
        file.relative,
        file.size,
        file.mtime_ns,
        tags.title,
        artist_id,
        album_id,
        album_artist_id,
        tags.genre,
        tags.track_number,
        tags.track_total,
        tags.disc_number,
        tags.disc_total,
        tags.year,
        tags.duration,
        tags.sample_rate,
        tags.channels,
        tags.bitrate_kbps,
        tags.musicbrainz_recording_id,
        tags.musicbrainz_release_track_id,
        now,
        tags.replay_gain.track_gain,
        tags.replay_gain.track_peak,
        tags.replay_gain.album_gain,
        tags.replay_gain.album_peak,
    ])?;
    Ok(())
}

fn artist_id(
    tx: &Transaction,
    name: Option<&str>,
    musicbrainz_id: Option<&str>,
) -> rusqlite::Result<Option<i64>> {
    name.map(|name| artist_id_for(tx, name, musicbrainz_id))
        .transpose()
}

/// The artist called `name`, created if needed. A MusicBrainz ID fills in
/// one the artist doesn't have yet.
fn artist_id_for(
    tx: &Transaction,
    name: &str,
    musicbrainz_id: Option<&str>,
) -> rusqlite::Result<i64> {
    tx.prepare_cached(
        "INSERT INTO artists (name, musicbrainz_id) VALUES (?1, ?2)
         ON CONFLICT (name) DO UPDATE SET
             musicbrainz_id = IFNULL(musicbrainz_id, excluded.musicbrainz_id)
         RETURNING id",
    )?
    .query_row(params![name, musicbrainz_id], |row| row.get(0))
}

/// The album `title` by `artist_id`, created if needed, filling in release
/// IDs it doesn't have yet.
fn album_id(
    tx: &Transaction,
    title: &str,
    artist_id: Option<i64>,
    tags: &Tags,
) -> rusqlite::Result<i64> {
    tx.prepare_cached(
        "INSERT INTO albums (title, artist_id, musicbrainz_release_id, musicbrainz_release_group_id)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (IFNULL(artist_id, 0), title) DO UPDATE SET
             musicbrainz_release_id = IFNULL(musicbrainz_release_id, excluded.musicbrainz_release_id),
             musicbrainz_release_group_id =
                 IFNULL(musicbrainz_release_group_id, excluded.musicbrainz_release_group_id)
         RETURNING id",
    )?
    .query_row(
        params![
            title,
            artist_id,
            tags.musicbrainz_release_id,
            tags.musicbrainz_release_group_id
        ],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime};

    use super::*;
    use crate::library::{add_folder, db, folders, remove_folder, tracks, TrackSummary};

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/tests/fixtures")
            .join(name)
    }

    /// A temporary library folder holding copies of fixtures.
    struct Library {
        _dir: tempfile::TempDir,
        root: PathBuf,
        conn: Connection,
        folder_id: i64,
    }

    impl Library {
        /// `files` maps fixture names to '/'-separated paths in the folder.
        fn new(files: &[(&str, &str)]) -> Library {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("Music");
            fs::create_dir(&root).unwrap();
            let conn = db::open_in_memory().unwrap();
            let folder = add_folder(&conn, &root).unwrap();
            let library = Library {
                root: PathBuf::from(folder.path),
                _dir: dir,
                conn,
                folder_id: folder.id,
            };
            for (fixture_name, relative) in files {
                library.copy(fixture_name, relative);
            }
            library
        }

        fn path(&self, relative: &str) -> PathBuf {
            crate::library::track_path(&self.root, relative)
        }

        fn copy(&self, fixture_name: &str, relative: &str) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::copy(fixture(fixture_name), path).unwrap();
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }

        fn scan(&mut self) -> ScanReport {
            scan_folder(&mut self.conn, self.folder_id, |_| {}).unwrap()
        }

        fn tracks(&self) -> Vec<TrackSummary> {
            tracks(&self.conn).unwrap()
        }

        /// Paths of the library's tracks, relative to the folder.
        fn relative_paths(&self) -> Vec<String> {
            self.tracks()
                .into_iter()
                .map(|track| relative_path(&self.root, Path::new(&track.path)).unwrap())
                .collect()
        }

        fn count(&self, table: &str) -> i64 {
            self.conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        }
    }

    const ALBUM_DIR: &str = "Various Artists/東京 Sessions";

    fn sample_library() -> Library {
        Library::new(&[
            (
                "tagged-vorbis.flac",
                "Various Artists/東京 Sessions/03 Café.flac",
            ),
            (
                "tagged-id3v23.mp3",
                "Various Artists/東京 Sessions/03 Café.mp3",
            ),
            ("wav-s16-44k.wav", "Loose/untitled.wav"),
        ])
    }

    fn touch(path: &Path) {
        let later = SystemTime::now() + Duration::from_secs(10);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(later)
            .unwrap();
    }

    #[test]
    fn first_scan_adds_every_audio_file() {
        let mut library = sample_library();
        library.write("Loose/cover.jpg", "not audio");
        library.write("Loose/notes.txt", "not audio");
        library.write("Loose/._untitled.wav", "AppleDouble metadata");
        library.write(".Trash/deleted.flac", "hidden folder");

        let mut updates = Vec::new();
        let report =
            scan_folder(&mut library.conn, library.folder_id, |p| updates.push(p)).unwrap();

        assert_eq!(
            report,
            ScanReport {
                folder_id: library.folder_id,
                added: 3,
                ..ScanReport::default()
            }
        );
        let progress = |read| ScanProgress {
            folder_id: library.folder_id,
            read,
            to_read: 3,
        };
        assert_eq!(updates, [progress(0), progress(3)]);
        assert_eq!(
            library.relative_paths(),
            [
                "Loose/untitled.wav",
                "Various Artists/東京 Sessions/03 Café.flac",
                "Various Artists/東京 Sessions/03 Café.mp3"
            ]
        );

        let tracks = library.tracks();
        assert_eq!(tracks[0].title, None);
        assert_eq!(tracks[0].artist, None);
        assert_eq!(tracks[0].album, None);
        assert!((tracks[0].duration - 22371.0 / 44100.0).abs() < 0.001);
        for tagged in &tracks[1..] {
            assert_eq!(tagged.title.as_deref(), Some("Café Déjà Vu"));
            assert_eq!(tagged.artist.as_deref(), Some("Ano Artist"));
            assert_eq!(tagged.album.as_deref(), Some("東京 Sessions"));
            assert_eq!(tagged.album_artist.as_deref(), Some("Various Artists"));
            assert_eq!(
                (tagged.disc_number, tagged.track_number),
                (Some(1), Some(3))
            );
        }

        // Both tagged files share one album and its two artists.
        assert_eq!(library.count("albums"), 1);
        assert_eq!(library.count("artists"), 2);
        let (release, album_artist_mbid): (String, String) = library
            .conn
            .query_row(
                "SELECT musicbrainz_release_id, artists.musicbrainz_id
                 FROM albums JOIN artists ON artists.id = albums.artist_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(release, "a1b2c3d4-0000-4000-8000-000000000001");
        assert_eq!(album_artist_mbid, "89ad4ac3-39f7-470e-963a-56509c546377");

        let folder = &folders(&library.conn).unwrap()[0];
        assert_eq!(folder.track_count, 3);
        assert!(folder.last_scan_at.is_some());
    }

    #[test]
    fn rescan_reads_only_changed_files() {
        let mut library = sample_library();
        library.scan();
        let before = library.tracks();

        let report = library.scan();
        assert_eq!((report.added, report.updated, report.unchanged), (0, 0, 3));

        // A new modification time alone triggers a re-read; so does new
        // contents under the same name.
        touch(&library.path(&format!("{ALBUM_DIR}/03 Café.mp3")));
        library.copy("wav-mono-48k.wav", "Loose/untitled.wav");
        let mut to_read = 0;
        let report = scan_folder(&mut library.conn, library.folder_id, |p| {
            to_read = p.to_read
        })
        .unwrap();
        assert_eq!((report.added, report.updated, report.unchanged), (0, 2, 1));
        assert_eq!(to_read, 2);

        let after = library.tracks();
        let ids = |tracks: &[TrackSummary]| tracks.iter().map(|t| t.id).collect::<Vec<_>>();
        assert_eq!(ids(&after), ids(&before), "updates keep track ids");
        assert!((after[0].duration - 24321.0 / 48000.0).abs() < 0.001);
    }

    #[test]
    fn removes_deleted_files_and_their_albums_and_artists() {
        let mut library = sample_library();
        library.scan();

        fs::remove_file(library.path(&format!("{ALBUM_DIR}/03 Café.flac"))).unwrap();
        let report = library.scan();
        assert_eq!((report.removed, report.unchanged), (1, 2));
        assert_eq!(library.count("albums"), 1);

        fs::remove_dir_all(library.path("Various Artists")).unwrap();
        let report = library.scan();
        assert_eq!((report.removed, report.unchanged), (1, 1));
        assert_eq!(library.relative_paths(), ["Loose/untitled.wav"]);
        assert_eq!(library.count("albums"), 0);
        assert_eq!(library.count("artists"), 0);
    }

    #[test]
    fn retagged_files_move_album() {
        let mut library = Library::new(&[("tagged-vorbis.flac", "a.flac")]);
        library.scan();
        // Same file name, now untagged: the album and artists go.
        library.copy("flac-44k.flac", "a.flac");
        let report = library.scan();
        assert_eq!(report.updated, 1);
        assert_eq!(library.tracks()[0].album, None);
        assert_eq!((library.count("albums"), library.count("artists")), (0, 0));
    }

    #[test]
    fn unreadable_files_are_reported_and_left_out() {
        let mut library = sample_library();
        library.write("broken.mp3", "not an mp3");
        let report = library.scan();
        assert_eq!(report.added, 3);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(
            Path::new(&report.failed[0].path),
            library.path("broken.mp3")
        );

        // A track that becomes unreadable leaves the library.
        library.write("Loose/untitled.wav", "truncated");
        let report = library.scan();
        assert_eq!((report.removed, report.failed.len()), (1, 2));
        assert_eq!(library.tracks().len(), 2);
    }

    #[test]
    fn missing_folder_keeps_its_tracks() {
        let mut library = sample_library();
        library.scan();

        // As when its drive is unmounted, the bookmark no longer resolves.
        fs::remove_dir_all(&library.root).unwrap();
        let error = scan_folder(&mut library.conn, library.folder_id, |_| {}).unwrap_err();
        assert!(
            error.to_string().starts_with("Folder not available"),
            "{error}"
        );
        assert_eq!(library.tracks().len(), 3);
    }

    #[cfg(target_vendor = "apple")]
    #[test]
    fn moved_folder_keeps_its_tracks() {
        let mut library = sample_library();
        library.scan();
        let ids: Vec<i64> = library.tracks().iter().map(|track| track.id).collect();

        // The bookmark follows the folder, and the tracks follow its path.
        let moved = library.root.with_file_name("Moved");
        fs::rename(&library.root, &moved).unwrap();
        let report = library.scan();
        assert_eq!((report.added, report.removed, report.unchanged), (0, 0, 3));
        library.root = moved;
        assert_eq!(
            library
                .tracks()
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            ids
        );
        assert!(library
            .tracks()
            .iter()
            .all(|track| Path::new(&track.path).starts_with(&library.root)));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_subfolder_keeps_its_tracks() {
        use std::os::unix::fs::PermissionsExt;

        let mut library = sample_library();
        library.scan();
        let album = library.path(ALBUM_DIR);
        fs::set_permissions(&album, fs::Permissions::from_mode(0o000)).unwrap();
        let report = library.scan();
        fs::set_permissions(&album, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!((report.removed, report.unchanged), (0, 3));
        assert_eq!(report.failed.len(), 1);
        assert_eq!(Path::new(&report.failed[0].path), album);
        assert_eq!(library.tracks().len(), 3);
    }

    #[cfg(unix)]
    #[test]
    fn follows_symlinks_without_looping() {
        let mut library = Library::new(&[("flac-44k.flac", "a.flac")]);
        let elsewhere = library.root.with_file_name("Elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::copy(fixture("mp3-44k.mp3"), elsewhere.join("b.mp3")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, library.path("Linked")).unwrap();
        std::os::unix::fs::symlink(&library.root, library.path("Loop")).unwrap();

        let report = library.scan();
        assert_eq!(report.added, 2);
        assert_eq!(report.failed.len(), 1, "the loop is reported once");
        assert_eq!(library.relative_paths(), ["Linked/b.mp3", "a.flac"]);
    }

    #[test]
    fn removing_a_folder_removes_its_tracks() {
        let mut library = sample_library();
        library.scan();
        remove_folder(&mut library.conn, library.folder_id).unwrap();
        assert_eq!(
            (
                library.count("tracks"),
                library.count("albums"),
                library.count("artists")
            ),
            (0, 0, 0)
        );
        assert!(remove_folder(&mut library.conn, library.folder_id).is_err());
    }

    #[test]
    fn relative_paths_use_forward_slashes() {
        let root: PathBuf = ["/Music"].iter().collect();
        let path: PathBuf = ["/Music", "A", "B", "c.flac"].iter().collect();
        assert_eq!(relative_path(&root, &path).as_deref(), Some("A/B/c.flac"));
        assert_eq!(relative_path(&root, &root).as_deref(), Some(""));
        assert_eq!(relative_path(&path, &root), None);
    }
}
