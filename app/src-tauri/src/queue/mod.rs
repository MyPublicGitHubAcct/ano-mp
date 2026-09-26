//! The play queue (`model`), hosted on the main thread next to the engine,
//! with its Tauri commands.
//!
//! The queue lives in a main-thread thread-local like the engine, and every
//! operation runs there (`run`), so it never waits on the engine from
//! another thread while holding its own lock. Commands that need the
//! database first (to turn track ids into what the queue shows) do that on
//! a blocking thread, then hop over.
//!
//! `next`, `previous`, `toggle` and `seek` are plain functions here, so the
//! OS media controls (PLAN.md Phase 3) can call them from the main thread.
//!
//! After every change the queue emits `queue-changed` with a `QueueState`
//! and is saved under `player.queue` in `settings`, with the position in
//! the current track and the volume. At launch it is restored paused and
//! not loaded: no file is opened until playback is asked for.

pub mod model;

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::anomp::{Engine, PlayerState};
use crate::audio;
use crate::library::browse::{self, GroupKey, RuleSpec};
use crate::library::commands::{on_library, LibraryState};
use crate::library::Error;
use model::{Player, Queue, QueueState, Repeat, Saved, TrackInfo, Uid};

thread_local! {
    static QUEUE: RefCell<Option<Queue>> = const { RefCell::new(None) };
}

/// Frontend event with a `QueueState` payload.
pub const QUEUE_CHANGED_EVENT: &str = "queue-changed";

const SETTINGS_KEY: &str = "player.queue";

/// What is saved: the queue, plus the volume.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SavedPlayer {
    #[serde(flatten)]
    queue: Saved,
    volume: Option<f64>,
}

/// The engine as the queue drives it, opening library files through their
/// folder's bookmark (`library::access`).
pub struct EnginePlayer<'a> {
    engine: &'a mut Engine,
    library: &'a LibraryState,
}

impl EnginePlayer<'_> {
    /// Resolves the track's folder and holds it open until `open` has
    /// opened the file (see `library::access`).
    fn open(
        &mut self,
        track_id: i64,
        open: impl FnOnce(&mut Engine, &std::path::Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let path = self.library.track_file(track_id)?;
        let _folder = self.library.open_folder_of(&path)?;
        open(self.engine, &path)
    }
}

impl Player for EnginePlayer<'_> {
    fn load(&mut self, track_id: i64) -> Result<(), String> {
        self.open(track_id, |engine, path| engine.load(path))
    }
    fn set_next(&mut self, track_id: Option<i64>) -> Result<(), String> {
        match track_id {
            Some(id) => self.open(id, |engine, path| engine.set_next(Some(path))),
            None => self.engine.set_next(None),
        }
    }
    fn play(&mut self) -> bool {
        self.engine.play()
    }
    fn pause(&mut self) {
        self.engine.pause()
    }
    fn stop(&mut self) {
        self.engine.stop()
    }
    fn seek(&mut self, seconds: f64) -> bool {
        self.engine.seek(seconds)
    }
    fn state(&self) -> PlayerState {
        self.engine.state()
    }
    fn position(&self) -> f64 {
        self.engine.position()
    }
    fn advance_count(&self) -> i64 {
        self.engine.advance_count()
    }
}

/// Restores the saved queue and volume. Call on the main thread after the
/// engine and the library have started.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let saved = match app.try_state::<LibraryState>() {
        Some(library) => {
            let conn = library.conn();
            let saved = load_saved(&conn).map_err(|e| e.to_string())?;
            let tracks = track_infos(&conn, &saved.queue.tracks).map_err(|e| e.to_string())?;
            Some((saved, tracks))
        }
        None => None,
    };
    let queue = match saved {
        Some((saved, tracks)) => {
            if let Some(volume) = saved.volume {
                let _ = audio::engine_mut(|engine| engine.set_volume(volume));
            }
            let tracks = tracks
                .into_iter()
                .map(|track| (track.track_id, track))
                .collect();
            Queue::restore(saved.queue, &tracks, seed())
        }
        None => Queue::new(seed()),
    };
    QUEUE.with_borrow_mut(|slot| *slot = Some(queue));
    Ok(())
}

