//! Safe wrappers over the anomp_core C API (core/include/anomp/anomp.h).
//! All `unsafe` FFI stays in this module.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::marker::PhantomData;
use std::ptr::NonNull;

#[repr(C)]
struct RawEngine {
    _private: [u8; 0],
}

#[repr(C)]
struct RawEvent {
    kind: c_int,
}

type RawEventCallback = extern "C" fn(event: *const RawEvent, user_data: *mut c_void);

const ANOMP_EVENT_DEVICE_CHANGED: c_int = 1;

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

/// Events the core reports through the engine's event callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The device list or the open output device changed.
    DeviceChanged,
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
        let mut error = [0 as c_char; 512];
        // SAFETY: the buffer and its length are valid for the call.
        let ok = unsafe {
            anomp_engine_open_default_device(self.raw.as_ptr(), error.as_mut_ptr(), error.len())
        };
        if ok != 0 {
            return Ok(());
        }
        // SAFETY: the core always NUL-terminates a non-empty buffer.
        Err(unsafe { CStr::from_ptr(error.as_ptr()) }
            .to_string_lossy()
            .into_owned())
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

extern "C" fn on_event(event: *const RawEvent, user_data: *mut c_void) {
    // SAFETY: the core passes a valid event for the duration of the call, and
    // `user_data` is the handler registered by `set_event_handler`.
    let (kind, handler) = unsafe { ((*event).kind, &mut *user_data.cast::<EventHandler>()) };
    if kind == ANOMP_EVENT_DEVICE_CHANGED {
        handler(Event::DeviceChanged);
    }
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
}
