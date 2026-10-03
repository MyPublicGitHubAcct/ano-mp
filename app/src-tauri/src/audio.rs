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
use std::sync::mpsc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

use crate::anomp::{DeviceInfo, Engine, Event, PlayerState, SignalPath};
use crate::settings::{self, EqualiserSettings, FeatureSettings, OutputSettings};

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
/// Frontend event with the volume (0 to 1), when something other than the
/// page changed it (the menu, the remote).
pub const PLAYER_VOLUME_EVENT: &str = "player-volume";

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
#[cfg_attr(test, derive(ts_rs::TS))]
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
    let app_for_init = app.clone();
    let app = app.clone();
    engine.set_event_handler(move |event| {
        crate::media::player_event(&event);
        let _ = match event {
            Event::DeviceChanged => {
                log::info!("device changed");
                reopen_chosen(&app);
                // Headphones may have come or gone with it.
                let current = settings::current(&app);
                let _ = engine_mut(|engine| {
                    apply_crossfeed(engine, &current.features);
                    apply_equaliser_to(engine, &current.equaliser);
                });
                app.emit(DEVICE_CHANGED_EVENT, ())
            }
            Event::StateChanged(state) => {
                crate::shell::playing_changed(&app, state == PlayerState::Playing);
                app.emit(PLAYER_STATE_EVENT, state)
            }
            Event::Position { position, duration } => {
                crate::history::position(&app, position, duration);
                crate::queue::tick(&app);
                app.emit(
                    PLAYER_POSITION_EVENT,
                    PositionPayload { position, duration },
                )
            }
            Event::TrackEnded { advanced } => {
                // The queue arms the next track, from inside the engine's
                // event dispatch, which the core allows (anomp.h).
                crate::queue::on_track_ended(&app, advanced);
                app.emit(PLAYER_TRACK_ENDED_EVENT, TrackEndedPayload { advanced })
            }
            Event::LoadFinished { request, result } => {
                crate::queue::on_load_finished(&app, request, result);
                Ok(())
            }
        };
    });
    let opened = open_output(&mut engine, &output);
    let current = settings::current(&app_for_init);
    apply_crossfeed(&mut engine, &current.features);
    apply_equaliser_to(&mut engine, &current.equaliser);
    ENGINE.with_borrow_mut(|slot| *slot = Some(engine));
    opened
}

/// Sets the engine's crossfeed as `features` say: their level, or off
/// when it's only for headphones and the OS doesn't say they're plugged in.
pub fn apply_crossfeed(engine: &mut Engine, features: &FeatureSettings) {
    let level = features.crossfeed.level();
    let on = !features.crossfeed_headphones_only || engine.output_is_headphones() == Some(true);
    engine.set_crossfeed(if on { level } else { 0 });
}

/// Sets the engine's equaliser as `settings` say, with the headphones
/// profile while the OS says headphones are plugged in, if it follows the
/// output (PLAN.md F15).
pub fn apply_equaliser_to(engine: &mut Engine, settings: &EqualiserSettings) {
    let headphones = if settings.follow_output {
        engine.output_is_headphones()
    } else {
        None
    };
    let applied = match settings.active(headphones) {
        Some(profile) => {
            let mut gains = [0.0; crate::anomp::EQ_BANDS];
            for (gain, value) in gains.iter_mut().zip(&profile.gains) {
                *gain = *value;
            }
            engine.set_equaliser(Some((&gains, profile.preamp)))
        }
        None => engine.set_equaliser(None),
    };
    if !applied {
        log::warn!("the equaliser's settings are out of range");
    }
}

/// The equaliser's settings changed.
pub fn apply_equaliser<R: Runtime>(app: &AppHandle<R>, settings: &EqualiserSettings) {
    let settings = settings.clone();
    let _ = with_engine(app, move |engine| apply_equaliser_to(engine, &settings));
}

/// The features changed: crossfeed, and practice mode's loop and tempo.
pub fn apply_features<R: Runtime>(app: &AppHandle<R>, features: &FeatureSettings) {
    let features = features.clone();
    let _ = with_engine(app, move |engine| {
        apply_crossfeed(engine, &features);
        if !features.practice_mode {
            let _ = engine.set_loop(None);
            engine.set_tempo(1.0, 0.0);
        }
    });
}

/// Opens the device `output` names with its buffer size, falling back on
/// the default device (with the same buffer size) if it can't be opened;
/// the error is the named device's.
fn open_output(engine: &mut Engine, output: &OutputSettings) -> Result<(), String> {
    let opened = engine.open_device(output.device.as_deref(), output.buffer_size);
    if opened.is_err() && output.device.is_some() {
        if let Err(error) = engine.open_device(None, output.buffer_size) {
            log::warn!("cannot open the default device either: {error}");
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
            log::info!("back to {chosen}");
            FAILED_RETRY.with_borrow_mut(|failed| *failed = None);
        }
        Ok(Some((Err(error), devices))) => {
            log::warn!("cannot reopen {chosen}: {error}");
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
pub(crate) fn with_engine<R, T>(
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
pub fn player_set_volume<R: Runtime>(app: AppHandle<R>, volume: f64) -> Result<(), String> {
    with_engine(&app, move |engine| engine.set_volume(volume))?;
    crate::queue::volume_changed(&app);
    Ok(())
}

/// Changes the volume by `step` (e.g. 0.05 up, -0.05 down), for the menu's
/// volume items; returns the new volume.
pub fn change_volume<R: Runtime>(app: &AppHandle<R>, step: f64) -> Result<f64, String> {
    let volume = with_engine(app, move |engine| {
        let volume = (engine.volume() + step).clamp(0.0, 1.0);
        engine.set_volume(volume);
        volume
    })?;
    crate::queue::volume_changed(app);
    Ok(volume)
}

/// Every step from the file to the speakers (O10).
#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SignalPathPayload {
    path: SignalPath,
    device: Option<DeviceInfo>,
    headphones: Option<bool>,
}

#[tauri::command]
pub fn player_signal_path<R: Runtime>(app: AppHandle<R>) -> Result<SignalPathPayload, String> {
    with_engine(&app, |engine| SignalPathPayload {
        path: engine.signal_path(),
        device: engine.device_info(),
        headphones: engine.output_is_headphones(),
    })
}

/// Practice mode (O12): plays at `rate` times the speed (0.5 to 1.5)
/// without changing the pitch, transposed by `semitones` (-12 to 12).
#[tauri::command]
pub fn player_set_tempo<R: Runtime>(
    app: AppHandle<R>,
    rate: f64,
    semitones: f64,
) -> Result<(), String> {
    let changing = rate != 1.0 || semitones != 0.0;
    if changing && !settings::current(&app).features.practice_mode {
        return Err(crate::coded::feature_off("practiceMode", "Practice mode"));
    }
    with_engine(&app, move |engine| engine.set_tempo(rate, semitones))?
        .then_some(())
        .ok_or_else(|| "The tempo must be 50% to 150%, and the pitch within an octave".to_string())
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Practice {
    /// The loop's start and end in seconds, if one is set.
    #[serde(rename = "loop")]
    loop_points: Option<(f64, f64)>,
    tempo: f64,
    semitones: f64,
}

#[tauri::command]
pub fn player_practice<R: Runtime>(app: AppHandle<R>) -> Result<Practice, String> {
    with_engine(&app, |engine| {
        let path = engine.signal_path();
        Practice {
            loop_points: engine.loop_points(),
            tempo: path.tempo,
            semitones: path.semitones,
        }
    })
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
