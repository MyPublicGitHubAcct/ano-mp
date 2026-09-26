//! Safe wrappers over the anomp_core C API (core/include/anomp/anomp.h).
//! All `unsafe` FFI stays in this module.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
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

#[repr(C)]
struct RawTags {
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
    album_artist: *const c_char,
    genre: *const c_char,
    track_number: c_int,
    track_total: c_int,
    disc_number: c_int,
    disc_total: c_int,
    year: c_int,
    duration: f64,
    sample_rate: c_int,
    channels: c_int,
    bitrate_kbps: c_int,
    musicbrainz_recording_id: *const c_char,
    musicbrainz_release_id: *const c_char,
    musicbrainz_release_group_id: *const c_char,
    musicbrainz_release_track_id: *const c_char,
    musicbrainz_artist_id: *const c_char,
    musicbrainz_album_artist_id: *const c_char,
    picture: *const u8,
    picture_size: usize,
    picture_mime_type: *const c_char,
}

const ANOMP_TAGS_PICTURE: c_int = 1;

#[repr(C)]
struct RawBookmark {
    data: *const u8,
    size: usize,
}

#[repr(C)]
struct RawFolderAccess {
    _private: [u8; 0],
}

type RawEventCallback = extern "C" fn(event: *const RawEvent, user_data: *mut c_void);

#[repr(C)]
struct RawMediaControls {
    _private: [u8; 0],
}

#[repr(C)]
struct RawMediaCommand {
    kind: c_int,
    position: f64,
}

#[repr(C)]
struct RawMediaTrack {
    title: *const c_char,
    artist: *const c_char,
    album: *const c_char,
}

type RawMediaCommandCallback =
    extern "C" fn(command: *const RawMediaCommand, user_data: *mut c_void);

const ANOMP_MEDIA_PLAY: c_int = 1;
const ANOMP_MEDIA_PAUSE: c_int = 2;
const ANOMP_MEDIA_TOGGLE: c_int = 3;
const ANOMP_MEDIA_NEXT: c_int = 4;
const ANOMP_MEDIA_PREVIOUS: c_int = 5;
const ANOMP_MEDIA_SEEK: c_int = 6;

const ANOMP_EVENT_DEVICE_CHANGED: c_int = 1;
const ANOMP_EVENT_STATE_CHANGED: c_int = 2;
const ANOMP_EVENT_POSITION: c_int = 3;
const ANOMP_EVENT_TRACK_ENDED: c_int = 4;

const ANOMP_STATE_EMPTY: c_int = 0;
const ANOMP_STATE_STOPPED: c_int = 1;
const ANOMP_STATE_PLAYING: c_int = 2;
const ANOMP_STATE_PAUSED: c_int = 3;

