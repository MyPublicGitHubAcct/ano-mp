//! The play queue (`model`), hosted on the main thread next to the engine,
//! with its Tauri commands.
//!
//! The queue lives in a main-thread thread-local like the engine, and every
//! operation runs there (`run`), so it never waits on the engine from
//! another thread while holding its own lock. Commands that need the
//! database first (to turn track ids into what the queue shows) do that on
//! a blocking thread, then hop over.
//!
//! `play`, `pause`, `toggle`, `next`, `previous` and `seek` are plain
//! functions here, which the OS media controls (`media`) call from the main
//! thread.
//!
//! After every change the queue emits `queue-changed` with a `QueueState`
//! and is saved: its items as rows of `queue_items` (`store`, PLAN.md H16),
//! written as the same edits the frontend gets, and the rest (the current
//! item, the position in it, repeat, shuffle, the volume) under
//! `player.queue` in `settings`. At launch it is restored paused and
//! not loaded: no file is opened until playback is asked for. Long tracks'
//! positions go to `track_positions` (PLAN.md F17).
//!
//! Files opened from outside the library (F5, `library::external`) are
//! queue items with negative track ids, played from their path.
//!
//! Tracks are opened off the main thread (`opening`, PLAN.md H11): the
//! queue shows one as loading until the engine reports it, and gives up on
//! it after `model::LOAD_TIMEOUT` (`check_loads`, which a timer thread runs
//! at the queue's `load_deadline`).

pub mod model;
mod opening;
pub mod radio;
pub(crate) mod store;

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::anomp::{Engine, PlayerState};
use crate::audio;
use crate::library::browse::{self, Filter, GroupKey, RuleSpec};
use crate::library::commands::{on_library, LibraryState};
use crate::library::external;
use crate::library::playback::TrackPlay;
use crate::library::Error;
use crate::settings::{self, FeatureSettings, PlaybackSettings};
use model::{
    ListLog, Opening, Player, Queue, QueueState, Repeat, Request, Saved, SleepTimer, TrackInfo, Uid,
};
use store::Store;

thread_local! {
    static QUEUE: RefCell<Option<Queue>> = const { RefCell::new(None) };
    /// The saved rows' mirror, beside the queue on the main thread.
    static STORE: RefCell<Option<Store>> = const { RefCell::new(None) };
}

/// Frontend event with a `QueueState` payload.
pub const QUEUE_CHANGED_EVENT: &str = "queue-changed";

const SETTINGS_KEY: &str = "player.queue";

/// What the setting holds besides the rows: small, so it is written with
/// every state.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct SavedPlayer {
    /// An index in the play order of the saved (library) items.
    current: Option<usize>,
    /// Seconds into the current track.
    position: f64,
    repeat: Repeat,
    /// Shuffle is on (which the rows show only when there are some).
    shuffled: bool,
    volume: Option<f64>,
}

/// The engine as the queue drives it, opening library files through their
/// folder's bookmark (`library::access`), each as `library::playback`
/// says: its part of the file, its gain, what to skip. Files open off the
/// main thread (`opening`).
pub struct EnginePlayer<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    engine: &'a mut Engine,
    library: &'a LibraryState,
    playback: PlaybackSettings,
    features: FeatureSettings,
}

impl<R: Runtime> EnginePlayer<'_, R> {
    /// How to play track `track_id`.
    fn track_play(&self, track_id: i64) -> Result<TrackPlay, String> {
        opening::track_play(self.library, track_id, &self.features)
    }

    /// Gives the engine's copy of the track its gain under the current
    /// settings.
    fn refresh_gain(&mut self, track_id: i64) -> Result<(), String> {
        let play = self.track_play(track_id)?;
        let gain = play.options(&self.playback).gain;
        self.engine.set_track_gain_at(&play.path, play.start, gain);
        Ok(())
    }

    fn ask(&self, track_id: i64, next: bool, crossfade: f64) -> Opening {
        Opening::Pending(opening::ask(
            self.app,
            opening::Ask {
                track_id,
                next,
                crossfade,
                playback: self.playback.clone(),
                features: self.features.clone(),
            },
        ))
    }
}

impl<R: Runtime> EnginePlayer<'_, R> {
    /// Practice mode (O12): loops the current track between two points, or
    /// clears the loop. The engine opens the file again for the jump, so
    /// its folder is held open meanwhile.
    fn set_loop(
        &mut self,
        track_id: i64,
        points: Option<(f64, f64)>,
    ) -> Result<Option<(f64, f64)>, String> {
        if points.is_some() {
            let play = self.track_play(track_id)?;
            let _folder = self.library.open_folder_of(&play.path)?;
            self.engine.set_loop(points)?;
        } else {
            self.engine.set_loop(None)?;
        }
        Ok(self.engine.loop_points())
    }
}

