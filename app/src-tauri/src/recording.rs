//! Recording what is played to a file (PLAN.md X6): the settings, the
//! recordings' folder, starting and stopping, and the cue sheets that let a
//! recording reopen as tracks.
//!
//! The core writes the file (`anomp_engine_record_start`): what is heard,
//! effects, equaliser and crossfeed included, before the volume. A change
//! of the device's sample rate starts another file, "name 2.wav"; a write
//! error or a full disk stops the recording (`Event::RecordingFailed`).
//!
//! The folder is picked once with the folder picker and kept as a
//! read-write security-scoped bookmark in the `settings` table (its own
//! row, not `AppSettings`, so a data export never carries it), resolved
//! for as long as a recording is written. Files are named by date and
//! time, never by title. While a recording runs, the tracks that become
//! current are noted, and when it stops each file gets a cue sheet
//! beside it (if the settings ask for one) naming the tracks at the
//! core's marks.
//!
//! Behind the `recording` feature switch, off by default: the files are
//! large (about 23 MB a minute as 48 kHz float). Logs (H9): counts and
//! the format at info, the file name at debug.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Runtime};

use crate::anomp::{self, FolderAccess, RecordingFormat, RecordingKind};
use crate::audio::{engine_mut, on_main, with_engine};
use crate::coded::coded;
use crate::library::commands::on_library;
use crate::queue::model::{QueueState, Uid};
use crate::settings::{self, FeatureSettings};

/// Frontend event with the new `RecordingState`, whenever a recording
/// starts or stops.
pub const RECORDING_EVENT: &str = "recording";
/// Frontend event with a `RecordingStopped`, when a recording stopped on
/// an error rather than at the user's request.
pub const RECORDING_FAILED_EVENT: &str = "recording-failed";

/// The `settings` row holding the recordings' folder and its bookmark.
const FOLDER_KEY: &str = "recording.folder";

/// The recording's format (`AppSettings.recording`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct RecordingSettings {
    pub format: RecordingKind,
    /// For WAV, AIFF, FLAC and Apple Lossless: 16, 24, or 32 (float, WAV
    /// only; the others record 24 then).
    pub bits: u32,
    /// For AAC and MP3: 96 to 320 kbps.
    pub bitrate_kbps: u32,
    /// Writes a cue sheet beside each file, naming its tracks.
    pub cue_sheet: bool,
}

impl Default for RecordingSettings {
    fn default() -> Self {
        RecordingSettings {
            format: RecordingKind::Wav,
            bits: 32,
            bitrate_kbps: 256,
            cue_sheet: true,
        }
    }
}

impl RecordingSettings {
    pub fn validate(&self) -> Result<(), String> {
        if ![16, 24, 32].contains(&self.bits) {
            return Err("Recordings' samples must be 16-bit, 24-bit or 32-bit".into());
        }
        if !(96..=320).contains(&self.bitrate_kbps) {
            return Err("A recording's bitrate must be 96 to 320 kbps".into());
        }
        Ok(())
    }

    /// The format the core is asked for.
    pub fn format(&self) -> RecordingFormat {
        let bits = if self.format == RecordingKind::Wav {
            self.bits
        } else {
            self.bits.min(24)
        };
        RecordingFormat {
            kind: self.format,
            bits,
            bitrate_kbps: self.bitrate_kbps,
        }
    }
}

/// Whether a recording runs, and where recordings go.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecordingState {
    pub recording: bool,
    /// Seconds recorded so far (the last recording's, once it stopped).
    pub seconds: f64,
    /// Times the file fell behind and samples were dropped.
    pub overruns: u64,
    /// The name of the file being written, or last written.
    pub file_name: Option<String>,
    /// The folder recordings go in, once picked.
    pub folder: Option<String>,
    /// The formats this build can write.
    pub formats: Vec<RecordingKind>,
}

/// A recording that has stopped.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecordingStopped {
    /// The files written (more than one if the sample rate changed), by name.
    pub files: Vec<String>,
    pub seconds: f64,
    pub overruns: u64,
    /// Why it stopped, if not at the user's request: a coded error.
    pub error: Option<String>,
}

