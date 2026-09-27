#include "PlayerEngine.h"
#include "FFmpegAudioFormat.h"

#include <signalsmith-stretch/signalsmith-stretch.h>

#include <array>
#include <cmath>
#include <cstring>
#include <utility>

namespace anomp
{
namespace
{
constexpr int outputChannels = 2;

float clampTrackGain (float gain)
{
    return gain > 0.0f ? juce::jmin (gain, PlayerEngine::maxTrackGain) : 0.0f; // NaN becomes 0.
}

// Read-ahead per track: about 3 s at 44.1 kHz. BufferingAudioSource prefills
// a quarter of a second when the track is opened.
constexpr int readAheadSamples = 1 << 17;
constexpr int readBlockSizeHint = 4096;

// Largest file-to-device rate ratio handled in one resampler pass; higher
// ratios are processed in smaller chunks.
constexpr int maxResampleRatio = 8;

juce::int64 toSamples (double seconds, double rate) { return static_cast<juce::int64> (std::llround (seconds * rate)); }
} // namespace

//==============================================================================
/** A decoded file (or part of one) and its read position, in file samples
    from the start of the part. Opened and freed on the message thread; read
    on the audio thread (or, with read-ahead, decoded on the read-ahead
    thread). */
struct PlayerEngine::Track
{
    Track (const juce::File& fileToPlay,
           std::unique_ptr<juce::AudioFormatReader> reader,
           juce::TimeSliceThread* thread,
           const TrackOptions& trackOptions,
           juce::int64 firstSample,
           juce::int64 numSamples)
        : file (fileToPlay),
          options (trackOptions),
          sampleRate (reader->sampleRate),
          offset (firstSample),
          length (numSamples),
          codec (reader->metadataValues[FFmpegAudioFormat::codecKey]),
          lossless (reader->metadataValues[FFmpegAudioFormat::losslessKey] == "1"),
          bits (reader->metadataValues[FFmpegAudioFormat::bitsKey].getIntValue()),
          bitrateKbps (
              static_cast<int> (reader->metadataValues[FFmpegAudioFormat::bitRateKey].getLargeIntValue() / 1000)),
          channels (static_cast<int> (reader->numChannels)),
          readerSource (reader.release(), true),
          gain (clampTrackGain (trackOptions.gain)),
          appliedGain (gain)
    {
        if (options.skipFrom >= 0.0 && options.skipTo > options.skipFrom)
        {
            skipFrom = juce::jlimit (juce::int64 { 0 }, length, toSamples (options.skipFrom, sampleRate));
            skipTo = juce::jlimit (skipFrom, length, toSamples (options.skipTo, sampleRate));
        }

        if (thread != nullptr)
        {
            buffered = std::make_unique<juce::BufferingAudioSource> (&readerSource, *thread, false, readAheadSamples,
                                                                     outputChannels, true);
            buffered->setNextReadPosition (offset); // Before the prefill, which reads from there.
            buffered->prepareToPlay (readBlockSizeHint, sampleRate);
        }
        else
        {
            readerSource.setNextReadPosition (offset);
            readerSource.prepareToPlay (readBlockSizeHint, sampleRate);
        }
    }

    juce::PositionableAudioSource& source() noexcept
    {
        return buffered != nullptr ? static_cast<juce::PositionableAudioSource&> (*buffered) : readerSource;
    }

    juce::int64 remaining() const noexcept { return length - position; }

    /** Reads up to `wanted` samples, stopping at the end of the track, and
        returns how many were read, with the track's gain applied (ramped
        from the previous read's if it changed). A mono file fills both
        channels. */
    int read (juce::AudioBuffer<float>& buffer, int startSample, int wanted)
    {
        const auto count = static_cast<int> (juce::jmin (static_cast<juce::int64> (wanted), remaining()));
        if (count > 0)
        {
            source().getNextAudioBlock (juce::AudioSourceChannelInfo (&buffer, startSample, count));
            position += count;

            if (! juce::exactlyEqual (appliedGain, gain))
                buffer.applyGainRamp (startSample, count, appliedGain, gain);
            else if (! juce::exactlyEqual (gain, 1.0f))
                buffer.applyGain (startSample, count, gain);
            appliedGain = gain;
        }
        return count;
    }

