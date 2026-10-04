//! "Copy diagnostics" and "Show logs" in Settings › About (PLAN.md H9),
//! and the third-party notices shown there (PLAN.md §8.2).
//!
//! The diagnostics are plain text for a bug report. They name no paths,
//! titles or artists: versions, the OS, the output device, library
//! counts, the folders' states (counts only), the feature switches and
//! online sources that are on, facts about the database, whether detailed
//! logging is on, and the log's last lines at info and above (which are
//! scrubbed, see `logging`), never the detailed log's.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use rusqlite::Connection;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager, Runtime};

use crate::library::availability::FolderStates;
use crate::library::commands::{LibraryState, RecoveryState};
use crate::library::{db, Error};

/// Log lines the diagnostics end with.
const LOG_LINES: usize = 100;

/// What the diagnostics say.
#[derive(Debug, Default, PartialEq)]
pub struct Facts {
    pub versions: Vec<(&'static str, String)>,
    pub output: Option<String>,
    /// (table, rows).
    pub counts: Vec<(&'static str, i64)>,
    /// (state, folders).
    pub folders: BTreeMap<String, usize>,
    pub features: Vec<String>,
    pub sources: Vec<String>,
    pub database: Vec<(&'static str, String)>,
    /// Whether detailed logging is on (`logging::detailed`).
    pub detailed_log: bool,
    pub log: Vec<String>,
}

/// The text "Copy diagnostics" copies.
pub fn render(facts: &Facts) -> String {
    let mut text = String::from("ano-mp diagnostics\n");
    let mut section = |title: &str, lines: Vec<String>| {
        let _ = write!(text, "\n## {title}\n");
        if lines.is_empty() {
            text.push_str("(none)\n");
        }
        for line in lines {
            let _ = writeln!(text, "{line}");
        }
    };
    let pairs = |pairs: &[(&str, String)]| -> Vec<String> {
        pairs
            .iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect()
    };
    section("Versions", pairs(&facts.versions));
    section("Output", facts.output.iter().cloned().collect());
    section(
        "Library",
        facts
            .counts
            .iter()
            .map(|(table, count)| format!("{table}: {count}"))
            .collect(),
    );
    section(
        "Folders",
        facts
            .folders
            .iter()
            .map(|(state, count)| format!("{state}: {count}"))
            .collect(),
    );
    section("Features on", facts.features.clone());
    section("Online sources on", facts.sources.clone());
    section("Database", pairs(&facts.database));
    let detailed = if facts.detailed_log { "on" } else { "off" };
    section("Logging", vec![format!("detailed: {detailed}")]);
    section("Log (last lines)", facts.log.clone());
    text
}

/// The library's counts, by table.
fn counts(conn: &Connection) -> Result<Vec<(&'static str, i64)>, Error> {
    const TABLES: [&str; 8] = [
        "folders",
        "tracks",
        "albums",
        "artists",
        "playlists",
        "plays",
        "track_favourites",
        "track_ratings",
    ];
    TABLES
        .into_iter()
        .map(|table| {
            let count = conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
            Ok((table, count))
        })
        .collect()
}

/// The names of the boolean switches that are on in `settings` (a
/// serialized settings struct).
fn switches_on(settings: &serde_json::Value) -> Vec<String> {
    let mut on: Vec<String> = settings
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, value)| value.as_bool() == Some(true))
        .map(|(name, _)| name.clone())
        .collect();
    on.sort();
    on
}

/// The OS and its version, e.g. "macos 26.1 (aarch64)".
fn os() -> String {
    let version = os_version().unwrap_or_else(|| "unknown".into());
    format!(
        "{} {version} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(target_vendor = "apple")]
fn os_version() -> Option<String> {
    let name = c"kern.osproductversion";
    let mut buffer = [0u8; 64];
    let mut size = buffer.len();
    // SAFETY: `name` is a C string, and `buffer` holds `size` bytes, which
    // sysctlbyname writes at most; no new value is set.
    let status = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return None;
    }
    let text = &buffer[..size.min(buffer.len())];
    let text = text.split(|&byte| byte == 0).next().unwrap_or(text);
    Some(String::from_utf8_lossy(text).into_owned())
}

#[cfg(not(target_vendor = "apple"))]
fn os_version() -> Option<String> {
    None
}

/// Gathers the facts. Blocking: not on the main thread.
fn gather<R: Runtime>(app: &AppHandle<R>) -> Facts {
    let settings = crate::settings::current(app);
    let mut facts = Facts {
        versions: vec![
            ("ano-mp", env!("CARGO_PKG_VERSION").into()),
            ("core", crate::anomp::version()),
            ("Tauri", tauri::VERSION.into()),
            (
                "webview",
                tauri::webview_version().unwrap_or_else(|e| e.to_string()),
            ),
            ("OS", os()),
            (
                "build",
                if cfg!(debug_assertions) {
                    "debug".into()
                } else {
                    "release".into()
                },
            ),
            (
                "sandboxed",
                std::env::var_os("APP_SANDBOX_CONTAINER_ID")
                    .is_some()
                    .to_string(),
            ),
        ],
        features: serde_json::to_value(&settings.features)
            .map(|value| switches_on(&value))
            .unwrap_or_default(),
        detailed_log: crate::logging::detailed(),
        log: crate::logging::recent_lines(app, LOG_LINES),
        ..Facts::default()
    };

    facts.output = match crate::audio::audio_output_status(app.clone()) {
        Ok(status) => Some(match status.current {
            Some(device) => format!(
                "{}, {} Hz, {} samples ({} devices)",
                device.name,
                device.sample_rate,
                device.buffer_size,
                status.devices.len()
            ),
            None => format!("no device open ({} devices)", status.devices.len()),
        }),
        Err(error) => Some(format!("unknown: {error}")),
    };

    if let Some(states) = app.try_state::<FolderStates>() {
        if let Some(library) = app.try_state::<LibraryState>() {
            let conn = library.conn();
            if let Ok(folders) = crate::library::folders(&conn) {
                for folder in folders {
                    let state = states.get(folder.id).state.code().to_owned();
                    *facts.folders.entry(state).or_default() += 1;
                }
            }
        }
    }

    if let Some(library) = app.try_state::<LibraryState>() {
        let conn = library.conn();
        facts.counts = counts(&conn).unwrap_or_default();
        if let Ok(services) = crate::metadata::settings::service_settings(&conn) {
            facts.sources = services
                .sources
                .iter()
                .filter(|source| services.is_usable(source.id))
                .map(|source| source.id.as_str().to_owned())
                .collect();
            if !services.online {
                facts.sources.insert(0, "(online off)".into());
            }
        }
        let schema: Result<i64, _> =
            conn.pragma_query_value(None, "user_version", |row| row.get(0));
        facts.database.push((
            "schema",
            schema.map_or_else(|e| e.to_string(), |v| v.to_string()),
        ));
        let path = library.db_path();
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        facts.database.push(("size", format!("{} KB", size / 1024)));
        facts
            .database
            .push(("copies", db::copies(path).len().to_string()));
    }
    if let Some(recovery) = app.try_state::<RecoveryState>() {
        facts
            .database
            .push(("launch check", recovery.check_summary()));
    }
    facts
}

/// The diagnostics, as text to copy.
#[tauri::command]
pub async fn diagnostics_text<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || render(&gather(&app)))
        .await
        .map_err(|e| e.to_string())
}

