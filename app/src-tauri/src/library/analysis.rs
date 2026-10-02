//! The loudness analysis (PLAN.md O1) and what the same pass finds for
//! the waveform seek bar (O2), segues and silences (O3) and the health
//! report (O4): a background thread decodes each track once with the
//! core's analyser (its own decoder, never the engine's) and keeps the
//! results in `track_analysis`, and each album's loudness, gated over its
//! tracks, in `album_analysis`.
//!
//! The whole library is analysed only while "loudness analysis" is on,
//! a few tracks at a time, resuming where it stopped. The track playing is
//! analysed ahead of that, and even with the analysis off while the
//! waveform seek bar is on, so its waveform appears. A track whose file
//! changes (size or modification time) is analysed again.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::access::{open_folder_of, FolderState};
use super::commands::LibraryState;
use super::{db, track_path, unix_now, Error};
use crate::anomp::{self, FileAnalysis};
use crate::settings::{self, FeatureSettings};

/// Frontend event with an `AnalysisProgress` payload.
pub const ANALYSIS_PROGRESS_EVENT: &str = "analysis-progress";
/// Frontend event with the ids of tracks just analysed.
pub const ANALYSIS_CHANGED_EVENT: &str = "analysis-changed";

/// Progress is reported at most this often.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

/// Waveform points are bytes: -127..=127 for -1..=1.
const ENVELOPE_SCALE: f32 = 127.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    /// Whether the whole library is being analysed.
    pub running: bool,
    /// Tracks analysed (or found undecodable) and still to do.
    pub done: u32,
    pub remaining: u32,
    /// Tracks the analysis couldn't decode.
    pub failed: u32,
}

/// A track's analysis, as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackAnalysis {
    pub error: Option<String>,
    pub duration: Option<f64>,
    /// LUFS.
    pub loudness: Option<f64>,
    /// dB to ReplayGain's reference (-18 LUFS).
    pub gain: Option<f64>,
    pub true_peak: Option<f64>,
    pub album_loudness: Option<f64>,
    pub leading_silence: Option<f64>,
    pub trailing_silence: Option<f64>,
    pub gap_start: Option<f64>,
    pub gap_length: Option<f64>,
    pub cutoff_hz: Option<f64>,
}

// ---- Storage ------------------------------------------------------------------

/// A histogram as stored: 16-bit little-endian counts (saturating), with
/// trailing empty bins left off.
pub fn encode_histogram(bins: &[u32]) -> Vec<u8> {
    let used = bins
        .iter()
        .rposition(|&count| count > 0)
        .map_or(0, |last| last + 1);
    bins[..used]
        .iter()
        .flat_map(|&count| (count.min(u32::from(u16::MAX)) as u16).to_le_bytes())
        .collect()
}

pub fn decode_histogram(bytes: &[u8]) -> Vec<u32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u32::from(u16::from_le_bytes(pair)))
        .collect()
}

/// Integrated loudness over the blocks of `histogram` (0.5 LU bins from
/// -70 LUFS), gated as BS.1770 does: every block counted is above the
/// absolute gate; those more than 10 LU below their mean are left out.
/// Each bin counts at its centre, which is within 0.01 LU on real music.
pub fn gated_loudness(histogram: &[u32], floor: f64, step: f64) -> Option<f64> {
    let energy = |bin: usize| 10f64.powf((floor + (bin as f64 + 0.5) * step + 0.691) / 10.0);
    let loudness = |energy: f64| -0.691 + 10.0 * energy.log10();
    let (mut sum, mut count) = (0.0, 0u64);
    for (bin, &n) in histogram.iter().enumerate() {
        sum += energy(bin) * f64::from(n);
        count += u64::from(n);
    }
    if count == 0 {
        return None;
    }
    let threshold = loudness(sum / count as f64) - 10.0;
    let (mut sum, mut count) = (0.0, 0u64);
    for (bin, &n) in histogram.iter().enumerate() {
        if floor + (bin as f64 + 0.5) * step >= threshold {
            sum += energy(bin) * f64::from(n);
            count += u64::from(n);
        }
    }
    (count > 0).then(|| loudness(sum / count as f64))
}

