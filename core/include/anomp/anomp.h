// Public C API for the ano-mp audio core.
// This is the only surface the Tauri (Rust) layer links against, so it must
// stay plain C: no C++ types, no exceptions across the boundary.
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Returns the core library version as "major.minor.patch". Never null. */
const char* anomp_version(void);

/** Returns 1 if the core can decode files with the given extension
    (e.g. "flac" or ".mp3", case-insensitive), otherwise 0. */
int anomp_can_decode_extension(const char* extension);

/* ---- Tags ----------------------------------------------------------------
   Unlike the engine, these functions may be called from any thread,
   concurrently (e.g. by a library scanner). */

/** A file's tags and audio properties. Strings are NUL-terminated UTF-8 and
    never null: "" when the file doesn't have the field. Several values of one
    field (e.g. two ARTIST comments) are joined with "; ". Numbers are 0 when
    absent. */
typedef struct anomp_tags
{
    const char* title;
    const char* artist;
    const char* album;
    const char* album_artist;
    const char* genre;
    int track_number;
    int track_total;
    int disc_number;
    int disc_total;
    int year;

    double duration; /**< Seconds, from the file's headers: lossy files can be
                          off by tens of milliseconds (encoder delay and
                          padding). The player measures exactly on load. */
    int sample_rate; /**< Hz. */
    int channels;
    int bitrate_kbps; /**< Average or nominal; 0 if unknown. */

    /* ReplayGain, NaN when the file doesn't say: gains in dB (Opus R128
       gains are converted to ReplayGain's reference level), peaks as linear
       sample values (1 is full scale). */
    double replaygain_track_gain;
    double replaygain_track_peak;
    double replaygain_album_gain;
    double replaygain_album_peak;

    /* MusicBrainz IDs, named after what they identify (Picard's "track id"
       is the recording, its "album id" the release). */
    const char* musicbrainz_recording_id;
    const char* musicbrainz_release_id;
    const char* musicbrainz_release_group_id;
    const char* musicbrainz_release_track_id;
    const char* musicbrainz_artist_id;
    const char* musicbrainz_album_artist_id;

    /** Embedded front cover (or else the first picture), only when read with
        ANOMP_TAGS_PICTURE: `picture_size` bytes, null when there is none. */
    const unsigned char* picture;
    size_t picture_size;
    const char* picture_mime_type; /**< e.g. "image/jpeg"; "" if unknown. */

    /* Classical works: the work this track is a movement of (ID3v2 TIT1,
       MP4 ©wrk, Vorbis WORK), the movement's name and number (MVNM/MVIN,
       ©mvn/©mvi, MOVEMENTNAME/MOVEMENTNUMBER), and who composed and
       conducted it. */
    const char* work;
    const char* movement_name;
    int movement_number;
    int movement_total;
    const char* composer;
    const char* conductor;

    /* Release dates as tagged, as precise as the tag is: "2004-05-01",
       "2004-05" or "2004"; "" if none. The original release's (ORIGINALDATE,
       TDOR) where the file has it. */
    const char* date;
    const char* original_date;

    /* Only with ANOMP_TAGS_LYRICS: the unsynced lyrics (ID3v2 USLT, MP4
       ©lyr, Vorbis LYRICS or UNSYNCEDLYRICS, which may itself hold LRC
       text), and synced lyrics from an ID3v2 SYLT frame as LRC text, one
       "[mm:ss.xx]line" per line. "" when absent. */
    const char* lyrics;
    const char* synced_lyrics;

    /* Only with ANOMP_TAGS_CHAPTERS: a cue sheet embedded as a CUESHEET tag
       (""), and the chapters the container records: MP4 and Matroska
       chapters, ID3v2 CHAP frames, Ogg CHAPTERxxx comments and a FLAC cue
       sheet's tracks. */
    const char* cuesheet;
    int chapter_count;
    const struct anomp_chapter* chapters; /**< `chapter_count` of them, null if none. */

    /** A rating the tags give, 1..100 (20 a star), 0 if none: an ID3v2 POPM
        frame's (as whole stars), else FMPS_RATING, else RATING, else MP4's
        rate. Read only; the app never writes ratings back. */
    int rating;
    /** 1 if the file is marked as part of a compilation (ID3v2 TCMP, MP4
        cpil, Vorbis COMPILATION), else 0. */
    int compilation;
    /** The credited artists of a multi-valued ARTISTS tag (MusicBrainz
        Picard's), joined with "; " like other fields; "" if none. */
    const char* artists;
} anomp_tags;

/** A chapter of a file, as anomp_tags lists them. */
typedef struct anomp_chapter
{
    double start;      /**< Seconds from the start of the audio. */
    double end;        /**< Seconds; -1 when it runs to the end of the file. */
    const char* title; /**< "" if untitled. */
} anomp_chapter;

/** Flags for anomp_read_tags. */
enum
{
    ANOMP_TAGS_PICTURE = 1, /**< Also copy out the embedded picture. */
    ANOMP_TAGS_LYRICS = 2,  /**< Also read the lyrics. */
    ANOMP_TAGS_CHAPTERS = 4 /**< Also read chapters and an embedded cue sheet. */
};

/** Reads the tags of the file at `path` (absolute, UTF-8) without modifying
    it. Fails for files that neither the tag reader nor the decoder can
    read. Returns null on failure and writes the error message to `error` (see
    anomp_engine_device_name for the buffer rules; `error` may be null).
    Free the result with anomp_tags_free. */
anomp_tags* anomp_read_tags(const char* path, int flags, char* error, size_t error_size);

/** Frees tags returned by anomp_read_tags. Null is ignored. */
void anomp_tags_free(anomp_tags* tags);

/** One value of a tag field, as anomp_file_info lists them. */
typedef struct anomp_tag_field
{
    const char* key; /**< TagLib's name for it, e.g. "TITLE", "MUSICBRAINZ_TRACKID". */
    const char* value;
} anomp_tag_field;

/** An embedded picture, as anomp_file_info lists them. */
typedef struct anomp_picture
{
    const char* type; /**< e.g. "Front Cover"; "" if unknown. */
    const char* mime_type;
    const char* description;
    const unsigned char* data;
    size_t size;
} anomp_picture;

/** Everything a file says about itself, for a "Get Info" view: every tag
    field, every picture, the kinds of tag, and the format as the decoder
    sees it (the same facts as anomp_signal_path's). */
