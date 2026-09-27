//! The app's settings (PLAN.md Phase 6): what the lists and album pages
//! show, playback (ReplayGain), the output device, and the visualizer. They
//! are stored as one JSON value under `app` in the library's `settings`
//! table; the sort rules (`library::rules`) and the online sources
//! (`metadata::settings`) keep their own keys and commands.
//!
//! Reading keeps whatever stored value is still usable, field by field (an
//! older or newer version may have written it), and falls back on the
//! default for the rest. Saving validates the whole value and applies it:
//! a new output device is opened before anything is stored, a new
//! ReplayGain setting reaches the tracks the engine has open, and a new
//! frame rate restarts the visualizer's analysis. `settings-changed`
//! carries the saved settings to the frontend.
//!
//! The TypeScript types for these and the other settings are generated
//! from the Rust ones (`bindings`, checked by `cargo test`).

use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::anomp::{ReplayGain, MAX_TRACK_GAIN};
use crate::library::commands::LibraryState;
use crate::library::covers::CoverBasis;
use crate::library::Error;

const SETTINGS_KEY: &str = "app";

/// Frontend event with the saved `AppSettings` as its payload.
pub const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub display: DisplaySettings,
    pub playback: PlaybackSettings,
    pub output: OutputSettings,
    pub visualizer: VisualizerSettings,
}

