//! Incremental folder scanner. Walks a library folder, reads the tags of new
//! and changed audio files with the core (in parallel), and brings the
//! folder's tracks in the database up to date. A file counts as changed when
//! its size or modification time differs from the last scan (or its cue
//! sheet's time, when cue sheets are read).
//!
//! A file is usually one track. With cue sheets on (PLAN.md O5), a file
//! that a cue sheet splits (a `.cue` next to it, or a CUESHEET tag), or
//! that has chapters, becomes a track per part, each a row with the part's
//! start and end; the cue's titles and performers override the tags.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

use super::access::open_folder;
use super::cue::{self, CuePart, CueSheet};
use super::{remove_orphans, unix_now, Error};
use crate::anomp::{self, TagParts, Tags};

/// Files read and written per transaction, so a first scan of a large folder
/// fills the library (and reports progress) as it goes.
const BATCH_SIZE: usize = 256;

/// Cue sheets larger than this aren't cue sheets.
const CUE_LIMIT: u64 = 256 << 10;

/// How a scan reads files.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScanOptions {
    /// Split files into their cue sheet's tracks or their chapters.
    pub parts: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub folder_id: i64,
    /// Tracks added, updated and removed; a file split into parts counts
    /// once per part.
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

/// A track already in the library: one part of its file.
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
    /// The parts of the file already in the library (none if it's new).
    known: Vec<Known>,
    /// The `.cue` next to it that splits it, if any.
    cue: Option<PathBuf>,
}

/// What reading a file gave: its tags, and the parts it plays as.
struct ReadFile {
    tags: Tags,
    parts: Vec<Part>,
}

/// One track of a file.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    start: f64,
    end: Option<f64>,
    number: Option<u32>,
    title: Option<String>,
    /// The track's artist from its cue sheet, overriding the tags.
    performer: Option<String>,
    /// From the cue sheet, overriding the tags: the album and its artist.
    album: Option<String>,
    album_artist: Option<String>,
}

impl Part {
    fn whole() -> Part {
        Part {
            start: 0.0,
            end: None,
            number: None,
            title: None,
            performer: None,
            album: None,
            album_artist: None,
        }
    }
}