/// Saves the queue and drops it. Call on the main thread before the engine
/// shuts down.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let queue = QUEUE.with_borrow_mut(Option::take);
    if let Some(queue) = queue {
        save(app, &queue);
    }
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64)
}

/// Runs `f` on the main thread with the queue and the engine, then emits
/// and saves the queue if it changed.
fn run<R, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut Queue, &mut EnginePlayer) -> T + Send + 'static,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
{
    let app = app.clone();
    audio::on_main(&app.clone(), move || {
        let library = app
            .try_state::<LibraryState>()
            .ok_or("The library is not available")?;
        QUEUE.with(|slot| {
            let mut slot = slot.try_borrow_mut().map_err(|_| "The queue is busy")?;
            let queue = slot.as_mut().ok_or("The queue is not available")?;
            let result = audio::engine_mut(|engine| {
                let mut player = EnginePlayer {
                    engine,
                    library: &library,
                };
                f(queue, &mut player)
            })?;
            publish(&app, queue);
            Ok(result)
        })
    })?
}

/// Emits and saves the queue's state if it changed.
fn publish<R: Runtime>(app: &AppHandle<R>, queue: &mut Queue) {
    if let Some(state) = queue.take_state() {
        for skipped in &state.skipped {
            eprintln!("[queue] skipped {}: {}", skipped.title, skipped.error);
        }
        let _ = app.emit(QUEUE_CHANGED_EVENT, &state);
        save(app, queue);
    }
}

fn save<R: Runtime>(app: &AppHandle<R>, queue: &Queue) {
    let (position, volume) =
        audio::engine_mut(|engine| (engine.position(), engine.volume())).unwrap_or((0.0, 1.0));
    let saved = SavedPlayer {
        queue: queue.saved(position),
        volume: Some(volume),
    };
    let Some(library) = app.try_state::<LibraryState>() else {
        return;
    };
    let result = serde_json::to_string(&saved)
        .map_err(|e| e.to_string())
        .and_then(|json| {
            library
                .conn()
                .execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    (SETTINGS_KEY, json),
                )
                .map_err(|e| e.to_string())
        });
    if let Err(error) = result {
        eprintln!("[queue] cannot save: {error}");
    }
}

fn load_saved(conn: &Connection) -> Result<SavedPlayer, Error> {
    let json: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    // Unreadable (e.g. from a newer version): start empty.
    Ok(json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

/// What the queue shows about each track in `ids`, in that order; ids not
/// in the library are left out.
pub fn track_infos(conn: &Connection, ids: &[i64]) -> Result<Vec<TrackInfo>, Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(ids).expect("ids serialize");
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.title, t.relative_path, artist.name, album.title, t.album_id, t.duration
         FROM tracks t
         LEFT JOIN artists artist ON artist.id = t.artist_id
         LEFT JOIN albums album ON album.id = t.album_id
         WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        let title: Option<String> = row.get(1)?;
        let relative: String = row.get(2)?;
        Ok(TrackInfo {
            track_id: row.get(0)?,
            title: title.unwrap_or_else(|| relative.rsplit('/').next().unwrap_or("").to_owned()),
            artist: row.get(3)?,
            album: row.get(4)?,
            album_id: row.get(5)?,
            duration: row.get(6)?,
        })
    })?;
    let by_id: HashMap<i64, TrackInfo> = rows
        .map(|row| row.map(|track| (track.track_id, track)))
        .collect::<Result<_, _>>()?;
    Ok(ids.iter().filter_map(|id| by_id.get(id).cloned()).collect())
}

// ---- Engine events and other hosts ------------------------------------------

