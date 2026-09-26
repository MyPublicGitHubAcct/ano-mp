//! Hosts the audio engine on the main thread and exposes it as Tauri commands.
//!
//! JUCE delivers its messages through the main run loop, which Tauri (tao)
//! runs, so the engine is created in `setup`, lives in a main-thread
//! thread-local, and is dropped on `RunEvent::Exit` before the process ends.

use std::cell::RefCell;
use std::sync::mpsc;

use tauri::{AppHandle, Emitter, Runtime};

use crate::anomp::{Engine, Event};

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
}

/// Frontend event emitted when the output device list or device changes.
pub const DEVICE_CHANGED_EVENT: &str = "audio-device-changed";

/// Creates the engine and opens the default output device. Call on the main thread.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let mut engine = Engine::new().ok_or("Failed to start the audio engine")?;
    let app = app.clone();
    engine.set_event_handler(move |event| match event {
        Event::DeviceChanged => {
            eprintln!("[audio] device changed");
            let _ = app.emit(DEVICE_CHANGED_EVENT, ());
        }
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
fn with_engine<R, T>(app: &AppHandle<R>, f: impl FnOnce(&mut Engine) -> T + Send + 'static) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
{
    let run = move || {
        ENGINE.with(|slot| match slot.try_borrow_mut() {
            Ok(mut slot) => slot.as_mut().map(f).ok_or_else(|| "Audio engine not running".to_string()),
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