impl<R: Runtime> Player for EnginePlayer<'_, R> {
    fn load(&mut self, track_id: i64) -> Opening {
        self.ask(track_id, false, 0.0)
    }
    fn set_next(&mut self, track_id: Option<i64>, crossfade: bool) -> Opening {
        let seconds = if crossfade {
            self.playback.crossfade
        } else {
            0.0
        };
        match track_id {
            Some(id) => self.ask(id, true, seconds),
            None => Opening::Done(self.engine.set_next_track(None)),
        }
    }
    fn cancel(&mut self, request: Request) {
        opening::cancel(self.engine, request);
    }
    fn volume(&self) -> f64 {
        self.engine.volume()
    }
    fn set_volume(&mut self, volume: f64) {
        self.engine.set_volume(volume);
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
    let (queue, store) = match app.try_state::<LibraryState>() {
        Some(library) => {
            let conn = library.conn();
            let features = settings::current(app).features;
            let (queue, store, volume) =
                restore(&conn, &features, seed()).map_err(|e| e.to_string())?;
            if let Some(volume) = volume {
                let _ = audio::engine_mut(|engine| engine.set_volume(volume));
            }
            (queue, store)
        }
        None => (Queue::new(seed()), Store::default()),
    };
    QUEUE.with_borrow_mut(|slot| *slot = Some(queue));
    STORE.with_borrow_mut(|slot| *slot = Some(store));
    Ok(())
}

/// Saves the queue and drops it. Call on the main thread before the engine
/// shuts down.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let queue = QUEUE.with_borrow_mut(Option::take);
    if let Some(mut queue) = queue {
        if let Ok(position) = audio::engine_mut(|engine| engine.position()) {
            queue.remember_position(&Position(position));
        }
        save_positions(app, &mut queue);
        save(app, &mut queue);
    }
    STORE.with_borrow_mut(Option::take);
}

/// A player that only knows where it is, for noting a position at quit.
struct Position(f64);

impl Player for Position {
    fn load(&mut self, _: i64) -> Opening {
        Opening::Done(Err("not a player".into()))
    }
    fn set_next(&mut self, _: Option<i64>, _: bool) -> Opening {
        Opening::Done(Ok(()))
    }
    fn cancel(&mut self, _: Request) {}
    fn volume(&self) -> f64 {
        1.0
    }
    fn set_volume(&mut self, _: f64) {}
    fn play(&mut self) -> bool {
        false
    }
    fn pause(&mut self) {}
    fn stop(&mut self) {}
    fn seek(&mut self, _: f64) -> bool {
        false
    }
    fn state(&self) -> PlayerState {
        PlayerState::Paused
    }
    fn position(&self) -> f64 {
        self.0
    }
    fn advance_count(&self) -> i64 {
        0
    }
}

/// Writes the long tracks' positions the queue noted.
fn save_positions<R: Runtime>(app: &AppHandle<R>, queue: &mut Queue) {
    let positions = queue.take_positions();
    if positions.is_empty() {
        return;
    }
    let Some(library) = app.try_state::<LibraryState>() else {
        return;
    };
    let conn = library.conn();
    for (track_id, position) in positions {
        let result = match position {
            Some(position) => conn.execute(
                "INSERT INTO track_positions (track_id, position, saved_at)
                 SELECT id, ?2, ?3 FROM tracks WHERE id = ?1
                 ON CONFLICT (track_id) DO UPDATE SET position = excluded.position,
                     saved_at = excluded.saved_at",
                rusqlite::params![track_id, position, crate::library::unix_now()],
            ),
            None => conn.execute(
                "DELETE FROM track_positions WHERE track_id = ?1",
                [track_id],
            ),
        };
        if let Err(error) = result {
            log::warn!("cannot save a position: {error}");
        }
    }
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64)
}

/// The clock sleep timers run by: Unix time in seconds.
fn clock() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_secs_f64())
}

/// Runs `f` on the main thread with the queue and the engine, then emits
/// and saves the queue if it changed.
fn run<R, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut Queue, &mut EnginePlayer<R>) -> T + Send + 'static,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
{
    let app = app.clone();
    let settings = settings::current(&app);
    let (playback, features) = (settings.playback, settings.features);
    audio::on_main(&app.clone(), move || {
        let library = app
            .try_state::<LibraryState>()
            .ok_or("The library is not available")?;
        QUEUE.with(|slot| {
            let mut slot = slot.try_borrow_mut().map_err(|_| "The queue is busy")?;
            let queue = slot.as_mut().ok_or("The queue is not available")?;
            queue.set_clock(clock());
            let result = audio::engine_mut(|engine| {
                let mut player = EnginePlayer {
                    app: &app,
                    engine,
                    library: &library,
                    playback,
                    features,
                };
                f(queue, &mut player)
            })?;
            save_positions(&app, queue);
            schedule_load_check(&app, queue.load_deadline());
            publish(&app, queue);
            Ok(result)
        })
    })?
}

thread_local! {
    /// The deadline a timer thread will check the queue's loads at.
    static LOAD_CHECK: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
}

/// Makes sure the queue's loads are checked at `deadline` (H11): a load
/// still opening then fails. Main thread.
fn schedule_load_check<R: Runtime>(app: &AppHandle<R>, deadline: Option<f64>) {
    let Some(deadline) = deadline else {
        return;
    };
    if LOAD_CHECK.get() == Some(deadline) {
        return;
    }
    LOAD_CHECK.set(Some(deadline));
    let wait = std::time::Duration::from_secs_f64((deadline - clock()).max(0.0) + 0.05);
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("anomp-load-timeout".into())
        .spawn(move || {
            std::thread::sleep(wait);
            if let Err(error) = run(&app, |queue, player| queue.check_loads(player, clock())) {
                log::warn!("{error}");
            }
        });
    if let Err(error) = spawned {
        log::warn!("cannot time a load: {error}");
    }
}