typedef struct anomp_file_info
{
    int field_count;
    const anomp_tag_field* fields; /**< Each value of each field; null if none. */
    int picture_count;
    const anomp_picture* pictures; /**< Null if none. */
    const char* tag_types;         /**< e.g. "ID3v2.4, ID3v1"; "" if untagged. */
    char codec[32];                /**< FFmpeg's codec name; "" if it can't decode the file. */
    int lossless;
    int bits_per_sample; /**< The source's, for lossless codecs; 0 otherwise. */
    int bitrate_kbps;    /**< 0 if unknown. */
    double sample_rate;  /**< Hz. */
    int channels;
    double duration;   /**< Seconds. */
    int64_t file_size; /**< Bytes. */
} anomp_file_info;

/** Reads the FileInfo of the file at `path` (absolute, UTF-8) without
    modifying it; any thread. Returns null on failure and writes the error as
    anomp_read_tags does. Free the result with anomp_file_info_free. */
anomp_file_info* anomp_read_file_info(const char* path, char* error, size_t error_size);

/** Frees a result of anomp_read_file_info. Null is ignored. */
void anomp_file_info_free(anomp_file_info* info);

/* ---- File analysis ------------------------------------------------------
   One pass over a file measuring what ReplayGain, a waveform seek bar and
   a library health check need. Like the tags, it may be called from any
   thread, concurrently, and uses its own decoder, never the engine's. */

/** What anomp_analyse_file measures. Levels are linear (1 is full scale)
    unless named in dB. */
typedef struct anomp_file_analysis
{
    double duration;    /**< Seconds decoded. */
    double sample_rate; /**< Hz. */
    int channels;

    /** EBU R128 integrated loudness in LUFS (a mono file counts as dual
        mono); NaN when it is silent throughout. */
    double integrated_lufs;
    double sample_peak;
    double true_peak; /**< Between samples too (4x oversampled). */

    /** How many 400 ms blocks fell in each `histogram_step` LU wide bin
        from `histogram_floor` LUFS up: enough to gate an album's loudness
        over all of its tracks later. */
    int histogram_count;
    const uint32_t* histogram;
    double histogram_floor;
    double histogram_step;

    /** Seconds below -60 dBFS at the start and at the end. */
    double leading_silence;
    double trailing_silence;
    /** The longest silence inside the track (touching neither end), at
        least 2 s long, e.g. before a hidden track; both 0 if none. */
    double gap_start;
    double gap_length;
    /** RMS of the first and last 50 ms in dBFS; -inf for digital silence. */
    double start_level_db;
    double end_level_db;

    /** Hz where the average spectrum drops off a cliff, as a lossy
        encoder's lowpass leaves it; 0 if it doesn't. */
    double cutoff_hz;

    /** The smallest and largest sample in each of `envelope_length` (at
        most 1000) equal slices of the track. */
    int envelope_length;
    const float* envelope_min;
    const float* envelope_max;
} anomp_file_analysis;

/** Called as a file's analysis starts and then about four times a second,
    with the fraction done (0..1); return 0 to cancel. */
typedef int (*anomp_analysis_progress)(double fraction, void* user_data);

/** Decodes the file at `path` (absolute, UTF-8) once and measures it, or
    only the part of it from `start` to `end` seconds (an `end` not after
    `start`, e.g. 0, is the end of the file) as anomp_track_options does.
    `progress` may be null. Returns null on failure or cancellation and
    writes the error message as anomp_read_tags does. Free the result with
    anomp_file_analysis_free. */
anomp_file_analysis* anomp_analyse_file(const char* path,
                                        double start,
                                        double end,
                                        anomp_analysis_progress progress,
                                        void* user_data,
                                        char* error,
                                        size_t error_size);

/** Frees a result of anomp_analyse_file. Null is ignored. */
void anomp_file_analysis_free(anomp_file_analysis* analysis);

/* ---- Folder access -------------------------------------------------------
   A sandboxed app (the macOS App Sandbox, iOS) may read a folder the user
   picked only until it quits, unless it saves a security-scoped bookmark and
   resolves it in later sessions. A bookmark also follows its folder when it
   is moved or renamed on the same volume. On platforms without a sandbox a
   bookmark holds the path. These functions may be called from any thread. */

/** Opaque bookmark bytes, to store as is. */
typedef struct anomp_bookmark
{
    const unsigned char* data;
    size_t size;
} anomp_bookmark;

/** Creates a bookmark for the folder at `path` (absolute, UTF-8), which the
    app must be able to read now: the user just picked it, or an
    anomp_folder_access for it is open. Returns null on failure and writes the
    error message to `error` (buffer rules of anomp_engine_device_name;
    `error` may be null). Free the result with anomp_bookmark_free. */
anomp_bookmark* anomp_bookmark_create(const char* path, char* error, size_t error_size);

/** anomp_bookmark_create for a folder the app will write to as well (on
    macOS the bookmark's scope then allows writing; the user must have picked
    the folder in an open panel, which grants that). */
anomp_bookmark* anomp_bookmark_create_writable(const char* path, char* error, size_t error_size);

/** Frees a bookmark returned by anomp_bookmark_create. Null is ignored. */
void anomp_bookmark_free(anomp_bookmark* bookmark);

typedef struct anomp_folder_access anomp_folder_access;

/** Resolves a bookmark and starts accessing its folder; the app can read the
    folder until anomp_folder_access_stop. Returns null on failure (e.g. the
    folder was deleted or its volume isn't mounted) and writes the error
    message as anomp_bookmark_create does. */
anomp_folder_access* anomp_folder_access_start(const unsigned char* bookmark,
                                               size_t bookmark_size,
                                               char* error,
                                               size_t error_size);

/** Where the folder is now (absolute, UTF-8), which may differ from where it
    was when the bookmark was made. Valid until anomp_folder_access_stop;
    "" for a null access. */
const char* anomp_folder_access_path(const anomp_folder_access* access);

/** Returns 1 if the bookmark is stale: create a new one for the path above
    while this access is open, and store it in place of the old one. */
int anomp_folder_access_is_stale(const anomp_folder_access* access);

/** Stops accessing the folder and frees `access`. Null is ignored. */
void anomp_folder_access_stop(anomp_folder_access* access);

