#include "PlayerEngine.h"

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
} // namespace

//==============================================================================
/** A decoded file and its read position, in file samples. Opened and freed on
    the message thread; read on the audio thread (or, with read-ahead, decoded
    on the read-ahead thread). */
struct PlayerEngine::Track
{
    Track (const juce::File& fileToPlay,
           std::unique_ptr<juce::AudioFormatReader> reader,
           juce::TimeSliceThread* thread,
           float initialGain)
        : file (fileToPlay),
          sampleRate (reader->sampleRate),
          length (reader->lengthInSamples),
          readerSource (reader.release(), true),
          gain (initialGain),
          appliedGain (initialGain)
    {
        if (thread != nullptr)
        {
            buffered = std::make_unique<juce::BufferingAudioSource> (&readerSource, *thread, false, readAheadSamples,
                                                                     outputChannels, true);
            buffered->prepareToPlay (readBlockSizeHint, sampleRate);
        }
        else
        {
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
        source().setNextReadPosition (newPosition);
    }

    const juce::File file;
    const double sampleRate;
    const juce::int64 length;
    juce::AudioFormatReaderSource readerSource;
    std::unique_ptr<juce::BufferingAudioSource> buffered; // Declared last: freed before readerSource.
    juce::int64 position = 0;
    float gain, appliedGain; // Changed under the player's lock.
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
PlayerEngine::PlayerEngine (juce::AudioFormatManager& formatsToUse, juce::TimeSliceThread* thread)
    : formats (formatsToUse),
      readAheadThread (thread),
      resampler (std::make_unique<Resampler>())
{
}

PlayerEngine::~PlayerEngine() = default;

std::unique_ptr<PlayerEngine::Track> PlayerEngine::openTrack (const juce::File& file,
                                                              float gain,
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

    return std::make_unique<Track> (file, std::move (reader), readAheadThread, clampTrackGain (gain));
}

//==============================================================================
juce::String PlayerEngine::load (const juce::File& file, float gain)
{
    juce::String error;
    auto track = openTrack (file, gain, error);
    if (track == nullptr)
        return error;

    // Freed after the lock is released.
    std::unique_ptr<Track> oldCurrent, oldNext, oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldCurrent = std::exchange (current, std::move (track));
        oldNext = std::move (next);
        oldRetired = std::move (retired);
        pendingEnded = false;
        state = State::stopped;
        appliedGain = 0.0f;
        configureRate();
        publishPosition();
    }
    return {};
}

juce::String PlayerEngine::setNext (const juce::File& file, float gain)
{
    if (getState() == State::empty)
        return "No track is loaded";

    juce::String error;
    auto track = openTrack (file, gain, error);
    if (track == nullptr)
        return error;

    std::unique_ptr<Track> oldNext, oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldNext = std::exchange (next, std::move (track));
        oldRetired = std::move (retired);
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
    return changed;
}

void PlayerEngine::clearNext()
{
    std::unique_ptr<Track> oldNext, oldRetired;
    {
        const juce::ScopedLock sl (lock);
        oldNext = std::move (next);
        oldRetired = std::move (retired);
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

    const auto target = static_cast<juce::int64> (std::llround (juce::jmax (0.0, seconds) * current->sampleRate));
    current->setPosition (juce::jmin (target, current->length));
    resampler->reset();
    publishPosition();
    return true;
}

void PlayerEngine::setVolume (float gain)
{
    volume = gain > 0.0f ? juce::jmin (gain, 1.0f) : 0.0f; // NaN becomes 0.
}

bool PlayerEngine::hasNext() const
{
    const juce::ScopedLock sl (lock);
    return next != nullptr;
}

//==============================================================================
void PlayerEngine::dispatchEvents()
{
    int advances = 0;
    bool ended = false;
    std::unique_ptr<Track> oldRetired;
    {
        const juce::ScopedLock sl (lock);
        advances = std::exchange (pendingAdvances, 0);
        ended = std::exchange (pendingEnded, false);
        oldRetired = std::move (retired);
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
    if (current == nullptr || current->buffered == nullptr)
        return true;

    return current->buffered->waitForNextAudioBlockReady (juce::AudioSourceChannelInfo (nullptr, 0, numSamples),
                                                          static_cast<juce::uint32> (timeoutMs));
}

//==============================================================================
void PlayerEngine::prepareToPlay (int samplesPerBlockExpected, double sampleRate)
{
    const juce::ScopedLock sl (lock);
    deviceRate = sampleRate;
    tap.setSampleRate (sampleRate);

    const auto chunkSize = juce::jmax (samplesPerBlockExpected, 256);
    scratch.setSize (outputChannels, chunkSize);
    resampler->prepare (chunkSize * maxResampleRatio + 8);
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

    // Pause and play fade over one block; volume changes ramp the same way.
    const auto targetGain = playing ? volume.load() : 0.0f;
    out.applyGainRamp (info.startSample, info.numSamples, appliedGain, targetGain);
    appliedGain = targetGain;

    if (current->remaining() == 0 && next == nullptr)
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
    if (current->remaining() == 0 && next != nullptr && ! juce::exactlyEqual (next->sampleRate, current->sampleRate))
    {
        handOff();
        configureRate();
    }

    if (resampler->isBypassed())
    {
        juce::AudioBuffer<float> view (output, outputChannels, numSamples);
        readSource (view, 0, numSamples);
    }
    else
    {
        resampler->process (output, numSamples, [this] (juce::AudioBuffer<float>& buffer, int start, int count)
                            { readSource (buffer, start, count); });
    }
}

int PlayerEngine::readSource (juce::AudioBuffer<float>& buffer, int startSample, int numSamples)
{
    int done = 0;

    while (done < numSamples)
    {
        if (current->remaining() == 0)
        {
            if (next == nullptr || ! juce::exactlyEqual (next->sampleRate, current->sampleRate))
                break;
            handOff();
        }

        done += current->read (buffer, startSample + done, numSamples - done);
    }

    if (done < numSamples)
        for (int ch = 0; ch < outputChannels; ++ch)
            buffer.clear (ch, startSample + done, numSamples - done);

    return done;
}

void PlayerEngine::handOff()
{
    // dispatchEvents() and every command free the previous one, and a second
    // hand-off needs a new next track, which only a command can set.
    jassert (retired == nullptr);
    retired = std::move (current);
    current = std::move (next);
    ++pendingAdvances;
    ++advanceCount;
}

void PlayerEngine::configureRate()
{
    if (current != nullptr && deviceRate > 0.0)
        resampler->setRatio (current->sampleRate / deviceRate);
    resampler->reset();
}

void PlayerEngine::rewind()
{
    current->setPosition (0);
    resampler->reset();
}

void PlayerEngine::publishPosition()
{
    positionSeconds = current != nullptr ? static_cast<double> (current->position) / current->sampleRate : 0.0;
    durationSeconds = current != nullptr ? static_cast<double> (current->length) / current->sampleRate : 0.0;
}
} // namespace anomp
