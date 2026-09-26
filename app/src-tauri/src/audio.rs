//! Hosts the audio engine on the main thread and exposes it as Tauri commands.
//!
//! JUCE delivers its messages through the main run loop, which Tauri (tao)
//! runs, so the engine is created in `setup`, lives in a main-thread
//! thread-local, and is dropped on `RunEvent::Exit` before the process ends.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::anomp::{Engine, Event, PlayerState};
use crate::library::access::OpenFolder;
use crate::library::commands::LibraryState;

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
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

#[derive(Serialize)]
pub struct PlayerStatus {
    state: PlayerState,
    position: f64,
    duration: f64,
    volume: f64,
}

/// Creates the engine and opens the default output device. Call on the main thread.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let mut engine = Engine::new().ok_or("Failed to start the audio engine")?;
    let app = app.clone();
    engine.set_event_handler(move |event| {
        let _ = match event {
            Event::DeviceChanged => {
                eprintln!("[audio] device changed");
                app.emit(DEVICE_CHANGED_EVENT, ())
            }
            Event::StateChanged(state) => app.emit(PLAYER_STATE_EVENT, state),
            Event::Position { position, duration } => app.emit(
                PLAYER_POSITION_EVENT,
                PositionPayload { position, duration },
            ),
            Event::TrackEnded { advanced } => {
                app.emit(PLAYER_TRACK_ENDED_EVENT, TrackEndedPayload { advanced })
            }
        };
    });
    let opened = engine.open_default_device();
    ENGINE.with_borrow_mut(|slot| *slot = Some(engine));
    opened
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
    let run = move || {
        ENGINE.with(|slot| match slot.try_borrow_mut() {
            Ok(mut slot) => slot
                .as_mut()
                .map(f)
                .ok_or_else(|| "Audio engine not running".to_string()),
            Err(_) => Err("Audio engine busy".to_string()),
        })
    };
    if is_main_thread() {
        return run();
    }
    let (tx, rx) = mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = tx.send(run());
    })
    .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

fn is_main_thread() -> bool {
    // Tauri runs its event loop, and so `setup`, on the process's main thread.
    std::thread::current().name() == Some("main")
}

#[tauri::command]
pub fn audio_device_name<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    with_engine(&app, |engine| engine.device_name())
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

#[tauri::command]
pub fn player_load<R: Runtime>(app: AppHandle<R>, path: PathBuf) -> Result<(), String> {
    let _folder = open_library_folder(&app, &path)?;
    with_engine(&app, move |engine| engine.load(&path))?
}

/// Sets the track that follows the current one gaplessly; `null` clears it.
#[tauri::command]
pub fn player_set_next<R: Runtime>(app: AppHandle<R>, path: Option<PathBuf>) -> Result<(), String> {
    let _folder = path
        .as_deref()
        .map(|path| open_library_folder(&app, path))
        .transpose()?;
    with_engine(&app, move |engine| engine.set_next(path.as_deref()))?
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