/** Whether `path` (absolute, UTF-8) is a cloud placeholder (PLAN.md H12):
    a dataless file, listed with its full size but downloaded only when
    something reads it (iCloud Drive with "Optimize Mac Storage"). Checking
    doesn't download it; reading its tags or audio would. Returns 1 if it is
    one, 0 if not (or the platform has none), -1 if it can't be checked (it
    doesn't exist, or a null `path`). The folder must be accessible. */
int anomp_file_is_dataless(const char* path);

/* ---- Logging -------------------------------------------------------------
   The core's log messages, JUCE's Logger and failed assertions (in every
   build) included, for the host to write with its own (PLAN.md H9). */

/** Log levels, most severe first. */
enum
{
    ANOMP_LOG_ERROR = 1,
    ANOMP_LOG_WARN = 2,
    ANOMP_LOG_INFO = 3,
    ANOMP_LOG_DEBUG = 4
};

/** `level` is an ANOMP_LOG_* value; `message` (UTF-8) is valid during the
    call only. Called on the thread that logs, which may be the audio
    thread: return quickly, and don't call back into the core. */
typedef void (*anomp_log_callback)(int level, const char* message, void* user_data);

/** Sends the core's log messages to `callback` (null stops them). Any
    thread; set it once, before anything else. */
void anomp_set_log_callback(anomp_log_callback callback, void* user_data);

/** Writes `message` to the core's log at `level`, as the core does: for
    tests. */
void anomp_log_write(int level, const char* message);

/* ---- Volumes -------------------------------------------------------------
   Reports volumes (drives, disk images, network shares) mounted and
   unmounted, so the host can check library folders on them again (macOS;
   elsewhere nothing is reported yet). Main thread only: the callback runs
   on it, never inside one of these calls. */

typedef struct anomp_volume_watcher anomp_volume_watcher;

/** `mounted` is 1 for a volume mounted, 0 for one unmounted; `path` is its
    mount point (UTF-8, "" if unknown), valid during the call only. */
typedef void (*anomp_volume_callback)(int mounted, const char* path, void* user_data);

/** Returns 1 if this platform reports volumes, 0 if not. */
int anomp_volume_watcher_supported(void);

/** Starts reporting volumes to `callback`. Returns null if `callback` is
    null or on failure. Stop with anomp_volume_watcher_stop. */
anomp_volume_watcher* anomp_volume_watcher_start(anomp_volume_callback callback, void* user_data);

/** Stops reporting and frees `watcher`. Null is ignored. */
void anomp_volume_watcher_stop(anomp_volume_watcher* watcher);

/** Reports a volume as the OS would, for tests: calls the callback now.
    `path` may be null. */
void anomp_volume_watcher_notify(anomp_volume_watcher* watcher, int mounted, const char* path);

/* ---- Engine ------------------------------------------------------------
   Every engine function must be called on the process's main thread, and
   event callbacks are delivered on it. The host must run the platform's main
   run loop (Tauri does). Passing a null engine is safe and does nothing.

   The event callback may call engine functions, e.g. to set the next track
   on ANOMP_EVENT_TRACK_ENDED, but not anomp_engine_destroy or
   anomp_engine_set_event_callback. The engine has taken what it reports
   before it calls back, so a call made there is safe; the rest of that
   dispatch reports the state as it is after the call. */

typedef struct anomp_engine anomp_engine;

/** Event kinds passed to anomp_event_callback. Player events are dispatched
    about every 50 ms, never from inside an engine call. */
enum
{
    ANOMP_EVENT_DEVICE_CHANGED = 1,  /**< Device list or open output device changed. */
    ANOMP_EVENT_STATE_CHANGED = 2,   /**< Player state changed; see `state`. */
    ANOMP_EVENT_POSITION = 3,        /**< Position or duration changed; see `position`, `duration`. */
    ANOMP_EVENT_TRACK_ENDED = 4,     /**< The current track played to its end; see `advanced`. */
    ANOMP_EVENT_LOAD_FINISHED = 5,   /**< An asynchronous load is done; see `request`, `result`, `error`. */
    ANOMP_EVENT_RECORDING_FAILED = 6 /**< The recording stopped on an error; see `result`, `error`. */
};

/** How an asynchronous load (anomp_engine_load_track_async) ended. */
enum
{
    ANOMP_LOAD_LOADED = 0,   /**< The track took its place. */
    ANOMP_LOAD_FAILED = 1,   /**< It couldn't be opened, and nothing changed; see `error`. */
    ANOMP_LOAD_CANCELLED = 2 /**< anomp_engine_cancel_load, or superseded by a later request. */
};

/** Player states. */
enum
{
    ANOMP_STATE_EMPTY = 0,   /**< No track loaded. */
    ANOMP_STATE_STOPPED = 1, /**< Track loaded, at the start. */
    ANOMP_STATE_PLAYING = 2,
    ANOMP_STATE_PAUSED = 3
};

typedef struct anomp_event
{
    int type;          /**< One of the ANOMP_EVENT_* values. */
    int state;         /**< STATE_CHANGED: the new ANOMP_STATE_* value. */
    int advanced;      /**< TRACK_ENDED: 1 if the next track took over gaplessly,
                            0 if playback stopped (a STATE_CHANGED follows). */
    double position;   /**< POSITION: seconds into the current track. */
    double duration;   /**< POSITION: length of the current track in seconds. */
    long long request; /**< LOAD_FINISHED: the id the request returned. */
    int result;        /**< LOAD_FINISHED: one of the ANOMP_LOAD_* values;
                            RECORDING_FAILED: one of the ANOMP_RECORDING_* values. */
    const char* error; /**< LOAD_FINISHED, RECORDING_FAILED: why it failed (UTF-8),
                            else ""; never null. */
} anomp_event;

/** `event` is only valid for the duration of the call. */
typedef void (*anomp_event_callback)(const anomp_event* event, void* user_data);

/** Starts the audio runtime. Returns null on failure. Destroy with
    anomp_engine_destroy. */
anomp_engine* anomp_engine_create(void);

void anomp_engine_destroy(anomp_engine* engine);

/** Sets (or, with a null callback, clears) the event callback. */
void anomp_engine_set_event_callback(anomp_engine* engine, anomp_event_callback callback, void* user_data);

/** Opens the default output device. Returns 1 on success, otherwise 0 and
    writes the error message as UTF-8 to `error` (see anomp_engine_device_name
    for the buffer rules; `error` may be null). */
int anomp_engine_open_default_device(anomp_engine* engine, char* error, size_t error_size);