/// A track the queue asked to open is open, or failed (`opening`). Main
/// thread.
fn load_finished<R: Runtime>(app: &AppHandle<R>, request: Request, result: Result<(), String>) {
    if let Err(error) = run(app, move |queue, player| {
        queue.load_finished(player, request, result)
    }) {
        log::warn!("{error}");
    }
}

/// A track the queue asked to open is a cloud placeholder, downloading
/// (`opening`, H12). Main thread.
fn downloading<R: Runtime>(app: &AppHandle<R>, request: Request) {
    log::info!("downloading a cloud placeholder to play it");
    if let Err(error) = run(app, move |queue, _| queue.downloading(request)) {
        log::warn!("{error}");
    }
}

/// The engine reported an asynchronous load. Main thread.
pub fn on_load_finished<R: Runtime>(
    app: &AppHandle<R>,
    id: crate::anomp::LoadRequest,
    result: crate::anomp::LoadResult,
) {
    opening::engine_finished(app, id, result);
}

/// Emits and saves the queue's state if it changed.
fn publish<R: Runtime>(app: &AppHandle<R>, queue: &mut Queue) {
    if let Some(state) = queue.take_state() {
        for skipped in &state.skipped {
            log::warn!("skipped track {}: {}", skipped.track_id, skipped.error);
            log::debug!("skipped {}", skipped.title);
        }
        let _ = app.emit(QUEUE_CHANGED_EVENT, &state);
        crate::media::queue_changed(app, &state);
        crate::history::queue_changed(app, &state);
        crate::recording::queue_changed(app, &state);
        // The album playing is looked up ahead of background work, and the
        // track analysed ahead of the library, for its waveform.
        let playing = state.current_item.as_ref().filter(|_| state.loaded);
        crate::metadata::worker::playing(app, playing.and_then(|item| item.track.album_id));
        if let Some(item) = playing.filter(|item| !item.track.external) {
            crate::library::analysis::playing(app, item.track.track_id);
        }
        save(app, queue);
        fill_radio(app, queue);
        SLEEPING.store(state.sleep.is_some(), Ordering::Relaxed);
        crate::shell::queue_changed(app, &state);
    }
}

/// Set while a sleep timer runs, so position reports tick it (and only
/// then).
static SLEEPING: AtomicBool = AtomicBool::new(false);

/// The engine reported its position (every 50 ms while playing): runs a
/// sleep timer. Main thread.
pub fn tick<R: Runtime>(app: &AppHandle<R>) {
    if SLEEPING.load(Ordering::Relaxed) {
        let _ = run(app, |queue, player| queue.tick(player, clock()));
    }
}

/// The user set the volume: a sleep timer's fade carries on from it.
pub fn volume_changed<R: Runtime>(app: &AppHandle<R>) {
    if SLEEPING.load(Ordering::Relaxed) {
        let _ = run(app, |queue, _| queue.volume_changed());
    }
}

/// Set while radio tracks are being picked, so one refill runs at a time.
static RADIO_FILLING: AtomicBool = AtomicBool::new(false);

/// Radio mode (or any queue, if the settings say so) near its end: picks
/// more tracks like the last one on a blocking thread, then adds them.
fn fill_radio<R: Runtime>(app: &AppHandle<R>, queue: &Queue) {
    let features = settings::current(app).features;
    if !features.library_radio || !(queue.is_radio() || features.radio_after_queue) {
        return;
    }
    let Some(seed) = queue.radio_seed(2) else {
        return;
    };
    if RADIO_FILLING.swap(true, Ordering::SeqCst) {
        return;
    }
    let exclude: HashSet<i64> = queue.track_ids().into_iter().collect();
    let unreadable = crate::library::availability::unreadable(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = on_library(&app, move |library| {
            let conn = library.conn();
            let picks = radio::picks(
                &conn,
                seed,
                &exclude,
                &unreadable,
                radio::BATCH,
                seed_random(),
            )?;
            with_reasons(&conn, picks, &features)
        })
        .await;
        match result {
            Ok(tracks) if !tracks.is_empty() => {
                let _ = run(&app, move |queue, player| {
                    // Only if it's still wanted.
                    if queue.radio_seed(2).is_some() {
                        queue.add(player, tracks, false);
                    }
                });
            }
            Ok(_) => {}
            Err(error) => log::warn!("{error}"),
        }
        RADIO_FILLING.store(false, Ordering::SeqCst);
    });
}

/// The picks as queue tracks, each with why it was picked.
fn with_reasons(
    conn: &Connection,
    picks: Vec<radio::Pick>,
    features: &FeatureSettings,
) -> Result<Vec<TrackInfo>, Error> {
    let ids: Vec<i64> = picks.iter().map(|pick| pick.track_id).collect();
    let reasons: HashMap<i64, String> = picks
        .into_iter()
        .map(|pick| (pick.track_id, pick.reason))
        .collect();
    let mut tracks = track_infos(conn, &ids, features)?;
    for track in &mut tracks {
        track.reason = reasons.get(&track.track_id).cloned();
    }
    Ok(tracks)
}

fn seed_random() -> u64 {
    seed()
}

