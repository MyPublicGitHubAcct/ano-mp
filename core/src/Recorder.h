#pragma once

#include <juce_audio_basics/juce_audio_basics.h>

#include <array>
#include <atomic>
#include <functional>
#include <memory>
#include <vector>

namespace anomp
{
/** What a recording is written as (PLAN.md X6). */
struct RecordingFormat
{
    enum class Kind
    {
        wav,  ///< PCM: 16-bit, 24-bit or 32-bit float; RF64 past 4 GB.
        aiff, ///< PCM: 16-bit or 24-bit.
        flac, ///< 16-bit or 24-bit.
        alac, ///< Apple Lossless in .m4a: 16-bit or 24-bit.
        aac,  ///< AAC-LC in .m4a, at bitrateKbps.
        mp3   ///< LAME, constant bitrateKbps.
    };

    Kind kind = Kind::wav;
    /** For the PCM and lossless kinds: 16, 24, or (WAV only) 32 for float. */
    int bits = 32;
    /** For AAC and MP3: 96 to 320. */
    int bitrateKbps = 256;

    static constexpr int minBitrateKbps = 96, maxBitrateKbps = 320;

    bool isLossy() const noexcept { return kind == Kind::aac || kind == Kind::mp3; }

    /** The file name extension, without the dot. */
    juce::String getExtension() const;

    /** Why this can't be written, or an empty string. */
    juce::String validate() const;
};

/** An error writing a recording: its message, and whether it was the disk
    filling up. */
struct RecordingError
{
    juce::String message;
    bool diskFull = false;

    bool failed() const noexcept { return message.isNotEmpty(); }
};

/** Writes one file of a recording: stereo float samples in, the format's
    file out. Used by the recorder's writer thread only. */
class RecordingEncoder
{
public:
    virtual ~RecordingEncoder() = default;

    /** Encodes `numSamples` stereo frames. */
    virtual RecordingError write (const float* left, const float* right, int numSamples) = 0;

    /** Flushes the encoder and finalises the file; called once, last. */
    virtual RecordingError finish() = 0;
};

/** Records what the player renders to files (PLAN.md X6).

    The audio thread only copies samples into a lock-free FIFO (push());
    a writer thread of the recorder's own drains it into the encoder. A
    FIFO that is full when the audio thread pushes (the writer fell behind)
    drops what doesn't fit and counts an overrun; the audio thread never
    waits. A write error stops the writer, which finalises what it wrote
    and reports the failure (takeFailure()), after which pushes are ignored.

    A change of sample rate (setSampleRate()) starts a new file at the frame
    the change happened, named after the first with a number: "name.wav",
    "name 2.wav", "name 3.wav".

    Threading: start(), stop(), takeFailure() and the getters on the
    message thread; push() and markTrack() on the audio thread, or with
    `audioLock` held; setSampleRate() with `audioLock` held (as
    AudioSource::prepareToPlay is). start() and stop() take `audioLock`
    to switch pushing on and off, so they never race the audio thread.
*/
class Recorder final
{
public:
    using EncoderFactory = std::function<std::unique_ptr<RecordingEncoder> (const juce::File& file,
                                                                            const RecordingFormat& format,
                                                                            double sampleRate,
                                                                            RecordingError& error)>;

    /** Frames of each channel the FIFO holds: about 2.7 s at 192 kHz. */
    static constexpr int defaultFifoFrames = 1 << 19;

    /** At most this many track marks are kept; later ones are dropped. */
    static constexpr int maxMarks = 4096;

    /** `factory` opens each file's encoder (openFFmpegEncoder in the app). */
    Recorder (juce::CriticalSection& audioLock, EncoderFactory factory, int fifoFrames = defaultFifoFrames);
    ~Recorder();

    /** Starts writing to `file` (its folder must exist) at `sampleRate`,
        opening it at once, so a bad folder or format fails here, with a
        track mark at its start if `markFirst`. Returns an empty string on
        success, otherwise the error. Fails while recording. */
    juce::String start (const juce::File& file, const RecordingFormat& format, double sampleRate, bool markFirst);

    /** Writes what the FIFO holds, finalises the file and stops. Does
        nothing unless recording (or failed and not yet stopped). */
    void stop();

    /** Whether start() succeeded and stop() hasn't been called since; true
        after a failure too, until stop(). */
    bool isRecording() const noexcept { return running; }

    enum class Failure
    {
        none,
        writeFailed, ///< The file couldn't be written; see the message.
        diskFull     ///< The disk filled up.
    };

    /** A failure the writer hit, reported once: the caller should stop(). */
    Failure takeFailure (juce::String& message);

    struct Status
    {
        bool recording = false;
        /** Frames in the files so far, and their length in seconds. */
        juce::int64 frames = 0;
        double seconds = 0.0;
        /** Times the FIFO was full when the audio thread pushed. */
        juce::int64 overruns = 0;
        /** Files written so far (one per sample rate), and the latest. */
        int files = 0;
        juce::File file;
    };
    Status getStatus() const;

    /** Where a track began, from markTrack(). */
    struct Mark
    {
        int file = 0;         ///< 0 for the first file.
        double seconds = 0.0; ///< Into that file.
    };
    /** Every mark since start(), in order; kept after stop() until the
        next start(). */
    std::vector<Mark> getMarks() const;

    /** The rate the next frames pushed are at: from here on they go to a
        new file. With `audioLock` held. */
    void setSampleRate (double sampleRate);

    /** Audio thread (or with `audioLock` held): copies a block of stereo
        samples, multiplied by a gain ramping from `gainFrom` to `gainTo`,
        into the FIFO. */
    void push (const float* left, const float* right, int numSamples, float gainFrom, float gainTo) noexcept;

    /** Audio thread (or with `audioLock` held): a track begins
        `framesAhead` frames after what has been pushed so far. */
    void markTrack (int framesAhead) noexcept;

    /** The name of the `index`th file (0-based) of a recording to `first`. */
    static juce::File numberedFile (const juce::File& first, int index);

private:
    class Writer;

    struct Split
    {
        juce::int64 frame; ///< The first pushed frame at the new rate.
        double rate;
    };

    juce::CriticalSection& audioLock;
    EncoderFactory factory;

    // The FIFO: one writer (the audio thread), one reader (the writer thread).
    juce::AbstractFifo fifo;
    juce::AudioBuffer<float> fifoData;

    // Switched under audioLock; read by the audio thread.
    std::atomic<bool> active { false };
    std::atomic<bool> running { false };
    std::atomic<juce::int64> pushed { 0 }, overruns { 0 };

    // The rate changes since start(), the first at frame 0 (splitsLock).
    juce::CriticalSection splitsLock;
    std::vector<Split> splits;

    // Track marks in pushed frames: written under audioLock, read by the
    // message thread up to markCount.
    std::array<juce::int64, maxMarks> marks {};
    std::atomic<int> markCount { 0 };

    // Kept by the writer thread, read by the getters.
    std::atomic<juce::int64> framesWritten { 0 };
    std::atomic<double> secondsWritten { 0.0 };
    std::atomic<int> filesWritten { 0 };
    std::atomic<Failure> failure { Failure::none };
    std::atomic<bool> failureReported { false };
    mutable juce::CriticalSection statusLock; // failureMessage, currentFile.
    juce::String failureMessage;
    juce::File firstFile, currentFile;
    RecordingFormat format;

    std::unique_ptr<Writer> writer;

    JUCE_DECLARE_NON_COPYABLE (Recorder)
};
} // namespace anomp