    void setPosition (juce::int64 newPosition)
    {
        position = newPosition;
        source().setNextReadPosition (offset + newPosition);
    }

    bool isFrom (const juce::File& other, double start) const
    {
        return file == other && std::abs (options.start - start) < 1e-6;
    }

    const juce::File file;
    const TrackOptions options;
    const double sampleRate;
    const juce::int64 offset, length;
    const juce::String codec;
    const bool lossless;
    const int bits, bitrateKbps, channels;
    juce::AudioFormatReaderSource readerSource;
    std::unique_ptr<juce::BufferingAudioSource> buffered; // Declared last: freed before readerSource.
    juce::int64 position = 0;
    juce::int64 skipFrom = -1, skipTo = -1; // Within the track; -1 when there's nothing (left) to skip.
    float gain, appliedGain;                // Changed under the player's lock.
};

//==============================================================================
/** Streaming stereo resampler from the file rate to the device rate, using
    JUCE's windowed-sinc interpolator. Keeps the input it has pulled but not
    yet consumed, so a stream that spans two tracks resamples as one. */
class PlayerEngine::Resampler
{
public:
    void prepare (int inputCapacity)
    {
        input.setSize (outputChannels, inputCapacity);
        reset();
    }

    int getInputCapacity() const noexcept { return input.getNumSamples(); }

    void reset() noexcept
    {
        for (auto& interpolator : interpolators)
            interpolator.reset();
        available = 0;
    }

    /** Input samples per output sample. */
    void setRatio (double newRatio) noexcept { ratio = newRatio; }
    bool isBypassed() const noexcept { return juce::exactlyEqual (ratio, 1.0); }

    /** Writes `numSamples` output samples; `pull (buffer, startSample, count)`
        must fill `count` input samples. */
    template <typename Pull>
    void process (float* const* output, int numSamples, Pull&& pull)
    {
        // An interpolator pass producing n samples consumes at most ceil (n * ratio) + 1.
        const auto maxChunk = juce::jmax (1, static_cast<int> ((input.getNumSamples() - 3) / ratio));

        for (int done = 0; done < numSamples;)
        {
            const auto count = juce::jmin (numSamples - done, maxChunk);
            const auto needed = static_cast<int> (std::ceil (count * ratio)) + 2;

            if (available < needed)
            {
                pull (input, available, needed - available);
                available = needed;
            }

            int used = 0;
            for (int ch = 0; ch < outputChannels; ++ch)
                used = interpolators[static_cast<size_t> (ch)].process (ratio, input.getReadPointer (ch),
                                                                        output[ch] + done, count);
            jassert (used <= available);

            for (int ch = 0; ch < outputChannels; ++ch)
            {
                auto* data = input.getWritePointer (ch);
                std::memmove (data, data + used, static_cast<size_t> (available - used) * sizeof (float));
            }

            available -= used;
            done += count;
        }
    }

private:
    std::array<juce::WindowedSincInterpolator, outputChannels> interpolators;
    juce::AudioBuffer<float> input;
    int available = 0;
    double ratio = 1.0;
};

//==============================================================================
/** Changes tempo and pitch independently (Signalsmith Stretch), at the file
    rate, between the tracks and the resampler. Configured (which allocates)
    on the message thread; processed on the audio thread. */
class PlayerEngine::Stretcher
{
public:
    void configure (double sampleRate, int maxOutput)
    {
        stretch.presetDefault (outputChannels, static_cast<float> (sampleRate));
        input.setSize (outputChannels, static_cast<int> (std::ceil (maxOutput * maxTempo)) + 2);
        reset();
    }

    /** False while it would play the audio as it is. */
    bool isActive() const noexcept { return active; }

    void set (double rate, double newSemitones)
    {
        const auto wasActive = active;
        tempo = rate;
        semitones = newSemitones;
        stretch.setTransposeSemitones (static_cast<float> (semitones));
        active = ! juce::exactlyEqual (rate, 1.0) || ! juce::exactlyEqual (semitones, 0.0);
        if (active && ! wasActive)
            reset();
    }

