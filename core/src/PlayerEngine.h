#pragma once

#include "Crossfeed.h"
#include "Equaliser.h"
#include "SignalTap.h"

#include <juce_audio_formats/juce_audio_formats.h>

#include <array>
#include <atomic>
#include <functional>
#include <memory>
#include <tuple>
#include <vector>

namespace anomp
{
/** Plays one track at a time, plus a pre-opened next track that takes over
    gaplessly at the end of the current one.

    The host owns the queue: after each advance (onTrackEnded (true)) it sets
    the new next track. PlayerEngine is a plain AudioSource with no device, so
    it can be rendered offline by calling getNextAudioBlock directly.

    A track can be part of a file (a cue sheet's track, a chapter, a trimmed
    track), between two times in it; positions and lengths are then within
    that part. Two parts of one file are separate tracks with readers of
    their own, so a hand-off between them is as gapless as between files.

    Signal path: per track, AudioFormatReaderSource -> BufferingAudioSource
    (read-ahead on a shared background thread); the current and next track are
    joined at the file sample rate, then (while practising) time-stretched,
    then one windowed-sinc resampler converts to the device rate, so the join
    is sample-exact whatever the device rate. The analysis tap sees that;
    the equaliser, crossfeed and the volume come after it.
    Tracks with different sample rates switch at the next chunk boundary,
    which leaves a few milliseconds of silence between them.

    Crossfade (PLAN.md F14): a next track set with `crossfade` seconds is
    read alongside the current one over the current track's last seconds,
    the two mixed with equal-power curves, and takes over (onTrackEnded)
    as the current track ends, already that far in. Only between tracks at
    one sample rate, and never while an A–B loop is on; otherwise the
    hand-off is as above. The host decides which hand-offs crossfade (none
    within an album).

    Threading: commands and dispatchEvents() run on the message thread; the
    status getters are safe on any thread. load() and setNext() open the
    file on the message thread; loadAsync() and setNextAsync() open it, and
    fill its read-ahead, on a thread of their own (PLAN.md H11), and
    dispatchEvents() hands the ready track over on the message thread.
    Tracks are never opened or freed on the audio thread.
*/
class PlayerEngine final : public juce::AudioSource
{
public:
    enum class State
    {
        empty,   ///< No track loaded.
        stopped, ///< Track loaded, at the start (after stop() or the end of the last track).
        playing,
        paused
    };

    /** How to play a file. */
    struct TrackOptions
    {
        /** The track's own linear gain (e.g. ReplayGain), clamped to
            0..maxTrackGain, applied as the track is read: before the
            resampler, the tap and the volume. */
        float gain = 1.0f;
        /** Seconds into the file where the track starts and ends; an end
            not after the start is the end of the file. */
        double start = 0.0, end = 0.0;
        /** Seconds within the track: reaching `skipFrom` jumps to `skipTo`,
            once. A negative `skipFrom`, or `skipTo` not after it, skips
            nothing. */
        double skipFrom = -1.0, skipTo = -1.0;
        /** For a next track only: seconds to crossfade into it over, up to
            maxCrossfade; 0 hands off gaplessly. */
        double crossfade = 0.0;
    };

    /** With a null `readAheadThread`, tracks decode on the audio thread; use
        that only for offline rendering and tests. */
    PlayerEngine (juce::AudioFormatManager& formats, juce::TimeSliceThread* readAheadThread);
    ~PlayerEngine() override;

    //==============================================================================
    /** Largest track gain (about +18 dB). */
    static constexpr float maxTrackGain = 8.0f;

    /** Shortest A–B loop. */
    static constexpr double minLoopSeconds = 0.25;

    /** Longest crossfade, in seconds. */
    static constexpr double maxCrossfade = 12.0;

    /** Tempo and transposition ranges for practising. */
    static constexpr double minTempo = 0.5, maxTempo = 1.5, maxSemitones = 12.0;

    /** Replaces the current track (and clears the next one), leaving the
        player stopped at the start. An end of the replaced track that
        dispatchEvents() has not yet reported is dropped.
        Returns an empty string on success, otherwise the error; on failure
        nothing changes. */
    juce::String load (const juce::File& file, const TrackOptions& options);
    juce::String load (const juce::File& file, float gain = 1.0f) { return load (file, TrackOptions { gain }); }

    /** Opens `file` as the track that follows the current one gaplessly,
        with its own options, whose gain takes over with it.
        Returns an empty string on success, otherwise the error. */
    juce::String setNext (const juce::File& file, const TrackOptions& options);
    juce::String setNext (const juce::File& file, float gain = 1.0f) { return setNext (file, TrackOptions { gain }); }

    /** Changes the gain of the current and next tracks opened from `file`,
        ramping from the old gain over the next read. Returns how many
        tracks changed. */
    int setTrackGain (const juce::File& file, float gain);

    /** setTrackGain for tracks of `file` whose options started at `start`. */
    int setTrackGainAt (const juce::File& file, double start, float gain);

    void clearNext();

    //==============================================================================
    /** Identifies an asynchronous load; 0 is none. */
    using LoadId = juce::int64;

