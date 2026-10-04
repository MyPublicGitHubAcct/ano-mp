//! The sandboxed bundle's self-test (PLAN.md H14): `ano-mp --self-test`,
//! compiled in only with the `self-test` feature, checks what only a
//! sandboxed bundle shows and exits with a status, without opening a window
//! or touching the app's own files. `scripts/self-test-bundle.py` builds
//! such a bundle and runs it; CI runs that through `check-all.py`.
//!
//! It writes the core's audio fixtures (embedded in the binary, as the
//! sandbox can always write its own temporary folder) into a scratch folder
//! under the temporary directory (the container's when sandboxed), then:
//! adds that folder to a scratch library (a security-scoped bookmark), scans
//! it, resolves the bookmark, reads the tagged fixtures' covers, decodes
//! every file, and plays the two shortest tracks through a gapless hand-off
//! on the default output device at volume 0, then records a second of the
//! shortest into a folder reached through a writable bookmark (PLAN.md X6).
//! Each stage prints one line,
//! `self-test: <stage>: ok|skipped|FAILED: <detail>`; a stage that needs a
//! failed one is skipped. The scratch folder is removed at the end.
//!
//! `--require-sandbox` fails the run unless the app is sandboxed (the
//! bundle check passes it); `--require-audio` fails it when there is no
//! output device instead of skipping playback (CI's runners have none).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::anomp::{self, Engine};
use crate::library::art::{self, ArtKey};
use crate::library::commands::LibraryState;
use crate::library::scanner::{self, ScanOptions};
use crate::library::{self, access, db, playback};
use crate::settings::{FeatureSettings, PlaybackSettings};

pub const ARG: &str = "--self-test";

/// How long the gapless hand-off may take (the fixtures last a few seconds).
const HAND_OFF_TIMEOUT: Duration = Duration::from_secs(30);

macro_rules! fixtures {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../core/tests/fixtures/", $name
        )))),*]
    };
}

/// The core's decoder fixtures: every format the app plays, two with
/// embedded covers.
const FIXTURES: &[(&str, &[u8])] = fixtures![
    "aac-44k.m4a",
    "aac-adts-44k.aac",
    "aac-adts-long-44k.aac",
    "aac-long-44k.m4a",
    "aiff-s16-44k.aiff",
    "alac-44k.m4a",
    "flac-44k.flac",
    "flac-long-48k-mono.flac",
    "mp3-44k.mp3",
    "mp3-noheader-44k.mp3",
    "mp3-vbr-44k.mp3",
    "mp3-vbr-long-44k.mp3",
    "opus-48k.opus",
    "opus-long-48k.opus",
    "tagged-id3v23.mp3",
    "tagged-vorbis.flac",
    "vorbis-44k.ogg",
    "vorbis-long-44k.ogg",
    "wav-mono-48k.wav",
    "wav-s16-44k.wav",
    "wma-44k.wma",
];

/// The fixtures with an embedded cover.
const WITH_COVERS: &str = "tagged-";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Options {
    pub require_sandbox: bool,
    pub require_audio: bool,
    /// Play through the output device. Off in tests, which don't run on
    /// the main thread the engine needs.
    pub audio: bool,
}

