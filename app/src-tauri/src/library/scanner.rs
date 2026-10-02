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
//!
//! Several folders are scanned together (`scan_folders`): every folder is
//! walked before any file is read, and a new file that is one that went
//! missing (the same size, and the same modification time, recording MBID
//! or tags) takes over its rows (PLAN.md F10). A file moved or renamed,
//! within a folder or from one to another, so keeps its track ids, and with
//! them its plays, ratings, favourites, playlist entries and preferences.
//! An album or artist left without tracks keeps the user's picks in
//! `kept_albums` and `kept_artists` until it comes back (`restore_kept`).
//!
//! A scan never empties a folder on its own (PLAN.md H22): when every
//! track of a folder, or most of a large one (`holds`), would go, they are
//! kept and the folder fails as `FolderState::MostlyGone` (or `Empty` if it
//! holds nothing at all), since an unmounted share or drive can leave an
//! empty folder at its path. The user removes them with `remove_missing`.
//! A file that fails to read because it has gone (a drive unmounted during
//! the scan) keeps its tracks too.
//!
//! Artists (PLAN.md F11): an artist tag that credits several artists ("A; B",
//! or a multi-valued ARTISTS tag) lists each in `track_artists`, the first
//! as the track's artist, keeping the tag's text as its credit. A file
//! without an album artist tag is filed under "Various Artists" when it is
//! marked as a compilation, or when its album title has three or more track
//! artists in its folder (`regroup_compilations`), else under its artist.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::UNIX_EPOCH;

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

use super::access::{open_folder, FolderState, FolderStatus, OpenFolder};
use super::cue::{self, CuePart, CueSheet};
use super::{remove_orphans, unix_now, Error};
use crate::anomp::{self, split_artists, TagParts, Tags};

/// The album artist of compilations without one (MusicBrainz's special
/// artist of that name).
pub const VARIOUS_ARTISTS: &str = "Various Artists";
const VARIOUS_ARTISTS_MBID: &str = "89ad4ac3-39f7-470e-963a-56509c546377";

/// Track artists an album title needs in one folder to be a compilation.
const COMPILATION_ARTISTS: usize = 3;

/// Files read and written per transaction, so a first scan of a large folder
/// fills the library (and reports progress) as it goes.
const BATCH_SIZE: usize = 256;

/// Cue sheets larger than this aren't cue sheets.
const CUE_LIMIT: u64 = 256 << 10;

/// A folder with at least this many tracks is held when a scan would
/// remove more than half of them (`holds`).
const HOLD_MIN_TRACKS: usize = 20;

/// Whether a scan that would remove `gone` of a folder's `known` tracks
/// keeps them and asks first: all of them, or more than half of a folder
/// of at least `HOLD_MIN_TRACKS`.
fn holds(known: usize, gone: usize) -> bool {
    gone > 0 && (gone == known || (known >= HOLD_MIN_TRACKS && gone * 2 > known))
}

/// How a scan reads files.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScanOptions {
    /// Split files into their cue sheet's tracks or their chapters.
    pub parts: bool,
    /// A scan the user didn't ask for (at launch, after changes on disk):
    /// its threads run at a low priority, and fewer of them (F9).
    pub background: bool,
    /// Remove missing tracks even when `holds` would keep them: the user
    /// said to.
    pub remove_missing: bool,
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
    /// Tracks whose file moved or was renamed, keeping their ids (F10).
    pub moved: usize,
    /// Tracks left as they were, including any under a folder that couldn't
    /// be read this time.
    pub unchanged: usize,
    /// Files and folders that couldn't be read. Such files are left out of
    /// the library; tracks under such folders are kept.
    pub failed: Vec<ScanFailure>,
    /// Set when the folder couldn't be scanned, or the scan kept tracks it
    /// didn't find (PLAN.md H22): why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<FolderStatus>,
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
#[derive(Debug, Clone)]
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
#[cfg(test)]
pub fn scan_folder(
    conn: &mut Connection,
    folder_id: i64,
    options: ScanOptions,
    progress: impl FnMut(ScanProgress),
) -> Result<ScanReport, Error> {
    scan_folders(conn, &[folder_id], options, progress)?
        .pop()
        .expect("a result per folder")
}

/// Scans library folders together, so a file moved from one to another
/// keeps its track (see the module's notes). Returns a result per folder,
/// in order: a folder that can't be opened (an unmounted drive) fails alone,
/// changing nothing of it. Fails as a whole only on a database error.
pub fn scan_folders(
    conn: &mut Connection,
    folder_ids: &[i64],
    options: ScanOptions,
    mut progress: impl FnMut(ScanProgress),
) -> Result<Vec<Result<ScanReport, Error>>, Error> {
    // Walk every folder first, holding each open until the scan ends.
    let mut results: Vec<Result<ScanReport, Error>> = Vec::with_capacity(folder_ids.len());
    let mut scans = Vec::new();
    let mut missing = Vec::new();
    for &folder_id in folder_ids {
        match walk_folder(conn, folder_id, options) {
            Ok((scan, gone)) => {
                results.push(Ok(ScanReport::default()));
                scans.push((results.len() - 1, scan));
                missing.push((folder_id, gone));
            }
            Err(Error::Invalid(message)) => results.push(Err(Error::Invalid(message))),
            Err(error) => return Err(error),
        }
    }

    let mut pool = MissingPool::load(conn, missing)?;
    for (_, scan) in &mut scans {
        read_and_write(conn, scan, options, &mut pool, &mut progress)?;
    }

    let tx = conn.transaction()?;
    let mut held = Vec::new();
    for (index, scan) in &mut scans {
        let gone = pool.unmatched(scan.folder_id);
        if !options.remove_missing && holds(scan.known, gone.len()) {
            let state = if scan.found == 0 {
                FolderState::Empty
            } else {
                FolderState::MostlyGone
            };
            log::warn!(
                "kept the {} of {} tracks a scan of folder {} didn't find ({})",
                gone.len(),
                scan.known,
                scan.folder_id,
                state.code()
            );
            held.push((*index, scan.root.clone(), state, gone.len()));
        } else {
            for id in gone {
                tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                    .execute([id])?;
                scan.report.removed += 1;
            }
            tx.execute(
                "UPDATE folders SET last_scan_at = ?1 WHERE id = ?2",
                params![unix_now(), scan.folder_id],
            )?;
        }
        regroup_compilations(&tx, scan.folder_id)?;
    }
    remove_orphans(&tx)?;
    restore_kept(&tx)?;
    tx.commit()?;

    for (index, scan) in scans {
        results[index] = Ok(scan.report);
    }
    for (index, root, state, gone) in held {
        results[index] = Err(Error::Invalid(crate::coded::folder_unavailable(
            &root.to_string_lossy(),
            state.code(),
            Some(&format!("{gone} tracks not found")),
        )));
    }
    Ok(results)
}