/// The folder as stored: where it was picked, and its bookmark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredFolder {
    path: String,
    bookmark: String,
}

/// A track that became current while recording: the first mark it can be
/// at, and what the cue sheet calls it.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    first_mark: usize,
    title: String,
    performer: Option<String>,
}

/// The recording running now. Main thread.
struct Session {
    /// Holds the folder open (the sandbox) while files are written.
    _access: FolderAccess,
    first: PathBuf,
    cue_sheet: bool,
    entries: Vec<Entry>,
    current: Option<Uid>,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

pub(crate) fn require<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if settings::current(app).features.recording {
        Ok(())
    } else {
        Err(crate::coded::feature_off("recording", "Recording"))
    }
}

fn load_folder(conn: &Connection) -> Result<Option<StoredFolder>, rusqlite::Error> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [FOLDER_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.and_then(|json| serde_json::from_str(&json).ok()))
}

fn store_folder(conn: &Connection, folder: &StoredFolder) -> Result<(), rusqlite::Error> {
    let json = serde_json::to_string(folder).expect("folder serializes");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [FOLDER_KEY, &json],
    )?;
    Ok(())
}

fn stored(path: &Path, bookmark: &[u8]) -> StoredFolder {
    StoredFolder {
        path: path.to_string_lossy().into_owned(),
        bookmark: base64::engine::general_purpose::STANDARD.encode(bookmark),
    }
}

/// Resolves the stored folder's bookmark, refreshing a stale one.
fn open_folder(conn: &Connection, folder: &StoredFolder) -> Result<FolderAccess, String> {
    let unavailable = |detail: &str| {
        coded(
            "recordingFolderUnavailable",
            &[("path", json!(folder.path)), ("detail", json!(detail))],
            format!("The recordings' folder can't be opened: {detail}"),
        )
    };
    let bookmark = base64::engine::general_purpose::STANDARD
        .decode(&folder.bookmark)
        .map_err(|e| unavailable(&e.to_string()))?;
    let access = FolderAccess::start(&bookmark).map_err(|e| unavailable(&e))?;
    if !access.path().is_dir() {
        return Err(unavailable("not a folder"));
    }
    if access.is_stale() {
        match anomp::create_writable_bookmark(access.path()) {
            Ok(fresh) => {
                if let Err(error) = store_folder(conn, &stored(access.path(), &fresh)) {
                    log::warn!("cannot store the recordings' folder: {error}");
                }
            }
            Err(error) => log::warn!("cannot refresh the recordings' folder's bookmark: {error}"),
        }
    }
    Ok(access)
}

/// The local time now, as (year, month, day, hour, minute, second).
fn local_now() -> (i32, u32, u32, u32, u32, u32) {
    let now = libc::time_t::try_from(crate::library::unix_now()).unwrap_or(0);
    // SAFETY: an all-zero `tm` is valid, and localtime_r only writes it.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: both pointers are valid for the call.
    unsafe { libc::localtime_r(&now, &mut tm) };
    let part = |value: libc::c_int| u32::try_from(value).unwrap_or(0);
    (
        tm.tm_year + 1900,
        part(tm.tm_mon + 1),
        part(tm.tm_mday),
        part(tm.tm_hour),
        part(tm.tm_min),
        part(tm.tm_sec),
    )
}

/// "ano-mp 2026-10-04 21.15.03": a recording's name, by when it began.
fn file_stem((year, month, day, hour, minute, second): (i32, u32, u32, u32, u32, u32)) -> String {
    format!("ano-mp {year:04}-{month:02}-{day:02} {hour:02}.{minute:02}.{second:02}")
}