/** Writes the open output device's name as NUL-terminated UTF-8 into
    `buffer` (truncated to fit; `buffer` may be null when `buffer_size` is 0).
    Returns the full length in bytes excluding the NUL, or 0 if no device is
    open. */
size_t anomp_engine_device_name(anomp_engine* engine, char* buffer, size_t buffer_size);

/** Looks for output devices (of the platform's default device type, e.g.
    Core Audio) and returns how many there are. */
int anomp_engine_output_device_count(anomp_engine* engine);

/** Writes the name of output device `index` (0-based, as found by the last
    anomp_engine_output_device_count) like anomp_engine_device_name. Returns
    0 for an index out of range. */
size_t anomp_engine_output_device_name(anomp_engine* engine, int index, char* buffer, size_t buffer_size);

/** Opens the output device called `name` (UTF-8; null or "" for the
    system's default device) with a buffer of `buffer_size` samples (0 for
    the device's default; a size the device doesn't offer also gets the
    default). Returns 1 on success, otherwise 0 and writes the error as
    anomp_engine_open_default_device does. A name that isn't an output
    device fails without closing the open device; a device that fails to
    open may leave none open. Playback carries on through the new device. */
int anomp_engine_open_device(anomp_engine* engine, const char* name, int buffer_size, char* error, size_t error_size);

/** The open output device's settings. */
typedef struct anomp_device_info
{
    int buffer_size;         /**< Samples per block. */
    int default_buffer_size; /**< What the device prefers. */
    double sample_rate;      /**< Hz. */
    double output_latency;   /**< Seconds from the player to the speakers: one buffer plus
                                  what the device reports. */
} anomp_device_info;

/** Fills `info` with the open device's settings and returns 1, or returns 0
    (leaving `info` alone) if no device is open or `info` is null. */
int anomp_engine_device_info(anomp_engine* engine, anomp_device_info* info);

/** Writes up to `capacity` of the buffer sizes the open device offers,
    smallest first, into `sizes` (may be null when `capacity` is 0), and
    returns how many there are; 0 if no device is open. */
int anomp_engine_buffer_sizes(anomp_engine* engine, int* sizes, int capacity);

/* ---- Player -------------------------------------------------------------
   One current track plus an optional next track that follows it gaplessly.
   The host owns the queue: on ANOMP_EVENT_TRACK_ENDED with `advanced` set,
   the next track has become current, and the host sets a new next track.
   Paths are absolute, UTF-8. Functions that take `error` write the message
   there on failure, with the buffer rules of anomp_engine_device_name. */

/** Largest track gain (about +18 dB); larger gains are clamped to it. */
#define ANOMP_MAX_TRACK_GAIN 8.0

/** Opens `path` as the current track (clearing the next one) and stops at its
    start. `gain` is the track's own linear gain, e.g. from ReplayGain,
    clamped to 0..ANOMP_MAX_TRACK_GAIN (1 leaves it as is); it applies
    before the volume and before the analysis sees the audio. A
    TRACK_ENDED without `advanced` for the replaced track that has not been
    dispatched yet is dropped. Returns 1 on success; on failure returns 0
    and nothing changes. */
int anomp_engine_load(anomp_engine* engine, const char* path, double gain, char* error, size_t error_size);

/** Opens `path` as the track after the current one, with its own `gain`
    (as for anomp_engine_load), which takes over with it sample-exactly; a
    null `path` clears it. Returns 1 on success, 0 on failure (the previous
    next track stays). */
int anomp_engine_set_next(anomp_engine* engine, const char* path, double gain, char* error, size_t error_size);

/** Changes the gain of the current and next tracks opened from `path` (the
    same absolute UTF-8 path they were opened with), ramping over a block, e.g.
    when ReplayGain is turned on. Matching by file rather than by current or
    next can't race a hand-off. Returns how many tracks it changed (0..2). */
int anomp_engine_set_track_gain(anomp_engine* engine, const char* path, double gain);

/** How a track is played: part of a file, and a stretch of it to skip. */
typedef struct anomp_track_options
{
    double gain;  /**< As for anomp_engine_load. */
    double start; /**< Seconds into the file where the track starts; 0 for its start. */
    double end;   /**< Seconds into the file where it ends; 0 (or anything not after
                       `start`) for the end of the file. Positions and durations the
                       engine reports are within the track, from `start`. */
    /** When the track reaches `skip_from` seconds (within the track) it
        jumps to `skip_to`, once, e.g. over a long silence. Negative
        `skip_from`, or `skip_to` not after it, skips nothing. */
    double skip_from;
    double skip_to;
    /** Only for a next track (anomp_engine_set_next_track): seconds over which
        it fades in while the current track fades out, ending as the current
        track ends; 0 (and anything not positive) hands off gaplessly. At most
        ANOMP_MAX_CROSSFADE, and shortened to fit either track. A next track at
        another sample rate, or one set while an A–B loop is on, hands off
        without a crossfade. */
    double crossfade;
} anomp_track_options;

/** Longest crossfade, in seconds. */
#define ANOMP_MAX_CROSSFADE 12.0

/** Options that play a whole file with `gain`, skip nothing and don't
    crossfade. */
anomp_track_options anomp_track_options_default(double gain);

/** anomp_engine_load for part of a file. A hand-off from one part to the
    next part of the same file is as gapless and sample-exact as one
    between files. A null `options` is the whole file at gain 1. */
int anomp_engine_load_track(anomp_engine* engine,
                            const char* path,
                            const anomp_track_options* options,
                            char* error,
                            size_t error_size);

/** anomp_engine_set_next for part of a file; a null `path` clears the next
    track. */
int anomp_engine_set_next_track(anomp_engine* engine,
                                const char* path,
                                const anomp_track_options* options,
                                char* error,
                                size_t error_size);

/* Asynchronous loads (PLAN.md H11). Opening a file can take seconds (a disk
   waking, a network share, a cloud file downloading), so these open it, and
   fill its read-ahead, on a thread of their own, then hand it over on the
   main thread at an event dispatch, soon after it is ready. Each returns a
   request id (> 0), reported exactly once by ANOMP_EVENT_LOAD_FINISHED,
   after any TRACK_ENDED of the same dispatch and before its STATE_CHANGED
   and POSITION. Requests take effect in the order they were made. The host
   must keep the file's folder accessible (anomp_folder_access) until its
   request is reported. */

