# The C API's types

Every type in `core/include/anomp/anomp.h`, the core's only public
surface, in alphabetical order. The header's unnamed enums are listed by
their constants' prefix (`ANOMP_STATE_*`). Rust declares each of these
again in `app/src-tauri/src/anomp.rs` and wraps it in a safe type, named
in each entry; the conventions they share (strings, ownership, threads,
errors) are in the developer guide's [The C API's
conventions](../developer-guide/04-core.md#the-c-apis-conventions).

Back to the [index](README.md).

### `anomp_analysis_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · the analysis thread · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

Receives each `anomp_analysis_frame` while audio plays, on the core's
analysis thread (`AnalysisThread`), never the main one; the frame's
pointers are valid during the call only. Set with
`anomp_engine_set_analysis_callback`; Rust's `Engine` passes the frame to
the visualizer as an `AnalysisFrame`.

### `anomp_analysis_config`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

How to analyse for the visualizer: the number of spectrum bands, the
waveform's length and the frames per second. Values outside the
documented ranges are refused. Rust: `AnalysisConfig`.

### `anomp_analysis_frame`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · the analysis thread · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

One analysis of what is playing: spectrum bands, chroma, peak and RMS
levels, the latest waveform, onset and beat, note energies, and a
`silent` flag sent once when audio stops. Made from the core's
`AnalysisFrame`; its pointers are valid only during the callback. Rust:
`AnalysisFrame`.

### `anomp_analysis_progress`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · the analysing thread · [D2: Analysing files](../developer-guide/04-core.md#analysing-files-fileanalyser)

Called as `anomp_analyse_file` starts and about four times a second, with
the fraction done; returning 0 cancels the analysis. Rust's
`analyse_file` wraps a closure in it, which the analysis worker uses to
stop when told to.

### `anomp_bookmark`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

A folder's security-scoped bookmark as opaque bytes, to store as is
(`folders.bookmark`). Made by `anomp_bookmark_create` (or `_writable`, for
the recordings' folder), freed by `anomp_bookmark_free`; the core's
`BookmarkHandle` owns the bytes. Rust returns them as a `Vec<u8>`.

### `anomp_chapter`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

One chapter of a file as `anomp_tags` lists them (read with
`ANOMP_TAGS_CHAPTERS`): its start and end in seconds and its title. The
scanner turns each into a track of the file. Rust: `Chapter`.

### `ANOMP_CROSSFEED_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

The crossfeed's presets, off, light, medium and strong, passed to
`anomp_engine_set_crossfeed` and reported in `anomp_signal_path`. The
core's `Crossfeed` holds the filters for each.

### `anomp_device_info`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

The open output device's settings: its buffer size and the size it
prefers, its sample rate, and the latency from the player to the speakers.
Filled from `AudioEngine::DeviceInfo`. Rust: `DeviceInfo`.

### `anomp_dock_menu`

Opaque handle · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h), [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The Dock icon's menu (macOS). Holds the host's `anomp_menu_callback` and
owns a `DockMenu`, declared last so it is destroyed first. Only one exists
at a time. Rust: `DockMenu`, used by `shell/dock.rs`.

### `ANOMP_EFFECT_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: Effects](../developer-guide/05-effects.md#the-chain)

The effects by number (reverb, chorus, freeze, echo, flanger, phaser,
tremolo, lo-fi), the same numbers as the effects library's `EffectType`:
a new effect goes at the end. `anomp_signal_path::effects` has a bit per
effect.

### `anomp_effect_info`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Effects](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

Describes one effect, from the effects library's catalogue (`EffectInfo`):
its stable id, its place in the chain, its default mix and its parameters.
Needs no engine. Rust: `EffectInfo`, which `effects.rs` checks against
`EffectsSettings`.

### `anomp_effect_param`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Effects](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

One parameter of an effect: its id, unit (`ANOMP_EFFECT_UNIT_*`), range,
default and whether it is best moved on a logarithmic scale. Made from
the effects library's `ParamInfo`. Rust: `EffectParam`.

### `ANOMP_EFFECT_UNIT_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: Effects](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

How an effect parameter's value reads: a ratio (shown as a percentage),
hertz, milliseconds, seconds or bits. The effects library's `Unit`; Rust:
`EffectUnit`.

### `anomp_engine`

Opaque handle · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h), [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

The audio engine: owns an `AudioEngine`, the host's event callback and the
output devices last listed. Every `anomp_engine_*` call happens on the main
thread, and events are delivered there. Rust: `Engine`, kept in
`audio.rs`'s main-thread `thread_local` and dropped on `RunEvent::Exit`.

### `anomp_event`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

One engine event (`ANOMP_EVENT_*`) with the fields its type uses: the new
state, whether a track ended by handing off, the position and duration, a
load request's id, result and error. Valid only during the
`anomp_event_callback`. Rust: `Event`.

### `ANOMP_EVENT_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

The engine's event types: device changed, state changed, position, track
ended, load finished and recording failed. Player events are dispatched
about every 50 ms from `AudioEngine`'s timer, never inside an engine call.

### `anomp_event_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The engine on the main thread](../developer-guide/06-rust-backend.md#the-engine-on-the-main-thread)

Receives the engine's `anomp_event`s on the main thread. It may call the
engine (the queue arms the next track from a track ended), but not destroy
it or replace the callback. Rust's `Engine` forwards each event to
`audio.rs`, which hands it to the queue and the frontend.

### `anomp_file_analysis`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Analysing files](../developer-guide/04-core.md#analysing-files-fileanalyser)

What `anomp_analyse_file` measures in one pass over a file: R128 loudness
and its histogram, sample and true peaks, silences at the ends and inside,
the levels at each end, a lossy encoder's cutoff, and an envelope for the
waveform seek bar. Owned by the core's `FileAnalysisHandle`. Rust:
`FileAnalysis`, stored by `library/analysis.rs`.

### `anomp_file_info`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

Everything a file says about itself, for Get Info: every tag field, every
picture, the kinds of tag, and the format as the decoder sees it. Owned
by the core's `FileInfoHandle`. Rust: `FileInfo`.

### `anomp_folder_access`

Opaque handle · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h), [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · any thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

A resolved bookmark: the folder can be read until
`anomp_folder_access_stop`. Owns a `FolderAccess` and the folder's path as
it is now; says whether the bookmark is stale. Rust: `FolderAccess`, held
by `library::access::open_folder`'s result until a file is open.

### `ANOMP_LOAD_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

How an asynchronous load ended (loaded, failed or cancelled), as
`ANOMP_EVENT_LOAD_FINISHED`'s `result`. The core's
`PlayerEngine::LoadResult`; Rust: `LoadResult`.

### `ANOMP_LOG_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Logging](../developer-guide/04-core.md#logging-log)

Log levels, most severe first: error, warn, info, debug. The core's
`Level`; Rust maps them to the `log` crate's levels.

### `anomp_log_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread, the audio thread included · [D2: Logging](../developer-guide/04-core.md#logging-log)

Receives the core's log lines (JUCE's `Logger` and failed assertions
included) on the thread that logs, so it must return quickly and not call
back into the core. Set once with `anomp_set_log_callback`; Rust's
`logging.rs` writes them with its own.

### `ANOMP_MEDIA_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The commands the OS's media controls send: play, pause, toggle, next,
previous and seek. Next and previous arrive only while enabled. The
core's `MediaControls::Command`; Rust: `MediaCommand`.

### `anomp_media_command`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

One command from the media controls: its `ANOMP_MEDIA_*` type and, for a
seek, the position. Valid only during the
`anomp_media_command_callback`. Rust: `MediaCommand`.

### `anomp_media_command_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Receives the media controls' commands on the main thread, never inside a
media-controls call. Rust's `MediaControls` forwards them to `media.rs`,
which routes them to the queue.

### `anomp_media_controls`

Opaque handle · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h), [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The OS's Now Playing and media keys: publishes the track, playback state,
artwork and navigation, and receives commands. Owns a `MediaControls` and
the host's callback. Rust: `MediaControls`, behind `media.rs`'s
`Publisher` trait.

### `anomp_media_track`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The track published to the media controls: title, artist and album
(a null or empty one is left out). Rust's `media.rs` fills it from its
`NowPlaying`.

### `anomp_menu_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Receives the id of the Dock menu item chosen, on the main thread. Rust's
`DockMenu` forwards it to `shell/dock.rs`.

### `anomp_menu_item`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

One Dock menu item: its id, title (none makes a separator), and whether it
is enabled and checked. Copied by `anomp_dock_menu_set_items` into
`DockMenu::Item`s. Rust: `MenuItem`.

### `anomp_picture`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

One embedded picture as `anomp_file_info` lists them: its type, MIME type,
description and bytes. Rust: `FilePicture`.

### `ANOMP_RECORD_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

The recording formats: WAV, AIFF, FLAC, ALAC, AAC and MP3. The core's
`RecordingFormat::Kind`; Rust: `RecordingKind`. A new one needs all three,
and the encoder in `build-ffmpeg.sh`.

### `anomp_record_format`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

The format to record in: an `ANOMP_RECORD_*` value, the bits for PCM and
lossless formats, and the bitrate for AAC and MP3. The core's
`RecordingFormat`; Rust: `RecordingFormat`.

### `anomp_recording`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

How the recording is going (or how the last one went): whether it is on,
frames and seconds written, overruns and files begun. From
`Recorder::Status`; Rust: `RecordingStatus`.

### `ANOMP_RECORDING_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

How a recording failed (the file couldn't be written, or the disk filled
up), as `ANOMP_EVENT_RECORDING_FAILED`'s `result`. The core's
`Recorder::Failure`.

### `anomp_recording_mark`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

Where a track began in a recording: which file and how many seconds into
it. From `Recorder::Mark`; `recording.rs` writes them out as a cue sheet.

### `anomp_signal_path`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

What happens to the sound now, for the signal path panel: the file's
format, track gain, tempo and pitch, resampling, crossfeed, equaliser,
effects, freeze, recording, volume and the device. From
`PlayerEngine::SignalInfo`; Rust: `SignalPath`.

### `ANOMP_STATE_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

The player's states: empty, stopped, playing and paused. The core's
`PlayerEngine::State`; Rust: `PlayerState`.

### `anomp_tag_field`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

One value of one tag field (TagLib's key and the value), as
`anomp_file_info` lists them for Get Info.

### `anomp_tags`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

A file's tags and audio properties as the scanner needs them: titles and
numbers, ReplayGain, MusicBrainz ids, works and movements, dates, rating,
compilation, and on request the picture, lyrics and chapters. Strings are
never null. Owned by the core's `TagsHandle`; made from `TrackTags`.
Rust: `Tags`.

### `ANOMP_TAGS_*`

C enum · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

Flags for `anomp_read_tags`: also copy out the picture, read the lyrics,
or read chapters and an embedded cue sheet. The core's `TagParts`; Rust:
`TagParts`.

### `anomp_track_options`

C struct · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Pressing play on a track](../developer-guide/08-flows.md#pressing-play-on-a-track)

How a track is played: its gain, the part of the file it is (start and
end), a stretch to skip, and for a next track the crossfade. The core's
`PlayerEngine::TrackOptions`; Rust: `TrackOptions`, made by
`library::playback::track_play`.

### `anomp_volume_callback`

Callback type · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Receives a volume mounted or unmounted and its mount point, on the main
thread. Rust's `VolumeWatcher` forwards it to `library::availability`,
which checks the folders on it again.

### `anomp_volume_watcher`

Opaque handle · [`core/include/anomp/anomp.h`](../../core/include/anomp/anomp.h), [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Reports volumes mounted and unmounted (macOS). Owns a `VolumeWatcher` and
the host's `anomp_volume_callback`. Rust: `VolumeWatcher`.