/// The waveform as stored: a (min, max) byte pair per point.
fn encode_envelope(min: &[f32], max: &[f32]) -> Vec<u8> {
    let byte = |value: f32| ((value.clamp(-1.0, 1.0) * ENVELOPE_SCALE).round() as i8) as u8;
    min.iter()
        .zip(max)
        .flat_map(|(&low, &high)| [byte(low), byte(high)])
        .collect()
}

/// Stores `track_id`'s analysis (or why it failed) for the file as it is
/// in the library, and brings its album's loudness up to date.
pub fn store(
    conn: &Connection,
    track_id: i64,
    result: &Result<FileAnalysis, String>,
) -> Result<(), Error> {
    let file: Option<(i64, i64, Option<i64>)> = conn
        .query_row(
            "SELECT file_size, file_mtime_ns, album_id FROM tracks WHERE id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((size, mtime, album_id)) = file else {
        return Ok(()); // Removed meanwhile.
    };
    let now = unix_now();
    match result {
        Ok(analysis) => {
            conn.execute(
                "INSERT OR REPLACE INTO track_analysis (
                     track_id, file_size, file_mtime_ns, analysed_at, error, duration, loudness,
                     sample_peak, true_peak, histogram, leading_silence, trailing_silence,
                     gap_start, gap_length, start_level, end_level, cutoff_hz, envelope)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                         ?16, ?17)",
                params![
                    track_id,
                    size,
                    mtime,
                    now,
                    analysis.duration,
                    analysis.loudness,
                    analysis.sample_peak,
                    analysis.true_peak,
                    encode_histogram(&analysis.histogram),
                    analysis.leading_silence,
                    analysis.trailing_silence,
                    analysis.gap.map(|(start, _)| start),
                    analysis.gap.map(|(_, length)| length),
                    analysis.start_level_db,
                    analysis.end_level_db,
                    analysis.cutoff_hz,
                    encode_envelope(&analysis.envelope_min, &analysis.envelope_max),
                ],
            )?;
        }
        Err(error) => {
            conn.execute(
                "INSERT OR REPLACE INTO track_analysis (track_id, file_size, file_mtime_ns,
                                                        analysed_at, error)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![track_id, size, mtime, now, error],
            )?;
        }
    }
    if let Some(album_id) = album_id {
        update_album(conn, album_id)?;
    }
    Ok(())
}

/// Recomputes an album's loudness and peak from its tracks' analyses.
pub fn update_album(conn: &Connection, album_id: i64) -> Result<(), Error> {
    let mut statement = conn.prepare_cached(
        "SELECT a.histogram, a.true_peak FROM tracks t
         JOIN track_analysis a ON a.track_id = t.id AND a.error IS NULL
             AND a.file_size = t.file_size AND a.file_mtime_ns = t.file_mtime_ns
         WHERE t.album_id = ?1",
    )?;
    let rows: Vec<(Option<Vec<u8>>, Option<f64>)> = statement
        .query_map([album_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        conn.execute("DELETE FROM album_analysis WHERE album_id = ?1", [album_id])?;
        return Ok(());
    }
    let mut histogram: Vec<u32> = Vec::new();
    let mut peak: Option<f64> = None;
    for (bytes, track_peak) in &rows {
        for (bin, count) in decode_histogram(bytes.as_deref().unwrap_or(&[]))
            .into_iter()
            .enumerate()
        {
            if histogram.len() <= bin {
                histogram.resize(bin + 1, 0);
            }
            histogram[bin] += count;
        }
        peak = match (peak, *track_peak) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
    }
    conn.execute(
        "INSERT OR REPLACE INTO album_analysis (album_id, loudness, peak, tracks)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            album_id,
            gated_loudness(&histogram, HISTOGRAM_FLOOR, HISTOGRAM_STEP),
            peak,
            rows.len() as i64
        ],
    )?;
    Ok(())
}