/** anomp_engine_load_track on a thread of its own. Until it is reported,
    nothing changes and the current track plays on. Cancels every request
    made before it. Returns 0, writing the error, if the request can't be
    made (a null engine, a path that isn't absolute). */
long long anomp_engine_load_track_async(anomp_engine* engine,
                                        const char* path,
                                        const anomp_track_options* options,
                                        char* error,
                                        size_t error_size);

/** anomp_engine_set_next_track on a thread of its own. Clears the next track
    at once, so the current one can't hand off to the track being replaced,
    and cancels earlier next requests. One made after a load still opening
    waits for it. With no track loaded when it is ready, it fails. Returns
    0 as anomp_engine_load_track_async does. */
long long anomp_engine_set_next_track_async(anomp_engine* engine,
                                            const char* path,
                                            const anomp_track_options* options,
                                            char* error,
                                            size_t error_size);

/** Cancels a request not yet reported (the next dispatch reports it
    cancelled); does nothing otherwise. A file still opening is opened to
    the end on its thread, then let go. The synchronous load and next-track
    functions cancel requests as their asynchronous forms would. */
void anomp_engine_cancel_load(anomp_engine* engine, long long request);

/** anomp_engine_set_track_gain for the track opened from `path` whose
    options started at `start` seconds, telling two parts of one file
    apart. */
int anomp_engine_set_track_gain_at(anomp_engine* engine, const char* path, double start, double gain);

/** Starts or resumes playback. Returns 0 if no track is loaded. */
int anomp_engine_play(anomp_engine* engine);

/** Pauses (with a short fade). Does nothing unless playing. */
void anomp_engine_pause(anomp_engine* engine);

/** Stops and rewinds to the start of the current track. */
void anomp_engine_stop(anomp_engine* engine);

/** Seeks to `seconds` into the current track, clamped to its length.
    Returns 0 if no track is loaded or `seconds` is not finite. */
int anomp_engine_seek(anomp_engine* engine, double seconds);

/** Sets the linear output gain, clamped to 0..1. */
void anomp_engine_set_volume(anomp_engine* engine, double gain);

double anomp_engine_volume(anomp_engine* engine);

/** Returns an ANOMP_STATE_* value. */
int anomp_engine_state(anomp_engine* engine);

/** Seconds into the current track, or 0 if none is loaded. */
double anomp_engine_position(anomp_engine* engine);

/** Length of the current track in seconds, or 0 if none is loaded. */
double anomp_engine_duration(anomp_engine* engine);

/* ---- Practice ------------------------------------------------------------
   An A–B loop and playing slower or faster without changing the pitch (or
   changing the pitch alone), for learning or transcribing a part. */

/** Loops the current track between `start` and `end` seconds (within the
    track), jumping back at `end` on the exact sample. The engine opens the
    file again for the jump, so call it while the file can be opened (its
    folder's bookmark resolved). A loop shorter than 0.25 s is refused.
    Returns 1 on success; a negative `start` clears the loop (returns 1).
    The loop ends when another track becomes current. */
int anomp_engine_set_loop(anomp_engine* engine, double start, double end, char* error, size_t error_size);

/** Writes the loop's start and end (either may be null) and returns 1, or
    returns 0 when no loop is set. */
int anomp_engine_loop(anomp_engine* engine, double* start, double* end);

/** Plays at `rate` times the normal speed (0.5..1.5) without changing the
    pitch, shifted by `semitones` (-12..12). 1 and 0 play the audio as it
    is. Returns 0, changing nothing, for values out of range. */
int anomp_engine_set_tempo(anomp_engine* engine, double rate, double semitones);

/* ---- Headphones ----------------------------------------------------------
   Crossfeed blends a delayed, low-passed part of each channel into the
   other, so hard-panned stereo is less tiring on headphones. */

enum
{
    ANOMP_CROSSFEED_OFF = 0,
    ANOMP_CROSSFEED_LIGHT = 1,  /**< 700 Hz, 4.5 dB. */
    ANOMP_CROSSFEED_MEDIUM = 2, /**< 700 Hz, 6 dB. */
    ANOMP_CROSSFEED_STRONG = 3  /**< 650 Hz, 9.5 dB. */
};

/** Sets the crossfeed (an ANOMP_CROSSFEED_* value); it applies after the
    analysis tap, so the visualizer shows the mix as it is. Returns 0 for
    an unknown level. */
int anomp_engine_set_crossfeed(anomp_engine* engine, int level);

/** 1 if the open output device plays through headphones as far as the OS
    can tell (the built-in output's headphone jack on macOS, the audio
    route on iOS), 0 if not, -1 if it can't tell. */
int anomp_engine_output_is_headphones(anomp_engine* engine);

/* ---- Equaliser -----------------------------------------------------------
   A 10-band graphic equaliser with a preamp, after the analysis tap (next to
   the crossfeed, before the volume), at ISO octave centres: 31, 62, 125,
   250, 500 Hz, 1, 2, 4, 8 and 16 kHz. */

#define ANOMP_EQ_BANDS 10
#define ANOMP_EQ_MAX_GAIN 12.0

/** Turns the equaliser on with `gains_db` (ANOMP_EQ_BANDS values, low band
    first, each within ±ANOMP_EQ_MAX_GAIN) and `preamp_db` (within the same
    range), or off with a null `gains_db`. Changes are smoothed over a few
    milliseconds. Returns 0, changing nothing, for values out of range. */
int anomp_engine_set_equaliser(anomp_engine* engine, const double* gains_db, double preamp_db);

/* ---- Effects -------------------------------------------------------------
   Real-time effects on what is playing (the anomp_effects library), after
   the resampler and before the analysis tap, so the visualizer shows what
   they make; the equaliser, crossfeed and volume come after them. They run
   in a fixed order (each effect's `position`) and carry on across a
   hand-off or a crossfade, so tails ring into the next track; a held
   freeze lets go when another track takes over. Each has a switch, a
   wet/dry mix and up to ANOMP_EFFECT_MAX_PARAMS parameters; every change
   glides, so none clicks. All off, the signal is untouched. Settings may be
   changed from any thread; they reach the audio thread without locks. */

#define ANOMP_EFFECT_COUNT 8
#define ANOMP_EFFECT_MAX_PARAMS 4

