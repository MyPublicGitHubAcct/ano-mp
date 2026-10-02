//! Safe wrappers over the anomp_core C API (core/include/anomp/anomp.h).
//! All `unsafe` FFI stays in this module.

use std::ffi::{c_char, c_int, c_longlong, c_void, CStr, CString};
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
    request: c_longlong,
    result: c_int,
    error: *const c_char,
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
    replaygain_track_gain: f64,
    replaygain_track_peak: f64,
    replaygain_album_gain: f64,
    replaygain_album_peak: f64,
    musicbrainz_recording_id: *const c_char,
    musicbrainz_release_id: *const c_char,
    musicbrainz_release_group_id: *const c_char,
    musicbrainz_release_track_id: *const c_char,
    musicbrainz_artist_id: *const c_char,
    musicbrainz_album_artist_id: *const c_char,
    picture: *const u8,
    picture_size: usize,
    picture_mime_type: *const c_char,
    work: *const c_char,
    movement_name: *const c_char,
    movement_number: c_int,
    movement_total: c_int,
    composer: *const c_char,
    conductor: *const c_char,
    date: *const c_char,
    original_date: *const c_char,
    lyrics: *const c_char,
    synced_lyrics: *const c_char,
    cuesheet: *const c_char,
    chapter_count: c_int,
    chapters: *const RawChapter,
    rating: c_int,
    compilation: c_int,
    artists: *const c_char,
}

#[repr(C)]
struct RawTagField {
    key: *const c_char,
    value: *const c_char,
}

#[repr(C)]
struct RawPicture {
    kind: *const c_char,
    mime_type: *const c_char,
    description: *const c_char,
    data: *const u8,
    size: usize,
}

#[repr(C)]
struct RawFileInfo {
    field_count: c_int,
    fields: *const RawTagField,
    picture_count: c_int,
    pictures: *const RawPicture,
    tag_types: *const c_char,
    codec: [c_char; 32],
    lossless: c_int,
    bits_per_sample: c_int,
    bitrate_kbps: c_int,
    sample_rate: f64,
    channels: c_int,
    duration: f64,
    file_size: i64,
}

#[repr(C)]
struct RawChapter {
    start: f64,
    end: f64,
    title: *const c_char,
}

const ANOMP_TAGS_PICTURE: c_int = 1;
const ANOMP_TAGS_LYRICS: c_int = 2;
const ANOMP_TAGS_CHAPTERS: c_int = 4;

#[repr(C)]
struct RawFileAnalysis {
    duration: f64,
    sample_rate: f64,
    channels: c_int,
    integrated_lufs: f64,
    sample_peak: f64,
    true_peak: f64,
    histogram_count: c_int,
    histogram: *const u32,
    histogram_floor: f64,
    histogram_step: f64,
    leading_silence: f64,
    trailing_silence: f64,
    gap_start: f64,
    gap_length: f64,
    start_level_db: f64,
    end_level_db: f64,
    cutoff_hz: f64,
    envelope_length: c_int,
    envelope_min: *const f32,
    envelope_max: *const f32,
}

type RawAnalysisProgress = extern "C" fn(fraction: f64, user_data: *mut c_void) -> c_int;

#[repr(C)]
struct RawTrackOptions {
    gain: f64,
    start: f64,
    end: f64,
    skip_from: f64,
    skip_to: f64,
    crossfade: f64,
}

#[repr(C)]
struct RawSignalPath {
    loaded: c_int,
    codec: [c_char; 32],
    lossless: c_int,
    bits_per_sample: c_int,
    bitrate_kbps: c_int,
    file_sample_rate: f64,
    file_channels: c_int,
    track_gain: f64,
    tempo: f64,
    semitones: f64,
    resampling: c_int,
    crossfeed: c_int,
    volume: f64,
    device_sample_rate: f64,
    device_buffer_size: c_int,
    equaliser: c_int,
    crossfade: f64,
}

#[repr(C)]
#[derive(Default)]
struct RawDeviceInfo {
    buffer_size: c_int,
    default_buffer_size: c_int,
    sample_rate: f64,
    output_latency: f64,
}

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

#[repr(C)]
struct RawVolumeWatcher {
    _private: [u8; 0],
}

type RawLogCallback = extern "C" fn(level: c_int, message: *const c_char, user_data: *mut c_void);

type RawVolumeCallback = extern "C" fn(mounted: c_int, path: *const c_char, user_data: *mut c_void);

#[repr(C)]
struct RawDockMenu {
    _private: [u8; 0],
}

#[repr(C)]
struct RawMenuItem {
    id: c_int,
    title: *const c_char,
    enabled: c_int,
    checked: c_int,
}

type RawMenuCallback = extern "C" fn(id: c_int, user_data: *mut c_void);

#[repr(C)]
struct RawAnalysisConfig {
    band_count: c_int,
    waveform_length: c_int,
    frames_per_second: f64,
}

#[repr(C)]
struct RawAnalysisFrame {
    silent: c_int,
    band_count: c_int,
    bands: *const f32,
    lowest_hz: f32,
    highest_hz: f32,
    chroma: *const f32,
    peak_left: f32,
    peak_right: f32,
    rms_left: f32,
    rms_right: f32,
    waveform_length: c_int,
    waveform_left: *const f32,
    waveform_right: *const f32,
    onset: f32,
    beat: c_int,
}

type RawAnalysisCallback = extern "C" fn(frame: *const RawAnalysisFrame, user_data: *mut c_void);

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
const ANOMP_EVENT_LOAD_FINISHED: c_int = 5;

