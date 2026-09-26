//! Safe wrappers over the anomp_core C API (core/include/anomp/anomp.h).
//! All `unsafe` FFI stays in this module.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr::NonNull;

use serde::Serialize;

#[repr(C)]
struct RawEngine {
    _private: [u8; 0],
}

#[repr(C)]
struct RawEvent {
    kind: c_int,
    state: c_int,
    advanced: c_int,
    position: f64,
    duration: f64,
}

type RawEventCallback = extern "C" fn(event: *const RawEvent, user_data: *mut c_void);

const ANOMP_EVENT_DEVICE_CHANGED: c_int = 1;
const ANOMP_EVENT_STATE_CHANGED: c_int = 2;
const ANOMP_EVENT_POSITION: c_int = 3;
const ANOMP_EVENT_TRACK_ENDED: c_int = 4;

const ANOMP_STATE_STOPPED: c_int = 1;
const ANOMP_STATE_PLAYING: c_int = 2;
const ANOMP_STATE_PAUSED: c_int = 3;

extern "C" {
    fn anomp_version() -> *const c_char;
    fn anomp_can_decode_extension(extension: *const c_char) -> c_int;

    fn anomp_engine_create() -> *mut RawEngine;
    fn anomp_engine_destroy(engine: *mut RawEngine);
    fn anomp_engine_set_event_callback(
        engine: *mut RawEngine,
        callback: Option<RawEventCallback>,
        user_data: *mut c_void,
    );
    fn anomp_engine_open_default_device(
        engine: *mut RawEngine,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_device_name(engine: *mut RawEngine, buffer: *mut c_char, size: usize) -> usize;
    fn anomp_engine_load(
        engine: *mut RawEngine,
        path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_set_next(
        engine: *mut RawEngine,
        path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_play(engine: *mut RawEngine) -> c_int;
    fn anomp_engine_pause(engine: *mut RawEngine);
    fn anomp_engine_stop(engine: *mut RawEngine);
    fn anomp_engine_seek(engine: *mut RawEngine, seconds: f64) -> c_int;
    fn anomp_engine_set_volume(engine: *mut RawEngine, gain: f64);
    fn anomp_engine_volume(engine: *mut RawEngine) -> f64;
    fn anomp_engine_state(engine: *mut RawEngine) -> c_int;
    fn anomp_engine_position(engine: *mut RawEngine) -> f64;
    fn anomp_engine_duration(engine: *mut RawEngine) -> f64;
    fn anomp_engine_play_test_tone(engine: *mut RawEngine, frequency_hz: f64) -> c_int;
    fn anomp_engine_stop_test_tone(engine: *mut RawEngine);
}

/// The core library version as "major.minor.patch".
pub fn version() -> String {
    // SAFETY: anomp_version returns a static, never-null, NUL-terminated string.
    unsafe { CStr::from_ptr(anomp_version()) }
        .to_string_lossy()
        .into_owned()
}

/// Whether the core can decode files with the given extension ("flac", ".mp3").
#[allow(dead_code)]
pub fn can_decode_extension(extension: &str) -> bool {
    let Ok(extension) = CString::new(extension) else {
        return false;
    };
    // SAFETY: the pointer is a valid NUL-terminated string for the whole call.
    unsafe { anomp_can_decode_extension(extension.as_ptr()) != 0 }
}

/// Player state, as reported by `Engine::state` and `Event::StateChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlayerState {
    /// No track loaded.
    Empty,
    /// A track is loaded, at its start.
    Stopped,
    Playing,
    Paused,
}

impl PlayerState {
    fn from_raw(state: c_int) -> PlayerState {
        match state {
            ANOMP_STATE_STOPPED => PlayerState::Stopped,
            ANOMP_STATE_PLAYING => PlayerState::Playing,
            ANOMP_STATE_PAUSED => PlayerState::Paused,
            _ => PlayerState::Empty,
        }
    }
}

/// Events the core reports through the engine's event callback. Player
/// events arrive about every 50 ms, never from inside an engine call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The device list or the open output device changed.
    DeviceChanged,
    StateChanged(PlayerState),
    /// Seconds into the current track, and its length.
    Position { position: f64, duration: f64 },
    /// The current track played to its end. If `advanced`, the next track
    /// took over gaplessly and the host should set a new next track;
    /// otherwise playback stopped.
    TrackEnded { advanced: bool },
}

type EventHandler = Box<dyn FnMut(Event)>;

/// The audio engine. Must be created, used and dropped on the main thread,
/// which must be running the platform run loop; `!Send` and `!Sync` enforce
/// that it stays on the thread that created it.
pub struct Engine {
    raw: NonNull<RawEngine>,
    // Double-boxed so the C side holds a thin pointer with a stable address.
    handler: Option<Box<EventHandler>>,
    _not_send: PhantomData<*mut ()>,
}

impl Engine {
    pub fn new() -> Option<Engine> {
        // SAFETY: no preconditions beyond the main-thread rule documented above.
        let raw = NonNull::new(unsafe { anomp_engine_create() })?;
        Some(Engine {
            raw,
            handler: None,
            _not_send: PhantomData,
        })
    }

    /// Calls `handler` on the main thread for each engine event.
    pub fn set_event_handler(&mut self, handler: impl FnMut(Event) + 'static) {
        let mut handler: Box<EventHandler> = Box::new(Box::new(handler));
        let user_data = (&mut *handler as *mut EventHandler).cast::<c_void>();
        // SAFETY: `user_data` points into `self.handler`, which outlives the
        // registration: it is replaced only after re-registering, and Drop
        // clears the callback before freeing it.
        unsafe { anomp_engine_set_event_callback(self.raw.as_ptr(), Some(on_event), user_data) };
        self.handler = Some(handler);
    }

    pub fn open_default_device(&mut self) -> Result<(), String> {
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; the error buffer is supplied by `with_error`.
        with_error(|error, size| unsafe { anomp_engine_open_default_device(raw, error, size) })
    }

    /// Opens `path` as the current track, clears the next track, and stops
    /// at the start. On failure nothing changes.
    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let path = path_to_cstring(path)?;
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine and `path` a valid C string for the call.
        with_error(|error, size| unsafe { anomp_engine_load(raw, path.as_ptr(), error, size) })
    }

    /// Opens `path` as the track that follows the current one gaplessly, or
    /// clears it with `None`. On failure the previous next track stays.
    pub fn set_next(&mut self, path: Option<&Path>) -> Result<(), String> {
        let path = path.map(path_to_cstring).transpose()?;
        let path_ptr = path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; `path_ptr` is null or points into
        // `path`, which lives until the end of this function.
        with_error(|error, size| unsafe { anomp_engine_set_next(raw, path_ptr, error, size) })
    }

    /// Starts or resumes playback. Returns false if no track is loaded.
    pub fn play(&mut self) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_play(self.raw.as_ptr()) != 0 }
    }

    pub fn pause(&mut self) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_pause(self.raw.as_ptr()) }
    }

    /// Stops and rewinds to the start of the current track.
    pub fn stop(&mut self) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_stop(self.raw.as_ptr()) }
    }

    /// Seeks within the current track (clamped to its length). Returns false
    /// if no track is loaded or `seconds` is not finite.
    pub fn seek(&mut self, seconds: f64) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_seek(self.raw.as_ptr(), seconds) != 0 }
    }

    /// Linear gain, clamped to 0..=1 by the core.
    pub fn set_volume(&mut self, gain: f64) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_set_volume(self.raw.as_ptr(), gain) }
    }

    pub fn volume(&self) -> f64 {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_volume(self.raw.as_ptr()) }
    }

    pub fn state(&self) -> PlayerState {
        // SAFETY: `raw` is a live engine.
        PlayerState::from_raw(unsafe { anomp_engine_state(self.raw.as_ptr()) })
    }

    /// Seconds into the current track.
    pub fn position(&self) -> f64 {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_position(self.raw.as_ptr()) }
    }

    /// Length of the current track in seconds.
    pub fn duration(&self) -> f64 {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_duration(self.raw.as_ptr()) }
    }

    /// Name of the open output device, or `None` if none is open.
    pub fn device_name(&self) -> Option<String> {
        // SAFETY: a null buffer with size 0 only queries the length.
        let len = unsafe { anomp_engine_device_name(self.raw.as_ptr(), std::ptr::null_mut(), 0) };
        if len == 0 {
            return None;
        }
        let mut buffer = vec![0u8; len + 1];
        // SAFETY: the buffer and its length are valid for the call.
        unsafe { anomp_engine_device_name(self.raw.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len()) };
        buffer.truncate(len);
        Some(String::from_utf8_lossy(&buffer).into_owned())
    }

    pub fn play_test_tone(&mut self, frequency_hz: f64) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_play_test_tone(self.raw.as_ptr(), frequency_hz) != 0 }
    }

    pub fn stop_test_tone(&mut self) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_stop_test_tone(self.raw.as_ptr()) }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // SAFETY: `raw` is live and is never used again; the callback is
        // cleared first so the core cannot call into a freed handler.
        unsafe {
            anomp_engine_set_event_callback(self.raw.as_ptr(), None, std::ptr::null_mut());
            anomp_engine_destroy(self.raw.as_ptr());
        }
    }
}