enum
{
    ANOMP_EFFECT_REVERB = 0,  /**< A room: size, damping, width, pre-delay. */
    ANOMP_EFFECT_CHORUS = 1,  /**< Drifting copies: rate, depth. */
    ANOMP_EFFECT_FREEZE = 2,  /**< Spectral freeze: holds the sound (anomp_engine_set_freeze); fade. */
    ANOMP_EFFECT_ECHO = 3,    /**< Repeats: time, feedback, tone, spread (ping-pong). */
    ANOMP_EFFECT_FLANGER = 4, /**< A swept comb: rate, depth, feedback. */
    ANOMP_EFFECT_PHASER = 5,  /**< Swept notches: rate, depth, feedback. */
    ANOMP_EFFECT_TREMOLO = 6, /**< Level or pan moving: rate, depth, stereo. */
    ANOMP_EFFECT_LOFI = 7     /**< Fewer bits, a lower rate: bits, sample rate. */
};

/** How a parameter's value reads. */
enum
{
    ANOMP_EFFECT_UNIT_RATIO = 0, /**< 0 to 1 (or -1 to 1); show as a percentage. */
    ANOMP_EFFECT_UNIT_HERTZ = 1,
    ANOMP_EFFECT_UNIT_MILLISECONDS = 2,
    ANOMP_EFFECT_UNIT_SECONDS = 3,
    ANOMP_EFFECT_UNIT_BITS = 4
};

typedef struct anomp_effect_param
{
    char id[32]; /**< Stable, lower camel case, e.g. "preDelay". */
    int unit;    /**< An ANOMP_EFFECT_UNIT_* value. */
    double min;
    double max;
    double default_value;
    int logarithmic; /**< 1 if best moved on a logarithmic scale. */
} anomp_effect_param;

typedef struct anomp_effect_info
{
    char id[32];  /**< Stable, lower camel case, e.g. "reverb". */
    int position; /**< Where it runs in the chain, 0 first. */
    double default_mix;
    int param_count;
    anomp_effect_param params[ANOMP_EFFECT_MAX_PARAMS];
} anomp_effect_info;

/** Describes `effect` (an ANOMP_EFFECT_* value). Needs no engine. Returns 0
    for an unknown effect or a null `info`. */
int anomp_effect_describe(int effect, anomp_effect_info* info);

/** Switches `effect` on or off with its wet/dry `mix` (0 to 1) and
    `param_count` parameters (as anomp_effect_describe lists them, each within
    its range; fewer leave the rest as they were). Returns 0, changing
    nothing, for an unknown effect, too many parameters, or a value out of
    range. Switching the freeze off lets go of it. */
int anomp_engine_set_effect(anomp_engine* engine,
                            int effect,
                            int enabled,
                            double mix,
                            const double* params,
                            int param_count);

/** Holds the spectral freeze's sound (1) or lets it go (0). Holding needs
    the freeze on. Returns 1 if it holds afterwards. */
int anomp_engine_set_freeze(anomp_engine* engine, int hold);

/** 1 while the freeze holds. A track taking over lets it go. */
int anomp_engine_freeze_held(anomp_engine* engine);

/* ---- Signal path ---------------------------------------------------------
   Every step between the file and the speakers, for showing the user. */

typedef struct anomp_signal_path
{
    int loaded;     /**< 0 when no track is loaded; the file fields are then 0 or "". */
    char codec[32]; /**< FFmpeg's codec name, e.g. "flac", "mp3". */
    int lossless;
    int bits_per_sample;     /**< The source's, for lossless codecs; 0 otherwise. */
    int bitrate_kbps;        /**< 0 if unknown. */
    double file_sample_rate; /**< Hz. */
    int file_channels;
    double track_gain; /**< Linear, as passed with the track. */
    double tempo;      /**< 1 unless practising. */
    double semitones;
    int resampling; /**< 1 if the file's rate differs from the device's. */
    int crossfeed;  /**< An ANOMP_CROSSFEED_* value. */
    double volume;  /**< Linear. */
    double device_sample_rate;
    int device_buffer_size;
    int equaliser;    /**< 1 while the equaliser is on. */
    double crossfade; /**< Seconds the next track is set to crossfade over; 0 if none. */
    int effects;      /**< Bit (1 << ANOMP_EFFECT_*) set for each effect on, or still ringing out. */
    int freeze_held;  /**< 1 while the freeze holds. */
    int recording;    /**< 1 while recording (anomp_engine_record_start). */
} anomp_signal_path;

/** Fills `path`; returns 0 for a null engine or `path`. */
int anomp_engine_signal_path(anomp_engine* engine, anomp_signal_path* path);

/** Switches the open output device to `sample_rate` Hz if it offers that
    rate, reopening it; playback carries on, with a short interruption.
    Returns 1 if the device runs at that rate afterwards (it already may
    have), 0 if it doesn't offer it or none is open. On macOS the rate is
    the device's, so other apps playing through it change too. */
int anomp_engine_set_device_sample_rate(anomp_engine* engine, double sample_rate);

/* ---- Recording -----------------------------------------------------------
   Writes what is played to a file until stopped (PLAN.md X6): after the
   effects, the equaliser and crossfeed, before the volume. Across track
   changes, gapless hand-offs and crossfades; nothing while paused (a pause
   fades out and in, as heard). The audio thread never waits for the file:
   samples it can't hand over are dropped and counted as overruns. A change
   of the device's sample rate starts another file, named after the first
   with a number ("name 2.wav"). A write error or a full disk stops the
   recording, finalises what was written and is reported by
   ANOMP_EVENT_RECORDING_FAILED. Main thread only, like the rest of the
   engine. */

/** Recording formats. */
enum
{
    ANOMP_RECORD_WAV = 0,  /**< PCM: bits 16, 24 or 32 (float); RF64 past 4 GB. */
    ANOMP_RECORD_AIFF = 1, /**< PCM: bits 16 or 24. */
    ANOMP_RECORD_FLAC = 2, /**< Bits 16 or 24. */
    ANOMP_RECORD_ALAC = 3, /**< Apple Lossless in .m4a: bits 16 or 24. */
    ANOMP_RECORD_AAC = 4,  /**< AAC-LC in .m4a at bitrate_kbps; at most 96 kHz (higher rates are resampled). */
    ANOMP_RECORD_MP3 = 5   /**< MP3 (LAME) at a constant bitrate_kbps; at most 48 kHz (resampled likewise). */
};
#define ANOMP_RECORD_FORMAT_COUNT 6
#define ANOMP_RECORD_MIN_BITRATE 96
#define ANOMP_RECORD_MAX_BITRATE 320