/// What the library's lists and pages show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettings {
    /// What track lists show besides the title, in order.
    pub track_columns: Vec<TrackColumn>,
    /// What an album's summary line says about its release, in order.
    pub album_facts: Vec<AlbumFact>,
    /// Album descriptions and artist biographies (from Wikipedia).
    pub show_descriptions: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        use AlbumFact::{Country, Date, Label, Type};
        use TrackColumn::{Album, Artist, Duration, TrackNumber};
        DisplaySettings {
            track_columns: vec![TrackNumber, Artist, Album, Duration],
            album_facts: vec![Date, Label, Country, AlbumFact::Format, Type],
            show_descriptions: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum TrackColumn {
    TrackNumber,
    Artist,
    Album,
    AlbumArtist,
    Year,
    Genre,
    Duration,
    /// The file's type, from its extension (FLAC, MP3…).
    Format,
    Bitrate,
    SampleRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum AlbumFact {
    /// The release date.
    Date,
    /// Labels and catalogue numbers.
    Label,
    Country,
    /// The media, e.g. "2 × CD".
    Format,
    /// Album, EP, live…
    Type,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSettings {
    pub replay_gain: ReplayGainMode,
    /// dB added to the ReplayGain tags' gains, -15 to 15.
    pub preamp: f64,
    /// dB for tracks without ReplayGain tags while it's on, -15 to 15.
    pub untagged_gain: f64,
    /// Lowers a track's gain so its tagged peak doesn't clip.
    pub prevent_clipping: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        PlaybackSettings {
            replay_gain: ReplayGainMode::Off,
            preamp: 0.0,
            untagged_gain: 0.0,
            prevent_clipping: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ReplayGainMode {
    #[default]
    Off,
    /// Each track at the same loudness.
    Track,
    /// Each album at the same loudness, keeping the differences between its
    /// tracks; the track gain for a track without an album gain.
    Album,
}

/// The largest preamp and untagged gain, in dB either way.
pub const MAX_GAIN_DB: f64 = 15.0;

impl PlaybackSettings {
    /// The linear gain for a track with `tags`: 1 with ReplayGain off,
    /// otherwise the tags' gain for the mode (falling back on the other
    /// kind) plus the preamp, or the untagged gain for a track without
    /// either; lowered so the matching peak doesn't clip if asked, and
    /// never above the engine's maximum.
    pub fn gain(&self, tags: &ReplayGain) -> f64 {
        let (gain, peak) = match self.replay_gain {
            ReplayGainMode::Off => return 1.0,
            ReplayGainMode::Track => (
                tags.track_gain.or(tags.album_gain),
                tags.track_gain
                    .and(tags.track_peak)
                    .or(tags.album_peak)
                    .or(tags.track_peak),
            ),
            ReplayGainMode::Album => (
                tags.album_gain.or(tags.track_gain),
                tags.album_gain
                    .and(tags.album_peak)
                    .or(tags.track_peak)
                    .or(tags.album_peak),
            ),
        };
        let db = gain.map_or(self.untagged_gain, |gain| gain + self.preamp);
        let mut linear = 10f64.powf(db / 20.0);
        if let (true, Some(peak)) = (self.prevent_clipping, peak.filter(|&peak| peak > 0.0)) {
            linear = linear.min(1.0 / peak);
        }
        linear.clamp(0.0, MAX_TRACK_GAIN)
    }
}

/// Where the audio goes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutputSettings {
    /// The output device's name; null for the system's default device.
    /// When it isn't there, the default plays until it comes back.
    pub device: Option<String>,
    /// Samples per block; null for the device's default.
    pub buffer_size: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct VisualizerSettings {
    /// The visualization's id (`app/src/lib/visualizer`); the frontend
    /// shows its first one for an id it doesn't know.
    pub visualization: String,
    /// Which albums the cover wall shows.
    pub cover_basis: CoverBasis,
    /// Analyses a second, 10 to 120.
    pub frame_rate: u32,
    /// Scales the spectrum, 0.25 to 4.
    pub sensitivity: f64,
    /// Colours from the current cover rather than the defaults.
    pub colors_from_cover: bool,
    /// Moves on to the next visualization this often; 0 never.
    pub cycle_seconds: u32,
}

impl Default for VisualizerSettings {
    fn default() -> Self {
        VisualizerSettings {
            visualization: "bars".into(),
            cover_basis: CoverBasis::Year,
            frame_rate: 60,
            sensitivity: 1.0,
            colors_from_cover: true,
            cycle_seconds: 0,
        }
    }
}

impl AppSettings {
    /// Checks every value, naming the first bad one.
    pub fn validate(&self) -> Result<(), Error> {
        let invalid = |message: String| Err(Error::Invalid(message));
        let display = &self.display;
        if !all_different(&display.track_columns) {
            return invalid("A track list column is repeated".into());
        }
        if !all_different(&display.album_facts) {
            return invalid("An album fact is repeated".into());
        }
        let playback = &self.playback;
        for (name, db) in [
            ("preamp", playback.preamp),
            ("gain for untagged tracks", playback.untagged_gain),
        ] {
            if !(-MAX_GAIN_DB..=MAX_GAIN_DB).contains(&db) {
                return invalid(format!(
                    "The {name} must be between -{MAX_GAIN_DB} and {MAX_GAIN_DB} dB"
                ));
            }
        }
        let output = &self.output;
        if let Some(device) = &output.device {
            if device.trim().is_empty() || device.len() > 256 {
                return invalid("The output device's name must be 1 to 256 bytes".into());
            }
        }
        if let Some(size) = output.buffer_size {
            if !(16..=16384).contains(&size) {
                return invalid("The buffer size must be 16 to 16384 samples".into());
            }
        }
        let visualizer = &self.visualizer;
        if visualizer.visualization.trim().is_empty() || visualizer.visualization.len() > 64 {
            return invalid("The visualization's id must be 1 to 64 bytes".into());
        }
        if !(10..=120).contains(&visualizer.frame_rate) {
            return invalid("The frame rate must be 10 to 120 a second".into());
        }
        if !(0.25..=4.0).contains(&visualizer.sensitivity) {
            return invalid("The sensitivity must be 0.25 to 4".into());
        }
        if visualizer.cycle_seconds > 3600 {
            return invalid("Visualizations can change at most once an hour".into());
        }
        Ok(())
    }

    /// Reads stored settings, keeping each usable value and the default for
    /// the rest: an unknown field is dropped, a value that doesn't parse or
    /// validate is replaced by its default, and a list keeps the items it
    /// can (e.g. not a column only a newer version knows).
    fn from_json(json: &str) -> AppSettings {
        let Ok(stored) = serde_json::from_str::<Value>(json) else {
            return AppSettings::default();
        };
        let mut merged = serde_json::to_value(AppSettings::default()).expect("settings serialize");
        merge(&mut merged, "", &stored);
        parse(&merged).unwrap_or_default()
    }
}

fn all_different<T: Eq + std::hash::Hash>(items: &[T]) -> bool {
    let mut seen = std::collections::HashSet::new();
    items.iter().all(|item| seen.insert(item))
}

/// The settings `value` describes, if it parses and validates.
fn parse(value: &Value) -> Option<AppSettings> {
    let settings = AppSettings::deserialize(value).ok()?;
    settings.validate().ok()?;
    Some(settings)
}

/// Copies into `root` each value of `stored` (the object at JSON pointer
/// `at`) whose field `root` has, as long as `root` stays valid.
fn merge(root: &mut Value, at: &str, stored: &Value) {
    let Value::Object(stored) = stored else {
        return;
    };
    for (key, value) in stored {
        let pointer = format!("{at}/{}", key.replace('~', "~0").replace('/', "~1"));
        let (is_object, is_array) = match root.pointer(&pointer) {
            Some(current) => (current.is_object(), current.is_array()),
            None => continue, // Not a field of this version's.
        };
        if is_object {
            merge(root, &pointer, value);
        } else if !try_replace(root, &pointer, value.clone()) {
            if let (true, Value::Array(items)) = (is_array, value) {
                // The items that can be kept, in order.
                let mut kept = Vec::new();
                for item in items {
                    kept.push(item.clone());
                    if !try_replace(root, &pointer, Value::Array(kept.clone())) {
                        kept.pop();
                    }
                }
            }
        }
    }
}

/// Puts `value` at `pointer` in `root` if `root` stays valid; returns
/// whether it did.
fn try_replace(root: &mut Value, pointer: &str, value: Value) -> bool {
    let slot = root.pointer_mut(pointer).expect("pointer checked by merge");
    let previous = std::mem::replace(slot, value);
    if parse(root).is_some() {
        return true;
    }
    *root.pointer_mut(pointer).expect("pointer checked by merge") = previous;
    false
}

pub fn load(conn: &Connection) -> Result<AppSettings, Error> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(stored.map_or_else(AppSettings::default, |json| AppSettings::from_json(&json)))
}

pub fn store(conn: &Connection, settings: &AppSettings) -> Result<(), Error> {
    settings.validate()?;
    let json = serde_json::to_string(settings).expect("settings serialize");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [SETTINGS_KEY, &json],
    )?;
    Ok(())
}

// ---- The app's copy ---------------------------------------------------------

/// The settings as saved, managed by Tauri.
#[derive(Default)]
pub struct SettingsState(Mutex<AppSettings>);

impl SettingsState {
    fn lock(&self) -> MutexGuard<'_, AppSettings> {
        // Replaced whole, so never left half-changed by a panic.
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The settings now; the defaults before `init`.
pub fn current<R: Runtime>(app: &AppHandle<R>) -> AppSettings {
    app.try_state::<SettingsState>()
        .map(|state| state.lock().clone())
        .unwrap_or_default()
}

/// Reads the settings. Call after the library has started, before the
/// engine, which opens the output device they name.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let settings = match app.try_state::<LibraryState>() {
        Some(library) => load(&library.conn()).map_err(|e| e.to_string())?,
        None => AppSettings::default(),
    };
    app.manage(SettingsState(Mutex::new(settings)));
    Ok(())
}

/// The settings, and the defaults a section can be reset to.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: AppSettings,
    pub defaults: AppSettings,
}

#[tauri::command]
pub fn settings_get<R: Runtime>(app: AppHandle<R>) -> SettingsPayload {
    SettingsPayload {
        settings: current(&app),
        defaults: AppSettings::default(),
    }
}

/// Validates and applies `settings`, then stores them and emits
/// `settings-changed`. If the output device named can't be opened, the one
/// before is reopened and nothing is saved.
#[tauri::command]
pub async fn settings_save<R: Runtime>(
    app: AppHandle<R>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    settings.validate().map_err(|e| e.to_string())?;
    let before = current(&app);
    if settings.output != before.output {
        if let Err(error) = crate::audio::apply_output(&app, &settings.output) {
            // Back to what was playing, as far as possible.
            let _ = crate::audio::apply_output(&app, &before.output);
            return Err(error);
        }
    }
    {
        let library = app
            .try_state::<LibraryState>()
            .ok_or("The library is not available")?;
        store(&library.conn(), &settings).map_err(|e| e.to_string())?;
    }
    if let Some(state) = app.try_state::<SettingsState>() {
        *state.lock() = settings.clone();
    }
    if settings.playback != before.playback {
        crate::queue::refresh_gains(&app);
    }
    if settings.visualizer.frame_rate != before.visualizer.frame_rate {
        crate::visualizer::restart(&app);
    }
    let _ = app.emit(SETTINGS_CHANGED_EVENT, &settings);
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::db;

    fn store_json(conn: &Connection, json: &str) {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [SETTINGS_KEY, json],
        )
        .unwrap();
    }

    #[test]
    fn defaults_until_something_is_saved_and_round_trip() {
        let conn = db::open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), AppSettings::default());
        AppSettings::default().validate().unwrap();

        let mut settings = AppSettings::default();
        settings.display.track_columns = vec![TrackColumn::Year, TrackColumn::Format];
        settings.display.show_descriptions = false;
        settings.playback.replay_gain = ReplayGainMode::Album;
        settings.playback.preamp = -3.5;
        settings.output.device = Some("USB DAC ♪".into());
        settings.output.buffer_size = Some(256);
        settings.visualizer.visualization = "scope".into();
        settings.visualizer.cover_basis = CoverBasis::Artist;
        settings.visualizer.cycle_seconds = 60;
        store(&conn, &settings).unwrap();
        assert_eq!(load(&conn).unwrap(), settings);

        let json: Value = serde_json::from_str(
            &conn
                .query_row("SELECT value FROM settings WHERE key = 'app'", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            json["display"]["trackColumns"],
            serde_json::json!(["year", "format"])
        );
        assert_eq!(json["playback"]["replayGain"], "album");
        assert_eq!(json["output"]["bufferSize"], 256);
    }

    #[test]
    fn refuses_invalid_settings() {
        let conn = db::open_in_memory().unwrap();
        let error = |edit: fn(&mut AppSettings)| {
            let mut settings = AppSettings::default();
            edit(&mut settings);
            store(&conn, &settings).unwrap_err().to_string()
        };
        assert!(error(|s| s.display.track_columns = vec![TrackColumn::Year; 2]).contains("column"));
        assert!(error(|s| s.display.album_facts = vec![AlbumFact::Date; 2]).contains("fact"));
        assert!(error(|s| s.playback.preamp = 15.5).contains("preamp"));
        assert!(error(|s| s.playback.untagged_gain = f64::NAN).contains("untagged"));
        assert!(error(|s| s.output.device = Some(" ".into())).contains("device"));
        assert!(error(|s| s.output.buffer_size = Some(8)).contains("buffer size"));
        assert!(error(|s| s.visualizer.visualization = String::new()).contains("visualization"));
        assert!(error(|s| s.visualizer.frame_rate = 0).contains("frame rate"));
        assert!(error(|s| s.visualizer.sensitivity = 5.0).contains("sensitivity"));
        assert!(error(|s| s.visualizer.cycle_seconds = 7200).contains("hour"));
        assert_eq!(
            load(&conn).unwrap(),
            AppSettings::default(),
            "nothing was stored"
        );
    }

    #[test]
    fn falls_back_on_unreadable_json() {
        let conn = db::open_in_memory().unwrap();
        for json in ["not json", "[1, 2]", "{}", "null", r#"{"display": 3}"#] {
            store_json(&conn, json);
            assert_eq!(load(&conn).unwrap(), AppSettings::default(), "{json}");
        }
    }

    #[test]
    fn keeps_the_usable_parts_of_stored_json() {
        let conn = db::open_in_memory().unwrap();
        store_json(
            &conn,
            r#"{
                "display": {
                    "trackColumns": ["year", "composer", "genre", "year", 7, "bitrate"],
                    "albumFacts": "date",
                    "showDescriptions": false,
                    "density": "compact"
                },
                "playback": {"replayGain": "smart", "preamp": 99, "untaggedGain": -6},
                "output": {"device": "Speakers", "bufferSize": "big"},
                "visualizer": {"visualization": "future-thing", "frameRate": 30, "sensitivity": 2.5},
                "futureSection": {"on": true}
            }"#,
        );
        let settings = load(&conn).unwrap();
        let defaults = AppSettings::default();
        assert_eq!(
            settings.display.track_columns,
            [TrackColumn::Year, TrackColumn::Genre, TrackColumn::Bitrate]
        );
        assert_eq!(settings.display.album_facts, defaults.display.album_facts);
        assert!(!settings.display.show_descriptions);
        assert_eq!(settings.playback.replay_gain, ReplayGainMode::Off);
        assert_eq!(settings.playback.preamp, 0.0);
        assert_eq!(settings.playback.untagged_gain, -6.0);
        assert_eq!(settings.output.device.as_deref(), Some("Speakers"));
        assert_eq!(settings.output.buffer_size, None);
        assert_eq!(settings.visualizer.visualization, "future-thing");
        assert_eq!(settings.visualizer.frame_rate, 30);
        assert_eq!(settings.visualizer.sensitivity, 2.5);
        assert_eq!(settings.visualizer.cover_basis, CoverBasis::Year);
    }

    fn tags(
        track: Option<f64>,
        track_peak: Option<f64>,
        album: Option<f64>,
        album_peak: Option<f64>,
    ) -> ReplayGain {
        ReplayGain {
            track_gain: track,
            track_peak,
            album_gain: album,
            album_peak,
        }
    }

    fn db(linear: f64) -> f64 {
        (20.0 * linear.log10() * 100.0).round() / 100.0
    }

    #[test]
    fn replay_gain_follows_the_mode() {
        let tagged = tags(Some(-6.0), Some(0.5), Some(-4.0), Some(0.8));
        let mut playback = PlaybackSettings::default();
        assert_eq!(playback.gain(&tagged), 1.0);

        playback.replay_gain = ReplayGainMode::Track;
        assert_eq!(db(playback.gain(&tagged)), -6.0);
        playback.replay_gain = ReplayGainMode::Album;
        assert_eq!(db(playback.gain(&tagged)), -4.0);

        // Each mode falls back on the other kind of gain.
        assert_eq!(db(playback.gain(&tags(Some(-2.0), None, None, None))), -2.0);
        playback.replay_gain = ReplayGainMode::Track;
        assert_eq!(db(playback.gain(&tags(None, None, Some(-3.0), None))), -3.0);

        // The preamp applies to tagged tracks; untagged ones get their own gain.
        playback.preamp = 2.0;
        playback.untagged_gain = -5.0;
        assert_eq!(db(playback.gain(&tagged)), -4.0);
        assert_eq!(db(playback.gain(&ReplayGain::default())), -5.0);
    }

    #[test]
    fn replay_gain_prevents_clipping_and_stays_in_range() {
        let loud_peak = tags(Some(6.0), Some(0.9), Some(8.0), Some(1.0));
        let mut playback = PlaybackSettings {
            replay_gain: ReplayGainMode::Track,
            ..PlaybackSettings::default()
        };
        // +6 dB would take a 0.9 peak over full scale: 1 / 0.9 instead.
        assert!((playback.gain(&loud_peak) - 1.0 / 0.9).abs() < 1e-9);
        playback.replay_gain = ReplayGainMode::Album;
        assert_eq!(playback.gain(&loud_peak), 1.0);
        playback.prevent_clipping = false;
        assert_eq!(db(playback.gain(&loud_peak)), 8.0);

        // A missing or nonsense peak doesn't limit; nothing exceeds the engine's maximum.
        let quiet = tags(Some(40.0), Some(0.0), None, None);
        playback.prevent_clipping = true;
        playback.replay_gain = ReplayGainMode::Track;
        assert_eq!(playback.gain(&quiet), MAX_TRACK_GAIN);
    }
}