/// Calls a C API function that reports failure as a message in a caller
/// buffer; `call` receives the buffer and its size and returns 1 on success.
fn with_error(call: impl FnOnce(*mut c_char, usize) -> c_int) -> Result<(), String> {
    let mut error = [0 as c_char; 1024];
    if call(error.as_mut_ptr(), error.len()) != 0 {
        return Ok(());
    }
    // SAFETY: the core always NUL-terminates a non-empty buffer.
    Err(unsafe { CStr::from_ptr(error.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

/// The C API takes UTF-8 paths.
fn path_to_cstring(path: &Path) -> Result<CString, String> {
    let text = path
        .to_str()
        .ok_or_else(|| format!("Path is not valid UTF-8: {}", path.display()))?;
    CString::new(text).map_err(|_| format!("Path contains a NUL byte: {}", path.display()))
}

extern "C" fn on_event(event: *const RawEvent, user_data: *mut c_void) {
    // SAFETY: the core passes a valid event for the duration of the call, and
    // `user_data` is the handler registered by `set_event_handler`.
    let (event, handler) = unsafe { (&*event, &mut *user_data.cast::<EventHandler>()) };
    let event = match event.kind {
        ANOMP_EVENT_DEVICE_CHANGED => Event::DeviceChanged,
        ANOMP_EVENT_STATE_CHANGED => Event::StateChanged(PlayerState::from_raw(event.state)),
        ANOMP_EVENT_POSITION => Event::Position {
            position: event.position,
            duration: event.duration,
        },
        ANOMP_EVENT_TRACK_ENDED => Event::TrackEnded {
            advanced: event.advanced != 0,
        },
        _ => return,
    };
    handler(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn decodes_flac_but_not_text() {
        assert!(can_decode_extension("flac"));
        assert!(can_decode_extension(".FLAC"));
        assert!(!can_decode_extension("txt"));
        assert!(!can_decode_extension("fl\0ac"));
    }

    #[test]
    fn paths_become_c_strings() {
        assert_eq!(
            path_to_cstring(Path::new("/Music/Café.flac")).unwrap().to_str(),
            Ok("/Music/Café.flac")
        );
        assert!(path_to_cstring(Path::new("/a\0b.flac")).is_err());
    }

    #[test]
    fn raw_states_map_to_player_states() {
        assert_eq!(PlayerState::from_raw(0), PlayerState::Empty);
        assert_eq!(PlayerState::from_raw(ANOMP_STATE_STOPPED), PlayerState::Stopped);
        assert_eq!(PlayerState::from_raw(ANOMP_STATE_PLAYING), PlayerState::Playing);
        assert_eq!(PlayerState::from_raw(ANOMP_STATE_PAUSED), PlayerState::Paused);
        assert_eq!(PlayerState::from_raw(99), PlayerState::Empty);
    }
}