const ANOMP_LOAD_LOADED: c_int = 0;
const ANOMP_LOAD_FAILED: c_int = 1;

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
    fn anomp_read_file_info(
        path: *const c_char,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut RawFileInfo;
    fn anomp_file_info_free(info: *mut RawFileInfo);
    fn anomp_analyse_file(
        path: *const c_char,
        start: f64,
        end: f64,
        progress: Option<RawAnalysisProgress>,
        user_data: *mut c_void,
        error: *mut c_char,
        error_size: usize,
    ) -> *mut RawFileAnalysis;
    fn anomp_file_analysis_free(analysis: *mut RawFileAnalysis);

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
    fn anomp_file_is_dataless(path: *const c_char) -> c_int;
    fn anomp_set_log_callback(callback: Option<RawLogCallback>, user_data: *mut c_void);
    #[cfg(test)]
    fn anomp_log_write(level: c_int, message: *const c_char);
    #[cfg(test)]
    fn anomp_volume_watcher_supported() -> c_int;
    fn anomp_volume_watcher_start(
        callback: Option<RawVolumeCallback>,
        user_data: *mut c_void,
    ) -> *mut RawVolumeWatcher;
    fn anomp_volume_watcher_stop(watcher: *mut RawVolumeWatcher);
    #[cfg(test)]
    fn anomp_volume_watcher_notify(
        watcher: *mut RawVolumeWatcher,
        mounted: c_int,
        path: *const c_char,
    );

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
    fn anomp_engine_output_device_count(engine: *mut RawEngine) -> c_int;
    fn anomp_engine_output_device_name(
        engine: *mut RawEngine,
        index: c_int,
        buffer: *mut c_char,
        size: usize,
    ) -> usize;
    fn anomp_engine_open_device(
        engine: *mut RawEngine,
        name: *const c_char,
        buffer_size: c_int,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_device_info(engine: *mut RawEngine, info: *mut RawDeviceInfo) -> c_int;
    fn anomp_engine_buffer_sizes(
        engine: *mut RawEngine,
        sizes: *mut c_int,
        capacity: c_int,
    ) -> c_int;
    fn anomp_engine_load(
        engine: *mut RawEngine,
        path: *const c_char,
        gain: f64,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_set_next(
        engine: *mut RawEngine,
        path: *const c_char,
        gain: f64,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_set_track_gain(engine: *mut RawEngine, path: *const c_char, gain: f64)
        -> c_int;
    #[allow(dead_code)] // Behind `Engine::load_track`, which nothing calls now.
    fn anomp_engine_load_track(
        engine: *mut RawEngine,
        path: *const c_char,
        options: *const RawTrackOptions,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_set_next_track(
        engine: *mut RawEngine,
        path: *const c_char,
        options: *const RawTrackOptions,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_load_track_async(
        engine: *mut RawEngine,
        path: *const c_char,
        options: *const RawTrackOptions,
        error: *mut c_char,
        error_size: usize,
    ) -> c_longlong;
    fn anomp_engine_set_next_track_async(
        engine: *mut RawEngine,
        path: *const c_char,
        options: *const RawTrackOptions,
        error: *mut c_char,
        error_size: usize,
    ) -> c_longlong;
    fn anomp_engine_cancel_load(engine: *mut RawEngine, request: c_longlong);
    fn anomp_engine_set_track_gain_at(
        engine: *mut RawEngine,
        path: *const c_char,
        start: f64,
        gain: f64,
    ) -> c_int;
    fn anomp_engine_set_loop(
        engine: *mut RawEngine,
        start: f64,
        end: f64,
        error: *mut c_char,
        error_size: usize,
    ) -> c_int;
    fn anomp_engine_loop(engine: *mut RawEngine, start: *mut f64, end: *mut f64) -> c_int;
    fn anomp_engine_set_tempo(engine: *mut RawEngine, rate: f64, semitones: f64) -> c_int;
    fn anomp_engine_set_crossfeed(engine: *mut RawEngine, level: c_int) -> c_int;
    fn anomp_engine_set_equaliser(
        engine: *mut RawEngine,
        gains_db: *const f64,
        preamp_db: f64,
    ) -> c_int;
    fn anomp_engine_output_is_headphones(engine: *mut RawEngine) -> c_int;
    fn anomp_engine_signal_path(engine: *mut RawEngine, path: *mut RawSignalPath) -> c_int;
    fn anomp_engine_set_device_sample_rate(engine: *mut RawEngine, sample_rate: f64) -> c_int;
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
    fn anomp_engine_set_analysis_callback(
        engine: *mut RawEngine,
        config: *const RawAnalysisConfig,
        callback: Option<RawAnalysisCallback>,
        user_data: *mut c_void,
    ) -> c_int;
    fn anomp_dock_menu_supported() -> c_int;
    fn anomp_dock_menu_create(
        callback: Option<RawMenuCallback>,
        user_data: *mut c_void,
    ) -> *mut RawDockMenu;
    fn anomp_dock_menu_destroy(menu: *mut RawDockMenu);
    fn anomp_dock_menu_set_items(menu: *mut RawDockMenu, items: *const RawMenuItem, count: c_int);
    #[cfg(test)]
    fn anomp_dock_menu_perform(menu: *mut RawDockMenu, id: c_int) -> c_int;
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
/// Whether the file at `path` is a cloud placeholder (PLAN.md H12): its
/// contents download when something reads it. Checking doesn't. `None` if
/// it can't be checked (it isn't there). Its folder must be accessible.
pub fn file_is_dataless(path: &Path) -> Option<bool> {
    let path = path_to_cstring(path).ok()?;
    // SAFETY: `path` is a valid C string for the call.
    match unsafe { anomp_file_is_dataless(path.as_ptr()) } {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

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
    pub replay_gain: ReplayGain,
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
    /// Classical works: the work this is a movement of, the movement's
    /// name and number, and who composed and conducted it.
    pub work: Option<String>,
    pub movement_name: Option<String>,
    pub movement_number: Option<u32>,
    pub movement_total: Option<u32>,
    pub composer: Option<String>,
    pub conductor: Option<String>,
    /// Release dates as precise as tagged: "2004-05-01", "2004-05" or
    /// "2004"; the original release's where the file says.
    pub date: Option<String>,
    pub original_date: Option<String>,
    /// Only with `TagParts::lyrics`: unsynced lyrics (which may be LRC
    /// text), and synced ones from an ID3v2 SYLT frame as LRC text.
    pub lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    /// Only with `TagParts::chapters`: an embedded cue sheet, and the
    /// chapters the container records.
    pub cue_sheet: Option<String>,
    pub chapters: Vec<Chapter>,
    /// A rating the tags give, 1 to 100 (20 a star): POPM, FMPS_RATING,
    /// RATING or MP4's rate. Read only, to seed the library's (PLAN.md F3).
    pub rating: Option<u8>,
    /// Marked as part of a compilation (TCMP, cpil, COMPILATION).
    pub compilation: bool,
    /// The credited artists of a multi-valued ARTISTS tag, if the file
    /// has one (PLAN.md F11).
    pub artists: Vec<String>,
}

/// A chapter of a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    /// Seconds from the start of the audio.
    pub start: f64,
    /// Seconds; `None` when it runs to the end of the file.
    pub end: Option<f64>,
    pub title: Option<String>,
}

/// What `read_tags_with` reads besides the tags.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TagParts {
    pub picture: bool,
    pub lyrics: bool,
    pub chapters: bool,
}

/// A file's ReplayGain tags (Opus R128 gains converted to ReplayGain's
/// reference level).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ReplayGain {
    /// dB.
    pub track_gain: Option<f64>,
    /// Linear; 1 is full scale.
    pub track_peak: Option<f64>,
    pub album_gain: Option<f64>,
    pub album_peak: Option<f64>,
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
    read_tags_with(
        path,
        TagParts {
            picture: include_picture,
            ..TagParts::default()
        },
    )
}

/// Reads the tags of the file at `path`, and the `parts` asked for, like
/// `read_tags`.
pub fn read_tags_with(path: &Path, parts: TagParts) -> Result<Tags, String> {
    let path = path_to_cstring(path)?;
    let flag = |on: bool, flag: c_int| if on { flag } else { 0 };
    let flags = flag(parts.picture, ANOMP_TAGS_PICTURE)
        | flag(parts.lyrics, ANOMP_TAGS_LYRICS)
        | flag(parts.chapters, ANOMP_TAGS_CHAPTERS);
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
                .then(|| {
                    // SAFETY: the core's non-null strings are NUL-terminated
                    // and live as long as `raw`.
                    unsafe { CStr::from_ptr(ptr) }
                        .to_string_lossy()
                        .into_owned()
                })
                .filter(|text| !text.is_empty())
        };
        let number = |value: c_int| u32::try_from(value).ok().filter(|&n| n > 0);
        let finite = |value: f64| value.is_finite().then_some(value);
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
            replay_gain: ReplayGain {
                track_gain: finite(raw.replaygain_track_gain),
                track_peak: finite(raw.replaygain_track_peak),
                album_gain: finite(raw.replaygain_album_gain),
                album_peak: finite(raw.replaygain_album_peak),
            },
            musicbrainz_recording_id: text(raw.musicbrainz_recording_id),
            musicbrainz_release_id: text(raw.musicbrainz_release_id),
            musicbrainz_release_group_id: text(raw.musicbrainz_release_group_id),
            musicbrainz_release_track_id: text(raw.musicbrainz_release_track_id),
            musicbrainz_artist_id: text(raw.musicbrainz_artist_id),
            musicbrainz_album_artist_id: text(raw.musicbrainz_album_artist_id),
            picture: (!raw.picture.is_null() && raw.picture_size > 0).then(|| Picture {
                mime_type: text(raw.picture_mime_type),
                // SAFETY: a non-null picture holds `picture_size` bytes.
                data: unsafe { std::slice::from_raw_parts(raw.picture, raw.picture_size) }.to_vec(),
            }),
            work: text(raw.work),
            movement_name: text(raw.movement_name),
            movement_number: number(raw.movement_number),
            movement_total: number(raw.movement_total),
            composer: text(raw.composer),
            conductor: text(raw.conductor),
            date: text(raw.date),
            original_date: text(raw.original_date),
            lyrics: text(raw.lyrics),
            synced_lyrics: text(raw.synced_lyrics),
            cue_sheet: text(raw.cuesheet),
            chapters: match usize::try_from(raw.chapter_count) {
                Ok(count) if !raw.chapters.is_null() && count > 0 => {
                    // SAFETY: non-null chapters hold `chapter_count` items.
                    unsafe { std::slice::from_raw_parts(raw.chapters, count) }
                        .iter()
                        .map(|chapter| Chapter {
                            start: chapter.start,
                            end: (chapter.end >= 0.0).then_some(chapter.end),
                            title: text(chapter.title),
                        })
                        .collect()
                }
                _ => Vec::new(),
            },
            rating: u8::try_from(raw.rating)
                .ok()
                .filter(|&r| (1..=100).contains(&r)),
            compilation: raw.compilation != 0,
            artists: text(raw.artists)
                .map(|artists| split_artists(&artists))
                .unwrap_or_default(),
        }
    }
}

/// The names in an artist field whose values were joined with "; " (or
/// typed as "A; B"), trimmed, empty ones left out.
pub fn split_artists(text: &str) -> Vec<String> {
    text.split(';')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `count` items at `ptr`, or none if it is null or `count` isn't positive.
///
/// # Safety
/// A non-null `ptr` must point to `count` valid items that outlive the slice.
unsafe fn slice_of<'a, T>(ptr: *const T, count: c_int) -> &'a [T] {
    match usize::try_from(count) {
        // SAFETY: the caller's guarantee, for a non-null `ptr`.
        Ok(count) if count > 0 && !ptr.is_null() => unsafe {
            std::slice::from_raw_parts(ptr, count)
        },
        _ => &[],
    }
}

/// Everything a file says about itself, for Get Info (PLAN.md F16).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    /// Each value of each tag field, as (TagLib's key, value).
    pub fields: Vec<(String, String)>,
    pub pictures: Vec<FilePicture>,
    /// e.g. "ID3v2.4, ID3v1"; empty if untagged.
    pub tag_types: String,
    /// FFmpeg's codec name; empty if it can't decode the file.
    pub codec: String,
    pub lossless: bool,
    pub bits_per_sample: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate: f64,
    pub channels: u32,
    pub duration: f64,
    pub file_size: u64,
}

/// An embedded picture.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePicture {
    /// e.g. "Front Cover".
    pub kind: String,
    pub mime_type: String,
    pub description: String,
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// Reads a file's `FileInfo` without modifying it; any thread.
pub fn read_file_info(path: &Path) -> Result<FileInfo, String> {
    let path = path_to_cstring(path)?;
    let mut raw = std::ptr::null_mut();
    with_error(|error, size| {
        // SAFETY: `path` is a valid C string and the error buffer is supplied
        // by `with_error`, both for the whole call.
        raw = unsafe { anomp_read_file_info(path.as_ptr(), error, size) };
        c_int::from(!raw.is_null())
    })?;
    // SAFETY: `raw` is a non-null result of anomp_read_file_info, read once
    // and then freed exactly once.
    unsafe {
        let info = FileInfo::from_raw(&*raw);
        anomp_file_info_free(raw);
        Ok(info)
    }
}

impl FileInfo {
    /// # Safety
    /// `raw` must come from anomp_read_file_info and not yet be freed.
    unsafe fn from_raw(raw: &RawFileInfo) -> FileInfo {
        let text = |ptr: *const c_char| {
            if ptr.is_null() {
                String::new()
            } else {
                // SAFETY: the core's non-null strings are NUL-terminated and
                // live as long as `raw`.
                unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            }
        };
        let positive = |value: c_int| u32::try_from(value).ok().filter(|&n| n > 0);
        // SAFETY: the core's arrays hold their counts' items and live as
        // long as `raw`.
        let (fields, pictures) = unsafe {
            (
                slice_of(raw.fields, raw.field_count),
                slice_of(raw.pictures, raw.picture_count),
            )
        };
        FileInfo {
            fields: fields
                .iter()
                .map(|field| (text(field.key), text(field.value)))
                .collect(),
            pictures: pictures
                .iter()
                .map(|picture| FilePicture {
                    kind: text(picture.kind),
                    mime_type: text(picture.mime_type),
                    description: text(picture.description),
                    data: if picture.data.is_null() {
                        Vec::new()
                    } else {
                        // SAFETY: a non-null picture holds `size` bytes.
                        unsafe { std::slice::from_raw_parts(picture.data, picture.size) }.to_vec()
                    },
                })
                .collect(),
            tag_types: text(raw.tag_types),
            codec: text(raw.codec.as_ptr()),
            lossless: raw.lossless != 0,
            bits_per_sample: positive(raw.bits_per_sample),
            bitrate_kbps: positive(raw.bitrate_kbps),
            sample_rate: raw.sample_rate,
            channels: positive(raw.channels).unwrap_or(0),
            duration: raw.duration,
            file_size: u64::try_from(raw.file_size).unwrap_or(0),
        }
    }
}

/// What one pass over a file (or part of one) measures: see
/// `anomp_file_analysis`.
#[derive(Debug, Clone, PartialEq)]
pub struct FileAnalysis {
    /// Seconds decoded.
    pub duration: f64,
    pub sample_rate: f64,
    /// EBU R128 integrated loudness in LUFS; `None` when silent throughout.
    pub loudness: Option<f64>,
    /// Linear, 1 is full scale.
    pub sample_peak: f64,
    pub true_peak: f64,
    /// Blocks per `histogram_step` LU from `histogram_floor` LUFS up.
    pub histogram: Vec<u32>,
    pub histogram_floor: f64,
    pub histogram_step: f64,
    /// Seconds below -60 dBFS at the start and the end.
    pub leading_silence: f64,
    pub trailing_silence: f64,
    /// The longest silence inside (at least 2 s), if any: start, length.
    pub gap: Option<(f64, f64)>,
    /// RMS of the first and last 50 ms in dBFS; `None` for digital silence.
    pub start_level_db: Option<f64>,
    pub end_level_db: Option<f64>,
    /// Where the spectrum drops off a cliff, if it does.
    pub cutoff_hz: Option<f64>,
    /// The smallest and largest sample in each slice of the track.
    pub envelope_min: Vec<f32>,
    pub envelope_max: Vec<f32>,
}

/// Decodes the file at `path` (from `start` to `end` seconds; an `end` not
/// after `start` is the end of the file) once and measures it. `progress`
/// gets the fraction done about four times a second and returns false to
/// cancel. May be called from any thread; never uses the engine.
pub fn analyse_file(
    path: &Path,
    start: f64,
    end: f64,
    mut progress: impl FnMut(f64) -> bool,
) -> Result<FileAnalysis, String> {
    extern "C" fn report(fraction: f64, user_data: *mut c_void) -> c_int {
        // SAFETY: `user_data` is the `&mut dyn FnMut` below, alive for the call.
        let progress = unsafe { &mut *user_data.cast::<&mut dyn FnMut(f64) -> bool>() };
        c_int::from(progress(fraction))
    }
    let path = path_to_cstring(path)?;
    let mut progress: &mut dyn FnMut(f64) -> bool = &mut progress;
    let user_data: *mut c_void = (&mut progress as *mut &mut dyn FnMut(f64) -> bool).cast();
    let mut raw = std::ptr::null_mut();
    with_error(|error, size| {
        // SAFETY: `path` and the error buffer are valid for the call, and
        // `user_data` points at `progress`, which outlives it.
        raw = unsafe {
            anomp_analyse_file(
                path.as_ptr(),
                start,
                end,
                Some(report),
                user_data,
                error,
                size,
            )
        };
        c_int::from(!raw.is_null())
    })?;
    // SAFETY: `raw` is a non-null result of anomp_analyse_file whose arrays
    // hold their counts; it is copied once and then freed exactly once.
    unsafe {
        let analysis = FileAnalysis::from_raw(&*raw);
        anomp_file_analysis_free(raw);
        Ok(analysis)
    }
}

impl FileAnalysis {
    /// # Safety
    /// `raw` must come from anomp_analyse_file and not yet be freed.
    unsafe fn from_raw(raw: &RawFileAnalysis) -> FileAnalysis {
        let floats = |data: *const f32, len: c_int| match usize::try_from(len) {
            // SAFETY: the core's non-null arrays hold their lengths' items.
            Ok(len) if !data.is_null() => unsafe { std::slice::from_raw_parts(data, len) }.to_vec(),
            _ => Vec::new(),
        };
        let finite = |value: f64| value.is_finite().then_some(value);
        FileAnalysis {
            duration: raw.duration,
            sample_rate: raw.sample_rate,
            loudness: finite(raw.integrated_lufs),
            sample_peak: raw.sample_peak,
            true_peak: raw.true_peak,
            histogram: match usize::try_from(raw.histogram_count) {
                Ok(len) if !raw.histogram.is_null() => {
                    // SAFETY: a non-null histogram holds `histogram_count` items.
                    unsafe { std::slice::from_raw_parts(raw.histogram, len) }.to_vec()
                }
                _ => Vec::new(),
            },
            histogram_floor: raw.histogram_floor,
            histogram_step: raw.histogram_step,
            leading_silence: raw.leading_silence,
            trailing_silence: raw.trailing_silence,
            gap: (raw.gap_length > 0.0).then_some((raw.gap_start, raw.gap_length)),
            start_level_db: finite(raw.start_level_db),
            end_level_db: finite(raw.end_level_db),
            cutoff_hz: (raw.cutoff_hz > 0.0).then_some(raw.cutoff_hz),
            envelope_min: floats(raw.envelope_min, raw.envelope_length),
            envelope_max: floats(raw.envelope_max, raw.envelope_length),
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
// SAFETY: as for `Send`; nothing mutates it through a shared reference.
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

/// How an asynchronous load (`Engine::load_track_async`) ended.
#[derive(Debug, Clone, PartialEq)]
pub enum LoadResult {
    /// The track took its place.
    Loaded,
    /// It couldn't be opened, and nothing changed.
    Failed(String),
    /// Cancelled, or superseded by a later request.
    Cancelled,
}

/// Identifies an asynchronous load; never 0.
pub type LoadRequest = i64;

/// Events the core reports through the engine's event callback. Player
/// events arrive about every 50 ms, never from inside an engine call; the
/// handler may call the engine.
#[derive(Debug, Clone, PartialEq)]
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
    /// An asynchronous load is done, soon after the file was opened (each
    /// request is reported once).
    LoadFinished {
        request: LoadRequest,
        result: LoadResult,
    },
}

type EventHandler = Box<dyn FnMut(Event)>;

/// How the core analyses the audio for the visualizer; the core refuses
/// values outside the ranges in `anomp.h`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalysisConfig {
    /// 4..=256 spectrum bands.
    pub bands: u32,
    /// 16..=2048 samples per channel.
    pub waveform_length: u32,
    /// 1..=120.
    pub frames_per_second: f64,
}

/// One analysis of what the player plays (`anomp_analysis_frame`), borrowed
/// from the core for the duration of the handler call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalysisFrame<'a> {
    /// No audio has played for a moment; everything else is zero.
    pub silent: bool,
    /// 0..=1, log-spaced from `lowest_hz` to `highest_hz`, low first.
    pub bands: &'a [f32],
    pub lowest_hz: f32,
    pub highest_hz: f32,
    /// Energy per pitch class, C first, the strongest 1.
    pub chroma: [f32; 12],
    /// Linear, left and right.
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    /// The latest samples per channel, from a rising zero crossing.
    pub left: &'a [f32],
    pub right: &'a [f32],
    /// How much louder the spectrum got since the previous frame, 0..=1.
    pub onset: f32,
    pub beat: bool,
}

impl AnalysisFrame<'_> {
    /// # Safety
    /// `raw`'s pointers must be null or valid for their lengths (the
    /// chroma for 12 values) while the result is used.
    unsafe fn from_raw(raw: &RawAnalysisFrame) -> AnalysisFrame<'_> {
        let slice = |data: *const f32, len: c_int| match usize::try_from(len) {
            // SAFETY: the caller's guarantee, for a non-null pointer.
            Ok(len) if !data.is_null() && len > 0 => unsafe {
                std::slice::from_raw_parts(data, len)
            },
            _ => &[],
        };
        let mut chroma = [0.0; 12];
        if !raw.chroma.is_null() {
            // SAFETY: the caller's guarantee: a non-null chroma holds 12 values.
            chroma.copy_from_slice(unsafe { std::slice::from_raw_parts(raw.chroma, 12) });
        }
        AnalysisFrame {
            silent: raw.silent != 0,
            bands: slice(raw.bands, raw.band_count),
            lowest_hz: raw.lowest_hz,
            highest_hz: raw.highest_hz,
            chroma,
            peak: [raw.peak_left, raw.peak_right],
            rms: [raw.rms_left, raw.rms_right],
            left: slice(raw.waveform_left, raw.waveform_length),
            right: slice(raw.waveform_right, raw.waveform_length),
            onset: raw.onset,
            beat: raw.beat != 0,
        }
    }
}