extern "C" {
    fn anomp_version() -> *const c_char;
    fn anomp_can_decode_extension(extension: *const c_char) -> c_int;
    fn anomp_read_tags(
        path: *const c_char,
        flags: c_int,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut RawTags;
    fn anomp_tags_free(tags: *mut RawTags);

    fn anomp_bookmark_create(
        path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut RawBookmark;
    fn anomp_bookmark_free(bookmark: *mut RawBookmark);
    fn anomp_folder_access_start(
        bookmark: *const u8,
        bookmark_size: usize,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut RawFolderAccess;
    fn anomp_folder_access_path(access: *const RawFolderAccess) -> *const c_char;
    fn anomp_folder_access_is_stale(access: *const RawFolderAccess) -> c_int;
    fn anomp_folder_access_stop(access: *mut RawFolderAccess);

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
    fn anomp_engine_advance_count(engine: *mut RawEngine) -> i64;
    fn anomp_media_controls_supported() -> c_int;
    fn anomp_media_controls_create(
        callback: Option<RawMediaCommandCallback>,
        user_data: *mut c_void,
    ) -> *mut RawMediaControls;
    fn anomp_media_controls_destroy(controls: *mut RawMediaControls);
    fn anomp_media_controls_set_track(
        controls: *mut RawMediaControls,
        track: *const RawMediaTrack,
    ) -> c_int;
    fn anomp_media_controls_set_playback(
        controls: *mut RawMediaControls,
        state: c_int,
        elapsed: f64,
        duration: f64,
    ) -> c_int;
    fn anomp_media_controls_set_artwork(
        controls: *mut RawMediaControls,
        data: *const u8,
        size: usize,
    ) -> c_int;
    fn anomp_media_controls_set_navigation(
        controls: *mut RawMediaControls,
        has_next: c_int,
        has_previous: c_int,
    );
    fn anomp_media_controls_clear(controls: *mut RawMediaControls);

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
pub fn can_decode_extension(extension: &str) -> bool {
    let Ok(extension) = CString::new(extension) else {
        return false;
    };
    // SAFETY: the pointer is a valid NUL-terminated string for the whole call.
    unsafe { anomp_can_decode_extension(extension.as_ptr()) != 0 }
}

/// A file's tags and audio properties. Fields the file doesn't have are
/// `None`; several values of one field are joined with "; ".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<u32>,
    /// Seconds, from the file's headers: lossy files can be off by tens of
    /// milliseconds. `Engine::duration` is exact once the track is loaded.
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: u32,
    pub bitrate_kbps: Option<u32>,
    /// MusicBrainz IDs, named after what they identify (Picard's "track id"
    /// is the recording, its "album id" the release).
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_release_track_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub musicbrainz_album_artist_id: Option<String>,
    /// The front cover, or else the first embedded picture; only read when
    /// asked for.
    pub picture: Option<Picture>,
}

/// An embedded picture.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    /// e.g. "image/jpeg"; `None` if unknown.
    pub mime_type: Option<String>,
    pub data: Vec<u8>,
}

/// Reads the tags of the file at `path` without modifying it, copying the
/// embedded picture only if `include_picture`. Unlike the engine, this may be
/// called from any thread.
pub fn read_tags(path: &Path, include_picture: bool) -> Result<Tags, String> {
    let path = path_to_cstring(path)?;
    let flags = if include_picture {
        ANOMP_TAGS_PICTURE
    } else {
        0
    };
    let mut raw = std::ptr::null_mut();
    with_error(|error, size| {
        // SAFETY: `path` is a valid C string and the error buffer is supplied
        // by `with_error`, both for the whole call.
        raw = unsafe { anomp_read_tags(path.as_ptr(), flags, error, size) };
        c_int::from(!raw.is_null())
    })?;
    // SAFETY: `raw` is a non-null result of anomp_read_tags, read once and
    // then freed exactly once.
    unsafe {
        let tags = Tags::from_raw(&*raw);
        anomp_tags_free(raw);
        Ok(tags)
    }
}

impl Tags {
    /// # Safety
    /// `raw` must come from anomp_read_tags and not yet be freed.
    unsafe fn from_raw(raw: &RawTags) -> Tags {
        let text = |ptr: *const c_char| {
            (!ptr.is_null())
                .then(|| CStr::from_ptr(ptr).to_string_lossy().into_owned())
                .filter(|text| !text.is_empty())
        };
        let number = |value: c_int| u32::try_from(value).ok().filter(|&n| n > 0);
        Tags {
            title: text(raw.title),
            artist: text(raw.artist),
            album: text(raw.album),
            album_artist: text(raw.album_artist),
            genre: text(raw.genre),
            track_number: number(raw.track_number),
            track_total: number(raw.track_total),
            disc_number: number(raw.disc_number),
            disc_total: number(raw.disc_total),
            year: number(raw.year),
            duration: raw.duration,
            sample_rate: number(raw.sample_rate).unwrap_or(0),
            channels: number(raw.channels).unwrap_or(0),
            bitrate_kbps: number(raw.bitrate_kbps),
            musicbrainz_recording_id: text(raw.musicbrainz_recording_id),
            musicbrainz_release_id: text(raw.musicbrainz_release_id),
            musicbrainz_release_group_id: text(raw.musicbrainz_release_group_id),
            musicbrainz_release_track_id: text(raw.musicbrainz_release_track_id),
            musicbrainz_artist_id: text(raw.musicbrainz_artist_id),
            musicbrainz_album_artist_id: text(raw.musicbrainz_album_artist_id),
            picture: (!raw.picture.is_null() && raw.picture_size > 0).then(|| Picture {
                mime_type: text(raw.picture_mime_type),
                data: std::slice::from_raw_parts(raw.picture, raw.picture_size).to_vec(),
            }),
        }
    }
}

/// Creates a bookmark for `folder`, the durable handle to a folder the user
/// picked: under the macOS App Sandbox (and on iOS) it is what lets the app
/// read the folder in later sessions, and it follows the folder when it is
/// moved on its volume. Elsewhere it holds the path. The app must be able to
/// read the folder now (the user just picked it, or a `FolderAccess` to it is
/// open). May be called from any thread.
pub fn create_bookmark(folder: &Path) -> Result<Vec<u8>, String> {
    let path = path_to_cstring(folder)?;
    let mut raw = std::ptr::null_mut();
    with_error(|error, size| {
        // SAFETY: `path` is a valid C string and the error buffer is supplied
        // by `with_error`, both for the whole call.
        raw = unsafe { anomp_bookmark_create(path.as_ptr(), error, size) };
        c_int::from(!raw.is_null())
    })?;
    // SAFETY: `raw` is a non-null result of anomp_bookmark_create whose
    // `data` holds `size` bytes; it is copied once and then freed exactly once.
    unsafe {
        let bookmark = std::slice::from_raw_parts((*raw).data, (*raw).size).to_vec();
        anomp_bookmark_free(raw);
        Ok(bookmark)
    }
}

/// Access to a bookmarked folder: the app can read it until this is dropped.
pub struct FolderAccess {
    raw: NonNull<RawFolderAccess>,
    path: PathBuf,
    stale: bool,
}

// SAFETY: the core's folder access may be used and stopped from any thread,
// and this wrapper only reads it after construction.
unsafe impl Send for FolderAccess {}
unsafe impl Sync for FolderAccess {}

impl FolderAccess {
    /// Resolves a bookmark from `create_bookmark` and starts accessing its
    /// folder. Fails if the folder is gone or its volume isn't mounted.
    pub fn start(bookmark: &[u8]) -> Result<FolderAccess, String> {
        let mut raw = std::ptr::null_mut();
        with_error(|error, size| {
            // SAFETY: the bookmark slice and the error buffer are valid for the call.
            raw = unsafe {
                anomp_folder_access_start(bookmark.as_ptr(), bookmark.len(), error, size)
            };
            c_int::from(!raw.is_null())
        })?;
        let raw = NonNull::new(raw).ok_or("Cannot resolve the bookmark")?;
        // SAFETY: `raw` is a live access; its path is a valid C string until it
        // is stopped, and is copied here.
        let (path, stale) = unsafe {
            (
                CStr::from_ptr(anomp_folder_access_path(raw.as_ptr()))
                    .to_string_lossy()
                    .into_owned(),
                anomp_folder_access_is_stale(raw.as_ptr()) != 0,
            )
        };
        Ok(FolderAccess {
            raw,
            path: path.into(),
            stale,
        })
    }

    /// Where the folder is now, which may differ from where it was when the
    /// bookmark was made.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the bookmark should be replaced with a new one for `path()`,
    /// created while this access is open.
    pub fn is_stale(&self) -> bool {
        self.stale
    }
}

impl Drop for FolderAccess {
    fn drop(&mut self) {
        // SAFETY: `raw` is live and is never used again.
        unsafe { anomp_folder_access_stop(self.raw.as_ptr()) }
    }
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
    fn to_raw(self) -> c_int {
        match self {
            PlayerState::Empty => ANOMP_STATE_EMPTY,
            PlayerState::Stopped => ANOMP_STATE_STOPPED,
            PlayerState::Playing => ANOMP_STATE_PLAYING,
            PlayerState::Paused => ANOMP_STATE_PAUSED,
        }
    }

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
/// events arrive about every 50 ms, never from inside an engine call; the
/// handler may call the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// The device list or the open output device changed.
    DeviceChanged,
    StateChanged(PlayerState),
    /// Seconds into the current track, and its length.
    Position {
        position: f64,
        duration: f64,
    },
    /// The current track played to its end. If `advanced`, the next track
    /// took over gaplessly and the host should set a new next track;
    /// otherwise playback stopped.
    TrackEnded {
        advanced: bool,
    },
}

type EventHandler = Box<dyn FnMut(Event)>;

/// The audio engine. Must be created, used and dropped on the main thread,
/// which must be running the platform run loop; `!Send` and `!Sync` enforce
/// that it stays on the thread that created it.
pub struct Engine {
    raw: NonNull<RawEngine>,
    // Double-boxed so the C side holds a thin pointer with a stable address,
    // and kept as a raw pointer (null if none) rather than a Box: the
    // handler may call engine functions, which borrow `self` mutably while
    // it runs.
    handler: *mut EventHandler,
    _not_send: PhantomData<*mut ()>,
}

impl Engine {
    pub fn new() -> Option<Engine> {
        // SAFETY: no preconditions beyond the main-thread rule documented above.
        let raw = NonNull::new(unsafe { anomp_engine_create() })?;
        Some(Engine {
            raw,
            handler: std::ptr::null_mut(),
            _not_send: PhantomData,
        })
    }

    /// Calls `handler` on the main thread for each engine event. The handler
    /// may call the engine (e.g. to set the next track when one ends), but
    /// must not replace itself.
    pub fn set_event_handler(&mut self, handler: impl FnMut(Event) + 'static) {
        let handler: *mut EventHandler = Box::into_raw(Box::new(Box::new(handler)));
        // SAFETY: `handler` stays valid while registered: the previous one is
        // freed only after this replaces it, and Drop clears the callback
        // before freeing it.
        unsafe {
            anomp_engine_set_event_callback(self.raw.as_ptr(), Some(on_event), handler.cast());
            if !self.handler.is_null() {
                drop(Box::from_raw(self.handler));
            }
        }
        self.handler = handler;
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

    /// How many times a next track has taken over since the engine was
    /// created, counted as it happens rather than when `TrackEnded` reports it.
    pub fn advance_count(&self) -> i64 {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_advance_count(self.raw.as_ptr()) }
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
        unsafe {
            anomp_engine_device_name(self.raw.as_ptr(), buffer.as_mut_ptr().cast(), buffer.len())
        };
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
        // cleared first so the core cannot call into a freed handler, and
        // then nothing else holds `handler`.
        unsafe {
            anomp_engine_set_event_callback(self.raw.as_ptr(), None, std::ptr::null_mut());
            anomp_engine_destroy(self.raw.as_ptr());
            if !self.handler.is_null() {
                drop(Box::from_raw(self.handler));
            }
        }
    }
}

/// A command from the OS's media controls (media keys, Control Center, the
/// Now Playing widget, the iOS lock screen).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    Play,
    Pause,
    Toggle,
    /// Only sent while enabled (`MediaControls::set_navigation`).
    Next,
    /// Only sent while enabled.
    Previous,
    /// To this many seconds into the track (finite, within its duration).
    Seek(f64),
}

impl MediaCommand {
    fn from_raw(raw: &RawMediaCommand) -> Option<MediaCommand> {
        Some(match raw.kind {
            ANOMP_MEDIA_PLAY => MediaCommand::Play,
            ANOMP_MEDIA_PAUSE => MediaCommand::Pause,
            ANOMP_MEDIA_TOGGLE => MediaCommand::Toggle,
            ANOMP_MEDIA_NEXT => MediaCommand::Next,
            ANOMP_MEDIA_PREVIOUS => MediaCommand::Previous,
            ANOMP_MEDIA_SEEK => MediaCommand::Seek(raw.position),
            _ => return None,
        })
    }
}

type MediaHandler = Box<dyn Fn(MediaCommand)>;

/// The OS's media controls: what is playing, as the system shows it, and
/// the commands it sends back. Main thread only, like `Engine`; commands
/// reach the handler on the main thread, from the OS, never from inside a
/// call on this object.
pub struct MediaControls {
    raw: NonNull<RawMediaControls>,
    // A raw pointer rather than a Box, so running the handler (from the OS)
    // never overlaps a unique borrow of it through `&mut self`.
    handler: *mut MediaHandler,
    _not_send: PhantomData<*mut ()>,
}

impl MediaControls {
    /// Whether this platform's media controls reach the OS.
    pub fn supported() -> bool {
        // SAFETY: no preconditions.
        unsafe { anomp_media_controls_supported() != 0 }
    }

    /// Starts receiving the OS's commands. Nothing is shown until a track is set.
    pub fn new(handler: impl Fn(MediaCommand) + 'static) -> Option<MediaControls> {
        let handler: *mut MediaHandler = Box::into_raw(Box::new(Box::new(handler)));
        // SAFETY: `handler` stays valid until Drop, which destroys the
        // controls (and so the callback) before freeing it.
        let raw = unsafe { anomp_media_controls_create(Some(on_media_command), handler.cast()) };
        match NonNull::new(raw) {
            Some(raw) => Some(MediaControls {
                raw,
                handler,
                _not_send: PhantomData,
            }),
            None => {
                // SAFETY: the core refused it, so nothing else holds it.
                drop(unsafe { Box::from_raw(handler) });
                None
            }
        }
    }

    /// Shows a new current track; the artwork, playback and navigation stay
    /// as they were.
    pub fn set_track(&mut self, title: &str, artist: Option<&str>, album: Option<&str>) {
        let text = |text: Option<&str>| {
            // Tags hold no NULs; if one did, show what comes before it.
            CString::new(text.unwrap_or("").split('\0').next().unwrap_or("")).unwrap_or_default()
        };
        let (title, artist, album) = (text(Some(title)), text(artist), text(album));
        let track = RawMediaTrack {
            title: title.as_ptr(),
            artist: artist.as_ptr(),
            album: album.as_ptr(),
        };
        // SAFETY: `raw` is live, and the strings outlive the call.
        unsafe { anomp_media_controls_set_track(self.raw.as_ptr(), &track) };
    }

    /// Shows whether it is playing (any other state shows as paused) and
    /// the position now, in seconds; the system moves it on while playing.
    /// Returns false, changing nothing, if a value is not finite or the
    /// duration is negative.
    pub fn set_playback(&mut self, state: PlayerState, elapsed: f64, duration: f64) -> bool {
        // SAFETY: `raw` is live.
        unsafe {
            anomp_media_controls_set_playback(self.raw.as_ptr(), state.to_raw(), elapsed, duration)
                != 0
        }
    }

    /// Shows encoded image bytes (JPEG, PNG) as the artwork, or clears it.
    /// Returns false, clearing it, if the platform cannot decode them.
    pub fn set_artwork(&mut self, image: Option<&[u8]>) -> bool {
        let image = image.unwrap_or(&[]);
        // SAFETY: `raw` is live and the slice is valid for the call.
        unsafe {
            anomp_media_controls_set_artwork(self.raw.as_ptr(), image.as_ptr(), image.len()) != 0
        }
    }

    /// Enables or disables the next and previous commands.
    pub fn set_navigation(&mut self, has_next: bool, has_previous: bool) {
        // SAFETY: `raw` is live.
        unsafe {
            anomp_media_controls_set_navigation(
                self.raw.as_ptr(),
                c_int::from(has_next),
                c_int::from(has_previous),
            )
        }
    }

    /// Clears everything shown.
    pub fn clear(&mut self) {
        // SAFETY: `raw` is live.
        unsafe { anomp_media_controls_clear(self.raw.as_ptr()) }
    }
}

impl Drop for MediaControls {
    fn drop(&mut self) {
        // SAFETY: `raw` is live and never used again; destroying it stops the
        // callback, after which nothing else holds `handler`.
        unsafe {
            anomp_media_controls_destroy(self.raw.as_ptr());
            drop(Box::from_raw(self.handler));
        }
    }
}

extern "C" fn on_media_command(command: *const RawMediaCommand, user_data: *mut c_void) {
    // SAFETY: the core passes a valid command for the duration of the call,
    // and `user_data` is the handler registered by `MediaControls::new`.
    let (command, handler) = unsafe { (&*command, &*user_data.cast::<MediaHandler>()) };
    if let Some(command) = MediaCommand::from_raw(command) {
        handler(command);
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
            path_to_cstring(Path::new("/Music/Café.flac"))
                .unwrap()
                .to_str(),
            Ok("/Music/Café.flac")
        );
        assert!(path_to_cstring(Path::new("/a\0b.flac")).is_err());
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/tests/fixtures")
            .join(name)
    }

    #[test]
    fn reads_tags_and_picture() {
        let tags = read_tags(&fixture("tagged-vorbis.flac"), true).unwrap();
        assert_eq!(tags.title.as_deref(), Some("Café Déjà Vu"));
        assert_eq!(tags.album.as_deref(), Some("東京 Sessions"));
        assert_eq!(tags.album_artist.as_deref(), Some("Various Artists"));
        assert_eq!((tags.track_number, tags.track_total), (Some(3), Some(12)));
        assert_eq!((tags.disc_number, tags.disc_total), (Some(1), Some(2)));
        assert_eq!(tags.year, Some(2004));
        assert_eq!(
            tags.musicbrainz_recording_id.as_deref(),
            Some("a1b2c3d4-0000-4000-8000-000000000000")
        );
        assert_eq!((tags.sample_rate, tags.channels), (44100, 2));
        assert!((tags.duration - 22371.0 / 44100.0).abs() < 0.001);

        let picture = tags.picture.unwrap();
        assert_eq!(picture.mime_type.as_deref(), Some("image/png"));
        assert!(picture.data.starts_with(b"\x89PNG"));

        let without_picture = read_tags(&fixture("tagged-vorbis.flac"), false).unwrap();
        assert_eq!(without_picture.picture, None);
        assert_eq!(without_picture.title, tags.title);
    }

    #[test]
    fn untagged_fields_are_none() {
        let tags = read_tags(&fixture("wav-s16-44k.wav"), true).unwrap();
        assert_eq!(tags.title, None);
        assert_eq!(tags.track_number, None);
        assert_eq!(tags.picture, None);
        assert_eq!(tags.sample_rate, 44100);
    }

    #[test]
    fn tag_errors_are_reported() {
        let error = read_tags(&fixture("missing.flac"), false).unwrap_err();
        assert!(error.starts_with("File not found"), "{error}");
        let error = read_tags(Path::new("relative.flac"), false).unwrap_err();
        assert!(error.starts_with("Path is not absolute"), "{error}");
    }

    #[test]
    fn bookmarks_resolve_to_their_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = std::fs::canonicalize(dir.path()).unwrap().join("Música");
        std::fs::create_dir(&folder).unwrap();

        let bookmark = create_bookmark(&folder).unwrap();
        let access = FolderAccess::start(&bookmark).unwrap();
        assert_eq!(access.path(), folder);
        assert!(!access.is_stale());
        drop(access);

        std::fs::remove_dir(&folder).unwrap();
        let error = FolderAccess::start(&bookmark).err().unwrap();
        assert!(error.starts_with("Cannot resolve the bookmark"), "{error}");
        let error = FolderAccess::start(b"not a bookmark").err().unwrap();
        assert!(error.starts_with("Cannot resolve the bookmark"), "{error}");
        let error = create_bookmark(&folder).unwrap_err();
        assert!(error.starts_with("Cannot create a bookmark"), "{error}");
        let error = create_bookmark(Path::new("relative")).unwrap_err();
        assert!(error.starts_with("Path is not absolute"), "{error}");
    }

    #[test]
    fn raw_states_map_to_player_states() {
        assert_eq!(PlayerState::from_raw(0), PlayerState::Empty);
        assert_eq!(
            PlayerState::from_raw(ANOMP_STATE_STOPPED),
            PlayerState::Stopped
        );
        assert_eq!(
            PlayerState::from_raw(ANOMP_STATE_PLAYING),
            PlayerState::Playing
        );
        assert_eq!(
            PlayerState::from_raw(ANOMP_STATE_PAUSED),
            PlayerState::Paused
        );
        assert_eq!(PlayerState::from_raw(99), PlayerState::Empty);
        for state in [
            PlayerState::Empty,
            PlayerState::Stopped,
            PlayerState::Playing,
            PlayerState::Paused,
        ] {
            assert_eq!(PlayerState::from_raw(state.to_raw()), state);
        }
    }

    #[test]
    fn raw_media_commands_map_to_commands() {
        let command = |kind, position| MediaCommand::from_raw(&RawMediaCommand { kind, position });
        assert_eq!(command(ANOMP_MEDIA_PLAY, 0.0), Some(MediaCommand::Play));
        assert_eq!(command(ANOMP_MEDIA_PAUSE, 0.0), Some(MediaCommand::Pause));
        assert_eq!(command(ANOMP_MEDIA_TOGGLE, 0.0), Some(MediaCommand::Toggle));
        assert_eq!(command(ANOMP_MEDIA_NEXT, 0.0), Some(MediaCommand::Next));
        assert_eq!(
            command(ANOMP_MEDIA_PREVIOUS, 0.0),
            Some(MediaCommand::Previous)
        );
        assert_eq!(
            command(ANOMP_MEDIA_SEEK, 12.5),
            Some(MediaCommand::Seek(12.5))
        );
        assert_eq!(command(0, 0.0), None);
        assert_eq!(command(99, 0.0), None);
    }

    #[test]
    fn media_controls_are_supported_on_apple_platforms() {
        assert_eq!(
            MediaControls::supported(),
            cfg!(any(target_os = "macos", target_os = "ios"))
        );
    }
}