/// A folder walked, and what its scan found so far.
struct FolderScan {
    folder_id: i64,
    root: PathBuf,
    /// Readable until the scan ends.
    _open: OpenFolder,
    report: ScanReport,
    pending: Vec<Pending>,
    /// Tracks the folder had before the scan.
    known: usize,
    /// Audio files the walk found.
    found: usize,
}

/// Opens and walks one folder: its new and changed files, and (into the
/// pool, later) the tracks whose files weren't found.
fn walk_folder(
    conn: &Connection,
    folder_id: i64,
    options: ScanOptions,
) -> Result<(FolderScan, Vec<(String, Vec<Known>)>), Error> {
    let folder = open_folder(conn, folder_id)?;
    let root = folder.path.clone();
    if !root.is_dir() {
        return Err(Error::Invalid(crate::coded::folder_unavailable(
            &root.to_string_lossy(),
            FolderState::Missing.code(),
            None,
        )));
    }

    let mut report = ScanReport {
        folder_id,
        ..ScanReport::default()
    };
    let mut known = known_tracks(conn, folder_id)?;
    let known_count = known.values().map(Vec::len).sum();
    let (pending, unreadable) = walk(&root, &mut known, options, &mut report);
    let found = pending.len() + (known_count - known.values().map(Vec::len).sum::<usize>());

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

    Ok((
        FolderScan {
            folder_id,
            root,
            _open: folder,
            report,
            pending,
            known: known_count,
            found,
        },
        missing,
    ))
}