/// Bands of the equaliser (`ANOMP_EQ_BANDS`), at 31 Hz to 16 kHz, and the
/// most each band or the preamp moves, in dB either way.
pub const EQ_BANDS: usize = 10;
pub const EQ_MAX_GAIN: f64 = 12.0;

/// Largest gain `Engine::load` and friends apply (about +18 dB); the core
/// clamps to it (`ANOMP_MAX_TRACK_GAIN`).
pub const MAX_TRACK_GAIN: f64 = 8.0;

/// How a track is played: part of a file, and a stretch of it to skip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackOptions {
    /// Linear (see `MAX_TRACK_GAIN`).
    pub gain: f64,
    /// Seconds into the file where the track starts.
    pub start: f64,
    /// Seconds into the file where it ends; `None` for the end of the file.
    pub end: Option<f64>,
    /// Seconds within the track: reaching the first jumps to the second, once.
    pub skip: Option<(f64, f64)>,
    /// For a next track: seconds to crossfade into it over (at most
    /// `MAX_CROSSFADE`); 0 hands off gaplessly.
    pub crossfade: f64,
}

/// Longest crossfade, in seconds (`ANOMP_MAX_CROSSFADE`).
pub const MAX_CROSSFADE: f64 = 12.0;

impl TrackOptions {
    fn to_raw(self) -> RawTrackOptions {
        RawTrackOptions {
            gain: self.gain,
            start: self.start,
            end: self.end.unwrap_or(0.0),
            skip_from: self.skip.map_or(-1.0, |(from, _)| from),
            skip_to: self.skip.map_or(-1.0, |(_, to)| to),
            crossfade: self.crossfade,
        }
    }
}