/// Shows the log files in the Finder.
#[tauri::command]
pub fn diagnostics_show_logs<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let dir = crate::logging::log_dir(&app)?;
    let file = dir.join(format!("{}.log", crate::logging::FILE_NAME));
    let target = if file.exists() { file } else { dir };
    tauri_plugin_opener::reveal_item_in_dir(target).map_err(|e| e.to_string())
}

/// The notices `scripts/make-notices.py` writes, bundled as a resource
/// (`bundle.resources` in tauri.conf.json).
const NOTICES: &str = "THIRD_PARTY_NOTICES";

/// The third-party notices, as text. Off the main thread: the file is a
/// few hundred KB.
#[tauri::command]
pub async fn diagnostics_notices<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let unreadable = |message: String| {
        log::warn!("the third-party notices can't be read: {message}");
        crate::coded::coded("noticesUnreadable", &[], message)
    };
    let path = app
        .path()
        .resolve(NOTICES, BaseDirectory::Resource)
        .map_err(|e| unreadable(e.to_string()))?;
    std::fs::read_to_string(path).map_err(|e| unreadable(e.to_string()))
}

/// When the process started, for `diagnostics_first_paint` (PLAN.md H18).
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
static PAINTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Notes the time the process started; `run` calls it first.
pub fn mark_start() {
    STARTED.get_or_init(std::time::Instant::now);
}

