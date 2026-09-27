//! Hosts the audio engine on the main thread and exposes it as Tauri commands.
//!
//! JUCE delivers its messages through the main run loop, which Tauri (tao)
//! runs, so the engine is created in `setup`, lives in a main-thread
//! thread-local, and is dropped on `RunEvent::Exit` before the process ends.
//!
//! The output device is the one the settings name (`settings::OutputSettings`),
//! or the system default. While the named one is missing (unplugged), the
//! default plays, and the named one is reopened when the device list shows
//! it again.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::anomp::{DeviceInfo, Engine, Event, PlayerState};
use crate::library::access::OpenFolder;
use crate::library::commands::LibraryState;
use crate::settings::{self, OutputSettings};

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    /// The device list when reopening the chosen device last failed, so a
    /// device that won't open isn't retried until the list changes.
    static FAILED_RETRY: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Frontend event emitted when the output device list or device changes.
pub const DEVICE_CHANGED_EVENT: &str = "audio-device-changed";
/// Frontend event with the new `PlayerState` as its payload.
pub const PLAYER_STATE_EVENT: &str = "player-state";
/// Frontend event with a `PositionPayload`, about every 50 ms while playing.
pub const PLAYER_POSITION_EVENT: &str = "player-position";
/// Frontend event with a `TrackEndedPayload`.
pub const PLAYER_TRACK_ENDED_EVENT: &str = "player-track-ended";

#[derive(Clone, Serialize)]
pub struct PositionPayload {
    position: f64,
    duration: f64,
}

#[derive(Clone, Serialize)]
pub struct TrackEndedPayload {
    /// True if the next track took over; the frontend should set a new one.
    advanced: bool,
}

/// The output devices, and which one plays.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutputStatus {
    /// Every output device there is now.
    pub devices: Vec<String>,
    /// The open device, if any.
    pub current: Option<DeviceInfo>,
    /// The settings name a device that isn't there: the default plays.
    pub chosen_missing: bool,
}

#[derive(Serialize)]
pub struct PlayerStatus {
    state: PlayerState,
    position: f64,
    duration: f64,
    volume: f64,
}

/// Creates the engine and opens the output device the settings name (or
/// the default). Call on the main thread, after `settings::init`.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let mut engine = Engine::new().ok_or("Failed to start the audio engine")?;
    let output = settings::current(app).output;
    let app = app.clone();
    engine.set_event_handler(move |event| {
        crate::media::player_event(event);
        let _ = match event {
            Event::DeviceChanged => {
                eprintln!("[audio] device changed");
                reopen_chosen(&app);
                app.emit(DEVICE_CHANGED_EVENT, ())
            }
            Event::StateChanged(state) => app.emit(PLAYER_STATE_EVENT, state),
            Event::Position { position, duration } => app.emit(
                PLAYER_POSITION_EVENT,
                PositionPayload { position, duration },
            ),
            Event::TrackEnded { advanced } => {
                // The queue arms the next track, from inside the engine's
                // event dispatch, which the core allows (anomp.h).
                crate::queue::on_track_ended(&app, advanced);
                app.emit(PLAYER_TRACK_ENDED_EVENT, TrackEndedPayload { advanced })
            }
        };
    });
    let opened = open_output(&mut engine, &output);
    ENGINE.with_borrow_mut(|slot| *slot = Some(engine));
    opened
}

/// Opens the device `output` names with its buffer size, falling back on
/// the default device (with the same buffer size) if it can't be opened;
/// the error is the named device's.
fn open_output(engine: &mut Engine, output: &OutputSettings) -> Result<(), String> {
    let opened = engine.open_device(output.device.as_deref(), output.buffer_size);
    if opened.is_err() && output.device.is_some() {
        if let Err(error) = engine.open_device(None, output.buffer_size) {
            eprintln!("[audio] cannot open the default device either: {error}");
        }
    }
    opened
}

/// Opens the output `output` names, for new settings. On failure the
/// default device plays and the error is returned.
pub fn apply_output<R: Runtime>(app: &AppHandle<R>, output: &OutputSettings) -> Result<(), String> {
    let output = output.clone();
    let result = with_engine(app, move |engine| open_output(engine, &output))?;
    FAILED_RETRY.with_borrow_mut(|failed| *failed = None);
    result
}

/// After a device change: reopens the device the settings name if it has
/// come back. Main thread (engine events arrive there).
fn reopen_chosen<R: Runtime>(app: &AppHandle<R>) {
    let output = settings::current(app).output;
    let Some(chosen) = output.device.clone() else {
        return;
    };
    let result = engine_mut(|engine| {
        if engine.device_name().as_deref() == Some(chosen.as_str()) {
            return None;
        }
        let devices = engine.output_devices();
        let retried = FAILED_RETRY.with_borrow(|failed| failed.as_ref() == Some(&devices));
        if retried || !devices.contains(&chosen) {
            return None;
        }
        Some((
            engine.open_device(Some(&chosen), output.buffer_size),
            devices,
        ))
    });
    match result {
        Ok(Some((Ok(()), _))) => {
            eprintln!("[audio] back to {chosen}");
            FAILED_RETRY.with_borrow_mut(|failed| *failed = None);
        }
        Ok(Some((Err(error), devices))) => {
            eprintln!("[audio] cannot reopen {chosen}: {error}");
            FAILED_RETRY.with_borrow_mut(|failed| *failed = Some(devices));
            // Opening it may have closed the one that was playing.
            let _ = engine_mut(|engine| {
                if engine.device_name().is_none() {
                    let _ = engine.open_device(None, output.buffer_size);
                }
            });
        }
        // The engine is busy: this event came from inside one of its calls,
        // which reports the change again when it's done.
        Ok(None) | Err(_) => {}
    }
}