impl Options {
    /// The options in the command line, if it asks for the self-test.
    pub fn from_args(args: &[String]) -> Option<Options> {
        if !args.iter().any(|arg| arg == ARG) {
            return None;
        }
        Some(Options {
            require_sandbox: args.iter().any(|arg| arg == "--require-sandbox"),
            require_audio: args.iter().any(|arg| arg == "--require-audio"),
            audio: true,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Passed(String),
    Skipped(String),
    Failed(String),
}

impl Outcome {
    pub fn line(&self, stage: &str) -> String {
        match self {
            Outcome::Passed(detail) => format!("self-test: {stage}: ok: {detail}"),
            Outcome::Skipped(detail) => format!("self-test: {stage}: skipped: {detail}"),
            Outcome::Failed(detail) => format!("self-test: {stage}: FAILED: {detail}"),
        }
    }
}

/// Runs the self-test if the command line asks for it, returning the exit
/// status; `None` to start the app as usual. Call on the main thread.
#[cfg_attr(not(feature = "self-test"), allow(dead_code))]
pub fn run_if_asked() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = Options::from_args(&args)?;
    let dir = std::env::temp_dir().join(format!("ano-mp-self-test-{}", std::process::id()));
    let passed = run(&dir, options, &mut |stage, outcome| {
        println!("{}", outcome.line(stage));
    });
    let _ = std::fs::remove_dir_all(&dir);
    println!("self-test: {}", if passed { "passed" } else { "FAILED" });
    Some(if passed { 0 } else { 1 })
}

/// Runs every stage in `dir` (created, and left for the caller to remove),
/// reporting each; returns whether none failed.
pub fn run(dir: &Path, options: Options, report: &mut dyn FnMut(&str, &Outcome)) -> bool {
    let mut passed = true;
    let mut stage = |name: &str, outcome: Outcome| {
        passed &= !matches!(outcome, Outcome::Failed(_));
        report(name, &outcome);
        matches!(outcome, Outcome::Passed(_))
    };

    stage("sandbox", sandbox(options));
    let music = match write_fixtures(dir) {
        Ok(music) => music,
        Err(error) => {
            stage("fixtures", Outcome::Failed(error));
            return false;
        }
    };
    stage(
        "fixtures",
        Outcome::Passed(format!("{} files", FIXTURES.len())),
    );

    let (conn, folder_id, outcome) = match scan(dir, &music) {
        Ok((conn, folder_id, added)) => {
            (conn, folder_id, Outcome::Passed(format!("{added} tracks")))
        }
        Err(error) => {
            stage("scan", Outcome::Failed(error));
            for name in ["bookmark", "covers", "decoding", "playback", "recording"] {
                stage(name, Outcome::Skipped("the scan failed".into()));
            }
            return false;
        }
    };
    stage("scan", outcome);
    stage("bookmark", outcome_of(bookmark(&conn, folder_id, &music)));
    let library = LibraryState::for_tests(conn);
    stage("covers", outcome_of(covers(&library)));
    let decoded = stage("decoding", outcome_of(decoding(&library)));
    let outcome = if !options.audio {
        Outcome::Skipped("not asked for".into())
    } else if !decoded {
        Outcome::Skipped("decoding failed".into())
    } else {
        playback(&library, options)
    };
    let played = stage("playback", outcome);
    let outcome = if !played {
        Outcome::Skipped("playback didn't run".into())
    } else {
        outcome_of(recording(&library, dir))
    };
    stage("recording", outcome);
    passed
}

fn outcome_of(result: Result<String, String>) -> Outcome {
    match result {
        Ok(detail) => Outcome::Passed(detail),
        Err(error) => Outcome::Failed(error),
    }
}

/// Whether the app runs in the App Sandbox: macOS sets this variable for
/// every sandboxed process.
fn sandbox(options: Options) -> Outcome {
    if std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some() {
        Outcome::Passed("sandboxed".into())
    } else if options.require_sandbox {
        Outcome::Failed("not sandboxed".into())
    } else {
        Outcome::Skipped("not sandboxed".into())
    }
}

/// Writes the fixtures into `dir`/music; returns that folder.
fn write_fixtures(dir: &Path) -> Result<PathBuf, String> {
    let music = dir.join("music");
    std::fs::create_dir_all(&music).map_err(|e| format!("cannot create the folder: {e}"))?;
    for (name, bytes) in FIXTURES {
        std::fs::write(music.join(name), bytes).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(music)
}

/// A scratch library in `dir` with `music` added (its bookmark made under
/// the sandbox) and scanned: every fixture a track, none failed.
fn scan(dir: &Path, music: &Path) -> Result<(Connection, i64, usize), String> {
    let mut conn = db::open(&dir.join("library.sqlite3")).map_err(|e| e.to_string())?;
    let folder = library::add_folder(&conn, music).map_err(|e| e.to_string())?;
    let report = scanner::scan_folders(&mut conn, &[folder.id], ScanOptions::default(), |_| {})
        .map_err(|e| e.to_string())?
        .pop()
        .ok_or("no result")?
        .map_err(|e| e.to_string())?;
    if let Some(failure) = report.failed.first() {
        return Err(format!(
            "{} of {} files failed, the first {}: {}",
            report.failed.len(),
            FIXTURES.len(),
            file_name(&failure.path),
            failure.error
        ));
    }
    if let Some(status) = &report.unavailable {
        return Err(format!("the folder is unavailable: {status:?}"));
    }
    if report.added != FIXTURES.len() {
        return Err(format!(
            "{} tracks added, expected {}",
            report.added,
            FIXTURES.len()
        ));
    }
    Ok((conn, folder.id, report.added))
}

/// The folder's stored bookmark resolves to the folder.
fn bookmark(conn: &Connection, folder_id: i64, music: &Path) -> Result<String, String> {
    let bookmark: Option<Vec<u8>> = conn
        .query_row(
            "SELECT bookmark FROM folders WHERE id = ?1",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let size = bookmark.map_or(0, |bookmark| bookmark.len());
    if size == 0 {
        return Err("no bookmark was stored".into());
    }
    let open = access::open_folder(conn, folder_id).map_err(|e| e.to_string())?;
    let expected = std::fs::canonicalize(music).map_err(|e| e.to_string())?;
    if open.path != expected {
        return Err("it resolved to another folder".into());
    }
    Ok(format!("{size} bytes, resolved"))
}

/// Each tagged fixture's album shows its embedded cover.
fn covers(library: &LibraryState) -> Result<String, String> {
    let keys: Vec<(String, ArtKey)> = {
        let conn = library.conn();
        let mut statement = conn
            .prepare(
                "SELECT relative_path, id, album_id FROM tracks
                 WHERE relative_path LIKE ?1 || '%' ORDER BY relative_path",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([WITH_COVERS], |row| {
                let album: Option<i64> = row.get(2)?;
                let key = album.map_or(ArtKey::Track(row.get(1)?), ArtKey::Album);
                Ok((row.get(0)?, key))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    let expected = FIXTURES
        .iter()
        .filter(|(name, _)| name.starts_with(WITH_COVERS))
        .count();
    if keys.len() != expected {
        return Err(format!("{} tagged tracks, expected {expected}", keys.len()));
    }
    for (name, key) in &keys {
        match art::lookup(library, *key).map_err(|e| e.to_string())? {
            Some(found) if !found.data.is_empty() => {}
            _ => return Err(format!("{name}: no cover")),
        }
    }
    Ok(format!("{} covers", keys.len()))
}

/// Every track decodes from start to end, its folder held open.
fn decoding(library: &LibraryState) -> Result<String, String> {
    let ids = track_ids(library, "ORDER BY id")?;
    for &id in &ids {
        let play = track_play(library, id)?;
        let _folder = library.open_folder_of(&play.path)?;
        anomp::analyse_file(&play.path, play.start, play.end.unwrap_or(0.0), |_| true)
            .map_err(|e| format!("{}: {e}", file_name(&play.path.to_string_lossy())))?;
    }
    Ok(format!("{} files", ids.len()))
}

/// The two shortest tracks play through a gapless hand-off, silently.
fn playback(library: &LibraryState, options: Options) -> Outcome {
    let mut engine = match Engine::new() {
        Some(engine) => engine,
        None => return Outcome::Failed("the engine didn't start".into()),
    };
    if let Err(error) = engine.open_default_device() {
        let detail = format!("no output device: {error}");
        return if options.require_audio {
            Outcome::Failed(detail)
        } else {
            Outcome::Skipped(detail)
        };
    }
    outcome_of(hand_off(library, &mut engine))
}

fn hand_off(library: &LibraryState, engine: &mut Engine) -> Result<String, String> {
    let ids = track_ids(library, "ORDER BY duration, id LIMIT 2")?;
    let [first, second] = ids[..] else {
        return Err("fewer than two tracks".into());
    };
    let settings = PlaybackSettings::default();
    let (first, second) = (track_play(library, first)?, track_play(library, second)?);
    // Held until the engine stops, as the queue does.
    let _folders = (
        library.open_folder_of(&first.path)?,
        library.open_folder_of(&second.path)?,
    );
    engine.set_volume(0.0);
    engine.load_track(&first.path, &first.options(&settings))?;
    engine.set_next_track(Some((&second.path, &second.options(&settings))))?;
    let before = engine.advance_count();
    if !engine.play() {
        return Err("didn't start playing".into());
    }
    let started = Instant::now();
    while engine.advance_count() == before {
        if started.elapsed() > HAND_OFF_TIMEOUT {
            engine.stop();
            return Err(format!(
                "no hand-off within {} s",
                HAND_OFF_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let device = engine.device_name().unwrap_or_default();
    engine.stop();
    Ok(format!(
        "handed off after {:.1} s on {device}",
        started.elapsed().as_secs_f64()
    ))
}

/// A second of the shortest track recorded into a folder reached through a
/// writable bookmark, as the recordings' folder is (PLAN.md X6).
fn recording(library: &LibraryState, dir: &Path) -> Result<String, String> {
    let folder = dir.join("recordings");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let bookmark = anomp::create_writable_bookmark(&folder)?;
    let access = anomp::FolderAccess::start(&bookmark)?;
    let file = access.path().join("self-test.flac");

    let mut engine = Engine::new().ok_or("the engine didn't start")?;
    engine.open_default_device()?;
    let ids = track_ids(library, "ORDER BY duration, id LIMIT 1")?;
    let track = track_play(library, *ids.first().ok_or("no tracks")?)?;
    let _folder = library.open_folder_of(&track.path)?;
    engine.set_volume(0.0);
    engine.load_track(&track.path, &track.options(&PlaybackSettings::default()))?;
    let format = anomp::RecordingFormat {
        kind: anomp::RecordingKind::Flac,
        bits: 24,
        bitrate_kbps: 256,
    };
    engine.record_start(&file, &format)?;
    if !engine.play() {
        engine.record_stop();
        return Err("didn't start playing".into());
    }
    std::thread::sleep(Duration::from_secs(1));
    engine.stop();
    engine.record_stop();
    let status = engine.recording();
    let size = std::fs::metadata(&file).map_err(|e| e.to_string())?.len();
    if status.frames == 0 || size == 0 {
        return Err(format!("{} frames, {size} bytes written", status.frames));
    }
    Ok(format!("{:.1} s, {size} bytes", status.seconds))
}

fn track_ids(library: &LibraryState, order: &str) -> Result<Vec<i64>, String> {
    let conn = library.conn();
    // `order` is one of this module's fixed clauses, never a value.
    let mut statement = conn
        .prepare(&format!("SELECT id FROM tracks {order}"))
        .map_err(|e| e.to_string())?;
    let ids = statement
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<i64>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ids)
}

fn track_play(library: &LibraryState, id: i64) -> Result<playback::TrackPlay, String> {
    library.track_play(id, &FeatureSettings::default())
}

/// The file name alone: the self-test's output never shows a path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stages(options: Options) -> (bool, Vec<(String, Outcome)>) {
        let dir = tempfile::tempdir().unwrap();
        let mut seen = Vec::new();
        let passed = run(dir.path(), options, &mut |stage, outcome| {
            seen.push((stage.to_owned(), outcome.clone()));
        });
        (passed, seen)
    }

    #[test]
    fn every_stage_passes_outside_the_sandbox_but_the_sandbox_and_playback() {
        let (passed, seen) = stages(Options::default());
        let names: Vec<&str> = seen.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "sandbox",
                "fixtures",
                "scan",
                "bookmark",
                "covers",
                "decoding",
                "playback",
                "recording"
            ]
        );
        for (name, outcome) in &seen {
            match name.as_str() {
                "sandbox" | "playback" | "recording" => assert!(
                    matches!(outcome, Outcome::Skipped(_)),
                    "{name}: {outcome:?}"
                ),
                _ => assert!(matches!(outcome, Outcome::Passed(_)), "{name}: {outcome:?}"),
            }
        }
        assert!(passed);
        assert_eq!(
            seen[2].1,
            Outcome::Passed(format!("{} tracks", FIXTURES.len()))
        );
        assert_eq!(seen[4].1, Outcome::Passed("2 covers".into()));
    }

    #[test]
    fn requiring_the_sandbox_fails_outside_it() {
        let (passed, seen) = stages(Options {
            require_sandbox: true,
            ..Options::default()
        });
        assert_eq!(seen[0].1, Outcome::Failed("not sandboxed".into()));
        assert!(!passed);
    }

    #[test]
    fn a_folder_that_cant_be_written_fails_and_stops() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut seen = Vec::new();
        let passed = run(file.path(), Options::default(), &mut |stage, outcome| {
            seen.push((stage.to_owned(), outcome.clone()));
        });
        assert!(!passed);
        assert_eq!(seen.len(), 2);
        assert!(matches!(seen[1].1, Outcome::Failed(_)));
    }

    #[test]
    fn the_command_line_asks_for_it() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert_eq!(Options::from_args(&args(&[])), None);
        assert_eq!(Options::from_args(&args(&["--require-sandbox"])), None);
        assert_eq!(
            Options::from_args(&args(&["--self-test", "--require-sandbox"])),
            Some(Options {
                require_sandbox: true,
                require_audio: false,
                audio: true
            })
        );
    }

    #[test]
    fn each_line_names_the_stage_and_its_result() {
        assert_eq!(
            Outcome::Passed("2 covers".into()).line("covers"),
            "self-test: covers: ok: 2 covers"
        );
        assert_eq!(
            Outcome::Failed("x".into()).line("scan"),
            "self-test: scan: FAILED: x"
        );
    }

    #[test]
    fn every_audio_fixture_is_embedded() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../core/tests/fixtures");
        let mut on_disk: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap())
            .filter(|entry| entry.file_type().unwrap().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        on_disk.sort();
        let embedded: Vec<&str> = FIXTURES.iter().map(|(name, _)| *name).collect();
        assert_eq!(on_disk, embedded);
    }
}