    enum class LoadResult
    {
        loaded,   ///< The track took its place.
        failed,   ///< It couldn't be opened; nothing changed. See the error.
        cancelled ///< cancelLoad(), or superseded by a later request.
    };

    /** load() on a thread of its own: opens `file` and fills its read-ahead
        there, then, at a dispatchEvents() after it is ready, replaces the
        current track as load() does. Until then nothing changes, and the
        current track (if any) plays on. Cancels every asynchronous request
        made before it. Every request is reported once, by onLoadFinished. */
    LoadId loadAsync (const juce::File& file, const TrackOptions& options);

    /** setNext() on a thread of its own, reported as loadAsync() is. Clears
        the next track at once, so the current one can't hand off to a track
        that is about to be replaced; cancels any earlier asynchronous next.
        A next requested after a loadAsync() still opening waits for it, then
        follows the track it loaded (or, if it failed, the one before). */
    LoadId setNextAsync (const juce::File& file, const TrackOptions& options);

    /** Cancels a request still opening; the next dispatchEvents() reports it
        cancelled. Does nothing for one already reported. A file still being
        opened is opened to the end on its thread, then let go. */
    void cancelLoad (LoadId id);

    /** Whether a request is still waiting to be reported. */
    bool isLoading (LoadId id) const;

    /** For tests: waits until no request is still opening (its thread has
        finished). Returns false on timeout. */
    bool waitForLoads (int timeoutMs) const;

    /** Called on the thread that opened a file, when it is ready to be
        handed over (or failed): the host should call dispatchEvents() soon.
        Set it before the first request. */
    std::function<void()> onLoadReady;

    /** Starts or resumes playback. Returns false if no track is loaded. */
    bool play();

    /** Pauses, fading out over one block. Does nothing unless playing. */
    void pause();

    /** Stops and rewinds to the start of the current track. */
    void stop();

    /** Moves to `seconds` into the current track, clamped to its length.
        Returns false if no track is loaded. */
    bool seek (double seconds);

    /** Linear gain, clamped to 0..1; changes are ramped over one block. */
    void setVolume (float gain);

    /** Loops the current track between `start` and `end` seconds (within
        the track), jumping back at `end` on the exact sample: a second
        reader, opened here, waits at `start`, and the two swap at each
        jump. Returns an empty string on success, otherwise the error. The
        loop ends when another track becomes current. */
    juce::String setLoop (double start, double end);
    void clearLoop();
    /** The loop's start and end in seconds; false if none is set. */
    bool getLoop (double& start, double& end) const;

    /** Plays `rate` times as fast without changing the pitch, transposed
        by `semitones`; 1 and 0 bypass the stretcher. Returns false, changing
        nothing, for values out of range. */
    bool setTempo (double rate, double semitones);

    /** Crossfeed for headphones, 0 (off) to Crossfeed::maxLevel. */
    void setCrossfeed (int level);
    int getCrossfeed() const noexcept { return crossfeedLevel.load(); }

    /** The graphic equaliser (see Equaliser): on with `gainsDb` and
        `preampDb`, or gliding to flat and off. */
    void setEqualiser (bool enabled, const Equaliser::Gains& gainsDb, double preampDb);

    //==============================================================================
    State getState() const noexcept { return state.load(); }
    double getPositionSeconds() const noexcept { return positionSeconds.load(); }
    double getDurationSeconds() const noexcept { return durationSeconds.load(); }
    float getVolume() const noexcept { return volume.load(); }
    bool hasNext() const;

    /** What the current track is and how it is played, for the signal path. */
    struct SignalInfo
    {
        bool loaded = false;
        juce::String codec;
        bool lossless = false;
        int bitsPerSample = 0, bitrateKbps = 0, channels = 0;
        double fileSampleRate = 0.0;
        float gain = 1.0f;
        double tempo = 1.0, semitones = 0.0;
        double deviceSampleRate = 0.0;
        int crossfeed = 0;
        bool equaliser = false;
        /** Seconds the next track crossfades over; 0 if it won't. */
        double crossfade = 0.0;
    };
    SignalInfo getSignalInfo() const;

    /** What the player renders while playing, before the volume is applied
        (so the visualizer doesn't shrink with it), at the device rate.
        Nothing is pushed while paused or stopped. */
    const SignalTap& getTap() const noexcept { return tap; }

    /** How many times a next track has taken over since the engine was
        created. onTrackEnded reports a hand-off at the next dispatchEvents();
        this counts it as it happens, so a host about to change the next
        track can tell whether the one it set is already playing. */
    juce::int64 getAdvanceCount() const noexcept { return advanceCount.load(); }

    //==============================================================================
    /** Reports what changed since the previous call through the callbacks
        below, and frees tracks the audio thread has finished with. The host
        calls this periodically on the message thread; nothing is reported
        from inside a command. The callbacks may call commands (e.g. setNext
        from onTrackEnded): what is reported is taken before the first one
        runs, and later callbacks in the same call see the state after it. */
    void dispatchEvents();