/// Every step between the file and the speakers (`anomp_signal_path`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalPath {
    pub loaded: bool,
    /// FFmpeg's codec name, e.g. "flac".
    pub codec: String,
    pub lossless: bool,
    /// The source's, for lossless codecs.
    pub bits_per_sample: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub file_sample_rate: f64,
    pub file_channels: u32,
    /// Linear.
    pub track_gain: f64,
    pub tempo: f64,
    pub semitones: f64,
    pub resampling: bool,
    pub crossfeed: u8,
    /// Linear.
    pub volume: f64,
    pub device_sample_rate: f64,
    pub device_buffer_size: u32,
    /// The equaliser is on (PLAN.md F15).
    pub equaliser: bool,
    /// Seconds the next track crossfades over; 0 if none (PLAN.md F14).
    pub crossfade: f64,
}

impl SignalPath {
    /// # Safety
    /// `raw.codec` must hold a NUL.
    unsafe fn from_raw(raw: &RawSignalPath) -> SignalPath {
        let positive = |value: c_int| u32::try_from(value).ok().filter(|&n| n > 0);
        SignalPath {
            loaded: raw.loaded != 0,
            // SAFETY: the caller's guarantee that `codec` holds a NUL.
            codec: unsafe { CStr::from_ptr(raw.codec.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
            lossless: raw.lossless != 0,
            bits_per_sample: positive(raw.bits_per_sample),
            bitrate_kbps: positive(raw.bitrate_kbps),
            file_sample_rate: raw.file_sample_rate,
            file_channels: positive(raw.file_channels).unwrap_or(0),
            track_gain: raw.track_gain,
            tempo: raw.tempo,
            semitones: raw.semitones,
            resampling: raw.resampling != 0,
            crossfeed: u8::try_from(raw.crossfeed).unwrap_or(0),
            volume: raw.volume,
            device_sample_rate: raw.device_sample_rate,
            device_buffer_size: positive(raw.device_buffer_size).unwrap_or(0),
            equaliser: raw.equaliser != 0,
            crossfade: raw.crossfade,
        }
    }
}

/// The open output device's settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub name: String,
    /// Samples per block.
    pub buffer_size: u32,
    pub default_buffer_size: u32,
    /// What the device offers, smallest first.
    pub buffer_sizes: Vec<u32>,
    /// Hz.
    pub sample_rate: f64,
    /// Seconds from the player to the speakers.
    pub output_latency: f64,
}

/// Called on the core's analysis thread, never the main thread.
type AnalysisHandler = Box<dyn FnMut(&AnalysisFrame<'_>) + Send>;

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
    // Double-boxed like `handler`; null while no analysis runs.
    analysis: *mut AnalysisHandler,
    _not_send: PhantomData<*mut ()>,
}

impl Engine {
    pub fn new() -> Option<Engine> {
        // SAFETY: no preconditions beyond the main-thread rule documented above.
        let raw = NonNull::new(unsafe { anomp_engine_create() })?;
        Some(Engine {
            raw,
            handler: std::ptr::null_mut(),
            analysis: std::ptr::null_mut(),
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

    /// Unused: the app opens devices with `open_device`, `None` for the default.
    #[allow(dead_code)]
    pub fn open_default_device(&mut self) -> Result<(), String> {
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; the error buffer is supplied by `with_error`.
        with_error(|error, size| unsafe { anomp_engine_open_default_device(raw, error, size) })
    }

    /// Opens the output device `name` (`None` for the system default) with
    /// `buffer_size` samples per block (`None`, or a size the device doesn't
    /// offer, for its default). A name that isn't an output device fails
    /// without touching the open device; one that fails to open may leave
    /// none open.
    pub fn open_device(
        &mut self,
        name: Option<&str>,
        buffer_size: Option<u32>,
    ) -> Result<(), String> {
        let name = name
            .map(|name| {
                CString::new(name).map_err(|_| "Device name contains a NUL byte".to_string())
            })
            .transpose()?;
        let name_ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
        let buffer_size = buffer_size.map_or(0, |size| c_int::try_from(size).unwrap_or(0));
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; `name_ptr` is null or points into
        // `name`, which lives until the end of this function.
        with_error(|error, size| unsafe {
            anomp_engine_open_device(raw, name_ptr, buffer_size, error, size)
        })
    }

    /// The names of the output devices there are now.
    pub fn output_devices(&mut self) -> Vec<String> {
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine.
        let count = unsafe { anomp_engine_output_device_count(raw) };
        (0..count)
            .map(|index| {
                read_string(|buffer, size| {
                    // SAFETY: `read_string` supplies a valid buffer and size.
                    unsafe { anomp_engine_output_device_name(raw, index, buffer, size) }
                })
            })
            .filter(|name| !name.is_empty())
            .collect()
    }

    /// The open device's settings, or `None` if no device is open.
    pub fn device_info(&self) -> Option<DeviceInfo> {
        let raw = self.raw.as_ptr();
        let mut info = RawDeviceInfo::default();
        // SAFETY: `raw` is a live engine and `info` valid for the call.
        if unsafe { anomp_engine_device_info(raw, &mut info) } == 0 {
            return None;
        }
        // SAFETY: a null array with capacity 0 only counts.
        let count = unsafe { anomp_engine_buffer_sizes(raw, std::ptr::null_mut(), 0) };
        let mut sizes = vec![0 as c_int; usize::try_from(count).unwrap_or(0)];
        // SAFETY: `sizes` holds `count` entries.
        let written = unsafe { anomp_engine_buffer_sizes(raw, sizes.as_mut_ptr(), count) };
        sizes.truncate(usize::try_from(written).unwrap_or(0));
        let positive = |value: c_int| u32::try_from(value).unwrap_or(0);
        Some(DeviceInfo {
            name: self.device_name().unwrap_or_default(),
            buffer_size: positive(info.buffer_size),
            default_buffer_size: positive(info.default_buffer_size),
            buffer_sizes: sizes.into_iter().map(positive).filter(|&n| n > 0).collect(),
            sample_rate: info.sample_rate,
            output_latency: info.output_latency,
        })
    }

    /// Opens `path` as the current track with its own linear `gain` (1
    /// leaves it as is; see `MAX_TRACK_GAIN`), clears the next track, and
    /// stops at the start. On failure nothing changes.
    // Only the /dev page uses it, in debug builds (PLAN.md H2).
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub fn load(&mut self, path: &Path, gain: f64) -> Result<(), String> {
        let path = path_to_cstring(path)?;
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine and `path` a valid C string for the call.
        with_error(|error, size| unsafe {
            anomp_engine_load(raw, path.as_ptr(), gain, error, size)
        })
    }

    /// Opens `path` as the track that follows the current one gaplessly,
    /// with its own `gain` (as for `load`), or clears it with `None`. On
    /// failure the previous next track stays.
    // Only the /dev page uses it, in debug builds (PLAN.md H2).
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub fn set_next(&mut self, next: Option<(&Path, f64)>) -> Result<(), String> {
        let path = next.map(|(path, _)| path_to_cstring(path)).transpose()?;
        let gain = next.map_or(1.0, |(_, gain)| gain);
        let path_ptr = path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; `path_ptr` is null or points into
        // `path`, which lives until the end of this function.
        with_error(|error, size| unsafe { anomp_engine_set_next(raw, path_ptr, gain, error, size) })
    }

    /// Changes the gain of the current and next tracks opened from `path`,
    /// e.g. when ReplayGain is turned on; returns how many changed. Matching
    /// by file can't race a hand-off. Unused: the queue tells parts of one
    /// file apart with `set_track_gain_at`.
    #[allow(dead_code)]
    pub fn set_track_gain(&mut self, path: &Path, gain: f64) -> usize {
        let Ok(path) = path_to_cstring(path) else {
            return 0;
        };
        // SAFETY: `raw` is a live engine and `path` a valid C string for the call.
        let changed =
            unsafe { anomp_engine_set_track_gain(self.raw.as_ptr(), path.as_ptr(), gain) };
        usize::try_from(changed).unwrap_or(0)
    }

    /// Opens part of `path` (see `TrackOptions`) as the current track, like
    /// `load`. Unused: the queue opens tracks with `load_track_async`.
    #[allow(dead_code)]
    pub fn load_track(&mut self, path: &Path, options: &TrackOptions) -> Result<(), String> {
        let path = path_to_cstring(path)?;
        let raw = self.raw.as_ptr();
        let options = options.to_raw();
        // SAFETY: `raw` is a live engine; `path` and `options` are valid for the call.
        with_error(|error, size| unsafe {
            anomp_engine_load_track(raw, path.as_ptr(), &options, error, size)
        })
    }

    /// Opens part of a file as the next track, like `set_next`; `None`
    /// clears it.
    pub fn set_next_track(&mut self, next: Option<(&Path, &TrackOptions)>) -> Result<(), String> {
        let path = next.map(|(path, _)| path_to_cstring(path)).transpose()?;
        let options = next.map(|(_, options)| options.to_raw());
        let path_ptr = path.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        let options_ptr = options.as_ref().map_or(std::ptr::null(), |o| o as *const _);
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; the pointers are null or point into
        // locals that live until the end of this function.
        with_error(|error, size| unsafe {
            anomp_engine_set_next_track(raw, path_ptr, options_ptr, error, size)
        })
    }

    /// `load_track` on a thread of the core's own (PLAN.md H11): returns at
    /// once, and `Event::LoadFinished` reports the request once the file is
    /// open and handed over. Until then the current track plays on. The
    /// file's folder must stay accessible until it is reported.
    pub fn load_track_async(
        &mut self,
        path: &Path,
        options: &TrackOptions,
    ) -> Result<LoadRequest, String> {
        let path = path_to_cstring(path)?;
        let options = options.to_raw();
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; `path` and `options` are valid for the call.
        with_request(|error, size| unsafe {
            anomp_engine_load_track_async(raw, path.as_ptr(), &options, error, size)
        })
    }

    /// `set_next_track` as `load_track_async` does `load_track`; clears the
    /// next track at once.
    pub fn set_next_track_async(
        &mut self,
        path: &Path,
        options: &TrackOptions,
    ) -> Result<LoadRequest, String> {
        let path = path_to_cstring(path)?;
        let options = options.to_raw();
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; `path` and `options` are valid for the call.
        with_request(|error, size| unsafe {
            anomp_engine_set_next_track_async(raw, path.as_ptr(), &options, error, size)
        })
    }

    /// Cancels a request not yet reported; it is reported as cancelled.
    pub fn cancel_load(&mut self, request: LoadRequest) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_cancel_load(self.raw.as_ptr(), request) }
    }

    /// `set_track_gain` for the track opened from `path` starting at
    /// `start` seconds, telling parts of one file apart.
    pub fn set_track_gain_at(&mut self, path: &Path, start: f64, gain: f64) -> usize {
        let Ok(path) = path_to_cstring(path) else {
            return 0;
        };
        // SAFETY: `raw` is a live engine and `path` a valid C string for the call.
        let changed = unsafe {
            anomp_engine_set_track_gain_at(self.raw.as_ptr(), path.as_ptr(), start, gain)
        };
        usize::try_from(changed).unwrap_or(0)
    }

    /// Loops the current track between `start` and `end` seconds, or clears
    /// the loop with `None`. The engine opens the file again, so its folder
    /// must be open (`library::access`).
    pub fn set_loop(&mut self, points: Option<(f64, f64)>) -> Result<(), String> {
        let (start, end) = points.unwrap_or((-1.0, 0.0));
        let raw = self.raw.as_ptr();
        // SAFETY: `raw` is a live engine; the error buffer is supplied by `with_error`.
        with_error(|error, size| unsafe { anomp_engine_set_loop(raw, start, end, error, size) })
    }

    /// The loop's start and end in seconds, if one is set.
    pub fn loop_points(&self) -> Option<(f64, f64)> {
        let (mut start, mut end) = (0.0, 0.0);
        // SAFETY: `raw` is a live engine and both outputs are valid for the call.
        (unsafe { anomp_engine_loop(self.raw.as_ptr(), &mut start, &mut end) } != 0)
            .then_some((start, end))
    }

    /// Plays at `rate` times the speed (0.5..=1.5) without changing the
    /// pitch, transposed by `semitones` (-12..=12). False for values out of
    /// range.
    pub fn set_tempo(&mut self, rate: f64, semitones: f64) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_set_tempo(self.raw.as_ptr(), rate, semitones) != 0 }
    }