fn save<R: Runtime>(app: &AppHandle<R>, queue: &mut Queue) {
    let (position, volume) =
        audio::engine_mut(|engine| (engine.position(), engine.volume())).unwrap_or((0.0, 1.0));
    let Some(library) = app.try_state::<LibraryState>() else {
        return;
    };
    let conn = library.conn();
    let result = STORE.with_borrow_mut(|store| {
        save_to(
            &conn,
            queue,
            store.get_or_insert_with(Store::default),
            position,
            volume,
        )
    });
    if let Err(error) = result {
        log::warn!("cannot save: {error}");
    }
}

/// Writes what changed: the list's edits to the rows (or the whole list
/// after a reset, or when the edits can't be written), and the setting.
pub(crate) fn save_to(
    conn: &Connection,
    queue: &mut Queue,
    store: &mut Store,
    position: f64,
    volume: f64,
) -> Result<(), Error> {
    match queue.take_stored() {
        ListLog::Clean => {}
        ListLog::Reset => store.reset(conn, queue.items(), queue.original_order())?,
        ListLog::Edits(edits) => {
            if let Err(error) = store.apply(conn, &edits) {
                log::warn!("rewriting the saved queue: {error}");
                store.reset(conn, queue.items(), queue.original_order())?;
            }
        }
    }
    let saved = queue.saved(position);
    let player = SavedPlayer {
        current: saved.current,
        position: saved.position,
        repeat: saved.repeat,
        shuffled: queue.original_order().is_some(),
        volume: Some(volume),
    };
    let json = serde_json::to_string(&player).expect("the saved player serializes");
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        (SETTINGS_KEY, json),
    )?;
    Ok(())
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

/// The saved queue, from the rows and the setting, not loaded (nothing
/// plays until asked), with the store mirroring its rows and the saved
/// volume. Rows whose track left the library are deleted.
pub(crate) fn restore(
    conn: &Connection,
    features: &FeatureSettings,
    seed: u64,
) -> Result<(Queue, Store, Option<f64>), Error> {
    let player = load_saved(conn)?;
    let (mut store, loaded) = Store::load(conn)?;
    let tracks: Vec<i64> = loaded.items.iter().map(|&(_, track)| track).collect();
    let uids: Vec<Uid> = loaded.items.iter().map(|&(uid, _)| uid).collect();
    let original = match loaded.original {
        Some(order) => {
            let index: HashMap<Uid, usize> =
                uids.iter().enumerate().map(|(i, &uid)| (uid, i)).collect();
            Some(
                order
                    .iter()
                    .filter_map(|uid| index.get(uid).copied())
                    .collect(),
            )
        }
        None => player.shuffled.then(Vec::new),
    };
    let infos = track_infos(conn, &tracks, features)?;
    let infos: HashMap<i64, TrackInfo> = infos
        .into_iter()
        .map(|track| (track.track_id, track))
        .collect();
    let saved = Saved {
        tracks,
        uids,
        original,
        current: player.current,
        position: player.position,
        repeat: player.repeat,
    };
    let queue = Queue::restore(saved, &infos, seed);
    let kept: Vec<Uid> = queue.items().iter().map(|item| item.uid).collect();
    store.retain(conn, &kept)?;
    Ok((queue, store, player.volume))
}

/// What the queue shows about each track in `ids`, in that order; ids not
/// in the library are left out. `features` decides which tracks are to be
/// skipped (O7) and which shuffle together (O3, O6, O7).
pub fn track_infos(
    conn: &Connection,
    ids: &[i64],
    features: &FeatureSettings,
) -> Result<Vec<TrackInfo>, Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let ids_json = serde_json::to_string(ids).expect("ids serialize");
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.title, t.relative_path, IFNULL(t.artist_credit, artist.name), album.title,
                t.album_id, t.duration,
                t.artist_id, IFNULL(tp.skip, ap.skip),
                (SELECT position FROM track_positions pos WHERE pos.track_id = t.id)
         FROM tracks t
         LEFT JOIN artists artist ON artist.id = t.artist_id
         LEFT JOIN albums album ON album.id = t.album_id
         LEFT JOIN track_prefs tp ON tp.track_id = t.id
         LEFT JOIN album_prefs ap ON ap.album_id = t.album_id
         WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids_json], |row| {
        let title: Option<String> = row.get(1)?;
        let relative: String = row.get(2)?;
        let skip: Option<i64> = row.get(8)?;
        Ok(TrackInfo {
            track_id: row.get(0)?,
            title: title.unwrap_or_else(|| relative.rsplit('/').next().unwrap_or("").to_owned()),
            artist: row.get(3)?,
            album: row.get(4)?,
            album_id: row.get(5)?,
            duration: row.get(6)?,
            artist_id: row.get(7)?,
            skip: features.playback_preferences && skip.unwrap_or(0) != 0,
            resume: row.get(9)?,
            ..TrackInfo::default()
        })
    })?;
    let mut by_id: HashMap<i64, TrackInfo> = rows
        .map(|row| row.map(|track| (track.track_id, track)))
        .collect::<Result<_, _>>()?;
    let albums: BTreeSet<i64> = by_id.values().filter_map(|track| track.album_id).collect();
    for album_id in albums {
        for (track_id, unit) in album_units(conn, album_id, features)? {
            if let Some(track) = by_id.get_mut(&track_id) {
                track.unit = Some(unit);
            }
        }
    }
    Ok(ids.iter().filter_map(|id| by_id.get(id).cloned()).collect())
}

/// Level (dBFS) above which a track's first or last 50 ms count as sound,
/// for segues (O3).
const SEGUE_LEVEL: f64 = -60.0;