    double getTempo() const noexcept { return tempo; }
    double getSemitones() const noexcept { return semitones; }

    void reset()
    {
        stretch.reset();
        carry = 0.0;
    }

    /** Writes `count` samples into `output` from `start`, pulling about
        `count * tempo` input samples with `pull (buffer, startSample, count)`. */
    template <typename Pull>
    void process (juce::AudioBuffer<float>& output, int start, int count, Pull&& pull)
    {
        const auto exact = count * tempo + carry;
        auto inputCount = static_cast<int> (exact);
        carry = exact - inputCount;
        inputCount = juce::jmin (inputCount, input.getNumSamples());
        if (inputCount > 0)
            pull (input, 0, inputCount);

        std::array<float*, outputChannels> in { input.getWritePointer (0), input.getWritePointer (1) };
        std::array<float*, outputChannels> out { output.getWritePointer (0, start), output.getWritePointer (1, start) };
        stretch.process (in.data(), inputCount, out.data(), count);
    }

private:
    signalsmith::stretch::SignalsmithStretch<float> stretch;
    juce::AudioBuffer<float> input;
    double tempo = 1.0, semitones = 0.0, carry = 0.0;
    bool active = false;
};

//==============================================================================
PlayerEngine::PlayerEngine (juce::AudioFormatManager& formatsToUse, juce::TimeSliceThread* thread)
    : formats (formatsToUse),
      readAheadThread (thread),
      resampler (std::make_unique<Resampler>())
{
}

PlayerEngine::~PlayerEngine() = default;

PlayerEngine::TrackPtr PlayerEngine::openTrack (const juce::File& file,
                                                const TrackOptions& options,
                                                juce::String& error) const
{
    if (! file.existsAsFile())
    {
        error = "File not found: " + file.getFullPathName();
        return nullptr;
    }

    std::unique_ptr<juce::AudioFormatReader> reader (formats.createReaderFor (file));

    if (reader == nullptr)
    {
        error = "Unsupported or unreadable file: " + file.getFullPathName();
        return nullptr;
    }

    if (reader->lengthInSamples <= 0 || reader->sampleRate <= 0.0 || reader->numChannels == 0)
    {
        error = "No audio in file: " + file.getFullPathName();
        return nullptr;
    }

    if (! std::isfinite (options.start) || ! std::isfinite (options.end))
    {
        error = "The track's start or end isn't a number";
        return nullptr;
    }

    const auto fileLength = reader->lengthInSamples;
    const auto first = juce::jmax (juce::int64 { 0 }, toSamples (options.start, reader->sampleRate));
    const auto last =
        options.end > options.start ? juce::jmin (fileLength, toSamples (options.end, reader->sampleRate)) : fileLength;
    if (first >= last)
    {
        error = "The track starts after the end of the file: " + file.getFullPathName();
        return nullptr;
    }

    return std::make_unique<Track> (file, std::move (reader), readAheadThread, options, first, last - first);
}

//==============================================================================
juce::String PlayerEngine::load (const juce::File& file, const TrackOptions& options)
{
    juce::String error;
    auto track = openTrack (file, options, error);
    if (track == nullptr)
        return error;

    // Freed after the lock is released.
    TrackPtr oldCurrent, oldNext, oldLoop;
    std::array<TrackPtr, 4> oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldCurrent = std::exchange (current, std::move (track));
        oldNext = std::move (next);
        oldLoop = std::move (loopTrack);
        loopStart = loopEnd = -1;
        loopRewindPending = false;
        oldRetired = takeRetired();
        pendingEnded = false;
        state = State::stopped;
        appliedGain = 0.0f;
        configureRate();
        publishPosition();
    }
    return {};
}