/// A file in `folder` for a recording begun at `when`, not already there.
fn new_file(folder: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut path = folder.join(format!("{stem}.{extension}"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{stem} ({n}).{extension}"));
        n += 1;
    }
    path
}

/// The `index`th file (0-based) of a recording to `first`, named as the
/// core names them (`Recorder::numberedFile`): "name.wav", "name 2.wav".
fn numbered(first: &Path, index: u32) -> PathBuf {
    if index == 0 {
        return first.to_path_buf();
    }
    let stem = first.file_stem().unwrap_or_default().to_string_lossy();
    let name = match first.extension() {
        Some(extension) => format!("{stem} {}.{}", index + 1, extension.to_string_lossy()),
        None => format!("{stem} {}", index + 1),
    };
    first.with_file_name(name)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A cue sheet's position, mm:ss:ff (75 frames a second).
fn cue_time(seconds: f64) -> String {
    let frames = (seconds.max(0.0) * 75.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        frames / 75 / 60,
        frames / 75 % 60,
        frames % 75
    )
}

fn cue_text(text: &str) -> String {
    text.replace('"', "'").replace(['\r', '\n'], " ")
}

/// The cue sheet for each file of a recording that has tracks: `marks` are
/// the core's (file, seconds), `entries` the tracks noted, each from the
/// mark it began at. A file after the first that starts mid-track begins
/// with that track.
fn cue_sheets(
    files: &[PathBuf],
    marks: &[(u32, f64)],
    entries: &[Entry],
) -> Vec<(PathBuf, String)> {
    let entry_at = |mark: usize| entries.iter().rev().find(|entry| entry.first_mark <= mark);
    let mut sheets = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let mut starts: Vec<(f64, Option<&Entry>)> = marks
            .iter()
            .enumerate()
            .filter(|(_, (in_file, _))| *in_file == index)
            .map(|(mark, &(_, seconds))| (seconds, entry_at(mark)))
            .collect();
        let before = marks.iter().rposition(|(in_file, _)| *in_file < index);
        if starts.first().is_none_or(|(seconds, _)| *seconds > 0.0) {
            if let Some(mark) = before {
                starts.insert(0, (0.0, entry_at(mark)));
            }
        }
        if starts.is_empty() {
            continue;
        }
        let kind = match file.extension().and_then(|e| e.to_str()) {
            Some("aiff") => "AIFF",
            Some("mp3") => "MP3",
            _ => "WAVE",
        };
        let mut text = format!(
            "REM COMMENT \"ano-mp\"\nFILE \"{}\" {kind}\n",
            cue_text(&file_name(file))
        );
        for (number, (seconds, entry)) in starts.iter().enumerate() {
            text += &format!("  TRACK {:02} AUDIO\n", number + 1);
            if let Some(entry) = entry {
                text += &format!("    TITLE \"{}\"\n", cue_text(&entry.title));
                if let Some(performer) = &entry.performer {
                    text += &format!("    PERFORMER \"{}\"\n", cue_text(performer));
                }
            }
            text += &format!("    INDEX 01 {}\n", cue_time(*seconds));
        }
        sheets.push((file.with_extension("cue"), text));
    }
    sheets
}

/// What the engine says about the recording, with the folder and formats.
fn state_of(engine: &anomp::Engine, folder: Option<String>) -> RecordingState {
    let status = engine.recording();
    RecordingState {
        recording: status.recording,
        seconds: status.seconds,
        overruns: status.overruns,
        file_name: status.file.as_deref().map(file_name),
        folder,
        formats: RecordingKind::ALL
            .into_iter()
            .filter(|kind| kind.available())
            .collect(),
    }
}

async fn stored_folder<R: Runtime>(app: &AppHandle<R>) -> Result<Option<StoredFolder>, String> {
    on_library(app, |library| {
        load_folder(&library.conn()).map_err(crate::library::Error::from)
    })
    .await
}

fn emit_state<R: Runtime>(app: &AppHandle<R>, state: &RecordingState) {
    let _ = app.emit(RECORDING_EVENT, state);
    crate::shell::recording_changed(app, state.recording);
}

/// Stops the engine's recording, if any, and writes its cue sheets. Main
/// thread; `error` is why it stopped, if it failed.
fn finish<R: Runtime>(app: &AppHandle<R>, error: Option<String>) -> Option<RecordingStopped> {
    let session = SESSION.with_borrow_mut(Option::take)?;
    let (status, marks, state) = engine_mut(|engine| {
        engine.record_stop();
        (
            engine.recording(),
            engine.recording_marks(),
            state_of(engine, None),
        )
    })
    .ok()?;
    let files: Vec<PathBuf> = (0..status.files.max(1))
        .map(|index| numbered(&session.first, index))
        .collect();
    if session.cue_sheet {
        for (path, text) in cue_sheets(&files, &marks, &session.entries) {
            if let Err(error) = std::fs::write(&path, text) {
                log::warn!("cannot write a cue sheet: {error}");
            }
        }
    }
    log::info!(
        "recording stopped: {} file(s), {:.0} s, {} overruns, {} tracks{}",
        files.len(),
        status.seconds,
        status.overruns,
        marks.len(),
        if error.is_some() { ", failed" } else { "" }
    );
    // Only now let go of the folder: the cue sheets are in it.
    drop(session);
    let stopped = RecordingStopped {
        files: files.iter().map(|path| file_name(path)).collect(),
        seconds: status.seconds,
        overruns: status.overruns,
        error,
    };
    if stopped.error.is_some() {
        let _ = app.emit(RECORDING_FAILED_EVENT, &stopped);
    }
    let mut state = state;
    state.folder = None; // The UI keeps the folder it has.
    emit_state(app, &state);
    crate::workbench::recording_finished(app);
    Some(stopped)
}

// ---- Hooks ------------------------------------------------------------------

/// Whether a recording runs. Main thread.
pub fn running() -> bool {
    SESSION.with_borrow(Option::is_some)
}

/// The engine stopped the recording on an error. Main thread.
pub fn failed<R: Runtime>(app: &AppHandle<R>, disk_full: bool, message: &str) {
    log::warn!(
        "recording failed{}",
        if disk_full { ": disk full" } else { "" }
    );
    let reason = if disk_full { "diskFull" } else { "writeFailed" };
    let error = coded(
        "recordingFailed",
        &[("reason", json!(reason)), ("detail", json!(message))],
        format!("The recording stopped: {message}"),
    );
    finish(app, Some(error));
}

/// The queue changed: notes a track that has become current, for the cue
/// sheet. Main thread, inside the queue's turn there, so the marks are
/// read once it's over.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let Some(item) = state.current_item.as_ref().filter(|_| state.loaded) else {
        return;
    };
    let noted = SESSION.with_borrow_mut(|session| match session {
        Some(session) if session.current != Some(item.uid) => {
            session.current = Some(item.uid);
            true
        }
        _ => false,
    });
    if !noted {
        return;
    }
    let (title, performer) = (item.track.title.clone(), item.track.artist.clone());
    let _ = app.run_on_main_thread(move || {
        let Ok(marks) = engine_mut(|engine| engine.recording_marks().len()) else {
            return;
        };
        SESSION.with_borrow_mut(|session| {
            if let Some(session) = session {
                session.entries.push(Entry {
                    first_mark: marks.saturating_sub(1),
                    title,
                    performer,
                });
            }
        });
    });
}