/// The shuffle units of an album's tracks: the whole album if the user
/// never wants it shuffled (O7), else each work's movements (O6), else each
/// run of tracks that segue into one another (O3), as `features` allows.
fn album_units(
    conn: &Connection,
    album_id: i64,
    features: &FeatureSettings,
) -> Result<Vec<(i64, i64)>, Error> {
    use std::hash::{Hash, Hasher};
    let unit = |parts: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        parts(&mut hasher);
        hasher.finish() as i64
    };
    if !features.segue_shuffle && !features.classical && !features.playback_preferences {
        return Ok(Vec::new());
    }
    let never_shuffle: bool = conn
        .query_row(
            "SELECT IFNULL(never_shuffle, 0) FROM album_prefs WHERE album_id = ?1",
            [album_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some_and(|value| value != 0);
    let mut statement = conn.prepare_cached(
        "SELECT t.id, t.work, a.start_level, a.end_level
         FROM tracks t
         LEFT JOIN track_analysis a ON a.track_id = t.id AND a.error IS NULL
             AND a.file_size = t.file_size AND a.file_mtime_ns = t.file_mtime_ns
         WHERE t.album_id = ?1
         ORDER BY IFNULL(t.disc_number, 1), t.track_number NULLS LAST, t.relative_path,
                  t.range_start",
    )?;
    let tracks: Vec<(i64, Option<String>, Option<f64>, Option<f64>)> = statement
        .query_map([album_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<_, _>>()?;

    if features.playback_preferences && never_shuffle {
        let key = unit(&|h| (1u8, album_id).hash(h));
        return Ok(tracks.iter().map(|track| (track.0, key)).collect());
    }
    let mut units = Vec::new();
    let mut run_start: Option<i64> = None;
    for (index, (id, work, start, _)) in tracks.iter().enumerate() {
        if let (true, Some(work)) = (
            features.classical,
            work.as_deref().filter(|w| !w.is_empty()),
        ) {
            units.push((*id, unit(&|h| (2u8, album_id, work).hash(h))));
            run_start = None;
            continue;
        }
        let sound = |level: Option<f64>| level.is_some_and(|level| level > SEGUE_LEVEL);
        let segues_in = features.segue_shuffle
            && index > 0
            && tracks[index - 1].1.as_deref().is_none_or(str::is_empty)
            && sound(tracks[index - 1].3)
            && sound(*start);
        if segues_in {
            let first = *run_start.get_or_insert(tracks[index - 1].0);
            let key = unit(&|h| (3u8, album_id, first).hash(h));
            if units
                .last()
                .is_none_or(|&(last, _)| last != tracks[index - 1].0)
            {
                units.push((tracks[index - 1].0, key));
            }
            units.push((*id, key));
        } else {
            run_start = None;
        }
    }
    Ok(units)
}

// ---- Engine events and other hosts ------------------------------------------

/// The engine reported `TrackEnded`. Main thread.
pub fn on_track_ended<R: Runtime>(app: &AppHandle<R>, advanced: bool) {
    if let Err(error) = run(app, move |queue, player| {
        queue.on_track_ended(player, advanced)
    }) {
        log::warn!("{error}");
    }
}

/// The ReplayGain settings changed: gives the tracks the engine has open
/// their new gains.
pub fn refresh_gains<R: Runtime>(app: &AppHandle<R>) {
    let result = run(app, |queue, player| {
        queue
            .engine_track_ids()
            .into_iter()
            .try_for_each(|track_id| player.refresh_gain(track_id))
    });
    if let Err(error) = result.and_then(|refreshed| refreshed) {
        log::warn!("cannot change the gain: {error}");
    }
}

/// Something else was loaded into the engine (the dev page).
#[cfg(debug_assertions)]
pub fn detach<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, _| queue.detach())
}

/// Re-reads the queued tracks' titles and so on, after a scan.
pub async fn refresh_tracks<R: Runtime>(app: &AppHandle<R>) {
    let Ok(ids) = run(app, |queue, _| queue.track_ids()) else {
        return;
    };
    let features = settings::current(app).features;
    let Ok(tracks) = on_library(app, move |library| {
        track_infos(&library.conn(), &ids, &features)
    })
    .await
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

pub fn play<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| queue.play(player))
}

pub fn pause<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| {
        queue.pause(player);
        // Nothing in the queue changes on pause, but save where it paused.
        queue.touch();
    })
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

/// Shuffle on or off, whichever it isn't (the Controls menu).
pub fn toggle_shuffle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| {
        let on = queue.state_shuffle();
        queue.set_shuffle(player, !on);
    })
}

pub fn set_repeat<R: Runtime>(app: &AppHandle<R>, repeat: Repeat) -> Result<(), String> {
    run(app, move |queue, player| queue.set_repeat(player, repeat))
}