    /** A track played to its end. `advanced` is true if the next track took
        over, false if playback stopped (a change to State::stopped follows). */
    std::function<void (bool advanced)> onTrackEnded;
    std::function<void (State)> onStateChanged;
    std::function<void (double positionSeconds, double durationSeconds)> onPositionChanged;
    /** An asynchronous request is done; `error` is empty unless it failed.
        Reported after the hand-offs and ends, before the state and position. */
    std::function<void (LoadId, LoadResult, const juce::String& error)> onLoadFinished;

    /** For offline rendering with a read-ahead thread: waits until the
        current track's next `numSamples` source samples are buffered (and
        the loop's waiting reader's). Call only while nothing else is calling
        getNextAudioBlock. */
    bool waitForReadAhead (int numSamples, int timeoutMs);

    //==============================================================================
    void prepareToPlay (int samplesPerBlockExpected, double sampleRate) override;
    void releaseResources() override;
    void getNextAudioBlock (const juce::AudioSourceChannelInfo& info) override;

private:
    struct Track;
    class Resampler;
    class Stretcher;
    using TrackPtr = std::unique_ptr<Track>;

    struct Opening;
    struct Openers;

    /** Opens a track: on the message thread for load() and setNext(), on an
        opening thread for the asynchronous requests. Gives up after the
        reader is open if `cancelled` is set. */
    static TrackPtr openTrack (juce::AudioFormatManager& formats,
                               juce::TimeSliceThread* readAheadThread,
                               const juce::File& file,
                               const TrackOptions& options,
                               juce::String& error,
                               const std::atomic<bool>* cancelled);

    /** Makes `track` the current one, as load() does. */
    void install (TrackPtr track);
    /** Makes `track` the next one, as setNext() does; an error if none is loaded. */
    juce::String installNext (TrackPtr track);
    LoadId request (const juce::File& file, const TrackOptions& options, bool asNext);
    /** Marks the asynchronous loads (current, next, or both) not yet
        reported as cancelled. */
    void cancelRequests (bool loads, bool nexts);
    /** Hands over the requests that are ready, in order; returns what to report. */
    std::vector<std::tuple<LoadId, LoadResult, juce::String>> takeFinishedLoads();

    // All below run with `lock` held.
    void renderChunk (float* const* output, int numSamples);
    void readStretched (juce::AudioBuffer<float>& buffer, int startSample, int numSamples);
    int readSource (juce::AudioBuffer<float>& buffer, int startSample, int numSamples);
    void handOff();
    /** Samples the next track fades in over, or 0 for a gapless hand-off. */
    juce::int64 crossfadeSamples() const;
    /** Reads `count` samples of the fade from the current and next tracks. */
    void readCrossfade (juce::AudioBuffer<float>& buffer, int startSample, int count);
    void jumpToLoopStart();
    void configureRate();
    void rewind();
    void publishPosition();
    /** Moves a track the audio thread is done with where dispatchEvents()
        frees it. */
    void retire (TrackPtr track);
    /** Takes every retired track (and the loop's), to free after the lock. */
    std::array<TrackPtr, 4> takeRetired();

    juce::AudioFormatManager& formats;
    juce::TimeSliceThread* readAheadThread;

    juce::CriticalSection lock;
    TrackPtr current, next;
    // Tracks the audio thread let go of, freed in dispatchEvents().
    std::array<TrackPtr, 3> retired;

    // The A–B loop, in samples within the current track: `loopTrack` is a
    // second reader of the current track waiting at the loop's start.
    TrackPtr loopTrack;
    juce::int64 loopStart = -1, loopEnd = -1;
    bool loopRewindPending = false; // loopTrack is the old current, still at the loop's end.

    std::unique_ptr<Resampler> resampler;
    std::unique_ptr<Stretcher> stretcher; // Created on the first setTempo().
    juce::AudioBuffer<float> scratch;
    SignalTap tap;
    Crossfeed crossfeed;
    std::atomic<int> crossfeedLevel { 0 };
    Equaliser equaliser;
    // The next track's samples during a crossfade, and the fade's length
    // once it has begun (0 while not fading).
    juce::AudioBuffer<float> fadeScratch;
    juce::int64 fadeLength = 0;
    double deviceRate = 0.0;
    float appliedGain = 0.0f; // Gain at the end of the previous block.
    int pendingAdvances = 0;
    bool pendingEnded = false;

    std::atomic<State> state { State::empty };
    std::atomic<double> positionSeconds { 0.0 }, durationSeconds { 0.0 };
    std::atomic<float> volume { 1.0f };
    std::atomic<juce::int64> advanceCount { 0 };

    // Asynchronous requests not yet reported, in the order they were made
    // (message thread only), and what their threads share with the engine.
    struct Request
    {
        LoadId id;
        bool asNext;
        std::shared_ptr<Opening> opening;
        bool cancelled = false;
    };
    std::vector<Request> requests;
    std::shared_ptr<Openers> openers;
    LoadId lastLoadId = 0;

    // Message thread only: what dispatchEvents() last reported.
    State reportedState = State::empty;
    double reportedPosition = -1.0, reportedDuration = -1.0;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (PlayerEngine)
};
} // namespace anomp