    /// Crossfeed for headphones: 0 off, 1..=3 stronger.
    pub fn set_crossfeed(&mut self, level: u8) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_set_crossfeed(self.raw.as_ptr(), c_int::from(level)) != 0 }
    }

    /// Turns the equaliser on with `EQ_BANDS` gains and a preamp, in dB
    /// (each within `EQ_MAX_GAIN`), or off with `None`. False, changing
    /// nothing, for values out of range.
    pub fn set_equaliser(&mut self, settings: Option<(&[f64; EQ_BANDS], f64)>) -> bool {
        let (gains, preamp) = match settings {
            Some((gains, preamp)) => (gains.as_ptr(), preamp),
            None => (std::ptr::null(), 0.0),
        };
        // SAFETY: `raw` is a live engine, and `gains` is null or points to
        // EQ_BANDS values for the whole call.
        unsafe { anomp_engine_set_equaliser(self.raw.as_ptr(), gains, preamp) != 0 }
    }

    /// Whether the output plays through headphones; `None` if the OS
    /// doesn't say.
    pub fn output_is_headphones(&self) -> Option<bool> {
        // SAFETY: `raw` is a live engine.
        match unsafe { anomp_engine_output_is_headphones(self.raw.as_ptr()) } {
            1 => Some(true),
            0 => Some(false),
            _ => None,
        }
    }

    /// Every step from the file to the speakers.
    pub fn signal_path(&self) -> SignalPath {
        // SAFETY: an all-zero RawSignalPath is valid (no pointers inside).
        let mut raw: RawSignalPath = unsafe { std::mem::zeroed() };
        // SAFETY: `raw` is a live engine and `raw` valid for the call; the
        // codec is NUL-terminated within its array.
        unsafe {
            anomp_engine_signal_path(self.raw.as_ptr(), &mut raw);
            SignalPath::from_raw(&raw)
        }
    }

    /// Switches the output device to `sample_rate` Hz if it offers it;
    /// true if it runs at that rate afterwards.
    pub fn set_device_sample_rate(&mut self, sample_rate: f64) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_set_device_sample_rate(self.raw.as_ptr(), sample_rate) != 0 }
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
        let raw = self.raw.as_ptr();
        let name = read_string(|buffer, size| {
            // SAFETY: `read_string` supplies a valid buffer and size.
            unsafe { anomp_engine_device_name(raw, buffer, size) }
        });
        (!name.is_empty()).then_some(name)
    }

    /// Starts analysing what the player plays, calling `handler` on the
    /// core's analysis thread (never this one) about `frames_per_second`
    /// times a second while audio plays, and once more with a silent frame
    /// when it stops. Replaces a running analysis. Returns false, leaving any
    /// running analysis as it was, if the core refuses `config`.
    pub fn start_analysis(
        &mut self,
        config: AnalysisConfig,
        handler: impl FnMut(&AnalysisFrame<'_>) + Send + 'static,
    ) -> bool {
        let raw_config = RawAnalysisConfig {
            band_count: c_int::try_from(config.bands).unwrap_or(c_int::MAX),
            waveform_length: c_int::try_from(config.waveform_length).unwrap_or(c_int::MAX),
            frames_per_second: config.frames_per_second,
        };
        let handler: *mut AnalysisHandler = Box::into_raw(Box::new(Box::new(handler)));
        // SAFETY: `raw` is a live engine; `handler` stays valid while
        // registered. The core has stopped the previous analysis, waiting for
        // its callback, before this returns, so the previous handler can be
        // freed; if the call fails, the new one was never registered.
        unsafe {
            let started = anomp_engine_set_analysis_callback(
                self.raw.as_ptr(),
                &raw_config,
                Some(on_analysis),
                handler.cast(),
            ) != 0;
            if !started {
                drop(Box::from_raw(handler));
                return false;
            }
            if !self.analysis.is_null() {
                drop(Box::from_raw(self.analysis));
            }
        }
        self.analysis = handler;
        true
    }

    /// Stops the analysis, if any, waiting for a handler call in progress.
    pub fn stop_analysis(&mut self) {
        // SAFETY: `raw` is a live engine; after the call the core no longer
        // uses the handler, so it can be freed.
        unsafe {
            anomp_engine_set_analysis_callback(
                self.raw.as_ptr(),
                std::ptr::null(),
                None,
                std::ptr::null_mut(),
            );
            if !self.analysis.is_null() {
                drop(Box::from_raw(self.analysis));
            }
        }
        self.analysis = std::ptr::null_mut();
    }

    pub fn is_analysing(&self) -> bool {
        !self.analysis.is_null()
    }

    // Only the /dev page uses it, in debug builds (PLAN.md H2).
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub fn play_test_tone(&mut self, frequency_hz: f64) -> bool {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_play_test_tone(self.raw.as_ptr(), frequency_hz) != 0 }
    }

    // Only the /dev page uses it, in debug builds (PLAN.md H2).
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub fn stop_test_tone(&mut self) {
        // SAFETY: `raw` is a live engine.
        unsafe { anomp_engine_stop_test_tone(self.raw.as_ptr()) }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Frees the analysis handler once the analysis thread has stopped.
        self.stop_analysis();
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

type MenuHandler = Box<dyn Fn(i32)>;

/// Sends the core's log messages (JUCE's Logger and failed assertions
/// included) to the `log` facade, with the target "core" (PLAN.md H9).
pub fn forward_core_log() {
    // SAFETY: `on_core_log` uses no user data and may run on any thread.
    unsafe { anomp_set_log_callback(Some(on_core_log), std::ptr::null_mut()) };
}

/// Stops sending the core's log messages anywhere.
#[cfg(test)]
pub fn stop_core_log() {
    // SAFETY: no preconditions.
    unsafe { anomp_set_log_callback(None, std::ptr::null_mut()) };
}

/// Writes to the core's log, as the core does; for tests.
#[cfg(test)]
pub fn core_log_write(level: log::Level, message: &str) {
    let message = CString::new(message).unwrap_or_default();
    let level = match level {
        log::Level::Error => 1,
        log::Level::Warn => 2,
        log::Level::Info => 3,
        log::Level::Debug | log::Level::Trace => 4,
    };
    // SAFETY: `message` is a C string valid for the call.
    unsafe { anomp_log_write(level, message.as_ptr()) };
}

extern "C" fn on_core_log(level: c_int, message: *const c_char, _user_data: *mut c_void) {
    if message.is_null() {
        return;
    }
    // SAFETY: the core passes a C string valid for the call.
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    let level = match level {
        1 => log::Level::Error,
        2 => log::Level::Warn,
        3 => log::Level::Info,
        _ => log::Level::Debug,
    };
    log::log!(target: "core", level, "{message}");
}

type VolumeHandler = Box<dyn Fn(bool, PathBuf)>;

/// Reports volumes mounted and unmounted (macOS; nothing elsewhere yet), so
/// library folders on them are checked again (PLAN.md H22). Main thread
/// only: the handler runs there, from the OS, never inside a call on this
/// object.
pub struct VolumeWatcher {
    raw: NonNull<RawVolumeWatcher>,
    handler: *mut VolumeHandler,
    _not_send: PhantomData<*mut ()>,
}

impl VolumeWatcher {
    /// Whether this platform reports volumes.
    #[cfg(test)]
    pub fn supported() -> bool {
        // SAFETY: no preconditions.
        unsafe { anomp_volume_watcher_supported() != 0 }
    }

    /// Starts reporting each volume mounted (`true`) or unmounted, with its
    /// mount point (empty if unknown), to `handler`.
    pub fn new(handler: impl Fn(bool, PathBuf) + 'static) -> Option<VolumeWatcher> {
        let handler: *mut VolumeHandler = Box::into_raw(Box::new(Box::new(handler)));
        // SAFETY: `handler` stays valid until Drop, which stops the watcher
        // (and so the callback) before freeing it.
        let raw = unsafe { anomp_volume_watcher_start(Some(on_volume), handler.cast()) };
        match NonNull::new(raw) {
            Some(raw) => Some(VolumeWatcher {
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

    /// Reports a volume as the OS would, for tests.
    #[cfg(test)]
    pub fn notify(&mut self, mounted: bool, path: &str) {
        let path = CString::new(path).unwrap_or_default();
        // SAFETY: `raw` is live and `path` outlives the call.
        unsafe {
            anomp_volume_watcher_notify(self.raw.as_ptr(), c_int::from(mounted), path.as_ptr())
        };
    }
}

impl Drop for VolumeWatcher {
    fn drop(&mut self) {
        // SAFETY: `raw` is live and never used again; stopping it ends the
        // callback, after which nothing else holds `handler`.
        unsafe {
            anomp_volume_watcher_stop(self.raw.as_ptr());
            drop(Box::from_raw(self.handler));
        }
    }
}

extern "C" fn on_volume(mounted: c_int, path: *const c_char, user_data: *mut c_void) {
    // SAFETY: `user_data` is the handler registered by `VolumeWatcher::new`,
    // alive until the watcher stops; `path` is null or a C string valid for
    // the call.
    let (handler, path) = unsafe {
        (
            &*user_data.cast::<VolumeHandler>(),
            if path.is_null() {
                String::new()
            } else {
                CStr::from_ptr(path).to_string_lossy().into_owned()
            },
        )
    };
    handler(mounted != 0, PathBuf::from(path));
}

/// An item of the Dock menu.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub id: i32,
    /// Empty for a separator.
    pub title: String,
    pub enabled: bool,
    pub checked: bool,
}

/// The Dock icon's menu (macOS; nothing elsewhere). Main thread only, like
/// `MediaControls`: choices reach the handler on the main thread, from the
/// OS, never inside a call on this object. One exists at a time.
pub struct DockMenu {
    raw: NonNull<RawDockMenu>,
    handler: *mut MenuHandler,
    _not_send: PhantomData<*mut ()>,
}

impl DockMenu {
    /// Whether this platform shows a Dock menu.
    pub fn supported() -> bool {
        // SAFETY: no preconditions.
        unsafe { anomp_dock_menu_supported() != 0 }
    }

    /// Starts showing a Dock menu, empty until `set_items`; `None` if one
    /// exists already.
    pub fn new(handler: impl Fn(i32) + 'static) -> Option<DockMenu> {
        let handler: *mut MenuHandler = Box::into_raw(Box::new(Box::new(handler)));
        // SAFETY: `handler` stays valid until Drop, which destroys the menu
        // (and so the callback) before freeing it.
        let raw = unsafe { anomp_dock_menu_create(Some(on_menu_choice), handler.cast()) };
        match NonNull::new(raw) {
            Some(raw) => Some(DockMenu {
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

    pub fn set_items(&mut self, items: &[MenuItem]) {
        let titles: Vec<CString> = items
            .iter()
            .map(|item| {
                CString::new(item.title.split('\0').next().unwrap_or("")).unwrap_or_default()
            })
            .collect();
        let raw: Vec<RawMenuItem> = items
            .iter()
            .zip(&titles)
            .map(|(item, title)| RawMenuItem {
                id: item.id,
                title: title.as_ptr(),
                enabled: c_int::from(item.enabled),
                checked: c_int::from(item.checked),
            })
            .collect();
        let count = c_int::try_from(raw.len()).unwrap_or(c_int::MAX);
        // SAFETY: `raw` is live, and the items and their titles outlive the
        // call, which copies them.
        unsafe { anomp_dock_menu_set_items(self.raw.as_ptr(), raw.as_ptr(), count) };
    }

    /// Chooses an item as the OS would, for tests.
    #[cfg(test)]
    pub fn perform(&mut self, id: i32) -> bool {
        // SAFETY: `raw` is live.
        unsafe { anomp_dock_menu_perform(self.raw.as_ptr(), id) != 0 }
    }
}

impl Drop for DockMenu {
    fn drop(&mut self) {
        // SAFETY: `raw` is live and never used again; destroying it stops the
        // callback, after which nothing else holds `handler`.
        unsafe {
            anomp_dock_menu_destroy(self.raw.as_ptr());
            drop(Box::from_raw(self.handler));
        }
    }
}

extern "C" fn on_menu_choice(id: c_int, user_data: *mut c_void) {
    // SAFETY: `user_data` is the handler registered by `DockMenu::new`, alive
    // until the menu is destroyed.
    let handler = unsafe { &*user_data.cast::<MenuHandler>() };
    handler(id);
}

extern "C" fn on_media_command(command: *const RawMediaCommand, user_data: *mut c_void) {
    // SAFETY: the core passes a valid command for the duration of the call,
    // and `user_data` is the handler registered by `MediaControls::new`.
    let (command, handler) = unsafe { (&*command, &*user_data.cast::<MediaHandler>()) };
    if let Some(command) = MediaCommand::from_raw(command) {
        handler(command);
    }
}

/// Reads a string from a C API function with the buffer rules of
/// `anomp_engine_device_name`: `call` gets a buffer and its size, and
/// returns the full length.
fn read_string(mut call: impl FnMut(*mut c_char, usize) -> usize) -> String {
    let len = call(std::ptr::null_mut(), 0);
    if len == 0 {
        return String::new();
    }
    let mut buffer = vec![0u8; len + 1];
    let written = call(buffer.as_mut_ptr().cast(), buffer.len()).min(len);
    buffer.truncate(written);
    String::from_utf8_lossy(&buffer).into_owned()
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

/// Like `with_error`, for a call returning a request id (0 on failure).
fn with_request(
    call: impl FnOnce(*mut c_char, usize) -> c_longlong,
) -> Result<LoadRequest, String> {
    let mut error = [0 as c_char; 1024];
    let request = call(error.as_mut_ptr(), error.len());
    if request != 0 {
        return Ok(request);
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
        ANOMP_EVENT_LOAD_FINISHED => Event::LoadFinished {
            request: event.request,
            result: match event.result {
                ANOMP_LOAD_LOADED => LoadResult::Loaded,
                ANOMP_LOAD_FAILED => LoadResult::Failed(if event.error.is_null() {
                    String::new()
                } else {
                    // SAFETY: a non-null `error` is a NUL-terminated string,
                    // valid for the duration of the call.
                    unsafe { CStr::from_ptr(event.error) }
                        .to_string_lossy()
                        .into_owned()
                }),
                _ => LoadResult::Cancelled,
            },
        },
        _ => return,
    };
    handler(event);
}

extern "C" fn on_analysis(frame: *const RawAnalysisFrame, user_data: *mut c_void) {
    // SAFETY: the core passes a valid frame for the duration of the call, and
    // `user_data` is the handler registered by `start_analysis`, which only
    // this (the analysis) thread calls.
    let (frame, handler) = unsafe {
        (
            AnalysisFrame::from_raw(&*frame),
            &mut *user_data.cast::<AnalysisHandler>(),
        )
    };
    handler(&frame);
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
    fn analyses_files_and_parts_of_them() {
        let mut calls = 0;
        let whole = analyse_file(&fixture("flac-44k.flac"), 0.0, 0.0, |_| {
            calls += 1;
            true
        })
        .unwrap();
        assert!(calls >= 1);
        assert!((whole.duration - 22371.0 / 44100.0).abs() < 1e-6);
        assert!(whole.loudness.is_some());
        assert!(whole.true_peak >= whole.sample_peak);
        assert_eq!(whole.histogram.len(), 150);
        assert_eq!((whole.histogram_floor, whole.histogram_step), (-70.0, 0.5));
        assert_eq!(whole.envelope_min.len(), 1000);
        assert_eq!(whole.envelope_max.len(), 1000);

        let part = analyse_file(&fixture("flac-44k.flac"), 0.1, 0.3, |_| true).unwrap();
        assert!((part.duration - 0.2).abs() < 1e-6);

        let error = analyse_file(&fixture("flac-44k.flac"), 0.0, 0.0, |_| false).unwrap_err();
        assert_eq!(error, "Cancelled");
        let error = analyse_file(Path::new("relative.flac"), 0.0, 0.0, |_| true).unwrap_err();
        assert!(error.starts_with("Path is not absolute"), "{error}");
    }

    #[test]
    fn reads_chapters_and_lyrics_only_when_asked() {
        let tags = read_tags_with(
            &fixture("tagged-vorbis.flac"),
            TagParts {
                lyrics: true,
                chapters: true,
                ..TagParts::default()
            },
        )
        .unwrap();
        assert!(tags.chapters.is_empty());
        assert_eq!(tags.lyrics, None);
        assert_eq!(tags.picture, None);
    }

    #[test]
    fn untagged_fields_are_none() {
        let tags = read_tags(&fixture("wav-s16-44k.wav"), true).unwrap();
        assert_eq!(tags.title, None);
        assert_eq!(tags.track_number, None);
        assert_eq!(tags.picture, None);
        assert_eq!(tags.replay_gain, ReplayGain::default());
        assert_eq!(tags.sample_rate, 44100);
    }

    #[test]
    fn replay_gain_is_none_unless_finite() {
        let raw = RawTags {
            title: std::ptr::null(),
            artist: std::ptr::null(),
            album: std::ptr::null(),
            album_artist: std::ptr::null(),
            genre: std::ptr::null(),
            track_number: 0,
            track_total: 0,
            disc_number: 0,
            disc_total: 0,
            year: 0,
            duration: 1.0,
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: 0,
            replaygain_track_gain: -6.5,
            replaygain_track_peak: f64::NAN,
            replaygain_album_gain: f64::INFINITY,
            replaygain_album_peak: 0.5,
            musicbrainz_recording_id: std::ptr::null(),
            musicbrainz_release_id: std::ptr::null(),
            musicbrainz_release_group_id: std::ptr::null(),
            musicbrainz_release_track_id: std::ptr::null(),
            musicbrainz_artist_id: std::ptr::null(),
            musicbrainz_album_artist_id: std::ptr::null(),
            picture: std::ptr::null(),
            picture_size: 0,
            picture_mime_type: std::ptr::null(),
            work: std::ptr::null(),
            movement_name: std::ptr::null(),
            movement_number: 0,
            movement_total: 0,
            composer: std::ptr::null(),
            conductor: std::ptr::null(),
            date: std::ptr::null(),
            original_date: std::ptr::null(),
            lyrics: std::ptr::null(),
            synced_lyrics: std::ptr::null(),
            cuesheet: std::ptr::null(),
            chapter_count: 3,
            chapters: std::ptr::null(),
            rating: 150,
            compilation: 1,
            artists: std::ptr::null(),
        };
        // SAFETY: every pointer is null, which `from_raw` allows.
        let tags = unsafe { Tags::from_raw(&raw) };
        assert_eq!(
            tags.replay_gain,
            ReplayGain {
                track_gain: Some(-6.5),
                track_peak: None,
                album_gain: None,
                album_peak: Some(0.5),
            }
        );
        assert_eq!(tags.rating, None, "out of range");
        assert!(tags.compilation);
        assert!(tags.artists.is_empty());
    }

    #[test]
    fn splits_artist_credits() {
        assert_eq!(split_artists("A; B ;; C"), ["A", "B", "C"]);
        assert_eq!(split_artists("AC/DC"), ["AC/DC"]);
        assert!(split_artists(" ; ").is_empty());
    }

    #[test]
    fn reads_every_field_of_a_file() {
        let info = read_file_info(&fixture("tagged-vorbis.flac")).unwrap();
        assert_eq!(info.codec, "flac");
        assert!(info.lossless);
        assert_eq!(info.bits_per_sample, Some(16));
        assert!(info
            .fields
            .contains(&("TITLE".to_owned(), "Café Déjà Vu".to_owned())));
        assert!(!info.pictures.is_empty());
        assert!(!info.pictures[0].data.is_empty());
        assert!(read_file_info(Path::new("/no/such.flac")).is_err());
    }

    #[test]
    fn dock_menu_reports_enabled_choices() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let seen = chosen.clone();
        let mut menu = DockMenu::new(move |id| seen.borrow_mut().push(id)).unwrap();
        let item = |id, title: &str, enabled| MenuItem {
            id,
            title: title.into(),
            enabled,
            checked: false,
        };
        menu.set_items(&[item(1, "Now: Song", false), item(2, "Pause", true)]);
        assert!(menu.perform(2));
        assert!(!menu.perform(1));
        assert_eq!(*chosen.borrow(), [2]);
    }

    #[test]
    fn volume_watcher_reports_volumes() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let volumes = Rc::new(RefCell::new(Vec::new()));
        let seen = volumes.clone();
        let mut watcher =
            VolumeWatcher::new(move |mounted, path| seen.borrow_mut().push((mounted, path)))
                .unwrap();
        watcher.notify(true, "/Volumes/Música");
        watcher.notify(false, "");
        assert_eq!(
            *volumes.borrow(),
            [
                (true, PathBuf::from("/Volumes/Música")),
                (false, PathBuf::new())
            ]
        );
        assert_eq!(VolumeWatcher::supported(), cfg!(target_os = "macos"));
    }

    #[test]
    fn reads_strings_of_any_length() {
        let text = "Built-in Output ♪";
        let copy = |buffer: *mut c_char, size: usize| {
            if size > 0 {
                let count = text.len().min(size - 1);
                // SAFETY: the caller's buffer holds `size` bytes.
                unsafe {
                    std::ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast(), count);
                    *buffer.add(count) = 0;
                }
            }
            text.len()
        };
        assert_eq!(read_string(copy), text);
        assert_eq!(read_string(|_, _| 0), "");
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

    #[test]
    fn raw_analysis_frames_become_slices() {
        let bands = [0.1f32, 0.5, 0.9];
        let chroma: [f32; 12] = std::array::from_fn(|i| i as f32 / 11.0);
        let left = [0.25f32, -0.25];
        let right = [0.5f32, -0.5];
        let raw = RawAnalysisFrame {
            silent: 0,
            band_count: 3,
            bands: bands.as_ptr(),
            lowest_hz: 30.0,
            highest_hz: 16000.0,
            chroma: chroma.as_ptr(),
            peak_left: 0.8,
            peak_right: 0.7,
            rms_left: 0.4,
            rms_right: 0.3,
            waveform_length: 2,
            waveform_left: left.as_ptr(),
            waveform_right: right.as_ptr(),
            onset: 0.2,
            beat: 1,
        };
        // SAFETY: every pointer is valid for its length for the whole test.
        let frame = unsafe { AnalysisFrame::from_raw(&raw) };
        assert_eq!(
            frame,
            AnalysisFrame {
                silent: false,
                bands: &bands,
                lowest_hz: 30.0,
                highest_hz: 16000.0,
                chroma,
                peak: [0.8, 0.7],
                rms: [0.4, 0.3],
                left: &left,
                right: &right,
                onset: 0.2,
                beat: true,
            }
        );

        // Null pointers and bad lengths read as empty.
        let empty = RawAnalysisFrame {
            silent: 1,
            band_count: -1,
            bands: bands.as_ptr(),
            chroma: std::ptr::null(),
            waveform_length: 2,
            waveform_left: std::ptr::null(),
            waveform_right: std::ptr::null(),
            beat: 0,
            ..raw
        };
        // SAFETY: as above; null pointers are never read.
        let frame = unsafe { AnalysisFrame::from_raw(&empty) };
        assert!(frame.silent && !frame.beat);
        assert!(frame.bands.is_empty() && frame.left.is_empty() && frame.right.is_empty());
        assert_eq!(frame.chroma, [0.0; 12]);
    }
}
