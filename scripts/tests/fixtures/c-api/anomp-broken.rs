// anomp_engine_load lost a parameter, anomp_engine_play is missing and
// anomp_engine_pause isn't in the header.
extern "C" {
    fn anomp_version() -> *const c_char;
    fn anomp_engine_set_event_callback(
        engine: *mut RawEngine,
        callback: Option<RawEventCallback>,
        user_data: *mut c_void,
    );
    fn anomp_engine_load(engine: *mut RawEngine, path: *const c_char, gain: f64, error: *mut c_char) -> c_int;
    fn anomp_engine_pause(engine: *mut RawEngine);
}