typedef struct anomp_record_format
{
    int format;       /**< An ANOMP_RECORD_* value. */
    int bits;         /**< For the PCM and lossless formats. */
    int bitrate_kbps; /**< For AAC and MP3: ANOMP_RECORD_MIN_BITRATE to ANOMP_RECORD_MAX_BITRATE. */
} anomp_record_format;

/** 1 if this build can record `format` (an ANOMP_RECORD_* value). Any thread. */
int anomp_record_format_available(int format);

/** The file name extension (without the dot, e.g. "m4a") of `format`, or ""
    for a value that isn't one. Any thread. */
const char* anomp_record_format_extension(int format);

/** Ways a recording fails, as ANOMP_EVENT_RECORDING_FAILED's `result`. */
enum
{
    ANOMP_RECORDING_WRITE_FAILED = 1, /**< The file couldn't be written; see `error`. */
    ANOMP_RECORDING_DISK_FULL = 2     /**< The disk filled up. */
};

/** Starts recording to `path` (absolute, UTF-8; its folder must exist and
    be writable, and an existing file is replaced) at the device's sample
    rate. The file is created at once, so a folder or format that won't do
    fails here. Returns 1 on success; 0 on failure (already recording, no
    device open, a format this build can't write or out of range), with the
    error written as for anomp_engine_load. */
int anomp_engine_record_start(anomp_engine* engine,
                              const char* path,
                              const anomp_record_format* format,
                              char* error,
                              size_t error_size);

/** Writes what is still buffered, finalises the file and stops recording.
    Does nothing unless recording. */
void anomp_engine_record_stop(anomp_engine* engine);

typedef struct anomp_recording
{
    int recording;    /**< 1 from anomp_engine_record_start to anomp_engine_record_stop. */
    int64_t frames;   /**< Frames written to the files so far. */
    double seconds;   /**< Their length. */
    int64_t overruns; /**< Times samples were dropped because the file fell behind. */
    int files;        /**< Files begun (one per sample rate). */
} anomp_recording;

/** Fills `status` (the last recording's, after it stopped); 0 for a null
    engine or `status`. */
int anomp_engine_recording(anomp_engine* engine, anomp_recording* status);

/** The path of the file being written (or last written) as UTF-8, with the
    buffer rules of anomp_engine_device_name; "" before any recording. */
size_t anomp_engine_recording_file(anomp_engine* engine, char* buffer, size_t buffer_size);

/** Where a track began in a recording. */
typedef struct anomp_recording_mark
{
    int file;       /**< Which file: 0 for the first, 1 for "name 2", and so on. */
    double seconds; /**< Into that file. */
} anomp_recording_mark;

/** Copies up to `capacity` of the recording's track marks, in order, into
    `marks` (which may be null when `capacity` is 0): one when the recording
    began with a track loaded, then one each time another track became
    current (at the start of a crossfade into it). Returns how many there
    are, which may be more than `capacity`. Kept after the recording stops,
    until the next one begins. */
int anomp_engine_recording_marks(anomp_engine* engine, anomp_recording_mark* marks, int capacity);

/** How many times a next track has taken over gaplessly since the engine was
    created. TRACK_ENDED reports a hand-off up to 50 ms late; this counts it
    as it happens, so before changing the next track (or loading) the host
    can tell whether the one it set has already become current. 0 for a null
    engine. */
int64_t anomp_engine_advance_count(anomp_engine* engine);

/* ---- Analysis ------------------------------------------------------------
   What the player plays, analysed for a visualizer: a spectrum, pitch
   classes, levels, the waveform and beats. It is measured before the
   volume, so turning the volume down doesn't shrink it. Analysis runs on a
   thread of its own only while a callback is set, and frames are delivered
   on that thread: the callback must not call any engine function. */

/** How to analyse. Values outside the documented ranges are refused. */
typedef struct anomp_analysis_config
{
    int band_count;           /**< 4..256 spectrum bands. */
    int waveform_length;      /**< 16..2048 samples per channel. */
    double frames_per_second; /**< 1..120. */
} anomp_analysis_config;

/** One analysis. Pointers are valid only for the duration of the callback. */
typedef struct anomp_analysis_frame
{
    /** 1 once no audio has played for 150 ms (paused, stopped, no device):
        every value below is 0. Sent once; the next frame comes when audio
        plays again. */
    int silent;

    /** `band_count` values, 0..1, log-spaced from `lowest_hz` to
        `highest_hz`, low first: each band's loudest component, 0 at
        -70 dB and 1 at full scale, tilted up 3 dB per octave around 1 kHz
        so typical music looks level. */
    int band_count;
    const float* bands;
    float lowest_hz;
    float highest_hz;

    /** 12 values, 0..1: energy per pitch class, C first, the strongest 1. */
    const float* chroma;

    /** Linear 0..1 per channel over about the last 40 ms. */
    float peak_left;
    float peak_right;
    float rms_left;
    float rms_right;

    /** The latest `waveform_length` samples per channel (-1..1), starting
        at a rising zero crossing where there is one near, so a periodic
        wave draws in place. */
    int waveform_length;
    const float* waveform_left;
    const float* waveform_right;

    /** How much louder the spectrum got since the previous frame, 0..1. */
    float onset;
    /** 1 when this frame starts a beat: a jump in the bass well above the
        last second and a half, at most 4 a second. */
    int beat;

    /** `note_count` values, 0..1, a semitone apart from MIDI note
        `lowest_note` (36, C2) up: the loudest component within half a
        semitone of each, on the bands' scale. */
    int note_count;
    int lowest_note;
    const float* notes;

    /** `band_count` values, -1..1: where each band's sound sits between
        the left (-1) and the right (+1) channel; 0 for a band too quiet to
        place. */
    const float* balance;
} anomp_analysis_frame;

typedef void (*anomp_analysis_callback)(const anomp_analysis_frame* frame, void* user_data);

/** Starts analysing with `config`, calling `callback` on the analysis
    thread about `frames_per_second` times a second while audio plays, or
    stops with a null callback (`config` is then ignored and may be null).
    Replaces any analysis running, waiting for its callback to return, so
    once this returns the previous callback and `user_data` are no longer
    used. Returns 1 on success, 0 for a null engine or an invalid config
    (the running analysis, if any, is left as it was). Main thread only. */
