# The core's types (C++)

Every class, struct and enum in `core/src/`, public to the core or private
to one file, in alphabetical order; nested types are named with their
owner (`PlayerEngine::TrackOptions`). Everything here is in the `anomp`
namespace and private to the core: Rust sees only the C API's types (see
[the C API page](c-api.md)). The developer guide's [chapter
4](../developer-guide/04-core.md) explains how they fit together.

Back to the [index](README.md).

### `AnalysisFrame`

Struct · [`core/src/SpectrumAnalyser.h`](../../core/src/SpectrumAnalyser.h) · the analysis thread · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

One analysis of the latest audio for the visualizer: spectrum bands,
chroma, note energies, stereo balance per band, peak and RMS levels, a
waveform starting at a rising zero crossing, onset and beat, or `silent`.
Filled by `SpectrumAnalyser`, sent by `AnalysisThread`, and copied out as
`anomp_analysis_frame`.

### `AnalysisThread`

Class · [`core/src/AnalysisThread.h`](../../core/src/AnalysisThread.h) · its own thread · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

Analyses the player's `SignalTap` on a thread of its own at a steady frame
rate and hands each `AnalysisFrame` to a callback on that thread. Only
analyses when the tap has new samples; after 150 ms without any, sends one
silent frame and then nothing. Owns a `SpectrumAnalyser`; `AudioEngine`
starts and replaces it.

### `AppleBackend`

Class · [`core/src/DockMenu_apple.mm`](../../core/src/DockMenu_apple.mm), [`core/src/MediaControls_apple.mm`](../../core/src/MediaControls_apple.mm) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Two private classes of this name, each the macOS `Backend` of its unit.
`DockMenu_apple.mm`'s adds `applicationDockMenu:` to the app delegate and
builds an `NSMenu` from the `DockMenu`'s items. `MediaControls_apple.mm`'s
publishes `MediaControls::State` to `MPNowPlayingInfoCenter` and registers
for `MPRemoteCommandCenter`'s commands.

### `AppleBackend::Registration`

Helper · [`core/src/MediaControls_apple.mm`](../../core/src/MediaControls_apple.mm)

A remote command and the token its handler was added with, so the backend
can remove it when it detaches.

### `AudioEngine`