/// The features changed: turning recording off stops it.
pub fn features_changed<R: Runtime>(app: &AppHandle<R>, features: &FeatureSettings) {
    let on = features.recording;
    let app = app.clone();
    let _ = on_main(&app.clone(), move || {
        if !on {
            finish(&app, None);
        }
        let recording = SESSION.with_borrow(Option::is_some);
        crate::shell::recording_changed(&app, recording);
    });
}

/// The app is quitting: finishes the recording before the engine goes.
/// Main thread.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    finish(app, None);
}

/// Starts or stops recording, for the menu. Main thread.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let recording = SESSION.with_borrow(Option::is_some);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = if recording {
            recording_stop(app.clone()).await.map(|_| ())
        } else {
            recording_start(app.clone()).await.map(|_| ())
        };
        if let Err(error) = result {
            log::warn!("record: {error}");
            let failed = RecordingStopped {
                files: Vec::new(),
                seconds: 0.0,
                overruns: 0,
                error: Some(error),
            };
            let _ = app.emit(RECORDING_FAILED_EVENT, &failed);
        }
    });
    Ok(())
}

// ---- Commands ---------------------------------------------------------------

/// Whether a recording runs, its progress, and the folder and formats.
#[tauri::command]
pub async fn recording_status<R: Runtime>(app: AppHandle<R>) -> Result<RecordingState, String> {
    let folder = stored_folder(&app).await?.map(|folder| folder.path);
    with_engine(&app, move |engine| state_of(engine, folder))
}