juce::String PlayerEngine::setNext (const juce::File& file, const TrackOptions& options)
{
    if (getState() == State::empty)
        return "No track is loaded";

    juce::String error;
    auto track = openTrack (file, options, error);
    if (track == nullptr)
        return error;

    TrackPtr oldNext;
    std::array<TrackPtr, 4> oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldNext = std::exchange (next, std::move (track));
        oldRetired = takeRetired();
    }
    return {};
}

int PlayerEngine::setTrackGain (const juce::File& file, float gain)
{
    const juce::ScopedLock sl (lock);
    int changed = 0;
    for (auto* track : { current.get(), next.get() })
    {
        if (track != nullptr && track->file == file)
        {
            track->gain = clampTrackGain (gain);
            ++changed;
        }
    }
    if (loopTrack != nullptr && loopTrack->file == file)
        loopTrack->gain = clampTrackGain (gain);
    return changed;
}

int PlayerEngine::setTrackGainAt (const juce::File& file, double start, float gain)
{
    const juce::ScopedLock sl (lock);
    int changed = 0;
    for (auto* track : { current.get(), next.get() })
    {
        if (track != nullptr && track->isFrom (file, start))
        {
            track->gain = clampTrackGain (gain);
            ++changed;
        }
    }
    if (loopTrack != nullptr && loopTrack->isFrom (file, start))
        loopTrack->gain = clampTrackGain (gain);
    return changed;
}

void PlayerEngine::clearNext()
{
    TrackPtr oldNext;
    std::array<TrackPtr, 4> oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldNext = std::move (next);
        oldRetired = takeRetired();
    }
}

bool PlayerEngine::play()
{
    const juce::ScopedLock sl (lock);
    if (current == nullptr)
        return false;

    state = State::playing;
    return true;
}

void PlayerEngine::pause()
{
    const juce::ScopedLock sl (lock);
    if (state == State::playing)
        state = State::paused;
}

void PlayerEngine::stop()
{
    const juce::ScopedLock sl (lock);
    if (current == nullptr)
        return;

    state = State::stopped;
    appliedGain = 0.0f;
    rewind();
    publishPosition();
}

bool PlayerEngine::seek (double seconds)
{
    const juce::ScopedLock sl (lock);
    if (current == nullptr || ! std::isfinite (seconds))
        return false;

    current->setPosition (juce::jlimit (juce::int64 { 0 }, current->length, toSamples (seconds, current->sampleRate)));
    resampler->reset();
    if (stretcher != nullptr)
        stretcher->reset();
    publishPosition();
    return true;
}

void PlayerEngine::setVolume (float gain)
{
    volume = gain > 0.0f ? juce::jmin (gain, 1.0f) : 0.0f; // NaN becomes 0.
}

juce::String PlayerEngine::setLoop (double start, double end)
{
    if (! std::isfinite (start) || ! std::isfinite (end) || start < 0.0 || end - start < minLoopSeconds - 1e-9)
        return "A loop must start at 0 s or later and last at least " + juce::String (minLoopSeconds) + " s";

    juce::File file;
    TrackOptions options;
    {
        const juce::ScopedLock sl (lock);
        if (current == nullptr)
            return "No track is loaded";
        file = current->file;
        options = current->options;
    }

    // The waiting reader never skips; the loop decides where it plays.
    options.skipFrom = options.skipTo = -1.0;
    juce::String error;
    auto spare = openTrack (file, options, error);
    if (spare == nullptr)
        return error;

    TrackPtr oldLoop;
    {
        const juce::ScopedLock sl (lock);
        // The current track may have changed while the file was opened.
        if (current == nullptr || ! current->isFrom (file, options.start))
            return "The track changed";

        const auto first =
            juce::jlimit (juce::int64 { 0 }, current->length - 1, toSamples (start, current->sampleRate));
        const auto last = juce::jlimit (first + 1, current->length, toSamples (end, current->sampleRate));
        spare->gain = spare->appliedGain = current->gain;
        spare->setPosition (first);
        oldLoop = std::exchange (loopTrack, std::move (spare));
        loopStart = first;
        loopEnd = last;
        loopRewindPending = false;

        // Already past the end: back to the start now.
        if (current->position >= loopEnd)
        {
            current->setPosition (loopStart);
            resampler->reset();
            publishPosition();
        }
    }
    return {};
}