int anomp_engine_set_analysis_callback(anomp_engine* engine,
                                       const anomp_analysis_config* config,
                                       anomp_analysis_callback callback,
                                       void* user_data);

/* ---- Media controls ------------------------------------------------------
   The OS's media controls: what is playing, shown by the system (Control
   Center and the menu-bar Now Playing widget on macOS, the lock screen on
   iOS), and the commands it sends back (media keys, those controls).
   Independent of the engine: the host publishes what its queue and player
   are doing, and routes the commands to them. On platforms without an
   implementation yet the functions accept everything and publish nothing.
   Like the engine, every function must be called on the main thread, and
   commands are delivered on it, never from inside one of these calls.
   Passing a null handle is safe and does nothing. */

typedef struct anomp_media_controls anomp_media_controls;

/** Command kinds passed to anomp_media_command_callback. */
enum
{
    ANOMP_MEDIA_PLAY = 1,
    ANOMP_MEDIA_PAUSE = 2,
    ANOMP_MEDIA_TOGGLE = 3,   /**< Play or pause. */
    ANOMP_MEDIA_NEXT = 4,     /**< Only while enabled (anomp_media_controls_set_navigation). */
    ANOMP_MEDIA_PREVIOUS = 5, /**< Only while enabled. */
    ANOMP_MEDIA_SEEK = 6      /**< To `position`, e.g. scrubbing the progress bar. */
};

typedef struct anomp_media_command
{
    int type;        /**< One of the ANOMP_MEDIA_* values. */
    double position; /**< SEEK: seconds into the track, finite and within the
                          published duration; 0 otherwise. */
} anomp_media_command;

/** `command` is only valid for the duration of the call. */
typedef void (*anomp_media_command_callback)(const anomp_media_command* command, void* user_data);

/** A track as published; each string is UTF-8, and null or "" leaves the
    field out. */
typedef struct anomp_media_track
{
    const char* title;
    const char* artist;
    const char* album;
} anomp_media_track;

/** Returns 1 if this platform's media controls reach the OS, 0 if they do
    nothing. */
int anomp_media_controls_supported(void);

/** Starts receiving the OS's commands, which go to `callback` (null ignores
    them). Nothing is published until a track is set. Returns null on
    failure. Destroy with anomp_media_controls_destroy. */
anomp_media_controls* anomp_media_controls_create(anomp_media_command_callback callback, void* user_data);

/** Clears what was published, stops receiving commands and frees
    `controls`. */
void anomp_media_controls_destroy(anomp_media_controls* controls);

/** Publishes a new current track. Artwork, playback and navigation stay as
    they were: set them too when they change. Returns 0 if `track` is null. */
int anomp_media_controls_set_track(anomp_media_controls* controls, const anomp_media_track* track);

/** Publishes the playback state (an ANOMP_STATE_* value: PLAYING, else
    paused), the position `elapsed` seconds into the track, and its
    `duration` in seconds (0 if unknown). Call on play, pause, seek and
    track change; the system moves the position on by itself while playing.
    `elapsed` is clamped to [0, duration]. Returns 0, changing nothing, if
    the state is unknown, a value is not finite or the duration is
    negative. */
int anomp_media_controls_set_playback(anomp_media_controls* controls, int state, double elapsed, double duration);

/** Publishes the artwork as encoded image bytes (JPEG or PNG; other
    formats the platform decodes also work), or clears it when `size` is 0.
    Returns 1 on success, 0 (and the artwork is cleared) if the platform
    cannot decode it or `data` is null with a non-zero `size`. */
int anomp_media_controls_set_artwork(anomp_media_controls* controls, const unsigned char* data, size_t size);

/** Enables (non-zero) or disables the next and previous commands, e.g.
    from whether the queue has an item to go to. Both start disabled. */
void anomp_media_controls_set_navigation(anomp_media_controls* controls, int has_next, int has_previous);

/** Clears everything published, e.g. when the queue is emptied. */
void anomp_media_controls_clear(anomp_media_controls* controls);

/** Handles `command` as if the OS had sent it: disabled or invalid commands
    are dropped, a seek is clamped, and the rest reach the callback before
    this returns (unlike the OS's, which never arrive inside a call). For
    tests; returns 1 if the callback was called. */
int anomp_media_controls_perform(anomp_media_controls* controls, const anomp_media_command* command);

/* ---- Dock menu ------------------------------------------------------------
   The menu the OS shows for the app's Dock icon (macOS): items the host
   lists, each reporting its id when chosen. On other platforms the
   functions accept everything and show nothing. Main thread only, like the
   media controls; choices are delivered on it, never inside one of these
   calls. */

typedef struct anomp_dock_menu anomp_dock_menu;

/** One item of the Dock menu. */
typedef struct anomp_menu_item
{
    int id;            /**< Reported when chosen. */
    const char* title; /**< UTF-8; null or "" makes a separator. */
    int enabled;       /**< 0 shows it greyed out (e.g. the current track's title). */
    int checked;       /**< 1 shows a check mark. */
} anomp_menu_item;

typedef void (*anomp_menu_callback)(int id, void* user_data);

/** Returns 1 if this platform has a Dock menu the core fills, 0 if not. */
int anomp_dock_menu_supported(void);

/** Starts showing a Dock menu (empty until items are set), whose choices go
    to `callback`. Returns null on failure or if another one exists. Destroy
    with anomp_dock_menu_destroy. */
anomp_dock_menu* anomp_dock_menu_create(anomp_menu_callback callback, void* user_data);

/** Removes the menu and frees `menu`. Null is ignored. */
void anomp_dock_menu_destroy(anomp_dock_menu* menu);

/** Replaces the menu's items with `count` items (copied). */
void anomp_dock_menu_set_items(anomp_dock_menu* menu, const anomp_menu_item* items, int count);

/** Chooses the item `id` as the OS would, for tests: returns 1 if it exists
    and is enabled (and the callback ran), else 0. */
int anomp_dock_menu_perform(anomp_dock_menu* menu, int id);

/* ---- Test tone ---------------------------------------------------------- */

/** Pauses the player and plays a sine test tone on the open device. Returns 1
    on success, 0 if no device is open or the frequency is not positive. */
int anomp_engine_play_test_tone(anomp_engine* engine, double frequency_hz);

/** Stops the tone and reconnects the player. */
void anomp_engine_stop_test_tone(anomp_engine* engine);

#ifdef __cplusplus
}
#endif