/// The core's histogram bins (`anomp_file_analysis`).
pub const HISTOGRAM_FLOOR: f64 = -70.0;
pub const HISTOGRAM_STEP: f64 = 0.5;

/// Tracks still to analyse: never analysed, or their file changed since.
/// Tracks whose file hasn't been read since a migration (-1) wait for the
/// scan.
/// Tracks never analysed, changed since, or whose folder couldn't be read
/// when they were tried (stored by earlier versions; see `run`).
const NEEDS_ANALYSIS: &str = "FROM tracks t
     LEFT JOIN track_analysis a ON a.track_id = t.id
     WHERE t.file_mtime_ns >= 0
       AND (a.track_id IS NULL OR a.file_size != t.file_size OR a.file_mtime_ns != t.file_mtime_ns
            OR a.error LIKE '{\"code\":\"folderUnavailable\"%')";

/// An analysis error that is a folder out of reach, not a failure.
pub const NOT_FOLDER_ERROR: &str = "error NOT LIKE '{\"code\":\"folderUnavailable\"%'";

/// Up to `limit` tracks to analyse next, an album at a time, leaving out
/// the folders `skip` (out of reach for now).
pub fn next_tracks(conn: &Connection, limit: usize, skip: &[i64]) -> Result<Vec<i64>, Error> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT t.id {NEEDS_ANALYSIS}
           AND t.folder_id NOT IN (SELECT value FROM json_each(?2))
         ORDER BY t.album_id NULLS LAST, t.id LIMIT ?1"
    ))?;
    let skip = serde_json::to_string(skip).unwrap_or_else(|_| "[]".into());
    let ids = statement
        .query_map(params![limit as i64, skip], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

pub fn needs_analysis(conn: &Connection, track_id: i64) -> Result<bool, Error> {
    Ok(conn
        .query_row(
            &format!("SELECT 1 {NEEDS_ANALYSIS} AND t.id = ?1"),
            [track_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn counts(conn: &Connection) -> Result<(u32, u32, u32), Error> {
    let remaining: u32 =
        conn.query_row(&format!("SELECT count(*) {NEEDS_ANALYSIS}"), [], |row| {
            row.get(0)
        })?;
    let (done, failed): (u32, u32) = conn.query_row(
        &format!(
            "SELECT count(*), count(CASE WHEN {NOT_FOLDER_ERROR} THEN 1 END) FROM track_analysis"
        ),
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((done, remaining, failed))
}

/// The waveform of track `track_id` for the seek bar: (min, max) pairs,
/// -127..=127; `None` until it's analysed.
pub fn waveform(conn: &Connection, track_id: i64) -> Result<Option<Vec<i8>>, Error> {
    let envelope: Option<Option<Vec<u8>>> = conn
        .query_row(
            "SELECT a.envelope FROM track_analysis a JOIN tracks t ON t.id = a.track_id
             WHERE a.track_id = ?1 AND a.file_size = t.file_size
               AND a.file_mtime_ns = t.file_mtime_ns",
            [track_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(envelope
        .flatten()
        .map(|bytes| bytes.into_iter().map(|byte| byte as i8).collect()))
}

pub fn track_analysis(conn: &Connection, track_id: i64) -> Result<Option<TrackAnalysis>, Error> {
    Ok(conn
        .query_row(
            "SELECT a.error, a.duration, a.loudness, a.true_peak, aa.loudness,
                    a.leading_silence, a.trailing_silence, a.gap_start, a.gap_length, a.cutoff_hz
             FROM track_analysis a JOIN tracks t ON t.id = a.track_id
             LEFT JOIN album_analysis aa ON aa.album_id = t.album_id
             WHERE a.track_id = ?1 AND a.file_size = t.file_size
               AND a.file_mtime_ns = t.file_mtime_ns",
            [track_id],
            |row| {
                let loudness: Option<f64> = row.get(2)?;
                Ok(TrackAnalysis {
                    error: row.get(0)?,
                    duration: row.get(1)?,
                    loudness,
                    gain: loudness.map(|l| super::playback::REFERENCE_LUFS - l),
                    true_peak: row.get(3)?,
                    album_loudness: row.get(4)?,
                    leading_silence: row.get(5)?,
                    trailing_silence: row.get(6)?,
                    gap_start: row.get(7)?,
                    gap_length: row.get(8)?,
                    cutoff_hz: row.get(9)?,
                })
            },
        )
        .optional()?)
}

/// Analyses track `track_id` from its file (resolving its folder's bookmark
/// first), giving up when `keep_going` says so. A folder that can't be read
/// gives a `folderUnavailable` error, which isn't the file's fault.
pub fn analyse_track(
    conn: &Connection,
    track_id: i64,
    keep_going: &dyn Fn() -> bool,
) -> Result<Result<FileAnalysis, String>, Error> {
    let file: Option<(String, String, f64, Option<f64>)> = conn
        .query_row(
            "SELECT f.path, t.relative_path, t.range_start, t.range_end
             FROM tracks t JOIN folders f ON f.id = t.folder_id WHERE t.id = ?1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((folder, relative, start, end)) = file else {
        return Ok(Err(crate::coded::gone(crate::coded::Gone::Track)));
    };
    let path = track_path(Path::new(&folder), &relative);
    // Readable until analysed.
    let _folder = match open_folder_of(conn, &path) {
        Ok(folder) => folder,
        Err(error) => return Ok(Err(error.to_string())),
    };
    Ok(anomp::analyse_file(
        &path,
        start,
        end.unwrap_or(0.0),
        |_| keep_going(),
    ))
}

// ---- The worker ---------------------------------------------------------------

#[derive(Default)]
struct State {
    /// The whole library, or only what's asked for.
    background: bool,
    /// Tracks asked for, first come first.
    requested: VecDeque<i64>,
    /// Whether requests are served (the analysis or the waveform is on).
    serve_requests: bool,
    stop: bool,
    /// A new library pass: counts are read again.
    changed: bool,
    /// Folders found out of reach are tried again (a scan finished, or a
    /// folder came back).
    retry: bool,
}

/// The analysis thread's controls, managed by Tauri.
#[derive(Default)]
pub struct AnalysisWorker {
    state: Mutex<State>,
    wake: Condvar,
    progress: Mutex<AnalysisProgress>,
}

impl AnalysisWorker {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn configure(&self, features: &FeatureSettings) {
        let mut state = self.lock();
        state.background = features.loudness_analysis;
        state.serve_requests = features.loudness_analysis || features.waveform_seek_bar;
        state.changed = true;
        self.wake.notify_all();
    }

    fn request(&self, track_id: i64) {
        let mut state = self.lock();
        if !state.requested.contains(&track_id) {
            state.requested.push_back(track_id);
        }
        self.wake.notify_all();
    }

    fn keep_going(&self, background: bool) -> bool {
        let state = self.lock();
        !state.stop && (state.background || !background)
    }
}

/// How many tracks are analysed at once in the background: a few, leaving
/// most of the machine to everything else.
fn parallelism() -> usize {
    std::thread::available_parallelism()
        .map_or(1, |count| count.get() / 4)
        .clamp(1, 3)
}

/// Starts the analysis thread. Call after the library and the settings.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let library = app
        .try_state::<LibraryState>()
        .ok_or("The library is not available")?;
    let db_path = library.db_path().to_path_buf();
    let worker = Arc::new(AnalysisWorker::default());
    worker.configure(&settings::current(app).features);
    let thread_worker = worker.clone();
    let thread_app = app.clone();
    std::thread::Builder::new()
        .name("analysis".into())
        .spawn(move || run(&thread_app, &thread_worker, &db_path))
        .map_err(|e| format!("Cannot start the analysis: {e}"))?;
    app.manage(worker);
    Ok(())
}

fn run<R: Runtime>(app: &AppHandle<R>, worker: &AnalysisWorker, db_path: &Path) {
    let conn = match db::open(db_path) {
        Ok(conn) => conn,
        Err(error) => {
            log::warn!("{error}");
            return;
        }
    };
    let mut last_progress = Instant::now() - PROGRESS_INTERVAL;
    // Folders that couldn't be read: their tracks wait, "not now" rather
    // than failed, until a scan or a folder's return says to try again
    // (PLAN.md H22).
    let mut waiting: Vec<i64> = Vec::new();
    loop {
        // What to do next: a request, else a batch of the library, else wait.
        let (requested, background) = {
            let mut state = worker.lock();
            if std::mem::take(&mut state.retry) {
                waiting.clear();
            }
            loop {
                if state.stop {
                    return;
                }
                if state.serve_requests {
                    if let Some(id) = state.requested.pop_front() {
                        break (Some(id), false);
                    }
                }
                if state.background {
                    break (None, true);
                }
                state = worker.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        };

        let ids = match requested {
            Some(id) => match needs_analysis(&conn, id) {
                Ok(true) => vec![id],
                Ok(false) => continue,
                Err(error) => {
                    log::warn!("{error}");
                    continue;
                }
            },
            None => match next_tracks(&conn, parallelism(), &waiting) {
                Ok(ids) => ids,
                Err(error) => {
                    log::warn!("{error}");
                    Vec::new()
                }
            },
        };
        if ids.is_empty() {
            // The library is done: wait for a scan, a request or a change.
            report(app, worker, &conn, false);
            let mut state = worker.lock();
            if state.requested.is_empty() && !state.changed && !state.stop {
                let _ = worker.wake.wait_timeout(state, Duration::from_secs(600));
            } else {
                state.changed = false;
            }
            continue;
        }

        let keep_going = || worker.keep_going(background);
        let results: Vec<(i64, Result<Result<FileAnalysis, String>, Error>)> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = ids
                    .iter()
                    .map(|&id| {
                        let keep_going = &keep_going;
                        let db_path = db_path.to_path_buf();
                        scope.spawn(move || {
                            // A connection per thread, for the folder lookups.
                            let result = db::open(&db_path)
                                .and_then(|conn| analyse_track(&conn, id, keep_going));
                            (id, result)
                        })
                    })
                    .collect();
                handles
                    .into_iter()
                    .filter_map(|handle| handle.join().ok())
                    .collect()
            });

        let mut analysed = Vec::new();
        for (id, result) in results {
            match result {
                Ok(Err(error)) if error == "Cancelled" => {}
                Ok(Err(error)) if FolderState::of_error(&error).is_some() => {
                    let folder: Option<i64> = conn
                        .query_row("SELECT folder_id FROM tracks WHERE id = ?1", [id], |row| {
                            row.get(0)
                        })
                        .optional()
                        .ok()
                        .flatten();
                    if let Some(folder) = folder.filter(|folder| !waiting.contains(folder)) {
                        log::info!("folder {folder} can't be read; its tracks wait");
                        waiting.push(folder);
                    }
                }
                Ok(result) => match store(&conn, id, &result) {
                    Ok(()) => analysed.push(id),
                    Err(error) => log::warn!("{error}"),
                },
                Err(error) => log::warn!("{error}"),
            }
        }
        if !analysed.is_empty() {
            let _ = app.emit(ANALYSIS_CHANGED_EVENT, &analysed);
        }
        if last_progress.elapsed() >= PROGRESS_INTERVAL {
            last_progress = Instant::now();
            report(app, worker, &conn, background);
        }
    }
}

fn report<R: Runtime>(
    app: &AppHandle<R>,
    worker: &AnalysisWorker,
    conn: &Connection,
    running: bool,
) {
    let Ok((done, remaining, failed)) = counts(conn) else {
        return;
    };
    let progress = AnalysisProgress {
        running: running && remaining > 0,
        done,
        remaining,
        failed,
    };
    let mut current = worker.progress.lock().unwrap_or_else(|e| e.into_inner());
    if *current != progress {
        *current = progress.clone();
        let _ = app.emit(ANALYSIS_PROGRESS_EVENT, &progress);
    }
}

fn with_worker<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(&AnalysisWorker)) {
    if let Some(worker) = app.try_state::<Arc<AnalysisWorker>>() {
        f(&worker);
    }
}

/// The settings changed: start, stop or keep going.
pub fn configure<R: Runtime>(app: &AppHandle<R>, features: &FeatureSettings) {
    with_worker(app, |worker| worker.configure(features));
}

/// A scan finished: new tracks may need analysing.
pub fn library_changed<R: Runtime>(app: &AppHandle<R>) {
    with_worker(app, |worker| {
        let mut state = worker.lock();
        state.changed = true;
        state.retry = true;
        worker.wake.notify_all();
    });
}

/// The track playing: analysed ahead of the rest, for its waveform.
pub fn playing<R: Runtime>(app: &AppHandle<R>, track_id: i64) {
    with_worker(app, |worker| worker.request(track_id));
}

pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    with_worker(app, |worker| {
        worker.lock().stop = true;
        worker.wake.notify_all();
    });
}

pub fn progress<R: Runtime>(app: &AppHandle<R>) -> AnalysisProgress {
    app.try_state::<Arc<AnalysisWorker>>()
        .map(|worker| {
            worker
                .progress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    #[test]
    fn histograms_round_trip_and_gate_like_the_core() {
        let mut bins = vec![0u32; 150];
        bins[94] = 200; // -23 LUFS
        bins[68] = 100; // -36 LUFS: gated out
        let bytes = encode_histogram(&bins);
        assert_eq!(bytes.len(), 95 * 2);
        let decoded = decode_histogram(&bytes);
        assert_eq!(&decoded[..], &bins[..95]);
        let loudness = gated_loudness(&decoded, HISTOGRAM_FLOOR, HISTOGRAM_STEP).unwrap();
        assert!((loudness - -22.75).abs() < 0.01, "{loudness}");
        assert_eq!(gated_loudness(&[], HISTOGRAM_FLOOR, HISTOGRAM_STEP), None);
        // Counts saturate at 16 bits.
        assert_eq!(decode_histogram(&encode_histogram(&[70000])), [65535]);
    }

    #[test]
    fn a_folder_out_of_reach_is_not_a_failure() {
        let library = Library::new([track("a/1.flac"), track("a/2.flac")]);
        let conn = &library.conn;
        let ids = next_tracks(conn, 10, &[]).unwrap();
        // As an earlier version stored it.
        let unavailable = crate::coded::folder_unavailable("/Music", "missing", None);
        store(conn, ids[0], &Err(unavailable)).unwrap();
        assert_eq!(counts(conn).unwrap(), (1, 2, 0), "tried again, not failed");
        assert!(needs_analysis(conn, ids[0]).unwrap());
        // The worker leaves out the folders it found out of reach.
        let folder: i64 = conn
            .query_row(
                "SELECT folder_id FROM tracks WHERE id = ?1",
                [ids[0]],
                |row| row.get(0),
            )
            .unwrap();
        assert!(next_tracks(conn, 10, &[folder]).unwrap().is_empty());
        assert_eq!(next_tracks(conn, 10, &[folder + 1]).unwrap().len(), 2);
    }

    #[test]
    fn stores_analyses_and_keeps_album_loudness() {
        let library = Library::new([track("a/1.flac").album("A"), track("a/2.flac").album("A")]);
        let conn = &library.conn;
        let ids = next_tracks(conn, 10, &[]).unwrap();
        assert_eq!(ids.len(), 2);
        let analysis = |bin: usize| {
            let mut histogram = vec![0; 150];
            histogram[bin] = 100;
            Ok(FileAnalysis {
                duration: 60.0,
                sample_rate: 44100.0,
                loudness: Some(-70.0 + bin as f64 * 0.5 + 0.25),
                sample_peak: 0.5,
                true_peak: 0.6 + bin as f64 / 1000.0,
                histogram,
                histogram_floor: -70.0,
                histogram_step: 0.5,
                leading_silence: 0.0,
                trailing_silence: 2.0,
                gap: None,
                start_level_db: Some(-20.0),
                end_level_db: None,
                cutoff_hz: Some(16000.0),
                envelope_min: vec![-1.0, -0.5],
                envelope_max: vec![1.0, 0.25],
            })
        };
        store(conn, ids[0], &analysis(94)).unwrap();
        assert_eq!(next_tracks(conn, 10, &[]).unwrap(), [ids[1]]);
        store(conn, ids[1], &analysis(96)).unwrap();
        assert!(next_tracks(conn, 10, &[]).unwrap().is_empty());

        let (loudness, peak, tracks): (f64, f64, i64) = conn
            .query_row(
                "SELECT loudness, peak, tracks FROM album_analysis",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert!(loudness > -22.75 && loudness < -21.75, "{loudness}");
        assert!((peak - 0.696).abs() < 1e-9);
        assert_eq!(tracks, 2);

        assert_eq!(
            waveform(conn, ids[0]).unwrap(),
            Some(vec![-127, 127, -64, 32])
        );
        let details = track_analysis(conn, ids[0]).unwrap().unwrap();
        assert_eq!(details.cutoff_hz, Some(16000.0));
        assert!((details.gain.unwrap() - (-18.0 - -22.75)).abs() < 1e-9);

        // A failure is kept, so the file isn't tried again until it changes.
        store(conn, ids[1], &Err("Cannot decode".into())).unwrap();
        assert!(next_tracks(conn, 10, &[]).unwrap().is_empty());
        assert_eq!(counts(conn).unwrap(), (2, 0, 1));
        conn.execute("UPDATE tracks SET file_size = 99 WHERE id = ?1", [ids[1]])
            .unwrap();
        assert!(needs_analysis(conn, ids[1]).unwrap());
        assert_eq!(waveform(conn, ids[1]).unwrap(), None);
        // The album now covers only the track still analysed.
        update_album(conn, 1).unwrap();
        let tracks: i64 = conn
            .query_row("SELECT tracks FROM album_analysis", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tracks, 1);
    }

    #[test]
    fn analyses_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open_in_memory().unwrap();
        let root = dir.path().join("Music");
        std::fs::create_dir(&root).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures/flac-44k.flac"),
            root.join("a.flac"),
        )
        .unwrap();
        let folder = crate::library::add_folder(&conn, &root).unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, relative_path, file_size, file_mtime_ns, duration,
                                 sample_rate, channels, scanned_at)
             VALUES (?1, 'a.flac', 1, 1, 0.5, 44100, 2, 0)",
            [folder.id],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        let result = analyse_track(&conn, id, &|| true).unwrap();
        let analysis = result.as_ref().unwrap();
        assert!((analysis.duration - 22371.0 / 44100.0).abs() < 1e-6);
        store(&conn, id, &result).unwrap();
        assert_eq!(waveform(&conn, id).unwrap().unwrap().len(), 2000);

        let cancelled = analyse_track(&conn, id, &|| false).unwrap();
        assert_eq!(cancelled.unwrap_err(), "Cancelled");
    }
}