/// Scans one library folder, first resolving its bookmark (which updates its
/// path if it moved). Fails, changing nothing else, if the folder itself is
/// missing (e.g. on an unmounted drive).
pub fn scan_folder(
    conn: &mut Connection,
    folder_id: i64,
    options: ScanOptions,
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
    let (pending, unreadable) = walk(root, &mut known, options, &mut report);

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
    report.unchanged += kept.iter().map(|(_, parts)| parts.len()).sum::<usize>();

    let to_read = pending.len();
    progress(ScanProgress {
        folder_id,
        read: 0,
        to_read,
    });
    let mut read = 0;
    for batch in pending.chunks(BATCH_SIZE) {
        let all_read = read_all(batch, options);
        let tx = conn.transaction()?;
        let now = unix_now();
        for (file, result) in batch.iter().zip(all_read) {
            match result {
                Ok(read) => write_file(&tx, folder_id, file, &read, now, &mut report)?,
                Err(error) => {
                    for part in &file.known {
                        tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                            .execute([part.id])?;
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
    for (_, parts) in &missing {
        for part in parts {
            tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                .execute([part.id])?;
            report.removed += 1;
        }
    }
    remove_orphans(&tx)?;
    tx.execute(
        "UPDATE folders SET last_scan_at = ?1 WHERE id = ?2",
        params![unix_now(), folder_id],
    )?;
    tx.commit()?;
    Ok(report)
}

/// The folder's tracks by path, each path's parts in order.
fn known_tracks(
    conn: &Connection,
    folder_id: i64,
) -> rusqlite::Result<HashMap<String, Vec<Known>>> {
    let mut statement = conn.prepare(
        "SELECT relative_path, id, file_size, file_mtime_ns FROM tracks
         WHERE folder_id = ?1 ORDER BY relative_path, range_start",
    )?;
    let rows = statement.query_map([folder_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Known {
                id: row.get(1)?,
                size: row.get(2)?,
                mtime_ns: row.get(3)?,
            },
        ))
    })?;
    let mut known: HashMap<String, Vec<Known>> = HashMap::new();
    for row in rows {
        let (relative, track) = row?;
        known.entry(relative).or_default().push(track);
    }
    Ok(known)
}

/// An audio file found by the walk.
struct Found {
    relative: String,
    path: PathBuf,
    size: i64,
    mtime_ns: i64,
}

/// Walks `root`, following symlinks and skipping hidden files and folders.
/// Removes each audio file it finds from `known`, counting it as unchanged
/// or returning it as pending. Also returns the relative paths of folders
/// (or files) it couldn't read, recording them as failures.
fn walk(
    root: &Path,
    known: &mut HashMap<String, Vec<Known>>,
    options: ScanOptions,
    report: &mut ScanReport,
) -> (Vec<Pending>, Vec<String>) {
    let mut found = Vec::new();
    let mut cues: HashMap<PathBuf, Vec<(PathBuf, i64)>> = HashMap::new();
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
        if !entry.file_type().is_file() {
            continue;
        }
        let is_cue = options.parts && has_extension(entry.path(), "cue");
        if !is_cue && !is_audio(entry.path(), &mut decodable) {
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
                if !is_cue {
                    unreadable.push(relative);
                }
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
        if is_cue {
            let dir = entry.path().parent().unwrap_or(root).to_path_buf();
            cues.entry(dir)
                .or_default()
                .push((entry.into_path(), mtime_ns));
        } else {
            found.push(Found {
                relative,
                path: entry.into_path(),
                size,
                mtime_ns,
            });
        }
    }

    let mut pending = Vec::new();
    let mut sheets: HashMap<PathBuf, Option<CueSheet>> = HashMap::new();
    for file in found {
        // A cue sheet next to the file makes it change when the sheet does.
        let cue = cues
            .get(file.path.parent().unwrap_or(root))
            .and_then(|dir| cue_for(&file.path, dir, &mut sheets));
        let mtime_ns = match &cue {
            Some((_, cue_mtime)) => file.mtime_ns.max(*cue_mtime),
            None => file.mtime_ns,
        };
        let previous = known.remove(&file.relative).unwrap_or_default();
        if !previous.is_empty()
            && previous
                .iter()
                .all(|track| track.size == file.size && track.mtime_ns == mtime_ns)
        {
            report.unchanged += previous.len();
            continue;
        }
        pending.push(Pending {
            relative: file.relative,
            path: file.path,
            size: file.size,
            mtime_ns,
            known: previous,
            cue: cue.map(|(path, _)| path),
        });
    }
    (pending, unreadable)
}

/// The cue sheet in `dir` (paths and modification times) that splits
/// `audio`: one named after it ("Album.cue" or "Album.flac.cue"), else one
/// whose FILE names it.
fn cue_for(
    audio: &Path,
    dir: &[(PathBuf, i64)],
    sheets: &mut HashMap<PathBuf, Option<CueSheet>>,
) -> Option<(PathBuf, i64)> {
    let name = audio.file_name()?.to_str()?;
    let stem = audio.file_stem()?.to_str()?;
    let named = |cue: &Path| {
        cue.file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|cue_stem| {
                cue_stem.eq_ignore_ascii_case(stem) || cue_stem.eq_ignore_ascii_case(name)
            })
    };
    let mut sheet = |cue: &Path| {
        sheets
            .entry(cue.to_path_buf())
            .or_insert_with(|| read_cue(cue))
            .clone()
    };
    if let Some((cue, mtime)) = dir.iter().find(|(cue, _)| named(cue)) {
        return sheet(cue).is_some().then(|| (cue.clone(), *mtime));
    }
    dir.iter()
        .find(|(cue, _)| {
            sheet(cue).is_some_and(|sheet| {
                sheet
                    .files
                    .iter()
                    .any(|file| cue_file_matches(&file.name, name))
            })
        })
        .map(|(cue, mtime)| (cue.clone(), *mtime))
}

/// Whether a cue sheet's FILE `listed` is the audio file `name`, allowing
/// for a rip to WAV compressed later.
fn cue_file_matches(listed: &str, name: &str) -> bool {
    let listed = listed.rsplit(['/', '\\']).next().unwrap_or(listed);
    let stem = |text: &str| {
        text.rsplit_once('.')
            .map_or(text, |(stem, _)| stem)
            .to_lowercase()
    };
    listed.eq_ignore_ascii_case(name) || stem(listed) == stem(name)
}

fn read_cue(path: &Path) -> Option<CueSheet> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > CUE_LIMIT {
        return None;
    }
    cue::parse(&cue::decode(&std::fs::read(path).ok()?))
}

fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(wanted))
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