/// Milliseconds from `mark_start` to the first call, or None on later
/// calls (a reload of the page isn't a launch).
fn first_paint_ms() -> Option<u128> {
    let started = STARTED.get()?;
    if PAINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(started.elapsed().as_millis())
}

/// The main window's page has painted for the first time: logs the time
/// since launch, for H18's launch budget (a count only).
#[tauri::command]
pub fn diagnostics_first_paint() {
    if let Some(ms) = first_paint_ms() {
        log::info!("first paint after {ms} ms");
    }
}

/// Discogs' non-affiliation notice, in its terms' words, which About shows
/// (PLAN.md §8.1).
#[tauri::command]
pub fn diagnostics_discogs_notice() -> &'static str {
    crate::metadata::discogs::NOTICE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_paint_is_timed_once_from_the_start() {
        mark_start();
        let first = first_paint_ms();
        assert!(first.is_some());
        assert_eq!(first_paint_ms(), None);
    }

    #[test]
    fn the_bundle_carries_the_notices_under_the_name_read() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let config: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(manifest.join("tauri.conf.json")).unwrap(),
        )
        .unwrap();
        let resources = config["bundle"]["resources"].as_object().unwrap();
        let (source, name) = resources
            .iter()
            .find(|(_, name)| name.as_str() == Some(NOTICES))
            .expect("THIRD_PARTY_NOTICES in bundle.resources");
        let text = std::fs::read_to_string(manifest.join(source)).unwrap();
        assert!(text.starts_with("ano-mp: third-party notices"));
        assert!(text.contains("FFmpeg") && text.contains("JUCE"));
        assert_eq!(name, NOTICES);
    }

    #[test]
    fn renders_every_section() {
        let facts = Facts {
            versions: vec![
                ("ano-mp", "0.1.0".into()),
                ("OS", "macos 26.1 (aarch64)".into()),
            ],
            output: Some("Speakers, 48000 Hz, 512 samples (2 devices)".into()),
            counts: vec![("tracks", 1200), ("plays", 30)],
            folders: [("available".to_owned(), 2), ("missing".to_owned(), 1)].into(),
            features: vec!["cueSheets".into()],
            sources: vec![],
            database: vec![("schema", "9".into())],
            detailed_log: true,
            log: vec!["t INFO  library: 3 folders".into()],
        };
        assert_eq!(
            render(&facts),
            "ano-mp diagnostics

## Versions
ano-mp: 0.1.0
OS: macos 26.1 (aarch64)

## Output
Speakers, 48000 Hz, 512 samples (2 devices)

## Library
tracks: 1200
plays: 30

## Folders
available: 2
missing: 1

## Features on
cueSheets

## Online sources on
(none)

## Database
schema: 9

## Logging
detailed: on

## Log (last lines)
t INFO  library: 3 folders
"
        );
    }

    #[test]
    fn lists_the_switches_that_are_on() {
        let settings = crate::settings::FeatureSettings {
            cue_sheets: true,
            lyrics: false,
            ..Default::default()
        };
        let on = switches_on(&serde_json::to_value(&settings).unwrap());
        assert!(on.contains(&"cueSheets".to_owned()), "{on:?}");
        assert!(!on.contains(&"lyrics".to_owned()));
    }

    #[test]
    fn counts_every_table() {
        let conn = db::open_in_memory().unwrap();
        let counts = counts(&conn).unwrap();
        assert_eq!(counts.len(), 8);
        assert!(counts.iter().all(|(_, count)| *count == 0));
    }

    #[test]
    fn names_the_os() {
        let os = os();
        assert!(os.starts_with(std::env::consts::OS), "{os}");
        #[cfg(target_vendor = "apple")]
        assert!(os_version().is_some_and(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit())));
    }
}