/// The engine reported `TrackEnded`. Main thread.
pub fn on_track_ended<R: Runtime>(app: &AppHandle<R>, advanced: bool) {
    if let Err(error) = run(app, move |queue, player| {
        queue.on_track_ended(player, advanced)
    }) {
        eprintln!("[queue] {error}");
    }
}

/// Something else was loaded into the engine (the dev page).
pub fn detach<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, _| queue.detach())
}

/// Re-reads the queued tracks' titles and so on, after a scan.
pub async fn refresh_tracks<R: Runtime>(app: &AppHandle<R>) {
    let Ok(ids) = run(app, |queue, _| queue.track_ids()) else {
        return;
    };
    let Ok(tracks) = on_library(app, move |library| track_infos(&library.conn(), &ids)).await
    else {
        return;
    };
    let tracks: HashMap<i64, TrackInfo> = tracks
        .into_iter()
        .map(|track| (track.track_id, track))
        .collect();
    let _ = run(app, move |queue, _| queue.update_tracks(&tracks));
}

pub fn next<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| queue.next(player))
}

pub fn previous<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| queue.previous(player))
}

pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| {
        queue.toggle(player);
        // Nothing in the queue changes on pause, but save where it paused.
        queue.touch();
    })
}

pub fn seek<R: Runtime>(app: &AppHandle<R>, seconds: f64) -> Result<(), String> {
    run(app, move |queue, player| queue.seek(player, seconds))
}

// ---- Commands ---------------------------------------------------------------

/// The whole queue state, list included.
#[tauri::command]
pub fn queue_state<R: Runtime>(app: AppHandle<R>) -> Result<QueueState, String> {
    run(&app, |queue, _| queue.state())
}

/// Replaces the queue with these tracks and plays from `start`.
#[tauri::command]
pub async fn queue_play<R: Runtime>(
    app: AppHandle<R>,
    track_ids: Vec<i64>,
    start: usize,
) -> Result<(), String> {
    let tracks = on_library(&app, move |library| {
        track_infos(&library.conn(), &track_ids)
    })
    .await?;
    run(&app, move |queue, player| {
        queue.replace(player, tracks, start, true)
    })
}

/// Replaces the queue with the tracks under a browse node (see
/// `browse::node_track_ids`) and plays from `start_track_id`, or the first.
/// `rule` is a rule id or a whole rule.
#[tauri::command]
pub async fn queue_play_node<R: Runtime>(
    app: AppHandle<R>,
    rule: RuleSpec,
    path: Vec<Option<GroupKey>>,
    recursive: bool,
    start_track_id: Option<i64>,
) -> Result<(), String> {
    let tracks = node_tracks(&app, rule, path, recursive).await?;
    let start = start_track_id
        .and_then(|id| tracks.iter().position(|track| track.track_id == id))
        .unwrap_or(0);
    run(&app, move |queue, player| {
        queue.replace(player, tracks, start, true)
    })
}

/// Adds tracks after the current one (`next`) or at the end.
#[tauri::command]
pub async fn queue_add<R: Runtime>(
    app: AppHandle<R>,
    track_ids: Vec<i64>,
    next: bool,
) -> Result<(), String> {
    let tracks = on_library(&app, move |library| {
        track_infos(&library.conn(), &track_ids)
    })
    .await?;
    run(&app, move |queue, player| queue.add(player, tracks, next))
}

/// Adds the tracks under a browse node, like `queue_add`.
#[tauri::command]
pub async fn queue_add_node<R: Runtime>(
    app: AppHandle<R>,
    rule: RuleSpec,
    path: Vec<Option<GroupKey>>,
    recursive: bool,
    next: bool,
) -> Result<(), String> {
    let tracks = node_tracks(&app, rule, path, recursive).await?;
    run(&app, move |queue, player| queue.add(player, tracks, next))
}