/// Drops the engine, shutting JUCE down. Call on the main thread.
pub fn shutdown() {
    ENGINE.with_borrow_mut(|slot| *slot = None);
}

/// Runs `f` with the engine on the main thread and returns its result.
fn with_engine<R, T>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut Engine) -> T + Send + 'static,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
{
    on_main(app, move || engine_mut(f))?
}

/// Runs `f` on the main thread, directly if already on it, and returns its
/// result. Never call it while holding a lock the main thread may take.
pub fn on_main<R, T>(
    app: &AppHandle<R>,
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
{
    if is_main_thread() {
        return Ok(f());
    }
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())
}

/// Runs `f` with the engine. Main thread only (see `on_main`).
pub fn engine_mut<T>(f: impl FnOnce(&mut Engine) -> T) -> Result<T, String> {
    debug_assert!(is_main_thread());
    ENGINE.with(|slot| match slot.try_borrow_mut() {
        Ok(mut slot) => slot
            .as_mut()
            .map(f)
            .ok_or_else(|| "Audio engine not running".to_string()),
        Err(_) => Err("Audio engine busy".to_string()),
    })
}

fn is_main_thread() -> bool {
    // Tauri runs its event loop, and so `setup`, on the process's main thread.
    std::thread::current().name() == Some("main")
}

#[tauri::command]
pub fn audio_device_name<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    with_engine(&app, |engine| engine.device_name())
}

/// The output devices there are, and the one playing.
#[tauri::command]
pub fn audio_output_status<R: Runtime>(app: AppHandle<R>) -> Result<OutputStatus, String> {
    let chosen = settings::current(&app).output.device;
    with_engine(&app, move |engine| {
        let devices = engine.output_devices();
        let current = engine.device_info();
        let chosen_missing = chosen.is_some_and(|chosen| !devices.contains(&chosen));
        OutputStatus {
            devices,
            current,
            chosen_missing,
        }
    })
}

#[tauri::command]
pub fn play_test_tone<R: Runtime>(app: AppHandle<R>, frequency: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.play_test_tone(frequency))?
        .then_some(())
        .ok_or_else(|| "No output device is open".to_string())
}

#[tauri::command]
pub fn stop_test_tone<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.stop_test_tone())
}

/// Opens the library folder holding `path`, if any: under the sandbox the
/// engine can open a library track only while its folder's bookmark is
/// resolved (`library::access`). A file stays readable once it is open.
fn open_library_folder<R: Runtime>(
    app: &AppHandle<R>,
    path: &Path,
) -> Result<Option<OpenFolder>, String> {
    app.try_state::<LibraryState>()
        .map_or(Ok(None), |library| library.open_folder_of(path))
}

/// Loads a file directly, for the dev page; the queue (`queue_*`) stops
/// following the engine until it next starts a track itself.
#[tauri::command]
pub fn player_load<R: Runtime>(app: AppHandle<R>, path: PathBuf) -> Result<(), String> {
    let _folder = open_library_folder(&app, &path)?;
    with_engine(&app, move |engine| engine.load(&path, 1.0))??;
    crate::queue::detach(&app)
}

/// Sets the file that follows the current track gaplessly (`null` clears
/// it), for the dev page; detaches the queue like `player_load`.
#[tauri::command]
pub fn player_set_next<R: Runtime>(app: AppHandle<R>, path: Option<PathBuf>) -> Result<(), String> {
    let _folder = path
        .as_deref()
        .map(|path| open_library_folder(&app, path))
        .transpose()?;
    with_engine(&app, move |engine| {
        engine.set_next(path.as_deref().map(|path| (path, 1.0)))
    })??;
    crate::queue::detach(&app)
}

#[tauri::command]
pub fn player_play<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.play())?
        .then_some(())
        .ok_or_else(|| "No track is loaded".to_string())
}

#[tauri::command]
pub fn player_pause<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.pause())
}

#[tauri::command]
pub fn player_stop<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    with_engine(&app, |engine| engine.stop())
}

#[tauri::command]
pub fn player_seek<R: Runtime>(app: AppHandle<R>, seconds: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.seek(seconds))?
        .then_some(())
        .ok_or_else(|| "No track is loaded".to_string())
}

#[tauri::command]
pub fn player_set_volume<R: Runtime>(app: AppHandle<R>, volume: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.set_volume(volume))
}

#[tauri::command]
pub fn player_status<R: Runtime>(app: AppHandle<R>) -> Result<PlayerStatus, String> {
    with_engine(&app, |engine| PlayerStatus {
        state: engine.state(),
        position: engine.position(),
        duration: engine.duration(),
        volume: engine.volume(),
    })
}
