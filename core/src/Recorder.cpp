#include "Recorder.h"

#include "Log.h"

#include <limits>
#include <utility>

namespace anomp
{
juce::String RecordingFormat::getExtension() const
{
    switch (kind)
    {
        case Kind::wav:  return "wav";
        case Kind::aiff: return "aiff";
        case Kind::flac: return "flac";
        case Kind::alac:
        case Kind::aac:  return "m4a";
        case Kind::mp3:  return "mp3";
    }
    return "wav";
}

juce::String RecordingFormat::validate() const
{
    if (isLossy())
    {
        if (bitrateKbps < minBitrateKbps || bitrateKbps > maxBitrateKbps)
            return "The bitrate must be " + juce::String (minBitrateKbps) + " to " + juce::String (maxBitrateKbps)
                   + " kbps";
        return {};
    }
    if (bits == 16 || bits == 24 || (bits == 32 && kind == Kind::wav))
        return {};
    return kind == Kind::wav ? "WAV records 16-bit, 24-bit or 32-bit float samples"
                             : "This format records 16-bit or 24-bit samples";
}

//==============================================================================
class Recorder::Writer final : public juce::Thread
{
public:
    Writer (Recorder& ownerIn, std::unique_ptr<RecordingEncoder> encoderIn, double rateIn)
        : juce::Thread ("anomp recorder"),
          owner (ownerIn),
          encoder (std::move (encoderIn)),
          rate (rateIn)
    {
    }

    void run() override
    {
        for (;;)
        {
            // stop() switches pushing off before it asks the thread to
            // exit, so the drain after seeing that takes the last samples.
            const auto exiting = threadShouldExit();
            if (! drain() || exiting)
                break;
            wait (20);
        }
        if (encoder != nullptr)
            check (std::exchange (encoder, nullptr)->finish());
    }

private:
    static constexpr int chunkFrames = 8192;

    /** Writes everything the FIFO holds; false after a failure. */
    bool drain()
    {
        while (owner.failure.load() == Failure::none)
        {
            const auto ready = owner.fifo.getNumReady();
            if (ready <= 0)
                return true;

            // A new file starts at the next rate change.
            juce::int64 boundary = std::numeric_limits<juce::int64>::max();
            {
                const juce::ScopedLock sl (owner.splitsLock);
                const auto nextSplit = static_cast<size_t> (fileIndex) + 1;
                if (nextSplit < owner.splits.size())
                {
                    const auto& split = owner.splits[nextSplit];
                    if (consumed >= split.frame)
                    {
                        if (! openNext (split.rate))
                            return false;
                        continue;
                    }
                    boundary = split.frame;
                }
            }

            const auto count = static_cast<int> (
                juce::jmin (static_cast<juce::int64> (juce::jmin (ready, chunkFrames)), boundary - consumed));
            int start1, size1, start2, size2;
            owner.fifo.prepareToRead (count, start1, size1, start2, size2);
            auto ok = write (start1, size1) && write (start2, size2);
            owner.fifo.finishedRead (size1 + size2);
            consumed += size1 + size2;
            if (! ok)
                return false;
        }
        return false;
    }

    bool write (int start, int size)
    {
        if (size <= 0)
            return true;
        if (! check (encoder->write (owner.fifoData.getReadPointer (0, start), owner.fifoData.getReadPointer (1, start),
                                     size)))
            return false;
        owner.framesWritten += size;
        owner.secondsWritten.store (owner.secondsWritten.load() + size / rate);
        return true;
    }

    bool openNext (double newRate)
    {
        if (! check (std::exchange (encoder, nullptr)->finish()))
            return false;

        ++fileIndex;
        rate = newRate;
        const auto file = numberedFile (owner.firstFile, fileIndex);
        RecordingError error;
        encoder = owner.factory (file, owner.format, rate, error);
        if (encoder == nullptr)
            return check (error.failed() ? error : RecordingError { "Cannot create " + file.getFileName() });

        {
            const juce::ScopedLock sl (owner.statusLock);
            owner.currentFile = file;
        }
        ++owner.filesWritten;
        log::info ("recording: file " + juce::String (fileIndex + 1) + " at " + juce::String (rate) + " Hz");
        return true;
    }

    /** Records the first failure; true if there was none. */
    bool check (const RecordingError& error)
    {
        if (! error.failed())
            return true;
        if (owner.failure.load() == Failure::none)
        {
            {
                const juce::ScopedLock sl (owner.statusLock);
                owner.failureMessage = error.message;
            }
            owner.active = false;
            owner.failure = error.diskFull ? Failure::diskFull : Failure::writeFailed;
            log::warn ("recording failed" + juce::String (error.diskFull ? " (disk full)" : ""));
        }
        return false;
    }