/// Reads the tags (and parts) of `files` on one thread per core, keeping
/// their order.
fn read_all(files: &[Pending], options: ScanOptions) -> Vec<Result<ReadFile, String>> {
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map_or(4, |count| count.get())
        .min(files.len());
    let mut results: Vec<Option<Result<ReadFile, String>>> = files.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut read = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file) = files.get(index) else { break };
                        read.push((index, read_file(file, options)));
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

fn read_file(file: &Pending, options: ScanOptions) -> Result<ReadFile, String> {
    let tags = anomp::read_tags_with(
        &file.path,
        TagParts {
            chapters: options.parts,
            ..TagParts::default()
        },
    )?;
    let parts = if options.parts {
        parts_of(file, &tags)
    } else {
        vec![Part::whole()]
    };
    Ok(ReadFile { tags, parts })
}

/// The tracks a file plays as: its cue sheet's (a `.cue` next to it, else
/// its CUESHEET tag), else its chapters, else the whole file.
fn parts_of(file: &Pending, tags: &Tags) -> Vec<Part> {
    let name = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let sheet = file
        .cue
        .as_deref()
        .and_then(read_cue)
        .or_else(|| tags.cue_sheet.as_deref().and_then(cue::parse));
    if let Some(sheet) = sheet {
        let parts = sheet.parts_for(name);
        if !parts.is_empty() {
            return parts
                .into_iter()
                .map(|part: CuePart| Part {
                    start: part.start,
                    end: part.end,
                    number: Some(part.number),
                    title: part.title,
                    // A track without its own performer is the album's.
                    performer: part.performer.or_else(|| sheet.performer.clone()),
                    album: sheet.title.clone(),
                    album_artist: sheet.performer.clone(),
                })
                .collect();
        }
    }
    if tags.chapters.len() >= 2 {
        return tags
            .chapters
            .iter()
            .enumerate()
            .map(|(index, chapter)| Part {
                start: chapter.start.max(0.0),
                end: chapter.end,
                number: u32::try_from(index + 1).ok(),
                title: chapter.title.clone(),
                performer: None,
                album: None,
                album_artist: None,
            })
            .collect();
    }
    vec![Part::whole()]
}

/// Writes a file's parts, keeping the ids of those already there, and
/// removes parts it no longer has.
fn write_file(
    tx: &Transaction,
    folder_id: i64,
    file: &Pending,
    read: &ReadFile,
    now: i64,
    report: &mut ScanReport,
) -> rusqlite::Result<()> {
    let mut kept = Vec::new();
    for part in &read.parts {
        let id = write_track(tx, folder_id, file, &read.tags, part, now)?;
        kept.push(id);
        if file.known.iter().any(|known| known.id == id) {
            report.updated += 1;
        } else {
            report.added += 1;
        }
    }
    for known in &file.known {
        if !kept.contains(&known.id) {
            tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                .execute([known.id])?;
            report.removed += 1;
        }
    }
    Ok(())
}