/// Keeps `path`, a folder the user just picked, as the recordings' folder.
#[tauri::command]
pub async fn recording_set_folder<R: Runtime>(
    app: AppHandle<R>,
    path: String,
) -> Result<RecordingState, String> {
    let folder = PathBuf::from(&path);
    let bookmark = anomp::create_writable_bookmark(&folder)?;
    let row = stored(&folder, &bookmark);
    on_library(&app, move |library| {
        store_folder(&library.conn(), &row).map_err(crate::library::Error::from)
    })
    .await?;
    log::info!("recordings' folder chosen");
    log::debug!("recordings go in {path}");
    recording_status(app).await
}

/// Starts recording what is played into the recordings' folder.
#[tauri::command]
pub async fn recording_start<R: Runtime>(app: AppHandle<R>) -> Result<RecordingState, String> {
    require(&app)?;
    let settings = settings::current(&app).recording;
    let (access, folder) = on_library(&app, |library| {
        let conn = library.conn();
        let folder = load_folder(&conn)?.ok_or_else(|| {
            crate::library::Error::Invalid(coded(
                "recordingNoFolder",
                &[],
                "Choose a folder for recordings in Settings › Recording",
            ))
        })?;
        let access = open_folder(&conn, &folder).map_err(crate::library::Error::Invalid)?;
        Ok((access, folder.path))
    })
    .await?;
    let format = settings.format();
    let path = new_file(
        access.path(),
        &file_stem(local_now()),
        format.kind.extension(),
    );
    let queue = crate::queue::queue_state(app.clone())?;
    let first = queue
        .current_item
        .as_ref()
        .filter(|_| queue.loaded)
        .map(|item| {
            (
                item.uid,
                item.track.title.clone(),
                item.track.artist.clone(),
            )
        });

    let state = with_engine(&app, move |engine| {
        if SESSION.with_borrow(Option::is_some) {
            return Err(coded("recordingRunning", &[], "Already recording"));
        }
        engine.record_start(&path, &format).map_err(|detail| {
            coded(
                "recordingStartFailed",
                &[("detail", json!(detail))],
                format!("Can't start recording: {detail}"),
            )
        })?;
        let mut session = Session {
            _access: access,
            first: path.clone(),
            cue_sheet: settings.cue_sheet,
            entries: Vec::new(),
            current: None,
        };
        if let Some((uid, title, performer)) = first {
            session.current = Some(uid);
            session.entries.push(Entry {
                first_mark: 0,
                title,
                performer,
            });
        }
        SESSION.with_borrow_mut(|slot| *slot = Some(session));
        log::info!("recording started ({:?})", format.kind);
        log::debug!("recording to {}", path.display());
        Ok(state_of(engine, Some(folder)))
    })??;
    emit_state(&app, &state);
    Ok(state)
}