    Recorder& owner;
    std::unique_ptr<RecordingEncoder> encoder;
    double rate;
    int fileIndex = 0;
    juce::int64 consumed = 0; // Pushed frames taken from the FIFO.
};

//==============================================================================
Recorder::Recorder (juce::CriticalSection& audioLockIn, EncoderFactory factoryIn, int fifoFrames)
    : audioLock (audioLockIn),
      factory (std::move (factoryIn)),
      fifo (fifoFrames),
      fifoData (2, fifoFrames)
{
}

Recorder::~Recorder() { stop(); }

juce::String Recorder::start (const juce::File& file,
                              const RecordingFormat& newFormat,
                              double sampleRate,
                              bool markFirst)
{
    if (running)
        return "Already recording";
    if (const auto problem = newFormat.validate(); problem.isNotEmpty())
        return problem;
    if (! (sampleRate > 0.0))
        return "No output device is open";

    RecordingError error;
    auto encoder = factory (file, newFormat, sampleRate, error);
    if (encoder == nullptr)
        return error.failed() ? error.message : "Cannot create " + file.getFileName();

    // Nothing pushes while `active` is false, so the FIFO can be reset.
    fifo.reset();
    pushed = 0;
    overruns = 0;
    marks[0] = 0;
    markCount = markFirst ? 1 : 0;
    framesWritten = 0;
    secondsWritten = 0.0;
    filesWritten = 1;
    failure = Failure::none;
    failureReported = false;
    format = newFormat;
    {
        const juce::ScopedLock sl (splitsLock);
        splits = { { 0, sampleRate } };
    }
    {
        const juce::ScopedLock sl (statusLock);
        failureMessage = {};
        firstFile = currentFile = file;
    }

    writer = std::make_unique<Writer> (*this, std::move (encoder), sampleRate);
    if (! writer->startThread())
    {
        writer = nullptr;
        return "Cannot start the recorder's thread";
    }
    running = true;
    {
        const juce::ScopedLock sl (audioLock);
        active = true;
    }
    return {};
}

void Recorder::stop()
{
    if (! running)
        return;
    {
        const juce::ScopedLock sl (audioLock);
        active = false;
    }
    writer->signalThreadShouldExit();
    writer->notify();
    writer->waitForThreadToExit (-1);
    writer = nullptr;
    running = false;
}

Recorder::Failure Recorder::takeFailure (juce::String& message)
{
    const auto failed = failure.load();
    if (failed == Failure::none || failureReported.exchange (true))
        return Failure::none;
    const juce::ScopedLock sl (statusLock);
    message = failureMessage;
    return failed;
}

Recorder::Status Recorder::getStatus() const
{
    Status status;
    status.recording = running;
    status.frames = framesWritten;
    status.seconds = secondsWritten;
    status.overruns = overruns;
    status.files = filesWritten;
    const juce::ScopedLock sl (statusLock);
    status.file = currentFile;
    return status;
}

std::vector<Recorder::Mark> Recorder::getMarks() const
{
    std::vector<Mark> result;
    const auto count = markCount.load (std::memory_order_acquire);
    const juce::ScopedLock sl (splitsLock);
    for (int i = 0; i < count; ++i)
    {
        const auto frame = marks[static_cast<size_t> (i)];
        size_t file = 0;
        while (file + 1 < splits.size() && splits[file + 1].frame <= frame)
            ++file;
        const auto& split = splits[file];
        result.push_back ({ static_cast<int> (file), static_cast<double> (frame - split.frame) / split.rate });
    }
    return result;
}

void Recorder::setSampleRate (double sampleRate)
{
    if (! running || ! (sampleRate > 0.0))
        return;
    const juce::ScopedLock sl (splitsLock);
    auto& last = splits.back();
    if (juce::exactlyEqual (last.rate, sampleRate))
        return;
    const auto frame = pushed.load();
    if (last.frame == frame)
        last.rate = sampleRate; // Nothing was pushed at the old rate.
    else
        splits.push_back ({ frame, sampleRate });
}

void Recorder::push (const float* left, const float* right, int numSamples, float gainFrom, float gainTo) noexcept
{
    if (! active.load (std::memory_order_acquire) || numSamples <= 0)
        return;

    const auto toWrite = juce::jmin (numSamples, fifo.getFreeSpace());
    if (toWrite < numSamples)
        ++overruns;
    if (toWrite <= 0)
        return;

    int start1, size1, start2, size2;
    fifo.prepareToWrite (toWrite, start1, size1, start2, size2);
    const auto copy = [&] (int start, int size, int from)
    {
        if (size <= 0)
            return;
        auto* outLeft = fifoData.getWritePointer (0, start);
        auto* outRight = fifoData.getWritePointer (1, start);
        if (juce::exactlyEqual (gainFrom, gainTo))
        {
            juce::FloatVectorOperations::multiply (outLeft, left + from, gainFrom, size);
            juce::FloatVectorOperations::multiply (outRight, right + from, gainFrom, size);
            return;
        }
        // As AudioBuffer::applyGainRamp steps it.
        const auto step = (gainTo - gainFrom) / static_cast<float> (numSamples);
        for (int i = 0; i < size; ++i)
        {
            const auto gain = gainFrom + step * static_cast<float> (from + i);
            outLeft[i] = left[from + i] * gain;
            outRight[i] = right[from + i] * gain;
        }
    };
    copy (start1, size1, 0);
    copy (start2, size2, size1);
    fifo.finishedWrite (size1 + size2);
    pushed.fetch_add (size1 + size2, std::memory_order_release);
}

void Recorder::markTrack (int framesAhead) noexcept
{
    if (! active.load (std::memory_order_acquire))
        return;
    const auto index = markCount.load (std::memory_order_relaxed);
    if (index >= maxMarks)
        return;
    marks[static_cast<size_t> (index)] = pushed.load() + framesAhead;
    markCount.store (index + 1, std::memory_order_release);
}

juce::File Recorder::numberedFile (const juce::File& first, int index)
{
    if (index <= 0)
        return first;
    return first.getSiblingFile (first.getFileNameWithoutExtension() + " " + juce::String (index + 1)
                                 + first.getFileExtension());
}
} // namespace anomp