void PlayerEngine::clearLoop()
{
    TrackPtr oldLoop;
    const juce::ScopedLock sl (lock);
    oldLoop = std::move (loopTrack);
    loopStart = loopEnd = -1;
    loopRewindPending = false;
}

bool PlayerEngine::getLoop (double& start, double& end) const
{
    const juce::ScopedLock sl (lock);
    if (loopTrack == nullptr || current == nullptr)
        return false;
    start = static_cast<double> (loopStart) / current->sampleRate;
    end = static_cast<double> (loopEnd) / current->sampleRate;
    return true;
}

bool PlayerEngine::setTempo (double rate, double semitones)
{
    if (! (rate >= minTempo && rate <= maxTempo) || ! (std::abs (semitones) <= maxSemitones))
        return false;

    if (stretcher == nullptr)
    {
        if (juce::exactlyEqual (rate, 1.0) && juce::exactlyEqual (semitones, 0.0))
            return true;

        // Configured for the current track's rate; others play through it
        // too, with time constants a little off.
        double rateNow = 44100.0;
        int capacity = 0;
        {
            const juce::ScopedLock sl (lock);
            if (current != nullptr)
                rateNow = current->sampleRate;
            capacity = juce::jmax (resampler->getInputCapacity(), scratch.getNumSamples());
        }
        auto created = std::make_unique<Stretcher>();
        created->configure (rateNow, juce::jmax (capacity, 8192));
        created->set (rate, semitones);
        const juce::ScopedLock sl (lock);
        stretcher = std::move (created);
        resampler->reset();
        return true;
    }

    const juce::ScopedLock sl (lock);
    stretcher->set (rate, semitones);
    return true;
}

void PlayerEngine::setCrossfeed (int level) { crossfeedLevel = juce::jlimit (0, Crossfeed::maxLevel, level); }

bool PlayerEngine::hasNext() const
{
    const juce::ScopedLock sl (lock);
    return next != nullptr;
}

PlayerEngine::SignalInfo PlayerEngine::getSignalInfo() const
{
    SignalInfo info;
    const juce::ScopedLock sl (lock);
    info.deviceSampleRate = deviceRate;
    info.crossfeed = crossfeedLevel.load();
    if (stretcher != nullptr && stretcher->isActive())
    {
        info.tempo = stretcher->getTempo();
        info.semitones = stretcher->getSemitones();
    }
    if (current != nullptr)
    {
        info.loaded = true;
        info.codec = current->codec;
        info.lossless = current->lossless;
        info.bitsPerSample = current->bits;
        info.bitrateKbps = current->bitrateKbps;
        info.channels = current->channels;
        info.fileSampleRate = current->sampleRate;
        info.gain = current->gain;
    }
    return info;
}

//==============================================================================
void PlayerEngine::dispatchEvents()
{
    int advances = 0;
    bool ended = false;
    std::array<TrackPtr, 4> oldRetired;
    {
        const juce::ScopedLock sl (lock);
        advances = std::exchange (pendingAdvances, 0);
        ended = std::exchange (pendingEnded, false);
        oldRetired = takeRetired();

        // The reader that played up to the loop's end waits at its start
        // for the next jump; its read-ahead refills in the background.
        if (loopRewindPending && loopTrack != nullptr)
        {
            loopTrack->setPosition (loopStart);
            loopRewindPending = false;
        }
    }

    for (int i = 0; i < advances; ++i)
        if (onTrackEnded)
            onTrackEnded (true);

    if (ended && onTrackEnded)
        onTrackEnded (false);

    if (const auto newState = getState(); newState != reportedState)
    {
        reportedState = newState;
        if (onStateChanged)
            onStateChanged (newState);
    }

    const auto position = getPositionSeconds();
    const auto duration = getDurationSeconds();

    if (! juce::exactlyEqual (position, reportedPosition) || ! juce::exactlyEqual (duration, reportedDuration))
    {
        reportedPosition = position;
        reportedDuration = duration;
        if (onPositionChanged)
            onPositionChanged (position, duration);
    }
}