/// Stops after the current item, or not if it already would (the Controls
/// menu).
pub fn toggle_stop_after_current<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    run(app, |queue, player| {
        let current = queue.current_item().map(|item| item.uid);
        let uid = if queue.stop_after() == current {
            None
        } else {
            current
        };
        queue.set_stop_after(player, uid);
    })
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
    let features = settings::current(&app).features;
    let tracks = on_library(&app, move |library| {
        track_infos(&library.conn(), &track_ids, &features)
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
    filter: Option<Filter>,
) -> Result<(), String> {
    let tracks = node_tracks(&app, rule, path, recursive, filter.unwrap_or_default()).await?;
    let start = start_track_id.and_then(|id| tracks.iter().position(|track| track.track_id == id));
    run(&app, move |queue, player| {
        queue.replace_from(player, tracks, start.unwrap_or(0), true, start.is_some())
    })
}

/// Adds tracks after the current one (`next`) or at the end.
#[tauri::command]
pub async fn queue_add<R: Runtime>(
    app: AppHandle<R>,
    track_ids: Vec<i64>,
    next: bool,
) -> Result<(), String> {
    let features = settings::current(&app).features;
    let tracks = on_library(&app, move |library| {
        track_infos(&library.conn(), &track_ids, &features)
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
    filter: Option<Filter>,
) -> Result<(), String> {
    let tracks = node_tracks(&app, rule, path, recursive, filter.unwrap_or_default()).await?;
    run(&app, move |queue, player| queue.add(player, tracks, next))
}

async fn node_tracks<R: Runtime>(
    app: &AppHandle<R>,
    rule: RuleSpec,
    path: Vec<Option<GroupKey>>,
    recursive: bool,
    filter: Filter,
) -> Result<Vec<TrackInfo>, String> {
    let features = settings::current(app).features;
    on_library(app, move |library| {
        let conn = library.conn();
        let ids = browse::node_track_ids_rule(&conn, &rule, &path, recursive, filter)?;
        track_infos(&conn, &ids, &features)
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

/// Moves items, keeping their order, to index `to` of the list after the
/// move.
#[tauri::command]
pub fn queue_move_items<R: Runtime>(
    app: AppHandle<R>,
    uids: Vec<Uid>,
    to: usize,
) -> Result<(), String> {
    run(&app, move |queue, player| {
        queue.move_items(player, &uids, to)
    })
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

/// Library radio (O9): replaces the queue with `track_id` and tracks like
/// it, and keeps adding more as it plays.
#[tauri::command]
pub async fn queue_start_radio<R: Runtime>(app: AppHandle<R>, track_id: i64) -> Result<(), String> {
    let features = settings::current(&app).features;
    if !features.library_radio {
        return Err(crate::coded::feature_off("libraryRadio", "Library radio"));
    }
    let unreadable = crate::library::availability::unreadable(&app);
    let tracks = on_library(&app, move |library| {
        let conn = library.conn();
        let picks = radio::picks(
            &conn,
            track_id,
            &HashSet::from([track_id]),
            &unreadable,
            radio::BATCH,
            seed_random(),
        )?;
        let mut tracks = track_infos(&conn, &[track_id], &features)?;
        tracks.extend(with_reasons(&conn, picks, &features)?);
        Ok(tracks)
    })
    .await?;
    if tracks.is_empty() {
        return Err(crate::coded::gone(crate::coded::Gone::Track));
    }
    run(&app, move |queue, player| queue.start_radio(player, tracks))
}

/// Practice mode (O12): loops the current track between `start` and `end`
/// seconds, or clears the loop when either is null. Returns the loop set.
#[tauri::command]
pub fn player_set_loop<R: Runtime>(
    app: AppHandle<R>,
    start: Option<f64>,
    end: Option<f64>,
) -> Result<Option<(f64, f64)>, String> {
    let points = start.zip(end);
    if points.is_some() && !settings::current(&app).features.practice_mode {
        return Err(crate::coded::feature_off("practiceMode", "Practice mode"));
    }
    run(&app, move |queue, player| {
        let track_id = queue
            .current_item()
            .filter(|_| queue.is_loaded())
            .map(|item| item.track.track_id)
            .ok_or("Nothing is playing")?;
        player.set_loop(track_id, points)
    })?
}

/// Ends radio mode; the queue stays as it is.
#[tauri::command]
pub fn queue_stop_radio<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    run(&app, |queue, _| queue.stop_radio())
}

/// Stops playback after item `uid` (F13), or not with null.
#[tauri::command]
pub fn queue_set_stop_after<R: Runtime>(app: AppHandle<R>, uid: Option<Uid>) -> Result<(), String> {
    run(&app, move |queue, player| queue.set_stop_after(player, uid))
}

/// A sleep timer, as the UI asks for it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SleepRequest {
    /// In this many minutes, 1 to 1440.
    Minutes {
        minutes: u32,
    },
    EndOfTrack,
    EndOfAlbum,
}

/// Starts a sleep timer (F13), or ends it with null.
#[tauri::command]
pub fn queue_set_sleep<R: Runtime>(
    app: AppHandle<R>,
    sleep: Option<SleepRequest>,
) -> Result<(), String> {
    let timer = match sleep {
        Some(SleepRequest::Minutes { minutes }) if (1..=1440).contains(&minutes) => {
            Some(SleepTimer::At {
                ends_at: clock() + f64::from(minutes) * 60.0,
                minutes,
            })
        }
        Some(SleepRequest::Minutes { .. }) => {
            return Err(crate::coded::coded(
                "sleepRange",
                &[],
                "A sleep timer runs for 1 minute to a day",
            ))
        }
        Some(SleepRequest::EndOfTrack) => Some(SleepTimer::EndOfTrack),
        Some(SleepRequest::EndOfAlbum) => Some(SleepTimer::EndOfAlbum),
        None => None,
    };
    run(&app, move |queue, player| queue.set_sleep(player, timer))
}

/// Plays files opened from the Finder or dropped on the window (F5): those
/// in the library as library tracks, others as they are, after the current
/// item, starting with the first. Files that can't be read are skipped;
/// fails if none can.
pub async fn open_files<R: Runtime>(app: &AppHandle<R>, paths: Vec<PathBuf>) -> Result<(), String> {
    let features = settings::current(app).features;
    let tracks = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || -> Result<Vec<TrackInfo>, String> {
            let library = app.try_state::<LibraryState>();
            let mut tracks = Vec::new();
            let mut errors = Vec::new();
            for path in paths {
                let in_library = match &library {
                    Some(library) => external::library_track(&library.conn(), &path)
                        .map_err(|e| e.to_string())?
                        .map(|id| track_infos(&library.conn(), &[id], &features))
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .and_then(|mut found| found.pop()),
                    None => None,
                };
                match in_library {
                    Some(track) => tracks.push(track),
                    None => match external::register(&path) {
                        Ok(track) => tracks.push(track),
                        Err(error) => errors.push(error),
                    },
                }
            }
            if tracks.is_empty() {
                return Err(errors
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "Nothing to play".into()));
            }
            Ok(tracks)
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    run(app, move |queue, player| queue.play_now(player, tracks))
}

#[tauri::command]
pub async fn queue_open_files<R: Runtime>(
    app: AppHandle<R>,
    paths: Vec<PathBuf>,
) -> Result<(), String> {
    open_files(&app, paths).await
}

/// The queue's track ids in order (library tracks only), e.g. to save it
/// as a playlist.
pub fn library_track_ids<R: Runtime>(app: &AppHandle<R>) -> Result<Vec<i64>, String> {
    run(app, |queue, _| {
        queue
            .track_ids()
            .into_iter()
            .filter(|&id| id >= 0)
            .collect()
    })
}

/// The queue for an export: library track ids, the current one's index
/// among them, the position in it, and the repeat mode.
pub fn snapshot<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(Vec<i64>, Option<usize>, f64, serde_json::Value), String> {
    run(app, |queue, player| {
        let saved = queue.saved(player.position());
        (
            saved.tracks,
            saved.current,
            saved.position,
            serde_json::to_value(saved.repeat).unwrap_or_default(),
        )
    })
}

/// Replaces an empty queue with imported tracks, paused at `current` and
/// `position`; leaves a queue that has anything alone.
pub async fn restore_imported<R: Runtime>(
    app: &AppHandle<R>,
    ids: Vec<i64>,
    current: Option<usize>,
    position: f64,
) -> Result<bool, String> {
    let features = settings::current(app).features;
    let tracks = on_library(app, move |library| {
        track_infos(&library.conn(), &ids, &features)
    })
    .await?;
    run(app, move |queue, player| {
        if !queue.track_ids().is_empty() || tracks.is_empty() {
            return false;
        }
        queue.replace_from(player, tracks, current.unwrap_or(0), false, true);
        if position > 0.0 {
            queue.seek(player, position);
        }
        true
    })
}

/// Library folders came back or went (PLAN.md H22): the queue passes over
/// the items of folders none of whose files can be opened now, and tries
/// the others.
pub async fn folders_changed<R: Runtime>(app: &AppHandle<R>) {
    let Ok(ids) = run(app, |queue, _| queue.track_ids()) else {
        return;
    };
    let unavailable = app
        .try_state::<crate::library::availability::FolderStates>()
        .map(|states| states.unreadable())
        .unwrap_or_default();
    let Ok(missing) = on_library(app, move |library| {
        crate::library::availability::tracks_in(&library.conn(), &ids, &unavailable)
    })
    .await
    else {
        return;
    };
    let _ = run(app, move |queue, player| {
        queue.set_unavailable_tracks(player, &missing);
    });
}

/// The settings changed: radio mode ends if library radio was turned off,
/// and tracks' shuffle units and skips follow the features.
pub async fn features_changed<R: Runtime>(app: &AppHandle<R>) {
    if !settings::current(app).features.library_radio {
        let _ = run(app, |queue, _| queue.stop_radio());
    }
    refresh_tracks(app).await;
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
        let infos = track_infos(
            &library.conn,
            &[ids[1], 999, ids[0], ids[1]],
            &FeatureSettings::default(),
        )
        .unwrap();
        let titles: Vec<&str> = infos.iter().map(|info| info.title.as_str()).collect();
        assert_eq!(titles, ["Untitled.flac", "One", "Untitled.flac"]);
        assert_eq!(infos[1].album.as_deref(), Some("X"));
        assert!(infos[1].album_id.is_some());
        assert_eq!(infos[0].album_id, None);
    }

    #[test]
    fn a_queue_saved_before_the_rows_restores_shuffled_where_it_was() {
        // Saved as migration 010 left it, then migrated (PLAN.md H16).
        let library = Library::from_conn(crate::library::db::open_in_memory_at(10).unwrap());
        for name in ["a", "b", "c", "d"] {
            library.add(
                library.folder_id,
                track(&format!("{name}.flac")).title(name),
            );
        }
        let ids: Vec<i64> = library
            .conn
            .prepare("SELECT id FROM tracks ORDER BY relative_path")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        // Shuffled as d, b, c (gone since), a; the order before was a b c d.
        let old = serde_json::json!({
            "tracks": [ids[3], ids[1], 9999, ids[0]],
            "original": [3, 1, 2, 0],
            "current": 1, "position": 30.0, "repeat": "one", "volume": 0.4,
        });
        library
            .conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                (SETTINGS_KEY, old.to_string()),
            )
            .unwrap();
        library
            .conn
            .execute_batch(include_str!("../library/migrations/011_queue_items.sql"))
            .unwrap();
        let (mut queue, mut store, volume) =
            restore(&library.conn, &FeatureSettings::default(), 1).unwrap();
        assert_eq!(volume, Some(0.4));
        let titles = |queue: &Queue| -> Vec<String> {
            queue
                .items()
                .iter()
                .map(|item| item.track.title.clone())
                .collect()
        };
        assert_eq!(titles(&queue), ["d", "b", "a"]);
        let state = queue.state();
        assert_eq!(state.current, Some(1));
        assert_eq!(state.resume_at, 30.0);
        assert_eq!(state.repeat, Repeat::One);
        assert!(state.shuffle);
        // The missing track's row is gone, and unshuffling gives a b d.
        let rows: i64 = library
            .conn
            .query_row("SELECT count(*) FROM queue_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 3);
        queue.set_shuffle(&mut Position(0.0), false);
        assert_eq!(titles(&queue), ["a", "b", "d"]);
        save_to(&library.conn, &mut queue, &mut store, 0.0, 0.4).unwrap();
        let (again, _, _) = restore(&library.conn, &FeatureSettings::default(), 1).unwrap();
        assert_eq!(titles(&again), ["a", "b", "d"]);
        assert!(again.original_order().is_none());
    }

    #[test]
    fn an_empty_queue_keeps_shuffle_on() {
        let library = Library::new([]);
        let mut queue = Queue::new(1);
        queue.set_shuffle(&mut Position(0.0), true);
        let mut store = Store::default();
        save_to(&library.conn, &mut queue, &mut store, 0.0, 1.0).unwrap();
        let (restored, _, _) = restore(&library.conn, &FeatureSettings::default(), 1).unwrap();
        assert_eq!(restored.original_order(), Some(&[][..]));
    }

    #[test]
    fn the_setting_reads_back_and_unreadable_ones_start_empty() {
        let library = Library::new([]);
        assert_eq!(load_saved(&library.conn).unwrap(), SavedPlayer::default());
        let saved = SavedPlayer {
            current: Some(1),
            position: 12.5,
            repeat: Repeat::One,
            shuffled: true,
            volume: Some(0.25),
        };
        library
            .conn
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                (SETTINGS_KEY, serde_json::to_string(&saved).unwrap()),
            )
            .unwrap();
        assert_eq!(load_saved(&library.conn).unwrap(), saved);
        library
            .conn
            .execute(
                "UPDATE settings SET value = 'nonsense' WHERE key = ?1",
                [SETTINGS_KEY],
            )
            .unwrap();
        assert_eq!(load_saved(&library.conn).unwrap(), SavedPlayer::default());
    }

    #[test]
    fn units_follow_segues_works_and_album_preferences() {
        let library = Library::new([
            track("a/1.flac").album("A").number(1),
            track("a/2.flac").album("A").number(2),
            track("a/3.flac").album("A").number(3),
            track("a/4.flac").album("A").number(4),
            track("a/5.flac").album("A").number(5),
        ]);
        let conn = &library.conn;
        conn.execute_batch(
            "UPDATE tracks SET work = 'Suite' WHERE relative_path IN ('a/4.flac', 'a/5.flac');
             INSERT INTO track_analysis (track_id, file_size, file_mtime_ns, analysed_at,
                                         start_level, end_level)
             SELECT id, 0, 0, 0, -20.0, CASE relative_path WHEN 'a/2.flac' THEN -90.0 ELSE -20.0 END
             FROM tracks;",
        )
        .unwrap();
        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM tracks ORDER BY relative_path")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let units = |features: &FeatureSettings| -> Vec<Option<i64>> {
            track_infos(conn, &ids, features)
                .unwrap()
                .into_iter()
                .map(|track| track.unit)
                .collect()
        };
        let features = FeatureSettings::default();
        let found = units(&features);
        // 1 runs into 2; 2 ends in silence; 4 and 5 are one work.
        assert!(found[0].is_some() && found[0] == found[1]);
        assert_eq!(found[2], None);
        assert!(found[3].is_some() && found[3] == found[4] && found[3] != found[0]);

        let none = FeatureSettings {
            segue_shuffle: false,
            classical: false,
            ..FeatureSettings::default()
        };
        assert!(units(&none).iter().all(Option::is_none));

        conn.execute(
            "INSERT INTO album_prefs (album_id, never_shuffle, skip) VALUES (1, 1, NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_prefs (track_id, skip) VALUES (?1, 1)",
            [ids[2]],
        )
        .unwrap();
        let whole = units(&features);
        assert!(whole.iter().all(|unit| unit.is_some() && *unit == whole[0]));
        let infos = track_infos(conn, &ids, &features).unwrap();
        assert!(infos[2].skip && !infos[1].skip);
        let off = FeatureSettings {
            playback_preferences: false,
            ..FeatureSettings::default()
        };
        assert!(track_infos(conn, &ids, &off)
            .unwrap()
            .iter()
            .all(|t| !t.skip));
    }
}