/// Stops recording; returns what was written.
#[tauri::command]
pub async fn recording_stop<R: Runtime>(app: AppHandle<R>) -> Result<RecordingStopped, String> {
    let handle = app.clone();
    on_main(&app, move || finish(&handle, None))?
        .ok_or_else(|| coded("recordingNotRunning", &[], "Not recording"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_by_when_they_began() {
        assert_eq!(
            file_stem((2026, 10, 4, 21, 15, 3)),
            "ano-mp 2026-10-04 21.15.03"
        );
        let dir = std::env::temp_dir().join(format!("anomp-recording-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = new_file(&dir, "ano-mp 2026-10-04 21.15.03", "wav");
        assert_eq!(file_name(&first), "ano-mp 2026-10-04 21.15.03.wav");
        std::fs::write(&first, b"").unwrap();
        let second = new_file(&dir, "ano-mp 2026-10-04 21.15.03", "wav");
        assert_eq!(file_name(&second), "ano-mp 2026-10-04 21.15.03 (2).wav");
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(numbered(&first, 0), first);
        assert_eq!(
            file_name(&numbered(&first, 1)),
            "ano-mp 2026-10-04 21.15.03 2.wav"
        );
        let (year, ..) = local_now();
        assert!(year >= 2026);
    }

    #[test]
    fn settings_check_their_ranges_and_fit_the_format() {
        let mut settings = RecordingSettings::default();
        settings.validate().unwrap();
        assert_eq!(settings.format().bits, 32);
        settings.format = RecordingKind::Flac;
        assert_eq!(settings.format().bits, 24, "only WAV records float");
        settings.bits = 20;
        assert!(settings.validate().unwrap_err().contains("16-bit"));
        settings.bits = 16;
        settings.bitrate_kbps = 500;
        assert!(settings.validate().unwrap_err().contains("bitrate"));

        // Stored values are read leniently.
        let read: RecordingSettings = serde_json::from_str(r#"{"format":"mp3"}"#).unwrap();
        assert_eq!(read.format, RecordingKind::Mp3);
        assert_eq!(read.bitrate_kbps, 256);
    }

    #[test]
    fn every_format_is_available_with_its_extension() {
        for kind in RecordingKind::ALL {
            assert!(kind.available(), "{kind:?}");
        }
        let extensions: Vec<&str> = RecordingKind::ALL.iter().map(|k| k.extension()).collect();
        assert_eq!(extensions, ["wav", "aiff", "flac", "m4a", "m4a", "mp3"]);
    }

    fn entry(first_mark: usize, title: &str) -> Entry {
        Entry {
            first_mark,
            title: title.into(),
            performer: Some("Band \"X\"".into()),
        }
    }

    #[test]
    fn cue_sheets_name_the_tracks_at_their_marks() {
        let first = PathBuf::from("/r/ano-mp 2026-10-04 21.15.03.flac");
        let files = vec![first.clone(), numbered(&first, 1)];
        // Three tracks; the third played twice (repeat one), and the rate
        // changed during it.
        let marks = [(0, 0.0), (0, 61.5), (0, 120.0), (0, 300.25), (1, 10.0)];
        let entries = [
            entry(0, "One"),
            entry(1, "Two"),
            entry(2, "Three"),
            entry(4, "Four"),
        ];
        let sheets = cue_sheets(&files, &marks, &entries);
        assert_eq!(sheets.len(), 2);
        assert_eq!(
            sheets[0].0,
            PathBuf::from("/r/ano-mp 2026-10-04 21.15.03.cue")
        );
        let text = &sheets[0].1;
        assert!(text.contains("FILE \"ano-mp 2026-10-04 21.15.03.flac\" WAVE"));
        assert!(text.contains("PERFORMER \"Band 'X'\""));
        assert!(text.contains("INDEX 01 01:01:38"));

        // The app's own reader takes it back as the tracks.
        let sheet = crate::library::cue::parse(text).unwrap();
        let parts = sheet.parts_for("ano-mp 2026-10-04 21.15.03.flac");
        let titles: Vec<_> = parts.iter().map(|p| p.title.clone().unwrap()).collect();
        assert_eq!(titles, ["One", "Two", "Three", "Three"]);
        assert!((parts[3].start - 300.25).abs() < 1.0 / 75.0);

        // The second file begins mid-track.
        let sheet = crate::library::cue::parse(&sheets[1].1).unwrap();
        let parts = sheet.parts_for("ano-mp 2026-10-04 21.15.03 2.flac");
        let titles: Vec<_> = parts.iter().map(|p| p.title.clone().unwrap()).collect();
        assert_eq!(titles, ["Three", "Four"]);
        assert_eq!((parts[0].start, parts[1].start), (0.0, 10.0));
    }

    #[test]
    fn a_recording_without_tracks_has_no_cue_sheet() {
        let files = vec![PathBuf::from("/r/a.wav")];
        assert!(cue_sheets(&files, &[], &[]).is_empty());
        // Marks the queue didn't name are tracks without titles.
        let sheets = cue_sheets(&files, &[(0, 0.0), (0, 5.0)], &[]);
        assert_eq!(sheets[0].1.matches("TRACK").count(), 2);
        assert!(!sheets[0].1.contains("TITLE"));
        assert_eq!(cue_time(3599.995), "60:00:00");
        assert_eq!(cue_time(3599.98), "59:59:74");
    }
}