bool PlayerEngine::waitForReadAhead (int numSamples, int timeoutMs)
{
    const auto ready = [numSamples, timeoutMs] (Track* track)
    {
        return track == nullptr || track->buffered == nullptr
               || track->buffered->waitForNextAudioBlockReady (juce::AudioSourceChannelInfo (nullptr, 0, numSamples),
                                                               static_cast<juce::uint32> (timeoutMs));
    };
    return ready (current.get()) && ready (loopTrack.get());
}

//==============================================================================
void PlayerEngine::prepareToPlay (int samplesPerBlockExpected, double sampleRate)
{
    const juce::ScopedLock sl (lock);
    deviceRate = sampleRate;
    tap.setSampleRate (sampleRate);
    crossfeed.prepare (sampleRate);

    const auto chunkSize = juce::jmax (samplesPerBlockExpected, 256);
    scratch.setSize (outputChannels, chunkSize);
    resampler->prepare (chunkSize * maxResampleRatio + 8);
    if (stretcher != nullptr && current != nullptr)
        stretcher->configure (current->sampleRate, resampler->getInputCapacity());
    configureRate();
}

void PlayerEngine::releaseResources() {}

void PlayerEngine::getNextAudioBlock (const juce::AudioSourceChannelInfo& info)
{
    const juce::ScopedLock sl (lock);

    const auto playing = state == State::playing;

    // Paused or stopped once the fade-out has finished: silent, position held.
    if (current == nullptr || deviceRate <= 0.0 || (! playing && juce::exactlyEqual (appliedGain, 0.0f)))
    {
        info.clearActiveBufferRegion();
        return;
    }

    auto& out = *info.buffer;

    for (int done = 0; done < info.numSamples;)
    {
        const auto count = juce::jmin (info.numSamples - done, scratch.getNumSamples());
        renderChunk (scratch.getArrayOfWritePointers(), count);

        if (out.getNumChannels() == 1)
        {
            out.copyFrom (0, info.startSample + done, scratch, 0, 0, count);
            out.addFrom (0, info.startSample + done, scratch, 1, 0, count);
            out.applyGain (0, info.startSample + done, count, 0.5f);
        }
        else
        {
            for (int ch = 0; ch < outputChannels; ++ch)
                out.copyFrom (ch, info.startSample + done, scratch, ch, 0, count);
        }

        done += count;
    }

    for (int ch = outputChannels; ch < out.getNumChannels(); ++ch)
        out.clear (ch, info.startSample, info.numSamples);

    if (playing)
    {
        const auto* left = out.getReadPointer (0, info.startSample);
        const auto* right = out.getNumChannels() > 1 ? out.getReadPointer (1, info.startSample) : left;
        tap.push (left, right, info.numSamples);
    }

    // Crossfeed after the tap, so the visualizer shows the mix as it is.
    if (const auto level = crossfeedLevel.load(); level != crossfeed.getLevel())
        crossfeed.setLevel (level);
    if (out.getNumChannels() >= 2)
        crossfeed.process (out.getWritePointer (0, info.startSample), out.getWritePointer (1, info.startSample),
                           info.numSamples);

    // Pause and play fade over one block; volume changes ramp the same way.
    const auto targetGain = playing ? volume.load() : 0.0f;
    out.applyGainRamp (info.startSample, info.numSamples, appliedGain, targetGain);
    appliedGain = targetGain;

    if (current->remaining() == 0 && next == nullptr && loopTrack == nullptr)
    {
        state = State::stopped;
        appliedGain = 0.0f;
        pendingEnded = true;
        rewind();
    }

    publishPosition();
}