/// Upserts one part of a file; returns its id.
fn write_track(
    tx: &Transaction,
    folder_id: i64,
    file: &Pending,
    tags: &Tags,
    part: &Part,
    now: i64,
) -> rusqlite::Result<i64> {
    let composer_id = artist_id(tx, tags.composer.as_deref(), None)?;
    // The cue sheet's performer and album override the tags.
    let artist_name = part.performer.as_deref().or(tags.artist.as_deref());
    let artist_id = artist_id(
        tx,
        artist_name,
        part.performer
            .is_none()
            .then_some(tags.musicbrainz_artist_id.as_deref())
            .flatten(),
    )?;
    let album_artist_name = part
        .album_artist
        .as_deref()
        .or(tags.album_artist.as_deref());
    let album_artist_id = match album_artist_name {
        Some(name) => artist_id_for(
            tx,
            name,
            part.album_artist
                .is_none()
                .then_some(tags.musicbrainz_album_artist_id.as_deref())
                .flatten(),
        )?
        .into(),
        None => artist_id,
    };
    let album_id = part
        .album
        .as_deref()
        .or(tags.album.as_deref())
        .map(|title| album_id(tx, title, album_artist_id, tags))
        .transpose()?;
    let duration = match part.end {
        Some(end) => end - part.start,
        None => (tags.duration - part.start).max(0.0),
    };
    // The original release's date if tagged, else the release's.
    let release_date = match (&tags.original_date, &tags.date) {
        (Some(original), Some(date)) if date.starts_with(original.as_str()) => Some(date),
        (Some(original), _) => Some(original),
        (None, date) => date.as_ref(),
    };

    // An upsert rather than REPLACE, which would give the track a new id;
    // `added_at` is set only on insert.
    tx.prepare_cached(
        "INSERT INTO tracks (
             folder_id, relative_path, range_start, range_end, file_size, file_mtime_ns, title,
             artist_id, album_id, album_artist_id, genre, track_number, track_total, disc_number,
             disc_total, year, duration, sample_rate, channels, bitrate_kbps,
             musicbrainz_recording_id, musicbrainz_release_track_id, scanned_at,
             replaygain_track_gain, replaygain_track_peak, replaygain_album_gain,
             replaygain_album_peak, added_at, release_date, work, movement_name, movement_number,
             movement_total, composer_id, conductor)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                 ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?23, ?28, ?29, ?30, ?31, ?32, ?33,
                 ?34)
         ON CONFLICT (folder_id, relative_path, range_start) DO UPDATE SET
             range_end = excluded.range_end,
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
             replaygain_album_peak = excluded.replaygain_album_peak,
             release_date = excluded.release_date,
             work = excluded.work,
             movement_name = excluded.movement_name,
             movement_number = excluded.movement_number,
             movement_total = excluded.movement_total,
             composer_id = excluded.composer_id,
             conductor = excluded.conductor
         RETURNING id",
    )?
    .query_row(
        params![
            folder_id,
            file.relative,
            part.start,
            part.end,
            file.size,
            file.mtime_ns,
            part.title.as_ref().or(tags.title.as_ref()),
            artist_id,
            album_id,
            album_artist_id,
            tags.genre,
            part.number.or(tags.track_number),
            // A cue sheet's own count of tracks isn't known here.
            if part.number.is_some() {
                None
            } else {
                tags.track_total
            },
            tags.disc_number,
            tags.disc_total,
            tags.year,
            duration,
            tags.sample_rate,
            tags.channels,
            tags.bitrate_kbps,
            // A file's recording id belongs to the whole file.
            part.number
                .is_none()
                .then_some(tags.musicbrainz_recording_id.as_ref())
                .flatten(),
            part.number
                .is_none()
                .then_some(tags.musicbrainz_release_track_id.as_ref())
                .flatten(),
            now,
            tags.replay_gain.track_gain,
            tags.replay_gain.track_peak,
            tags.replay_gain.album_gain,
            tags.replay_gain.album_peak,
            release_date,
            tags.work,
            tags.movement_name,
            tags.movement_number,
            tags.movement_total,
            composer_id,
            tags.conductor,
        ],
        |row| row.get(0),
    )
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
            scan_folder(&mut self.conn, self.folder_id, PARTS, |_| {}).unwrap()
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

    const PARTS: ScanOptions = ScanOptions { parts: true };

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
        let report = scan_folder(&mut library.conn, library.folder_id, PARTS, |p| {
            updates.push(p)
        })
        .unwrap();

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
        let report = scan_folder(&mut library.conn, library.folder_id, PARTS, |p| {
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
        let error = scan_folder(&mut library.conn, library.folder_id, PARTS, |_| {}).unwrap_err();
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

    fn column<T: rusqlite::types::FromSql>(library: &Library, sql: &str) -> Vec<T> {
        library
            .conn
            .prepare(sql)
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn a_cue_sheet_splits_its_file_into_tracks() {
        let mut library = Library::new(&[("flac-44k.flac", "Rip/Live.flac")]);
        library.write(
            "Rip/Live.cue",
            "PERFORMER \"The Quartet\"\nTITLE \"Live\"\nFILE \"Live.wav\" WAVE\n\
             TRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\n\
             TRACK 02 AUDIO\nTITLE \"Encore\"\nPERFORMER \"Guest\"\nINDEX 01 00:00:15\n",
        );
        let report = library.scan();
        assert_eq!((report.added, report.failed.len()), (2, 0));
        let rows: Vec<(String, f64, Option<f64>, Option<u32>, String, String)> = library
            .conn
            .prepare(
                "SELECT t.title, t.range_start, t.range_end, t.track_number, ar.name, al.title
                 FROM tracks t JOIN artists ar ON ar.id = t.artist_id
                 JOIN albums al ON al.id = t.album_id ORDER BY t.range_start",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows[0].0, "Opening");
        assert_eq!((rows[0].1, rows[0].2), (0.0, Some(0.2)));
        assert_eq!(rows[0].4, "The Quartet");
        assert_eq!(rows[1].0, "Encore");
        assert_eq!((rows[1].1, rows[1].2, rows[1].3), (0.2, None, Some(2)));
        assert_eq!(rows[1].4, "Guest");
        assert_eq!(rows[1].5, "Live");
        let durations: Vec<f64> =
            column(&library, "SELECT duration FROM tracks ORDER BY range_start");
        assert!((durations[0] - 0.2).abs() < 1e-9);
        assert!((durations[1] - (22371.0 / 44100.0 - 0.2)).abs() < 0.01);

        // Unchanged: nothing is read again, and the ids stay.
        let ids: Vec<i64> = column(&library, "SELECT id FROM tracks ORDER BY range_start");
        let report = library.scan();
        assert_eq!((report.unchanged, report.added, report.updated), (2, 0, 0));

        // A changed sheet re-reads the file, keeping the parts it still has.
        touch(&library.path("Rip/Live.cue"));
        fs::write(
            library.path("Rip/Live.cue"),
            "FILE \"Live.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\n",
        )
        .unwrap();
        touch(&library.path("Rip/Live.cue"));
        let report = library.scan();
        assert_eq!((report.updated, report.removed), (1, 1));
        let after: Vec<i64> = column(&library, "SELECT id FROM tracks");
        assert_eq!(after, [ids[0]]);
        let ends: Vec<Option<f64>> = column(&library, "SELECT range_end FROM tracks");
        assert_eq!(ends, [None]);
    }

    #[test]
    fn without_parts_a_cued_file_is_one_track() {
        let mut library = Library::new(&[("flac-44k.flac", "Rip/Live.flac")]);
        library.write(
            "Rip/Live.cue",
            "FILE \"Live.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n\
             TRACK 02 AUDIO\nINDEX 01 00:00:15\n",
        );
        let report = scan_folder(
            &mut library.conn,
            library.folder_id,
            ScanOptions::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(report.added, 1);
    }

    #[test]
    fn keeps_when_a_track_was_added_and_reads_new_tags() {
        let mut library = sample_library();
        library.scan();
        let added: Vec<i64> = column(&library, "SELECT added_at FROM tracks");
        assert!(added.iter().all(|&at| at > 1_700_000_000));
        library
            .conn
            .execute("UPDATE tracks SET added_at = 5", [])
            .unwrap();
        touch(&library.path("Loose/untitled.wav"));
        let report = library.scan();
        assert_eq!(report.updated, 1);
        let added: Vec<i64> = column(&library, "SELECT added_at FROM tracks");
        assert!(added.iter().all(|&at| at == 5), "{added:?}");

        // The tagged FLAC's full date.
        let dates: Vec<Option<String>> = column(
            &library,
            "SELECT release_date FROM tracks ORDER BY relative_path",
        );
        assert!(dates.contains(&Some("2004".into())) || dates.iter().any(Option::is_some));
    }
}
