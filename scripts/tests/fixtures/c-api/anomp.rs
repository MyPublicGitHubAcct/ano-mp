// An excerpt in the shape of app/src-tauri/src/anomp.rs.
use std::os::raw::{c_char, c_int, c_void};

type RawEventCallback = extern "C" fn(event: *const RawEvent, user_data: *mut c_void);

extern "C" {
    fn anomp_version() -> *const c_char;
    fn anomp_engine_set_event_callback(
        engine: *mut RawEngine,
        callback: Option<RawEventCallback>,
        user_data: *mut c_void,
    );
    // fn anomp_engine_stop(engine: *mut RawEngine);
    fn anomp_engine_load(
        engine: *mut RawEngine,
        path: *const c_char,
        gain: f64,
        error: *mut c_char,
        size: usize,
    ) -> c_int;
    fn anomp_engine_play(engine: *mut RawEngine);
}

/// A safe wrapper, outside the extern block.
pub fn version() -> String {
    unsafe { anomp_version() };
    String::new()
}