void PlayerEngine::renderChunk (float* const* output, int numSamples)
{
    // A next track at a different rate takes over at a chunk boundary, with
    // fresh resampler state; same-rate tracks join inside readSource().
    if (current->remaining() == 0 && next != nullptr && loopTrack == nullptr
        && ! juce::exactlyEqual (next->sampleRate, current->sampleRate))
    {
        handOff();
        configureRate();
    }

    const auto stretching = stretcher != nullptr && stretcher->isActive();
    const auto pull = [this, stretching] (juce::AudioBuffer<float>& buffer, int start, int count)
    {
        if (stretching)
            readStretched (buffer, start, count);
        else
            readSource (buffer, start, count);
    };

    if (resampler->isBypassed())
    {
        juce::AudioBuffer<float> view (output, outputChannels, numSamples);
        pull (view, 0, numSamples);
    }
    else
    {
        resampler->process (output, numSamples, pull);
    }
}

void PlayerEngine::readStretched (juce::AudioBuffer<float>& buffer, int startSample, int numSamples)
{
    stretcher->process (buffer, startSample, numSamples, [this] (juce::AudioBuffer<float>& input, int start, int count)
                        { readSource (input, start, count); });
}

int PlayerEngine::readSource (juce::AudioBuffer<float>& buffer, int startSample, int numSamples)
{
    int done = 0;

    while (done < numSamples)
    {
        // The loop comes first: its end may be the track's.
        if (loopTrack != nullptr && current->position >= loopEnd)
        {
            jumpToLoopStart();
            continue;
        }

        // A stretch to skip (e.g. a long silence before a hidden track).
        if (current->skipFrom >= 0 && current->position >= current->skipFrom)
        {
            current->setPosition (current->skipTo);
            current->skipFrom = current->skipTo = -1;
        }

        if (current->remaining() == 0)
        {
            if (next == nullptr || ! juce::exactlyEqual (next->sampleRate, current->sampleRate))
                break;
            handOff();
            continue;
        }

        auto limit = juce::jmin (static_cast<juce::int64> (numSamples - done), current->remaining());
        if (loopTrack != nullptr)
            limit = juce::jmin (limit, loopEnd - current->position);
        if (current->skipFrom > current->position)
            limit = juce::jmin (limit, current->skipFrom - current->position);

        done += current->read (buffer, startSample + done, static_cast<int> (limit));
    }

    if (done < numSamples)
        for (int ch = 0; ch < outputChannels; ++ch)
            buffer.clear (ch, startSample + done, numSamples - done);

    return done;
}

void PlayerEngine::handOff()
{
    retire (std::move (current));
    current = std::move (next);
    // A loop belongs to the track that was playing.
    if (loopTrack != nullptr)
        retire (std::move (loopTrack));
    loopStart = loopEnd = -1;
    loopRewindPending = false;
    ++pendingAdvances;
    ++advanceCount;
}

void PlayerEngine::jumpToLoopStart()
{
    if (loopRewindPending)
    {
        // The spare reader hasn't been sent back yet (a very short loop, or
        // a busy message thread): seek this one, which may briefly starve
        // its read-ahead.
        current->setPosition (loopStart);
        return;
    }
    std::swap (current, loopTrack);
    loopRewindPending = true;
}

void PlayerEngine::retire (TrackPtr track)
{
    for (auto& slot : retired)
    {
        if (slot == nullptr)
        {
            slot = std::move (track);
            return;
        }
    }
    // dispatchEvents() and every command empty the slots, and at most two
    // tracks are let go of between them.
    jassertfalse;
    retired.back() = std::move (track);
}

std::array<PlayerEngine::TrackPtr, 4> PlayerEngine::takeRetired()
{
    return { std::move (retired[0]), std::move (retired[1]), std::move (retired[2]), nullptr };
}

void PlayerEngine::configureRate()
{
    if (current != nullptr && deviceRate > 0.0)
        resampler->setRatio (current->sampleRate / deviceRate);
    resampler->reset();
    if (stretcher != nullptr)
        stretcher->reset();
}

void PlayerEngine::rewind()
{
    current->setPosition (0);
    resampler->reset();
    if (stretcher != nullptr)
        stretcher->reset();
}

void PlayerEngine::publishPosition()
{
    positionSeconds = current != nullptr ? static_cast<double> (current->position) / current->sampleRate : 0.0;
    durationSeconds = current != nullptr ? static_cast<double> (current->length) / current->sampleRate : 0.0;
}
} // namespace anomp