async fn node_tracks<R: Runtime>(
    app: &AppHandle<R>,
    rule: RuleSpec,
    path: Vec<Option<GroupKey>>,
    recursive: bool,
) -> Result<Vec<TrackInfo>, String> {
    on_library(app, move |library| {
        let conn = library.conn();
        let ids = browse::node_track_ids_rule(&conn, &rule, &path, recursive)?;
        track_infos(&conn, &ids)
    })
    .await
}

#[tauri::command]
pub fn queue_remove<R: Runtime>(app: AppHandle<R>, uids: Vec<Uid>) -> Result<(), String> {
    run(&app, move |queue, player| queue.remove(player, &uids))
}

/// Moves an item to index `to` of the list after the move.
#[tauri::command]
pub fn queue_move<R: Runtime>(app: AppHandle<R>, uid: Uid, to: usize) -> Result<(), String> {
    run(&app, move |queue, player| queue.move_item(player, uid, to))
}

#[tauri::command]
pub fn queue_clear<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    run(&app, |queue, player| queue.clear(player))
}

#[tauri::command]
pub fn queue_jump<R: Runtime>(app: AppHandle<R>, uid: Uid) -> Result<(), String> {
    run(&app, move |queue, player| queue.jump(player, uid))
}

#[tauri::command]
pub fn queue_next<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    next(&app)
}

#[tauri::command]
pub fn queue_previous<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    previous(&app)
}

/// Plays or pauses; after a relaunch, the first play opens the track.
#[tauri::command]
pub fn queue_toggle<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    toggle(&app)
}

/// Seeks in the current track, or sets where it will start if it isn't
/// open yet.
#[tauri::command]
pub fn queue_seek<R: Runtime>(app: AppHandle<R>, seconds: f64) -> Result<(), String> {
    seek(&app, seconds)
}

#[tauri::command]
pub fn queue_set_shuffle<R: Runtime>(app: AppHandle<R>, shuffle: bool) -> Result<(), String> {
    run(&app, move |queue, player| {
        queue.set_shuffle(player, shuffle)
    })
}

#[tauri::command]
pub fn queue_set_repeat<R: Runtime>(app: AppHandle<R>, repeat: Repeat) -> Result<(), String> {
    run(&app, move |queue, player| queue.set_repeat(player, repeat))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::test_library::{track, Library};

    #[test]
    fn reads_track_infos_in_the_order_asked() {
        let library = Library::new([
            track("a/1.flac").title("One").artist("A").album("X"),
            track("a/Untitled.flac").artist("B"),
        ]);
        let ids: Vec<i64> = library
            .conn
            .prepare("SELECT id FROM tracks ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let infos = track_infos(&library.conn, &[ids[1], 999, ids[0], ids[1]]).unwrap();
        let titles: Vec<&str> = infos.iter().map(|info| info.title.as_str()).collect();
        assert_eq!(titles, ["Untitled.flac", "One", "Untitled.flac"]);
        assert_eq!(infos[1].album.as_deref(), Some("X"));
        assert!(infos[1].album_id.is_some());
        assert_eq!(infos[0].album_id, None);
    }

    #[test]
    fn saved_state_round_trips_through_settings() {
        let library = Library::new([]);
        assert_eq!(load_saved(&library.conn).unwrap().queue, Saved::default());
        let saved = SavedPlayer {
            queue: Saved {
                tracks: vec![3, 1, 2],
                original: Some(vec![1, 2, 0]),
                current: Some(1),
                position: 12.5,
                repeat: Repeat::One,
            },
            volume: Some(0.25),
        };
        library
            .conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                (SETTINGS_KEY, serde_json::to_string(&saved).unwrap()),
            )
            .unwrap();
        let loaded = load_saved(&library.conn).unwrap();
        assert_eq!(loaded.queue, saved.queue);
        assert_eq!(loaded.volume, Some(0.25));

        library
            .conn
            .execute(
                "UPDATE settings SET value = 'nonsense' WHERE key = ?1",
                [SETTINGS_KEY],
            )
            .unwrap();
        assert_eq!(load_saved(&library.conn).unwrap().queue, Saved::default());
    }
}