/// The TypeScript types of the settings, generated from these Rust types
/// into `app/src/lib/generated/settings.ts`. The test fails when the file
/// is out of date; `ANOMP_WRITE_BINDINGS=1 cargo test bindings` rewrites
/// it.
#[cfg(test)]
mod bindings {
    use std::path::Path;

    use ts_rs::{Config, TS};

    use super::*;
    use crate::library::rules;
    use crate::metadata::settings as metadata;

    fn declare<T: TS>(cfg: &Config, out: &mut String) {
        if let Some(docs) = T::docs() {
            out.push_str(&docs);
        }
        out.push_str("export ");
        out.push_str(&T::decl(cfg));
        out.push_str("\n\n");
    }

    fn generate() -> String {
        let cfg = Config::new().with_large_int("number");
        let mut out = String::from(
            "// Generated by `cargo test` from the Rust types (src-tauri/src/settings.rs,\n\
             // `bindings`); don't edit. After changing them, run\n\
             // `ANOMP_WRITE_BINDINGS=1 cargo test bindings` in src-tauri.\n\n",
        );
        let out = &mut out;
        declare::<AppSettings>(&cfg, out);
        declare::<SettingsPayload>(&cfg, out);
        declare::<DisplaySettings>(&cfg, out);
        declare::<TrackColumn>(&cfg, out);
        declare::<AlbumFact>(&cfg, out);
        declare::<PlaybackSettings>(&cfg, out);
        declare::<ReplayGainMode>(&cfg, out);
        declare::<OutputSettings>(&cfg, out);
        declare::<VisualizerSettings>(&cfg, out);
        declare::<CoverBasis>(&cfg, out);
        declare::<crate::audio::OutputStatus>(&cfg, out);
        declare::<crate::anomp::DeviceInfo>(&cfg, out);
        declare::<rules::SortSettings>(&cfg, out);
        declare::<rules::SortRule>(&cfg, out);
        declare::<rules::Level>(&cfg, out);
        declare::<rules::TrackKey>(&cfg, out);
        declare::<rules::AlbumOrder>(&cfg, out);
        declare::<crate::metadata::commands::MetadataSettings>(&cfg, out);
        declare::<metadata::ServiceSettings>(&cfg, out);
        declare::<metadata::SourceSettings>(&cfg, out);
        declare::<metadata::SourceInfo>(&cfg, out);
        declare::<metadata::SourceId>(&cfg, out);
        declare::<metadata::Kind>(&cfg, out);
        out.trim_end().to_owned() + "\n"
    }

    #[test]
    fn bindings_are_up_to_date() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/settings.ts");
        let generated = generate();
        if std::env::var_os("ANOMP_WRITE_BINDINGS").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &generated).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "{} is out of date: run `ANOMP_WRITE_BINDINGS=1 cargo test bindings` in src-tauri",
            path.display()
        );
    }
}
