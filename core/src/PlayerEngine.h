#pragma once

#include "SignalTap.h"

#include <juce_audio_formats/juce_audio_formats.h>

#include <atomic>
#include <functional>
#include <memory>

namespace anomp
{
/** Plays one track at a time, plus a pre-opened next track that takes over
    gaplessly at the end of the current one.

    The host owns the queue: after each advance (onTrackEnded (true)) it sets
    the new next track. PlayerEngine is a plain AudioSource with no device, so
    it can be rendered offline by calling getNextAudioBlock directly.

    Signal path: per track, AudioFormatReaderSource -> BufferingAudioSource
    (read-ahead on a shared background thread); the current and next track are
    joined at the file sample rate, then one windowed-sinc resampler converts
    to the device rate, so the join is sample-exact whatever the device rate.
    Tracks with different sample rates switch at the next chunk boundary,
    which leaves a few milliseconds of silence between them.

    Threading: commands and dispatchEvents() run on the message thread; the
    status getters are safe on any thread. Tracks are opened and freed on the
    message thread, never on the audio thread.
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

    /** With a null `readAheadThread`, tracks decode on the audio thread; use
        that only for offline rendering and tests. */
    PlayerEngine (juce::AudioFormatManager& formats, juce::TimeSliceThread* readAheadThread);
    ~PlayerEngine() override;

    //==============================================================================
    /** Replaces the current track (and clears the next one), leaving the
        player stopped at the start. An end of the replaced track that
        dispatchEvents() has not yet reported is dropped. Returns an empty
        string on success, otherwise the error; on failure nothing changes. */
    juce::String load (const juce::File& file);

    /** Opens `file` as the track that follows the current one gaplessly.
        Returns an empty string on success, otherwise the error. */
    juce::String setNext (const juce::File& file);

    void clearNext();

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

    //==============================================================================
    State getState() const noexcept { return state.load(); }
    double getPositionSeconds() const noexcept { return positionSeconds.load(); }
    double getDurationSeconds() const noexcept { return durationSeconds.load(); }
    float getVolume() const noexcept { return volume.load(); }
    bool hasNext() const;

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

    /** For offline rendering with a read-ahead thread: waits until the
        current track's next `numSamples` source samples are buffered. Call
        only while nothing else is calling getNextAudioBlock. */
    bool waitForReadAhead (int numSamples, int timeoutMs);

    //==============================================================================
    void prepareToPlay (int samplesPerBlockExpected, double sampleRate) override;
    void releaseResources() override;
    void getNextAudioBlock (const juce::AudioSourceChannelInfo& info) override;

private:
    struct Track;
    class Resampler;

    std::unique_ptr<Track> openTrack (const juce::File& file, juce::String& error) const;

    // All below run with `lock` held.
    void renderChunk (float* const* output, int numSamples);
    int readSource (juce::AudioBuffer<float>& buffer, int startSample, int numSamples);
    void handOff();
    void configureRate();
    void rewind();
    void publishPosition();

    juce::AudioFormatManager& formats;
    juce::TimeSliceThread* readAheadThread;

    juce::CriticalSection lock;
    std::unique_ptr<Track> current, next;
    std::unique_ptr<Track> retired; // Handed off on the audio thread, freed in dispatchEvents().
    std::unique_ptr<Resampler> resampler;
    juce::AudioBuffer<float> scratch;
    SignalTap tap;
    double deviceRate = 0.0;
    float appliedGain = 0.0f; // Gain at the end of the previous block.
    int pendingAdvances = 0;
    bool pendingEnded = false;

    std::atomic<State> state { State::empty };
    std::atomic<double> positionSeconds { 0.0 }, durationSeconds { 0.0 };
    std::atomic<float> volume { 1.0f };
    std::atomic<juce::int64> advanceCount { 0 };

    // Message thread only: what dispatchEvents() last reported.
    State reportedState = State::empty;
    double reportedPosition = -1.0, reportedDuration = -1.0;

    JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR (PlayerEngine)
};
} // namespace anomp