/// Reads the folder's new and changed files a batch at a time and writes
/// them, a new file taking over a missing one's rows where it matches one.
fn read_and_write(
    conn: &mut Connection,
    scan: &mut FolderScan,
    options: ScanOptions,
    pool: &mut MissingPool,
    progress: &mut impl FnMut(ScanProgress),
) -> Result<(), Error> {
    let folder_id = scan.folder_id;
    let to_read = scan.pending.len();
    progress(ScanProgress {
        folder_id,
        read: 0,
        to_read,
    });
    let mut read = 0;
    for batch in scan.pending.chunks(BATCH_SIZE) {
        let all_read = read_all(batch, options);
        let tx = conn.transaction()?;
        let now = unix_now();
        for (file, result) in batch.iter().zip(all_read) {
            match result {
                Ok(read) => {
                    let moved = if file.known.is_empty() {
                        pool.take_match(file, &read.tags)
                    } else {
                        None
                    };
                    match moved {
                        Some(parts) => {
                            for part in &parts {
                                tx.prepare_cached(
                                    "UPDATE tracks SET folder_id = ?1, relative_path = ?2
                                     WHERE id = ?3",
                                )?
                                .execute(params![
                                    folder_id,
                                    file.relative,
                                    part.id
                                ])?;
                            }
                            let mut counts = ScanReport::default();
                            write_file(&tx, folder_id, file, &parts, &read, now, &mut counts)?;
                            scan.report.moved += counts.updated;
                            scan.report.added += counts.added;
                            scan.report.removed += counts.removed;
                        }
                        None => write_file(
                            &tx,
                            folder_id,
                            file,
                            &file.known,
                            &read,
                            now,
                            &mut scan.report,
                        )?,
                    }
                }
                Err(error) => {
                    // A file that went during the scan (its drive unmounted)
                    // keeps its tracks until a scan finds it gone.
                    if file.path.exists() {
                        for part in &file.known {
                            tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                                .execute([part.id])?;
                            scan.report.removed += 1;
                        }
                    } else {
                        scan.report.unchanged += file.known.len();
                    }
                    scan.report.failed.push(ScanFailure {
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
    Ok(())
}

/// A file whose tracks weren't found where they were, as the library
/// remembers it.
struct MissingFile {
    folder_id: i64,
    parts: Vec<Known>,
    mtime_ns: i64,
    recording_id: Option<String>,
    /// Of its first track, as shown: lower case, trimmed.
    title: String,
    artist: String,
    album: String,
    /// Of the whole file (its parts together), in seconds.
    duration: f64,
    /// Taken over by a new file.
    matched: bool,
}

/// The files the walks didn't find, by size, for new files to match.
struct MissingPool {
    by_size: HashMap<i64, Vec<MissingFile>>,
}

impl MissingPool {
    /// The missing files of each folder, as the library has them.
    fn load(
        conn: &Connection,
        missing: Vec<(i64, Vec<(String, Vec<Known>)>)>,
    ) -> Result<MissingPool, Error> {
        let mut by_size: HashMap<i64, Vec<MissingFile>> = HashMap::new();
        let mut identity = conn.prepare(
            "SELECT t.musicbrainz_recording_id, t.title, IFNULL(t.artist_credit, artist.name),
                    album.title,
                    (SELECT sum(duration) FROM tracks p
                     WHERE p.folder_id = t.folder_id AND p.relative_path = t.relative_path)
             FROM tracks t
             LEFT JOIN artists artist ON artist.id = t.artist_id
             LEFT JOIN albums album ON album.id = t.album_id
             WHERE t.id = ?1",
        )?;
        for (folder_id, files) in missing {
            for (_, parts) in files {
                let Some(first) = parts.first() else {
                    continue;
                };
                let (recording_id, title, artist, album, duration): (
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    Option<f64>,
                ) = identity.query_row([first.id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                })?;
                by_size.entry(first.size).or_default().push(MissingFile {
                    folder_id,
                    mtime_ns: first.mtime_ns,
                    recording_id,
                    title: normal(title.as_deref()),
                    artist: normal(artist.as_deref()),
                    album: normal(album.as_deref()),
                    duration: duration.unwrap_or(0.0),
                    parts,
                    matched: false,
                });
            }
        }
        Ok(MissingPool { by_size })
    }

    /// The parts of the missing file that `file` (read as `tags`) is, if
    /// any: the same size, and the same modification time or recording
    /// MBID, or else the same length (within a second), title, artist and
    /// album. A file matches once.
    fn take_match(&mut self, file: &Pending, tags: &Tags) -> Option<Vec<Known>> {
        let candidates = self.by_size.get_mut(&file.size)?;
        let title = normal(tags.title.as_deref());
        let artist = normal(tags.artist.as_deref());
        let album = normal(tags.album.as_deref());
        let same = |missing: &MissingFile| {
            if missing.matched {
                return false;
            }
            if missing.mtime_ns > 0 && missing.mtime_ns == file.mtime_ns {
                return true;
            }
            if let (Some(a), Some(b)) = (&missing.recording_id, &tags.musicbrainz_recording_id) {
                return a.eq_ignore_ascii_case(b);
            }
            (missing.duration - tags.duration).abs() < 1.0
                && missing.title == title
                && missing.artist == artist
                && missing.album == album
        };
        // The one with the same tags first, where several could be it.
        let index = candidates
            .iter()
            .position(|m| same(m) && m.title == title && m.artist == artist)
            .or_else(|| candidates.iter().position(same))?;
        let found = &mut candidates[index];
        found.matched = true;
        Some(found.parts.clone())
    }

    /// The track ids of `folder_id`'s missing files that no new file took
    /// over.
    fn unmatched(&self, folder_id: i64) -> Vec<i64> {
        self.by_size
            .values()
            .flatten()
            .filter(|missing| missing.folder_id == folder_id && !missing.matched)
            .flat_map(|missing| missing.parts.iter().map(|part| part.id))
            .collect()
    }
}

/// Text compared case-insensitively and ignoring surrounding spaces.
fn normal(text: Option<&str>) -> String {
    text.unwrap_or("").trim().to_lowercase()
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
    let cores = std::thread::available_parallelism().map_or(4, |count| count.get());
    let workers = if options.background {
        (cores / 2).max(1)
    } else {
        cores
    }
    .min(files.len());
    let mut results: Vec<Option<Result<ReadFile, String>>> = files.iter().map(|_| None).collect();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    if options.background {
                        super::watch::lower_priority();
                    }
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
    known: &[Known],
    read: &ReadFile,
    now: i64,
    report: &mut ScanReport,
) -> rusqlite::Result<()> {
    let mut kept = Vec::new();
    for part in &read.parts {
        let id = write_track(tx, folder_id, file, &read.tags, part, now)?;
        kept.push(id);
        if known.iter().any(|known| known.id == id) {
            report.updated += 1;
        } else {
            report.added += 1;
        }
    }
    for known in known {
        if !kept.contains(&known.id) {
            tx.prepare_cached("DELETE FROM tracks WHERE id = ?1")?
                .execute([known.id])?;
            report.removed += 1;
        }
    }
    Ok(())
}

/// Upserts one part of a file with its credited artists and the rating its
/// tags give; returns its id.
fn write_track(
    tx: &Transaction,
    folder_id: i64,
    file: &Pending,
    tags: &Tags,
    part: &Part,
    now: i64,
) -> rusqlite::Result<i64> {
    let composer_id = artist_id(tx, tags.composer.as_deref(), None)?;

    // The credited artists: the cue sheet's performer (which overrides the
    // tags), else the ARTISTS tag's, else the artist tag's, split at ';'.
    let credit_text = part.performer.as_deref().or(tags.artist.as_deref());
    let names: Vec<String> = match (&part.performer, tags.artists.is_empty()) {
        (Some(performer), _) => split_artists(performer),
        (None, false) => tags.artists.clone(),
        (None, true) => tags
            .artist
            .as_deref()
            .map(split_artists)
            .unwrap_or_default(),
    };
    // Each artist's MBID, where the tags give one per credited artist.
    let mbids: Vec<String> = if part.performer.is_none() {
        tags.musicbrainz_artist_id
            .as_deref()
            .map(split_artists)
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let mut artist_ids = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let mbid = (mbids.len() == names.len()).then(|| mbids[index].as_str());
        let id = artist_id_for(tx, name, mbid)?;
        if !artist_ids.contains(&id) {
            artist_ids.push(id);
        }
    }
    let artist_id = artist_ids.first().copied();
    // The tag's text, where it says more than the first artist's name.
    let artist_credit = credit_text
        .map(str::trim)
        .filter(|text| names.len() != 1 || *text != names[0]);

    // The cue sheet's album and its artist override the tags; a file with
    // no album artist is filed under its artist, or "Various Artists" if it
    // is marked as a compilation (see also `regroup_compilations`).
    let album_artist_name = part
        .album_artist
        .as_deref()
        .or(tags.album_artist.as_deref());
    let album_artist_id = match album_artist_name {
        Some(name) => Some(artist_id_for(
            tx,
            name,
            part.album_artist
                .is_none()
                .then_some(tags.musicbrainz_album_artist_id.as_deref())
                .flatten()
                .filter(|mbid| !mbid.contains(';')),
        )?),
        None if tags.compilation => Some(various_artists(tx)?),
        None => artist_id,
    };
    let album_id = part
        .album
        .as_deref()
        .or(tags.album.as_deref())
        .map(|title| {
            album_id(
                tx,
                title,
                album_artist_id,
                tags.musicbrainz_release_id.as_deref(),
                tags.musicbrainz_release_group_id.as_deref(),
            )
        })
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
    let id: i64 = tx
        .prepare_cached(
            "INSERT INTO tracks (
             folder_id, relative_path, range_start, range_end, file_size, file_mtime_ns, title,
             artist_id, album_id, album_artist_id, genre, track_number, track_total, disc_number,
             disc_total, year, duration, sample_rate, channels, bitrate_kbps,
             musicbrainz_recording_id, musicbrainz_release_track_id, scanned_at,
             replaygain_track_gain, replaygain_track_peak, replaygain_album_gain,
             replaygain_album_peak, added_at, release_date, work, movement_name, movement_number,
             movement_total, composer_id, conductor, artist_credit, compilation,
             album_artist_tagged)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                 ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?23, ?28, ?29, ?30, ?31, ?32, ?33,
                 ?34, ?35, ?36, ?37)
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
             conductor = excluded.conductor,
             artist_credit = excluded.artist_credit,
             compilation = excluded.compilation,
             album_artist_tagged = excluded.album_artist_tagged
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
                artist_credit,
                tags.compilation,
                album_artist_name.is_some(),
            ],
            |row| row.get(0),
        )?;

    tx.prepare_cached("DELETE FROM track_artists WHERE track_id = ?1")?
        .execute([id])?;
    for (position, artist) in artist_ids.iter().enumerate() {
        tx.prepare_cached(
            "INSERT INTO track_artists (track_id, artist_id, position) VALUES (?1, ?2, ?3)",
        )?
        .execute(params![id, artist, position as i64])?;
    }

    // A rating in the file's tags seeds the track's, and follows the tags
    // until the user rates it (PLAN.md F3). A file's rating is the whole
    // file's, not its parts'.
    if let (Some(rating), None) = (tags.rating, part.number) {
        let stars = ((u32::from(rating) + 10) / 20).clamp(1, 5);
        tx.prepare_cached(
            "INSERT INTO track_ratings (track_id, rating, source) VALUES (?1, ?2, 'tags')
             ON CONFLICT (track_id) DO UPDATE SET rating = excluded.rating
             WHERE track_ratings.source = 'tags'",
        )?
        .execute(params![id, stars])?;
    }
    Ok(id)
}

/// The "Various Artists" artist, created if needed.
fn various_artists(tx: &Transaction) -> rusqlite::Result<i64> {
    artist_id_for(tx, VARIOUS_ARTISTS, Some(VARIOUS_ARTISTS_MBID))
}

/// Files a folder's tracks that have no album artist tag and aren't marked
/// as compilations: under "Various Artists" where their album title has
/// `COMPILATION_ARTISTS` or more track artists in one folder of the
/// library folder, else under their own artist. Moves only tracks whose
/// filing changes.
fn regroup_compilations(tx: &Transaction, folder_id: i64) -> rusqlite::Result<()> {
    struct Row {
        id: i64,
        relative: String,
        artist_id: Option<i64>,
        album_artist_id: Option<i64>,
        title: String,
        release_id: Option<String>,
        release_group_id: Option<String>,
    }
    let rows: Vec<Row> = tx
        .prepare_cached(
            "SELECT t.id, t.relative_path, t.artist_id, t.album_artist_id, album.title,
                    album.musicbrainz_release_id, album.musicbrainz_release_group_id
             FROM tracks t JOIN albums album ON album.id = t.album_id
             WHERE t.folder_id = ?1 AND t.album_artist_tagged = 0 AND t.compilation = 0",
        )?
        .query_map([folder_id], |row| {
            Ok(Row {
                id: row.get(0)?,
                relative: row.get(1)?,
                artist_id: row.get(2)?,
                album_artist_id: row.get(3)?,
                title: row.get(4)?,
                release_id: row.get(5)?,
                release_group_id: row.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut groups: HashMap<(&str, String), Vec<&Row>> = HashMap::new();
    for row in &rows {
        let dir = row.relative.rsplit_once('/').map_or("", |(dir, _)| dir);
        groups
            .entry((dir, row.title.to_lowercase()))
            .or_default()
            .push(row);
    }
    let mut various = None;
    for group in groups.values() {
        let artists: HashSet<i64> = group.iter().filter_map(|row| row.artist_id).collect();
        let compilation = artists.len() >= COMPILATION_ARTISTS;
        for row in group {
            let wanted = if compilation {
                Some(match various {
                    Some(id) => id,
                    None => *various.insert(various_artists(tx)?),
                })
            } else {
                row.artist_id
            };
            if wanted == row.album_artist_id {
                continue;
            }
            let album = album_id(
                tx,
                &row.title,
                wanted,
                row.release_id.as_deref(),
                row.release_group_id.as_deref(),
            )?;
            tx.prepare_cached(
                "UPDATE tracks SET album_artist_id = ?1, album_id = ?2 WHERE id = ?3",
            )?
            .execute(params![wanted, album, row.id])?;
        }
    }
    Ok(())
}

/// Gives albums and artists that came back the picks the user made for
/// them before they went (`kept_albums`, `kept_artists`, filled by
/// `library::remove_orphans`), unless they have picks of their own now, and
/// forgets picks kept for over a year.
pub(super) fn restore_kept(tx: &Transaction) -> rusqlite::Result<()> {
    type KeptAlbum = (i64, String, Option<String>, Option<String>, String);
    let albums: Vec<KeptAlbum> = tx
        .prepare_cached("SELECT id, title, artist, musicbrainz_release_id, picks FROM kept_albums")?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    for (kept_id, title, artist, release_id, picks) in albums {
        let album: Option<i64> = tx
            .prepare_cached(
                "SELECT al.id FROM albums al LEFT JOIN artists ar ON ar.id = al.artist_id
                 WHERE ((?3 IS NOT NULL AND al.musicbrainz_release_id = ?3)
                        OR (al.title = ?1 AND ar.name IS ?2))
                   AND NOT EXISTS (SELECT 1 FROM album_links
                                   WHERE album_id = al.id AND chosen_by = 'user')
                   AND NOT EXISTS (SELECT 1 FROM album_art WHERE album_id = al.id)
                   AND NOT EXISTS (SELECT 1 FROM album_prefs WHERE album_id = al.id)
                   AND NOT EXISTS (SELECT 1 FROM album_favourites WHERE album_id = al.id)
                 ORDER BY al.musicbrainz_release_id IS ?3 DESC, al.id LIMIT 1",
            )?
            .query_row(params![title, artist, release_id], |row| row.get(0))
            .optional()?;
        if let Some(album_id) = album {
            let picks: serde_json::Value = serde_json::from_str(&picks).unwrap_or_default();
            restore_album_picks(tx, album_id, &picks)?;
            tx.execute("DELETE FROM kept_albums WHERE id = ?1", [kept_id])?;
        }
    }

    let artists: Vec<(String, String)> = tx
        .prepare_cached("SELECT name, picks FROM kept_artists")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (name, picks) in artists {
        let artist: Option<i64> = tx
            .prepare_cached(
                "SELECT a.id FROM artists a WHERE a.name = ?1
                   AND NOT EXISTS (SELECT 1 FROM artist_links
                                   WHERE artist_id = a.id AND chosen_by = 'user')
                   AND NOT EXISTS (SELECT 1 FROM artist_favourites WHERE artist_id = a.id)",
            )?
            .query_row([&name], |row| row.get(0))
            .optional()?;
        if let Some(artist_id) = artist {
            let picks: serde_json::Value = serde_json::from_str(&picks).unwrap_or_default();
            for link in picks["links"].as_array().into_iter().flatten() {
                tx.prepare_cached(
                    "INSERT INTO artist_links (artist_id, source, status, external_id, score,
                                               chosen_by, details, checked_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6, ?7)
                     ON CONFLICT (artist_id, source) DO UPDATE SET
                         status = excluded.status, external_id = excluded.external_id,
                         score = excluded.score, chosen_by = 'user', details = excluded.details,
                         checked_at = excluded.checked_at",
                )?
                .execute(params![
                    artist_id,
                    link["source"].as_str(),
                    link["status"].as_str(),
                    link["externalId"].as_str(),
                    link["score"].as_f64().unwrap_or(1.0),
                    link["details"].as_str(),
                    link["checkedAt"].as_i64().unwrap_or(0),
                ])?;
            }
            if let Some(added_at) = picks["favourite"].as_i64() {
                tx.execute(
                    "INSERT OR IGNORE INTO artist_favourites (artist_id, added_at) VALUES (?1, ?2)",
                    params![artist_id, added_at],
                )?;
            }
            tx.execute("DELETE FROM kept_artists WHERE name = ?1", [&name])?;
        }
    }

    const YEAR: i64 = 365 * 24 * 60 * 60;
    tx.execute(
        "DELETE FROM kept_albums WHERE kept_at < ?1",
        [unix_now() - YEAR],
    )?;
    tx.execute(
        "DELETE FROM kept_artists WHERE kept_at < ?1",
        [unix_now() - YEAR],
    )?;
    Ok(())
}

/// Writes an album's kept picks (see `library::remove_orphans`).
fn restore_album_picks(
    tx: &Transaction,
    album_id: i64,
    picks: &serde_json::Value,
) -> rusqlite::Result<()> {
    for link in picks["links"].as_array().into_iter().flatten() {
        tx.prepare_cached(
            "INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                      details, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6, ?7)
             ON CONFLICT (album_id, source) DO UPDATE SET
                 status = excluded.status, external_id = excluded.external_id,
                 score = excluded.score, chosen_by = 'user', details = excluded.details,
                 checked_at = excluded.checked_at",
        )?
        .execute(params![
            album_id,
            link["source"].as_str(),
            link["status"].as_str(),
            link["externalId"].as_str(),
            link["score"].as_f64().unwrap_or(1.0),
            link["details"].as_str(),
            link["checkedAt"].as_i64().unwrap_or(0),
        ])?;
    }
    if let Some(source) = picks["art"]["source"].as_str() {
        tx.execute(
            "INSERT OR REPLACE INTO album_art (album_id, source, reference) VALUES (?1, ?2, ?3)",
            params![album_id, source, picks["art"]["reference"].as_str()],
        )?;
    }
    let prefs = &picks["prefs"];
    if prefs.is_object() {
        tx.execute(
            "INSERT OR REPLACE INTO album_prefs (album_id, skip, never_shuffle, gain_offset)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                album_id,
                prefs["skip"].as_i64(),
                prefs["neverShuffle"].as_i64(),
                prefs["gainOffset"].as_f64()
            ],
        )?;
    }
    if let Some(added_at) = picks["favourite"].as_i64() {
        tx.execute(
            "INSERT OR IGNORE INTO album_favourites (album_id, added_at) VALUES (?1, ?2)",
            params![album_id, added_at],
        )?;
    }
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
    release_id: Option<&str>,
    release_group_id: Option<&str>,
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
        params![title, artist_id, release_id, release_group_id],
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

    const PARTS: ScanOptions = ScanOptions {
        parts: true,
        background: false,
        remove_missing: false,
    };

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
        assert!(crate::coded::is(&error, "folderUnavailable"), "{error}");
        assert_eq!(library.tracks().len(), 3);
    }

    #[test]
    fn a_folder_that_reads_as_empty_keeps_its_tracks() {
        let mut library = sample_library();
        library.scan();
        let ids: Vec<i64> = library.tracks().iter().map(|track| track.id).collect();
        crate::library::playlists::create(&mut library.conn, "Mine", None, &ids).unwrap();

        // An unmounted share leaves an empty folder at its path.
        fs::remove_dir_all(&library.root).unwrap();
        fs::create_dir(&library.root).unwrap();
        let error = scan_folder(&mut library.conn, library.folder_id, PARTS, |_| {}).unwrap_err();
        assert_eq!(
            FolderState::of_error(&error),
            Some(FolderState::Empty),
            "{error}"
        );
        assert_eq!(
            library
                .tracks()
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(library.count("playlist_items"), 3);

        // Told to, the scan removes them.
        let options = ScanOptions {
            remove_missing: true,
            ..PARTS
        };
        let report = scan_folder(&mut library.conn, library.folder_id, options, |_| {}).unwrap();
        assert_eq!(report.removed, 3);
        assert!(library.tracks().is_empty());
    }

    #[test]
    fn a_scan_that_would_remove_most_of_a_folder_keeps_it() {
        let mut library = Library::new(&[]);
        for i in 0..HOLD_MIN_TRACKS {
            library.copy("wav-s16-44k.wav", &format!("{i:02}.wav"));
        }
        library.scan();
        // Half can go: a user tidying up.
        for i in 0..HOLD_MIN_TRACKS / 2 {
            fs::remove_file(library.path(&format!("{i:02}.wav"))).unwrap();
        }
        assert_eq!(library.scan().removed, HOLD_MIN_TRACKS / 2);
        assert!(!holds(HOLD_MIN_TRACKS, HOLD_MIN_TRACKS / 2));
        // Losing one of two is fine below the minimum; all of them isn't.
        assert!(!holds(2, 1));
        assert!(holds(2, 2));
        assert!(holds(HOLD_MIN_TRACKS, HOLD_MIN_TRACKS / 2 + 1));

        // Most of what's left goes at once, a file still there.
        let mut library = Library::new(&[]);
        for i in 0..HOLD_MIN_TRACKS {
            library.copy("wav-s16-44k.wav", &format!("{i:02}.wav"));
        }
        library.scan();
        for i in 1..HOLD_MIN_TRACKS {
            fs::remove_file(library.path(&format!("{i:02}.wav"))).unwrap();
        }
        let error = scan_folder(&mut library.conn, library.folder_id, PARTS, |_| {}).unwrap_err();
        assert_eq!(
            FolderState::of_error(&error),
            Some(FolderState::MostlyGone),
            "{error}"
        );
        assert_eq!(library.tracks().len(), HOLD_MIN_TRACKS);
    }

    #[test]
    fn files_moved_to_another_folder_are_not_held() {
        let mut library =
            Library::new(&[("flac-44k.flac", "a.flac"), ("wav-s16-44k.wav", "b.wav")]);
        let other_root = library._dir.path().join("Other");
        fs::create_dir(&other_root).unwrap();
        let other = add_folder(&library.conn, &other_root).unwrap();
        let both = [library.folder_id, other.id];
        scan_folders(&mut library.conn, &both, PARTS, |_| {}).unwrap();
        let ids: Vec<i64> = library.tracks().iter().map(|track| track.id).collect();

        // Every file of the first folder moves to the second.
        for name in ["a.flac", "b.wav"] {
            fs::rename(library.path(name), other_root.join(name)).unwrap();
        }
        let results = scan_folders(&mut library.conn, &both, PARTS, |_| {}).unwrap();
        assert!(results.iter().all(Result::is_ok));
        let mut after: Vec<i64> = library.tracks().iter().map(|track| track.id).collect();
        after.sort_unstable();
        assert_eq!(after, ids);
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

    fn id_of(library: &Library, relative: &str) -> i64 {
        library
            .conn
            .query_row(
                "SELECT id FROM tracks WHERE relative_path = ?1",
                [relative],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn a_renamed_file_keeps_its_track_and_what_hangs_on_it() {
        let mut library = sample_library();
        library.scan();
        let id = id_of(&library, "Loose/untitled.wav");
        library
            .conn
            .execute_batch(&format!(
                "INSERT INTO track_favourites (track_id, added_at) VALUES ({id}, 1);
                 INSERT INTO plays (track_id, played_at, seconds) VALUES ({id}, 1, 10);"
            ))
            .unwrap();

        fs::create_dir_all(library.path("Elsewhere")).unwrap();
        fs::rename(
            library.path("Loose/untitled.wav"),
            library.path("Elsewhere/renamed.wav"),
        )
        .unwrap();
        let report = library.scan();
        assert_eq!(
            (report.moved, report.added, report.removed, report.unchanged),
            (1, 0, 0, 2)
        );
        assert_eq!(id_of(&library, "Elsewhere/renamed.wav"), id);
        assert_eq!(library.count("track_favourites"), 1);
        assert_eq!(library.count("plays"), 1);
    }

    #[test]
    fn a_copied_and_renamed_file_matches_by_its_tags() {
        let mut library = sample_library();
        library.scan();
        let from = format!("{ALBUM_DIR}/03 Café.flac");
        let id = id_of(&library, &from);
        // A copy has a new modification time: the tags and length tell.
        library.copy("tagged-vorbis.flac", "Copied/Café.flac");
        touch(&library.path("Copied/Café.flac"));
        fs::remove_file(library.path(&from)).unwrap();
        let report = library.scan();
        assert_eq!((report.moved, report.added, report.removed), (1, 0, 0));
        assert_eq!(id_of(&library, "Copied/Café.flac"), id);

        // A different file is no move: one goes, the other comes.
        fs::remove_file(library.path("Loose/untitled.wav")).unwrap();
        library.copy("wav-mono-48k.wav", "Loose/other.wav");
        let report = library.scan();
        assert_eq!((report.moved, report.added, report.removed), (0, 1, 1));
    }

    #[test]
    fn a_file_moved_to_another_folder_keeps_its_track() {
        let mut library = sample_library();
        let second = library.root.with_file_name("More music");
        fs::create_dir(&second).unwrap();
        let second_id = add_folder(&library.conn, &second).unwrap().id;
        let both = [library.folder_id, second_id];
        scan_folders(&mut library.conn, &both, PARTS, |_| {}).unwrap();
        let id = id_of(&library, "Loose/untitled.wav");

        fs::rename(
            library.path("Loose/untitled.wav"),
            second.join("untitled.wav"),
        )
        .unwrap();
        let reports = scan_folders(&mut library.conn, &both, PARTS, |_| {}).unwrap();
        let reports: Vec<ScanReport> = reports.into_iter().map(Result::unwrap).collect();
        assert_eq!((reports[0].removed, reports[1].moved), (0, 1));
        let (folder, relative): (i64, String) = library
            .conn
            .query_row(
                "SELECT folder_id, relative_path FROM tracks WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((folder, relative.as_str()), (second_id, "untitled.wav"));

        // A folder that can't be opened fails alone.
        fs::remove_dir_all(&second).unwrap();
        let reports = scan_folders(&mut library.conn, &both, PARTS, |_| {}).unwrap();
        assert!(reports[0].is_ok());
        assert!(reports[1]
            .as_ref()
            .is_err_and(|e| crate::coded::is(e, "folderUnavailable")));
    }

    #[test]
    fn an_album_that_comes_back_gets_the_users_picks_back() {
        let mut library = sample_library();
        library.scan();
        let album: i64 = library
            .conn
            .query_row("SELECT id FROM albums", [], |row| row.get(0))
            .unwrap();
        library
            .conn
            .execute_batch(&format!(
                "INSERT INTO album_art (album_id, source, reference) VALUES ({album}, 'folder', 'a.jpg');
                 INSERT INTO album_links (album_id, source, status, external_id, score, chosen_by,
                                          checked_at)
                 VALUES ({album}, 'musicbrainz', 'matched', 'rel', 1, 'user', 5);
                 INSERT INTO album_favourites (album_id, added_at) VALUES ({album}, 9);
                 INSERT INTO artist_favourites (artist_id, added_at)
                 SELECT artist_id, 3 FROM albums;"
            ))
            .unwrap();

        // Removing the folder keeps the picks aside; adding it back restores them.
        let path = library.root.clone();
        remove_folder(&mut library.conn, library.folder_id).unwrap();
        assert_eq!(
            (library.count("albums"), library.count("kept_albums")),
            (0, 1)
        );
        assert_eq!(library.count("kept_artists"), 1);
        library.folder_id = add_folder(&library.conn, &path).unwrap().id;
        library.scan();
        assert_eq!(library.count("kept_albums"), 0);
        let (source, reference, chosen, favourite): (String, String, String, i64) = library
            .conn
            .query_row(
                "SELECT art.source, art.reference, links.chosen_by, fav.added_at
                 FROM album_art art JOIN album_links links ON links.album_id = art.album_id
                 JOIN album_favourites fav ON fav.album_id = art.album_id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            (
                source.as_str(),
                reference.as_str(),
                chosen.as_str(),
                favourite
            ),
            ("folder", "a.jpg", "user", 9)
        );
        assert_eq!(library.count("artist_favourites"), 1);
        assert_eq!(library.count("kept_artists"), 0);
    }

    /// Writes one synthetic file's track straight from `tags`.
    fn write(library: &mut Library, relative: &str, tags: &Tags) -> i64 {
        let file = Pending {
            relative: relative.into(),
            path: library.path(relative),
            size: 1,
            mtime_ns: 1,
            known: Vec::new(),
            cue: None,
        };
        let tx = library.conn.transaction().unwrap();
        let id = write_track(&tx, library.folder_id, &file, tags, &Part::whole(), 0).unwrap();
        regroup_compilations(&tx, library.folder_id).unwrap();
        remove_orphans(&tx).unwrap();
        tx.commit().unwrap();
        id
    }

    fn tags(title: &str, artist: &str, album: &str) -> Tags {
        Tags {
            title: Some(title.into()),
            artist: Some(artist.into()),
            album: Some(album.into()),
            duration: 1.0,
            sample_rate: 44100,
            channels: 2,
            ..Tags::default()
        }
    }

    fn credited(library: &Library, id: i64) -> (Vec<String>, Option<String>, String) {
        let names: Vec<String> = library
            .conn
            .prepare(
                "SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
                 WHERE ta.track_id = ?1 ORDER BY ta.position",
            )
            .unwrap()
            .query_map([id], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let (credit, first): (Option<String>, String) = library
            .conn
            .query_row(
                "SELECT t.artist_credit, a.name FROM tracks t JOIN artists a ON a.id = t.artist_id
                 WHERE t.id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        (names, credit, first)
    }

    #[test]
    fn an_artist_tag_may_credit_several_artists() {
        let mut library = Library::new(&[]);
        let id = write(
            &mut library,
            "a.flac",
            &tags("Duet", "Singer; Rapper", "Songs"),
        );
        assert_eq!(
            credited(&library, id),
            (
                vec!["Singer".into(), "Rapper".into()],
                Some("Singer; Rapper".into()),
                "Singer".into()
            )
        );

        // A multi-valued ARTISTS tag names them; the artist tag is the credit.
        let mut both = tags("Duet", "Singer feat. Rapper", "Songs");
        both.artists = vec!["Singer".into(), "Rapper".into()];
        let id = write(&mut library, "b.flac", &both);
        assert_eq!(
            credited(&library, id).1.as_deref(),
            Some("Singer feat. Rapper")
        );

        // One artist: no credit of its own.
        let id = write(&mut library, "c.flac", &tags("Solo", "Singer", "Songs"));
        assert_eq!(
            credited(&library, id),
            (vec!["Singer".into()], None, "Singer".into())
        );
        // "A; B" as an artist of its own doesn't exist.
        let joined: i64 = library
            .conn
            .query_row(
                "SELECT count(*) FROM artists WHERE name LIKE '%;%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(joined, 0);
        // The search index finds the track by its whole credit.
        let found: i64 = library
            .conn
            .query_row(
                "SELECT count(*) FROM tracks_search WHERE tracks_search MATCH 'artist : feat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(found, 1);
    }

    fn album_artists(library: &Library) -> Vec<(String, String)> {
        column::<String>(
            library,
            "SELECT al.title || ' / ' || IFNULL(ar.name, '-') FROM tracks t
             JOIN albums al ON al.id = t.album_id LEFT JOIN artists ar ON ar.id = t.album_artist_id
             ORDER BY t.relative_path",
        )
        .into_iter()
        .map(|text| {
            let (album, artist) = text.split_once(" / ").unwrap();
            (album.to_owned(), artist.to_owned())
        })
        .collect()
    }

    #[test]
    fn compilations_are_one_album_under_various_artists() {
        let mut library = Library::new(&[]);
        // Marked as a compilation: at once.
        let mut marked = tags("One", "A", "Marked");
        marked.compilation = true;
        write(&mut library, "m/1.flac", &marked);
        // Three artists under one title in one folder, no album artist.
        write(&mut library, "hits/1.flac", &tags("One", "A", "Hits"));
        write(&mut library, "hits/2.flac", &tags("Two", "B", "Hits"));
        assert_eq!(
            album_artists(&library),
            [
                ("Hits".into(), "A".into()),
                ("Hits".into(), "B".into()),
                ("Marked".into(), VARIOUS_ARTISTS.into())
            ],
            "two artists aren't enough"
        );
        write(&mut library, "hits/3.flac", &tags("Three", "C", "Hits"));
        let found = album_artists(&library);
        assert!(found[..3]
            .iter()
            .all(|(album, artist)| album == "Hits" && artist == VARIOUS_ARTISTS));
        let hits: i64 = library
            .conn
            .query_row(
                "SELECT count(*) FROM albums WHERE title = 'Hits'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1);

        // The same title in another folder is another album; an album
        // artist tag is kept.
        let mut tagged = tags("Four", "D", "Hits");
        tagged.album_artist = Some("D".into());
        write(&mut library, "other/1.flac", &tagged);
        assert_eq!(album_artists(&library)[4], ("Hits".into(), "D".into()));

        // Fewer than three again: back under their own artists.
        library
            .conn
            .execute("DELETE FROM tracks WHERE relative_path = 'hits/3.flac'", [])
            .unwrap();
        write(&mut library, "hits/2.flac", &tags("Two", "B", "Hits"));
        assert_eq!(album_artists(&library)[0], ("Hits".into(), "A".into()));
    }

    #[test]
    fn ratings_come_from_the_tags_until_the_user_rates() {
        let mut library = Library::new(&[]);
        let rating = |library: &Library, id: i64| -> (Option<i64>, String) {
            library
                .conn
                .query_row(
                    "SELECT rating, source FROM track_ratings WHERE track_id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        };
        let mut rated = tags("One", "A", "X");
        rated.rating = Some(80);
        let id = write(&mut library, "a.flac", &rated);
        assert_eq!(rating(&library, id), (Some(4), "tags".into()));
        rated.rating = Some(1);
        write(&mut library, "a.flac", &rated);
        assert_eq!(rating(&library, id), (Some(1), "tags".into()));

        library
            .conn
            .execute(
                "UPDATE track_ratings SET rating = NULL, source = 'user' WHERE track_id = ?1",
                [id],
            )
            .unwrap();
        rated.rating = Some(100);
        write(&mut library, "a.flac", &rated);
        assert_eq!(
            rating(&library, id),
            (None, "user".into()),
            "cleared stays cleared"
        );
    }
}