Class · [`core/src/AudioEngine.h`](../../core/src/AudioEngine.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

Owns JUCE's runtime (through `JuceRuntime`), the output device, the
read-ahead thread and the `PlayerEngine`; opens and lists devices, switches
sample rates, plays the test tone, starts the visualizer's
`AnalysisThread`, and dispatches the player's events from a main-thread
timer. Created, used and destroyed on the main thread, inside
`anomp_engine`.

### `AudioEngine::DeviceInfo`

Struct · [`core/src/AudioEngine.h`](../../core/src/AudioEngine.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

The open device's buffer size, its preferred and offered sizes, sample
rate and output latency. Copied out as `anomp_device_info`.

### `BookmarkHandle`

Struct · [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · [D2: The C API's conventions](../developer-guide/04-core.md#the-c-apis-conventions)

An `anomp_bookmark` that owns the bytes behind its pointer, freed by
`anomp_bookmark_free`.

### `Crossfeed`

Class · [`core/src/Crossfeed.h`](../../core/src/Crossfeed.h) · audio thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

Headphone crossfeed after Bauer's method: each ear also hears the other
channel low-passed, with a matching high boost on the direct signal. Three
levels (`ANOMP_CROSSFEED_*`). Owned by `PlayerEngine` and processed on the
audio thread; changed through the player.

### `DockMenu`

Class · [`core/src/DockMenu.h`](../../core/src/DockMenu.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The Dock icon's menu, platform-neutral: keeps the items and calls the
handler for an enabled one chosen; a `DockMenu::Backend` per OS shows it.
One exists at a time, owned by `anomp_dock_menu`.

### `DockMenu::Backend`

Struct · [`core/src/DockMenu.h`](../../core/src/DockMenu.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The platform part of `DockMenu`: attach, detach and update. The base does
nothing (the fallback, and `DockMenu_none.cpp`'s); macOS's is
`AppleBackend`.

### `DockMenu::Item`

Struct · [`core/src/DockMenu.h`](../../core/src/DockMenu.h) · main thread

One Dock menu item: id, title (empty for a separator), enabled and
checked. Copied from `anomp_menu_item`.

### `Equaliser`

Class · [`core/src/Equaliser.h`](../../core/src/Equaliser.h) · audio thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

A 10-band graphic equaliser with a preamp: RBJ peaking biquads an octave
wide from 31 Hz to 16 kHz. Changes glide (at most 120 dB a second), and
turned off it glides to flat, then bypasses itself. Owned by
`PlayerEngine`; its targets change under the lock the audio thread holds.

### `Equaliser::Biquad`

Helper · [`core/src/Equaliser.h`](../../core/src/Equaliser.h)

One band's filter coefficients and its state for both channels.

### `FFmpegAudioFormat`

Class · [`core/src/FFmpegAudioFormat.h`](../../core/src/FFmpegAudioFormat.h) · any thread · [D2: Decoding](../developer-guide/04-core.md#decoding-ffmpegaudioformat)

The one JUCE `AudioFormat` the core registers: decodes every supported
file type with FFmpeg into float samples at the file's rate, trims encoder
delay and padding, and seeks to exact samples. Creates
`FFmpegAudioFormatReader`s; reads a file's chapters without decoding.
Registered by `FormatRegistry`. Only its `.cpp` (and `FFmpegEncoder`'s)
include FFmpeg.

### `FFmpegAudioFormat::Chapter`

Struct · [`core/src/FFmpegAudioFormat.h`](../../core/src/FFmpegAudioFormat.h) · any thread · [D2: Decoding](../developer-guide/04-core.md#decoding-ffmpegaudioformat)

A chapter a container records (MP4 and Matroska chapters, ID3v2 CHAP,
Ogg CHAPTERxxx comments, a FLAC cue sheet's tracks): start, end and title.
`readTags` copies them into `TrackTags::Chapter`.

### `FFmpegAudioFormatReader`

Class · [`core/src/FFmpegAudioFormat.cpp`](../../core/src/FFmpegAudioFormat.cpp) · read-ahead thread · [D2: Decoding](../developer-guide/04-core.md#decoding-ffmpegaudioformat)

A JUCE `AudioFormatReader` over FFmpeg: demuxes through a custom I/O
context on JUCE's stream, decodes, converts to float with libswresample,
and keeps sample positions exact across seeks (rewinding reopens the
demuxer; see `docs/design/phase-1-playback-engine.md`). Sets the codec,
lossless, bits and bit rate keys its format names.

### `FFmpegAudioFormatReader::IndexEntry`

Helper · [`core/src/FFmpegAudioFormat.cpp`](../../core/src/FFmpegAudioFormat.cpp)

A packet's byte position, timestamp and size, in the exact seek index the
reader builds by reading every packet once for files that can't seek
accurately by timestamp.

### `FFmpegEncoder`

Class · [`core/src/FFmpegEncoder.cpp`](../../core/src/FFmpegEncoder.cpp) · the recorder's writer thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

The `RecordingEncoder` the app uses: writes one recording file with
FFmpeg's encoder and muxer for its format (`Spec`), resampling where a
lossy format can't take the rate, and reports a full disk apart from
other errors. Made by `openFFmpegEncoder`.

### `FileAnalysis`

Struct · [`core/src/FileAnalyser.h`](../../core/src/FileAnalyser.h) · any thread · [D2: Analysing files](../developer-guide/04-core.md#analysing-files-fileanalyser)

What one pass over a file measures: R128 loudness and its histogram,
sample and true peaks, silences at the ends and the longest inside, the
level at each end, a lossy encoder's cutoff, and a 1000-point envelope.
Filled by `analyseFile`; owned by `FileAnalysisHandle` across the C API.

### `FileAnalysisHandle`

Struct · [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · [D2: The C API's conventions](../developer-guide/04-core.md#the-c-apis-conventions)

An `anomp_file_analysis` that owns the `FileAnalysis` its arrays point
into, freed by `anomp_file_analysis_free`.

### `FileInfo`

Struct · [`core/src/TagReader.h`](../../core/src/TagReader.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

Everything a file says about itself, for Get Info: every tag field TagLib
reads, every picture, the tag types, and the format as the decoder sees
it. Filled by `readFileInfo`; copied out through `FileInfoHandle`.

### `FileInfo::Picture`

Struct · [`core/src/TagReader.h`](../../core/src/TagReader.h) · any thread

One embedded picture: type, MIME type, description and bytes.

### `FileInfoHandle`

Struct · [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · [D2: The C API's conventions](../developer-guide/04-core.md#the-c-apis-conventions)

An `anomp_file_info` that owns the strings and arrays behind its
pointers, freed by `anomp_file_info_free`.

### `FileInfoHandle::PictureText`

Helper · [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp)

The strings and bytes of one picture, owned for an `anomp_picture`.

### `FileStatus`

Struct · [`core/src/FileStatus.h`](../../core/src/FileStatus.h) · any thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

What the file system says about a file without reading it: whether it is
a cloud placeholder (dataless), checked with `stat()`, which doesn't
download it. One implementation per platform; behind
`anomp_file_is_dataless`.

### `FileStatus::Dataless`

Enum · [`core/src/FileStatus.h`](../../core/src/FileStatus.h) · any thread

No, yes (a placeholder: reading it downloads it) or unknown (it couldn't
be checked).

### `FolderAccess`

Class · [`core/src/FolderAccess.h`](../../core/src/FolderAccess.h) · any thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Durable access to a folder the user picked: creates security-scoped
bookmarks (read-only or writable) and, while an instance lives, keeps its
resolved folder readable, saying where it is now and whether the bookmark
is stale. `FolderAccess_apple.mm` and `FolderAccess_unsandboxed.cpp`
implement it. Owned by `anomp_folder_access`.

### `FormatRegistry`

Class · [`core/src/FormatRegistry.h`](../../core/src/FormatRegistry.h) · any thread · [D2: Decoding](../developer-guide/04-core.md#decoding-ffmpegaudioformat)

Owns the `juce::AudioFormatManager` with every format the player supports
(`FFmpegAudioFormat`), and answers which extensions it decodes. The C API
keeps one as a function-local static.

### `Forwarder`

Class · [`core/src/Log.cpp`](../../core/src/Log.cpp) · any thread · [D2: Logging](../developer-guide/04-core.md#logging-log)

JUCE's `Logger`, forwarded to the core's log: failed assertions as errors,
everything else as info. A static that lets go of JUCE as it is
destroyed.

### `JuceRuntime`

Struct · [`core/src/AudioEngine.h`](../../core/src/AudioEngine.h) · main thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

JUCE's runtime (`ScopedJuceInitialiser_GUI`) as `AudioEngine`'s first base,
so it shuts down after everything else in the engine, its other bases
included.

### `KWeighting`

Class · [`core/src/FileAnalyser.h`](../../core/src/FileAnalyser.h) · any thread · [D2: Analysing files](../developer-guide/04-core.md#analysing-files-fileanalyser)

BS.1770's K-weighting for one channel (a high shelf, then a high pass),
which the loudness measurement filters each sample through.

### `KWeighting::Biquad`

Helper · [`core/src/FileAnalyser.h`](../../core/src/FileAnalyser.h)

One stage of the K-weighting filter and its state.

### `Level`

Enum · [`core/src/Log.h`](../../core/src/Log.h) · any thread · [D2: Logging](../developer-guide/04-core.md#logging-log)

`anomp::log`'s levels (error, warn, info, debug), numbered as
`ANOMP_LOG_*`.

### `MediaControls`

Class · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The OS's media controls, platform-neutral: keeps and checks the published
`State`, tells its `Backend` what changed, and gates the commands that
come back (next and previous only while enabled, seeks clamped). Owned by
`anomp_media_controls`.

### `MediaControls::Backend`

Struct · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

The platform part of `MediaControls`: attach, detach, publish what
changed, and say whether it can show an artwork. The base does nothing
(the fallback, and `MediaControls_none.cpp`'s); macOS's is
`AppleBackend`.

### `MediaControls::Change`

Enum · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread

Flags for what an update changed (track, playback, artwork, navigation),
so a backend can skip the rest.

### `MediaControls::Command`

Enum · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread

The commands the OS sends: play, pause, toggle, next, previous and seek.
`ANOMP_MEDIA_*` in the C API.

### `MediaControls::Playback`

Enum · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread

Stopped (no track), paused or playing.

### `MediaControls::State`

Struct · [`core/src/MediaControls.h`](../../core/src/MediaControls.h) · main thread

Everything published: whether there is a track, its title, artist and
album, playback, elapsed and duration, the artwork's bytes, and whether
next and previous are enabled.

### `OutputRoute`

Struct · [`core/src/OutputRoute.h`](../../core/src/OutputRoute.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Whether an output device plays through headphones, as far as the OS says,
so crossfeed can turn itself on. `OutputRoute_apple.mm` and
`OutputRoute_none.cpp`; asked by `AudioEngine::outputIsHeadphones`.

### `OutputRoute::Headphones`

Enum · [`core/src/OutputRoute.h`](../../core/src/OutputRoute.h)

No, yes or unknown.

### `PlayerEngine`

Class · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · main thread for commands, audio thread for rendering · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

Plays the current track and a pre-opened next one, handing off gaplessly
or crossfading; seeks, loops, practice tempo and pitch, volume, and the
signal path's order (stretch, resampler, effects, analysis tap, equaliser,
crossfeed, recording, volume). A plain `juce::AudioSource` with no device,
so tests render it offline; owned by `AudioEngine`. Opens files on the
main thread or, asynchronously, on threads of its own, and never opens or
frees a track on the audio thread. Design:
`docs/design/phase-1-playback-engine.md`.

### `PlayerEngine::LoadResult`

Enum · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

How an asynchronous load ended: loaded, failed, or cancelled (by
`cancelLoad` or a later request). `ANOMP_LOAD_*` in the C API.

### `PlayerEngine::Openers`

Struct · [`core/src/PlayerEngine.cpp`](../../core/src/PlayerEngine.cpp) · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

What the opening threads share with the engine, which they may outlive (a
read stuck on a disk that went away): a count of those running and how to
tell the engine one is ready. The engine waits up to two seconds for them
as it goes.

### `PlayerEngine::Opening`

Struct · [`core/src/PlayerEngine.cpp`](../../core/src/PlayerEngine.cpp) · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

One asynchronous request's work, shared between the engine and the thread
opening it: the file and options in, the opened `Track` or an error out,
and `cancelled` and `done` flags.

### `PlayerEngine::Request`

Struct · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · main thread · [D2: Loading off the main thread](../developer-guide/04-core.md#loading-off-the-main-thread)

An asynchronous request not yet reported: its id, whether it is for the
next track, its `Opening`, and whether it was cancelled. Kept in request
order, so each is reported once and in order.

### `PlayerEngine::Resampler`

Class · [`core/src/PlayerEngine.cpp`](../../core/src/PlayerEngine.cpp) · audio thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

Streaming stereo resampler from the file rate to the device rate (JUCE's
windowed-sinc interpolator). Keeps input it pulled but didn't use, so a
stream spanning two tracks resamples as one and the join stays
sample-exact.

### `PlayerEngine::SignalInfo`

Struct · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · any thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

What the current track is and how it is played, for the signal path
panel: codec, format, gain, tempo, resampling, crossfeed, equaliser,
crossfade, effects, freeze and recording. Copied out as
`anomp_signal_path`.

### `PlayerEngine::State`

Enum · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · any thread

Empty, stopped, playing or paused. `ANOMP_STATE_*` in the C API.

### `PlayerEngine::Stretcher`

Class · [`core/src/PlayerEngine.cpp`](../../core/src/PlayerEngine.cpp) · audio thread · [D2: The signal path](../developer-guide/04-core.md#the-signal-path)

Changes tempo and pitch independently with Signalsmith Stretch, at the
file rate, between the tracks and the resampler; inactive (and the audio
untouched) unless practising. Configured on the main thread.

### `PlayerEngine::Track`

Struct · [`core/src/PlayerEngine.cpp`](../../core/src/PlayerEngine.cpp) · main thread and audio thread · [D2: The engine](../developer-guide/04-core.md#the-engine-audioengine-and-playerengine)

A decoded file, or part of one, and its read position: the reader, its
buffered read-ahead source, the options, the part's first sample and
length, what is left to skip, and its gain. Opened and freed on the main
thread (or an opening thread); read on the audio thread.

### `PlayerEngine::TrackOptions`

Struct · [`core/src/PlayerEngine.h`](../../core/src/PlayerEngine.h) · main thread · [D2: Gapless hand-off and crossfade](../developer-guide/04-core.md#gapless-hand-off-and-crossfade)

How to play a file: the track's own gain, the part of the file it is
(start and end), a stretch within it to skip once, and for a next track
the crossfade into it. `anomp_track_options` in the C API.

### `Preset`

Helper · [`core/src/Crossfeed.cpp`](../../core/src/Crossfeed.cpp)

A crossfeed level's cutoff and feed in dB.

### `Recorder`

Class · [`core/src/Recorder.h`](../../core/src/Recorder.h) · main thread, audio thread and its writer thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

Records what the player renders: the audio thread only copies into a
lock-free FIFO, and a `Recorder::Writer` thread drains it into a
`RecordingEncoder`. A full FIFO drops and counts an overrun; a write error
stops the writer and is reported once; a sample-rate change starts a new,
numbered file. Keeps track marks for the cue sheet. Owned by
`PlayerEngine`.

### `Recorder::Failure`

Enum · [`core/src/Recorder.h`](../../core/src/Recorder.h) · main thread

None, write failed or disk full. `ANOMP_RECORDING_*` in the C API.

### `Recorder::Mark`

Struct · [`core/src/Recorder.h`](../../core/src/Recorder.h) · main thread

Where a track began: which file and how many seconds into it. Copied out
as `anomp_recording_mark`.

### `Recorder::Split`

Helper · [`core/src/Recorder.h`](../../core/src/Recorder.h)

A sample-rate change: the first frame at the new rate, and the rate,
where the next file begins.

### `Recorder::Status`

Struct · [`core/src/Recorder.h`](../../core/src/Recorder.h) · main thread

Whether recording, frames and seconds written, overruns, files begun and
the latest file. Copied out as `anomp_recording`.

### `Recorder::Writer`

Class · [`core/src/Recorder.cpp`](../../core/src/Recorder.cpp) · its own thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

The recorder's writer thread: drains the FIFO into the encoder, opens the
next file at each `Split`, and on an error finalises what it wrote and
records the failure. Everything that can wait or fail happens here.

### `RecordingEncoder`

Class · [`core/src/Recorder.h`](../../core/src/Recorder.h) · the recorder's writer thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

The interface that writes one file of a recording: stereo float frames in
(`write`), then `finish` once. `FFmpegEncoder` in the app; tests pass
fakes through `Recorder::EncoderFactory`.

### `RecordingError`

Struct · [`core/src/Recorder.h`](../../core/src/Recorder.h) · the recorder's writer thread

An error writing a recording: its message and whether the disk was full.

### `RecordingFormat`

Struct · [`core/src/Recorder.h`](../../core/src/Recorder.h) · any thread · [D2: Recording](../developer-guide/04-core.md#recording-recorder-and-ffmpegencoder)

What a recording is written as: the kind, the bits for PCM and lossless
kinds, the bitrate for AAC and MP3; gives the extension and says why a
combination can't be written. `anomp_record_format` in the C API.

### `RecordingFormat::Kind`

Enum · [`core/src/Recorder.h`](../../core/src/Recorder.h) · [D2: A recording format](../developer-guide/11-recipes.md#a-recording-format)

WAV, AIFF, FLAC, ALAC, AAC or MP3. `ANOMP_RECORD_*` in the C API and
Rust's `RecordingKind`.

### `SignalTap`

Class · [`core/src/SignalTap.h`](../../core/src/SignalTap.h) · audio thread writes, any thread reads · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

The latest stereo samples the player rendered, in a lock-free ring of
relaxed atomics: the audio thread pushes, and readers copy the most
recent window without taking it. Fed by `PlayerEngine` after the effects;
read by `AnalysisThread`.

### `Spec`

Helper · [`core/src/FFmpegEncoder.cpp`](../../core/src/FFmpegEncoder.cpp)

The muxer, encoder, sample format and raw bits each `RecordingFormat` is
written with.

### `SpectrumAnalyser`

Class · [`core/src/SpectrumAnalyser.h`](../../core/src/SpectrumAnalyser.h) · the analysis thread · [D2: Analysis for the visualizer](../developer-guide/04-core.md#analysis-for-the-visualizer)

Turns windows of stereo samples into `AnalysisFrame`s: FFTs for the bands,
chroma and notes, the balance per band, levels, a still waveform, and a
beat detector over the bass's last 1.5 s. Not thread-safe; owned by
`AnalysisThread`.

### `SpectrumAnalyser::Band`

Helper · [`core/src/SpectrumAnalyser.h`](../../core/src/SpectrumAnalyser.h)

A band's or note's FFT bins and its tilt in dB.

### `SpectrumAnalyser::Ring`

Helper · [`core/src/SpectrumAnalyser.h`](../../core/src/SpectrumAnalyser.h)

A fixed-size ring of the latest values, oldest overwritten first: the
beat detector's flux history.

### `TagParts`

Struct · [`core/src/TagReader.h`](../../core/src/TagReader.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

Flags for what `readTags` reads besides the tags: the picture, the lyrics
and the chapters. `ANOMP_TAGS_*` in the C API.

### `TagsHandle`

Struct · [`core/src/anomp_c_api.cpp`](../../core/src/anomp_c_api.cpp) · [D2: The C API's conventions](../developer-guide/04-core.md#the-c-apis-conventions)

An `anomp_tags` that owns the strings, picture and chapters behind its
pointers, freed by `anomp_tags_free`.

### `TrackTags`

Struct · [`core/src/TagReader.h`](../../core/src/TagReader.h) · any thread · [D2: Tags](../developer-guide/04-core.md#tags-tagreader)

A file's tags and audio properties: titles, numbers, ReplayGain,
MusicBrainz ids, the picture, works and movements, dates, rating,
compilation, credited artists, and on request lyrics, an embedded cue
sheet and chapters. Filled by `readTags` with TagLib (falling back to the
decoder for the duration); copied out as `anomp_tags`.

### `TrackTags::Chapter`

Struct · [`core/src/TagReader.h`](../../core/src/TagReader.h) · any thread

A chapter's start, end (-1 for the end of the file) and title.
`anomp_chapter` in the C API.

### `TruePeakMeter`

Class · [`core/src/FileAnalyser.cpp`](../../core/src/FileAnalyser.cpp) · any thread · [D2: Analysing files](../developer-guide/04-core.md#analysing-files-fileanalyser)

4× oversampling for the true peak (BS.1770 Annex 2): a windowed-sinc
lowpass, 12 taps a phase, finding the largest value between samples.

### `VolumeWatcher`

Class · [`core/src/VolumeWatcher.h`](../../core/src/VolumeWatcher.h) · main thread · [D2: Platform code](../developer-guide/04-core.md#platform-code)

Tells the host when a volume is mounted or unmounted, so folders on it are
checked again. Owns a `VolumeWatcher::Platform`; owned by
`anomp_volume_watcher`.

### `VolumeWatcher::Platform`

Struct · [`core/src/VolumeWatcher_apple.mm`](../../core/src/VolumeWatcher_apple.mm), [`core/src/VolumeWatcher_none.cpp`](../../core/src/VolumeWatcher_none.cpp) · main thread

The platform part of `VolumeWatcher`: on macOS the `NSWorkspace` mount
and unmount observers; elsewhere empty.
