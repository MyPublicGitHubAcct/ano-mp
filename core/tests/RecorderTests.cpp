#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "FFmpegEncoder.h"
#include "FormatRegistry.h"
#include "PlayerEngine.h"
#include "Recorder.h"

#include <array>
#include <chrono>
#include <cmath>
#include <memory>
#include <thread>
#include <vector>

// Recording (PLAN.md X6): the player rendered offline as in
// PlayerEngineTests.cpp, its recording read back and compared with what it
// rendered, sample for sample.

namespace
{
using Channels = std::array<std::vector<float>, 2>;
using Kind = anomp::RecordingFormat::Kind;
using State = anomp::PlayerEngine::State;

constexpr int blockSize = 512;

juce::File fixtureFile (const char* name) { return juce::File (ANOMP_TEST_FIXTURES_DIR).getChildFile (name); }

int length (const Channels& audio) { return static_cast<int> (audio[0].size()); }

/** A folder of its own, removed afterwards. */
struct TempFolder
{
    TempFolder()
        : folder (juce::File::getSpecialLocation (juce::File::tempDirectory)
                      .getChildFile ("anomp-recorder-" + juce::Uuid().toString()))
    {
        REQUIRE (folder.createDirectory());
    }
    ~TempFolder() { folder.deleteRecursively(); }

    juce::File folder;
};

struct Decoded
{
    Channels audio;
    double sampleRate = 0.0;
    juce::String codec;
};

Decoded decodeFile (const juce::File& file)
{
    anomp::FormatRegistry registry;
    std::unique_ptr<juce::AudioFormatReader> reader (registry.manager().createReaderFor (file));
    REQUIRE (reader != nullptr);
    juce::AudioBuffer<float> buffer (2, static_cast<int> (reader->lengthInSamples));
    REQUIRE (reader->read (&buffer, 0, buffer.getNumSamples(), 0, true, true));

    Decoded result;
    result.sampleRate = reader->sampleRate;
    result.codec = reader->metadataValues["anomp.codec"];
    for (int ch = 0; ch < 2; ++ch)
        result.audio[static_cast<size_t> (ch)].assign (buffer.getReadPointer (ch),
                                                       buffer.getReadPointer (ch) + buffer.getNumSamples());
    return result;
}

float maxDifference (const Channels& a, int aStart, const Channels& b, int bStart, int count)
{
    float result = 0.0f;
    for (size_t ch = 0; ch < 2; ++ch)
        for (int i = 0; i < count; ++i)
            result = juce::jmax (
                result, std::abs (a[ch][static_cast<size_t> (aStart + i)] - b[ch][static_cast<size_t> (bStart + i)]));
    return result;
}

anomp::RecordingFormat floatWav()
{
    anomp::RecordingFormat format;
    format.kind = Kind::wav;
    format.bits = 32;
    return format;
}

/** A player rendered offline in blocks, keeping what it output. */
struct Harness
{
    explicit Harness (double deviceRate, anomp::Recorder::EncoderFactory encoders = {})
        : player (registry.manager(), nullptr, std::move (encoders))
    {
        player.prepareToPlay (blockSize, deviceRate);
    }

    void renderBlock()
    {
        juce::AudioBuffer<float> block (2, blockSize);
        block.clear();
        player.getNextAudioBlock (juce::AudioSourceChannelInfo (block));
        for (int ch = 0; ch < 2; ++ch)
            output[static_cast<size_t> (ch)].insert (output[static_cast<size_t> (ch)].end(), block.getReadPointer (ch),
                                                     block.getReadPointer (ch) + blockSize);
    }

    void render (int blocks)
    {
        for (int i = 0; i < blocks; ++i)
            renderBlock();
    }

    int renderUntilStopped (int maxSamples)
    {
        while (player.getState() == State::playing && length (output) < maxSamples)
            renderBlock();
        return length (output);
    }

    anomp::FormatRegistry registry;
    anomp::PlayerEngine player;
    Channels output;
};

/** An encoder that keeps what it is given, and can be made to fail. */
struct FakeFiles
{
    struct File
    {
        juce::File path;
        double rate = 0.0;
        Channels audio;
        bool finished = false;
    };
    std::vector<File> files;
    int failAfterWrites = -1; // Fails the write after this many; -1 never.
    bool diskFull = false;
    int writeDelayMs = 0;

    anomp::Recorder::EncoderFactory factory()
    {
        return [this] (const juce::File& path, const anomp::RecordingFormat&, double rate, anomp::RecordingError&)
        {
            files.push_back ({ path, rate, {}, false });
            return std::make_unique<Encoder> (*this, files.size() - 1);
        };
    }

    struct Encoder final : anomp::RecordingEncoder
    {
        Encoder (FakeFiles& ownerIn, size_t indexIn) : owner (ownerIn), index (indexIn) {}

        anomp::RecordingError write (const float* left, const float* right, int numSamples) override
        {
            if (owner.writeDelayMs > 0)
                std::this_thread::sleep_for (std::chrono::milliseconds (owner.writeDelayMs));
            if (owner.failAfterWrites == 0)
                return { "No space left on device", owner.diskFull };
            if (owner.failAfterWrites > 0)
                --owner.failAfterWrites;
            auto& audio = owner.files[index].audio;
            audio[0].insert (audio[0].end(), left, left + numSamples);
            audio[1].insert (audio[1].end(), right, right + numSamples);
            return {};
        }

        anomp::RecordingError finish() override
        {
            owner.files[index].finished = true;
            return {};
        }

        FakeFiles& owner;
        size_t index;
    };
};

/** A stereo test signal: two sines a fifth apart, at -6 dBFS. */
Channels sines (int numSamples, double rate)
{
    Channels result;
    for (size_t ch = 0; ch < 2; ++ch)
    {
        result[ch].resize (static_cast<size_t> (numSamples));
        const auto frequency = ch == 0 ? 440.0 : 660.0;
        for (int i = 0; i < numSamples; ++i)
            result[ch][static_cast<size_t> (i)] =
                0.5f * static_cast<float> (std::sin (juce::MathConstants<double>::twoPi * frequency * i / rate));
    }
    return result;
}

/** Pushes `audio` through a Recorder writing `format` to `file`. */
void record (const Channels& audio, double rate, const anomp::RecordingFormat& format, const juce::File& file)
{
    juce::CriticalSection lock;
    anomp::Recorder recorder (lock, anomp::openFFmpegEncoder);
    REQUIRE (recorder.start (file, format, rate, false) == "");
    for (int start = 0; start < length (audio); start += blockSize)
    {
        const auto count = juce::jmin (blockSize, length (audio) - start);
        const juce::ScopedLock sl (lock);
        recorder.push (audio[0].data() + start, audio[1].data() + start, count, 1.0f, 1.0f);
    }
    recorder.stop();
    CHECK (recorder.getStatus().overruns == 0);
    CHECK (recorder.getStatus().frames == length (audio));
}
} // namespace

TEST_CASE ("Recording formats check their settings", "[recording]")
{
    anomp::RecordingFormat format;
    CHECK (format.validate() == "");
    format.bits = 20;
    CHECK (format.validate().contains ("16-bit, 24-bit or 32-bit"));
    format.kind = Kind::flac;
    format.bits = 32;
    CHECK (format.validate().contains ("16-bit or 24-bit"));
    format.bits = 24;
    CHECK (format.validate() == "");
    format.kind = Kind::mp3;
    format.bitrateKbps = 64;
    CHECK (format.validate().contains ("96 to 320"));
    format.bitrateKbps = 320;
    CHECK (format.validate() == "");

    CHECK (anomp::RecordingFormat { Kind::alac }.getExtension() == "m4a");
    CHECK (anomp::RecordingFormat { Kind::aac }.getExtension() == "m4a");
    CHECK (anomp::RecordingFormat { Kind::aiff }.getExtension() == "aiff");

    const juce::File first ("/tmp/ano-mp 2026-10-04 21.15.03.wav");
    CHECK (anomp::Recorder::numberedFile (first, 0) == first);
    CHECK (anomp::Recorder::numberedFile (first, 1).getFileName() == "ano-mp 2026-10-04 21.15.03 2.wav");
}

TEST_CASE ("Every recording format can be encoded", "[recording][ffmpeg]")
{
    for (const auto kind : { Kind::wav, Kind::aiff, Kind::flac, Kind::alac, Kind::aac, Kind::mp3 })
    {
        INFO ("kind " << static_cast<int> (kind));
        CHECK (anomp::canEncode (kind));
    }
}

TEST_CASE ("A recording is what the player played, across a gapless hand-off", "[recording][gapless]")
{
    TempFolder temp;
    const auto file = temp.folder.getChildFile ("gapless.wav");
    Harness h (44100.0);

    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.startRecording (file, floatWav()) == "");
    CHECK (h.player.getSignalInfo().recording);
    REQUIRE (h.player.play());
    const auto firstLength = static_cast<int> (decodeFile (fixtureFile ("flac-44k.flac")).audio[0].size());
    const auto stoppedAt = h.renderUntilStopped (200000);
    h.player.stopRecording();
    CHECK_FALSE (h.player.getSignalInfo().recording);

    const auto status = h.player.getRecorder().getStatus();
    CHECK (status.frames == stoppedAt);
    CHECK (status.files == 1);
    CHECK (status.overruns == 0);
    CHECK (status.file == file);

    const auto recorded = decodeFile (file);
    CHECK (recorded.sampleRate == 44100.0);
    REQUIRE (length (recorded.audio) == stoppedAt);
    // The first block fades in; the volume's ramp and the recording's round
    // differently there. After it, exactly.
    CHECK (maxDifference (recorded.audio, 0, h.output, 0, blockSize) < 1e-6f);
    CHECK (maxDifference (recorded.audio, blockSize, h.output, blockSize, stoppedAt - blockSize) == 0.0f);

    const auto marks = h.player.getRecorder().getMarks();
    REQUIRE (marks.size() == 2);
    CHECK (marks[0].file == 0);
    CHECK (marks[0].seconds == 0.0);
    const auto handOff = marks[1].seconds * 44100.0;
    CHECK (handOff <= firstLength);
    CHECK (handOff > firstLength - blockSize);
}

TEST_CASE ("A recording takes a crossfade as it was heard", "[recording][crossfade]")
{
    TempFolder temp;
    const auto file = temp.folder.getChildFile ("crossfade.wav");
    Harness h (44100.0);
    const auto firstLength = static_cast<int> (decodeFile (fixtureFile ("flac-44k.flac")).audio[0].size());

    anomp::PlayerEngine::TrackOptions options;
    options.crossfade = 0.1;
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.setNext (fixtureFile ("wav-s16-44k.wav"), options).isEmpty());
    REQUIRE (h.player.startRecording (file, floatWav()) == "");
    REQUIRE (h.player.play());
    const auto stoppedAt = h.renderUntilStopped (200000);
    h.player.stopRecording();

    const auto recorded = decodeFile (file);
    REQUIRE (length (recorded.audio) == stoppedAt);
    CHECK (maxDifference (recorded.audio, blockSize, h.output, blockSize, stoppedAt - blockSize) == 0.0f);

    // The second track's mark is where the fade begins.
    const auto marks = h.player.getRecorder().getMarks();
    REQUIRE (marks.size() == 2);
    const auto fadeStart = firstLength - 4410;
    CHECK (marks[1].seconds * 44100.0 <= fadeStart);
    CHECK (marks[1].seconds * 44100.0 > fadeStart - blockSize);
}

TEST_CASE ("A recording leaves out the time paused, fading as heard", "[recording]")
{
    FakeFiles files;
    Harness h (44100.0, files.factory());
    REQUIRE (h.player.load (fixtureFile ("flac-long-48k-mono.flac")).isEmpty());
    REQUIRE (h.player.startRecording (juce::File ("/tmp/paused.wav"), floatWav()) == "");
    REQUIRE (h.player.play());
    h.render (10);
    h.player.pause();
    h.render (6); // One block fades out; then silence, which isn't recorded.
    REQUIRE (h.player.play());
    h.render (10);
    h.player.stopRecording();

    REQUIRE (files.files.size() == 1);
    CHECK (files.files[0].finished);
    const auto& recorded = files.files[0].audio;
    REQUIRE (length (recorded) == 21 * blockSize);

    // The blocks played, the fade-out, then the blocks after the pause.
    CHECK (maxDifference (recorded, 0, h.output, 0, 11 * blockSize) < 1e-6f);
    CHECK (maxDifference (recorded, 11 * blockSize, h.output, 16 * blockSize, 10 * blockSize) < 1e-6f);
    // The fade-out ends near silence and the resume starts from it.
    CHECK (std::abs (recorded[0][11 * blockSize - 1]) < 0.01f);
    CHECK (std::abs (recorded[0][11 * blockSize]) < 0.01f);
}

TEST_CASE ("A recording ignores the volume", "[recording]")
{
    FakeFiles files;
    Harness h (44100.0, files.factory());
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    h.player.setVolume (0.25f);
    REQUIRE (h.player.startRecording (juce::File ("/tmp/volume.wav"), floatWav()) == "");
    REQUIRE (h.player.play());
    h.render (8);
    h.player.stopRecording();

    const auto& recorded = files.files.at (0).audio;
    REQUIRE (length (recorded) == 8 * blockSize);
    float error = 0.0f;
    for (size_t ch = 0; ch < 2; ++ch)
        for (int i = blockSize; i < 8 * blockSize; ++i)
            error = juce::jmax (error, std::abs (recorded[ch][static_cast<size_t> (i)] * 0.25f
                                                 - h.output[ch][static_cast<size_t> (i)]));
    CHECK (error < 1e-6f);
}

TEST_CASE ("A change of sample rate starts a numbered file", "[recording]")
{
    TempFolder temp;
    const auto file = temp.folder.getChildFile ("ano-mp 2026-10-04 21.15.03.wav");
    Harness h (44100.0);
    REQUIRE (h.player.load (fixtureFile ("flac-long-48k-mono.flac")).isEmpty());
    REQUIRE (h.player.startRecording (file, floatWav()) == "");
    REQUIRE (h.player.play());
    h.render (20);
    h.player.prepareToPlay (blockSize, 48000.0);
    h.player.prepareToPlay (blockSize, 48000.0); // The same rate again changes nothing.
    h.render (30);
    h.player.stopRecording();

    const auto status = h.player.getRecorder().getStatus();
    CHECK (status.files == 2);
    CHECK (status.frames == 50 * blockSize);
    CHECK (status.seconds == Catch::Approx (20.0 * blockSize / 44100.0 + 30.0 * blockSize / 48000.0));
    const auto second = temp.folder.getChildFile ("ano-mp 2026-10-04 21.15.03 2.wav");
    CHECK (status.file == second);

    const auto first = decodeFile (file);
    CHECK (first.sampleRate == 44100.0);
    CHECK (length (first.audio) == 20 * blockSize);
    const auto next = decodeFile (second);
    CHECK (next.sampleRate == 48000.0);
    CHECK (length (next.audio) == 30 * blockSize);
    CHECK (maxDifference (next.audio, 0, h.output, 20 * blockSize, 30 * blockSize) == 0.0f);

    // A track loaded now is marked in the second file.
    REQUIRE (h.player.startRecording (temp.folder.getChildFile ("again.wav"), floatWav()) == "");
    h.render (4);
    REQUIRE (h.player.load (fixtureFile ("flac-44k.flac")).isEmpty());
    REQUIRE (h.player.play());
    h.render (4);
    h.player.prepareToPlay (blockSize, 44100.0);
    REQUIRE (h.player.load (fixtureFile ("wav-s16-44k.wav")).isEmpty());
    REQUIRE (h.player.play());
    h.render (4);
    h.player.stopRecording();
    const auto marks = h.player.getRecorder().getMarks();
    REQUIRE (marks.size() == 3);
    CHECK (marks[0].file == 0);
    CHECK (marks[1].file == 0);
    CHECK (marks[1].seconds == Catch::Approx (4.0 * blockSize / 48000.0));
    CHECK (marks[2].file == 1);
    CHECK (marks[2].seconds == 0.0);
}

TEST_CASE ("A write error stops the recording and keeps what was written", "[recording]")
{
    const auto diskFull = GENERATE (false, true);
    FakeFiles files;
    files.failAfterWrites = 1;
    files.diskFull = diskFull;
    Harness h (44100.0, files.factory());
    std::vector<std::pair<anomp::Recorder::Failure, juce::String>> failures;
    h.player.onRecordingFailed = [&] (anomp::Recorder::Failure failure, const juce::String& message)
    {
        failures.emplace_back (failure, message);
    };

    REQUIRE (h.player.load (fixtureFile ("flac-long-48k-mono.flac")).isEmpty());
    REQUIRE (h.player.startRecording (juce::File ("/tmp/failing.wav"), floatWav()) == "");
    REQUIRE (h.player.play());
    for (int i = 0; i < 200 && failures.empty(); ++i)
    {
        h.render (20);
        std::this_thread::sleep_for (std::chrono::milliseconds (5));
        h.player.dispatchEvents();
    }

    REQUIRE (failures.size() == 1);
    CHECK (failures[0].first
           == (diskFull ? anomp::Recorder::Failure::diskFull : anomp::Recorder::Failure::writeFailed));
    CHECK (failures[0].second == "No space left on device");
    CHECK_FALSE (h.player.getRecorder().isRecording());
    REQUIRE (files.files.size() == 1);
    CHECK (files.files[0].finished);
    CHECK (length (files.files[0].audio) > 0);

    // Reported once, and playing carries on.
    h.render (4);
    h.player.dispatchEvents();
    CHECK (failures.size() == 1);
    CHECK (h.player.getState() == State::playing);
}

TEST_CASE ("A writer that falls behind costs samples, never the audio thread", "[recording]")
{
    FakeFiles files;
    files.writeDelayMs = 50;
    juce::CriticalSection lock;
    anomp::Recorder recorder (lock, files.factory(), 4096);
    REQUIRE (recorder.start (juce::File ("/tmp/slow.wav"), floatWav(), 44100.0, false) == "");

    const auto audio = sines (blockSize, 44100.0);
    const auto began = std::chrono::steady_clock::now();
    for (int i = 0; i < 64; ++i)
    {
        const juce::ScopedLock sl (lock);
        recorder.push (audio[0].data(), audio[1].data(), blockSize, 1.0f, 1.0f);
    }
    const auto took = std::chrono::steady_clock::now() - began;
    CHECK (took < std::chrono::milliseconds (40));
    recorder.stop();

    const auto status = recorder.getStatus();
    CHECK (status.overruns > 0);
    CHECK (status.frames < 64 * blockSize);
    CHECK (status.frames == length (files.files.at (0).audio));
}

TEST_CASE ("A recording can't start without a device or with a bad folder", "[recording]")
{
    TempFolder temp;
    anomp::FormatRegistry registry;
    anomp::PlayerEngine player (registry.manager(), nullptr);
    CHECK (player.startRecording (temp.folder.getChildFile ("a.wav"), floatWav()) == "No output device is open");

    player.prepareToPlay (blockSize, 44100.0);
    const auto missing = temp.folder.getChildFile ("missing").getChildFile ("a.wav");
    CHECK (player.startRecording (missing, floatWav()).startsWith ("Cannot create a.wav"));
    CHECK_FALSE (player.getRecorder().isRecording());

    auto format = floatWav();
    format.bits = 8;
    CHECK (player.startRecording (temp.folder.getChildFile ("a.wav"), format).isNotEmpty());

    REQUIRE (player.startRecording (temp.folder.getChildFile ("a.wav"), floatWav()) == "");
    CHECK (player.startRecording (temp.folder.getChildFile ("b.wav"), floatWav()) == "Already recording");
    player.stopRecording();
    CHECK (temp.folder.getChildFile ("a.wav").existsAsFile());
}

TEST_CASE ("Each format records the signal", "[recording][ffmpeg]")
{
    struct Case
    {
        Kind kind;
        int bits;
        double rate;
        float tolerance;    // Largest sample difference, for lossless formats.
        double decodedRate; // The file's rate.
        const char* codec;
    };
    const auto c = GENERATE (
        Case { Kind::wav, 32, 44100.0, 0.0f, 44100.0, "pcm_f32le" },
        Case { Kind::wav, 24, 48000.0, 1.0f / (1 << 22), 48000.0, "pcm_s24le" },
        Case { Kind::wav, 16, 44100.0, 1.0f / (1 << 13), 44100.0, "pcm_s16le" },
        Case { Kind::aiff, 24, 44100.0, 1.0f / (1 << 22), 44100.0, "pcm_s24be" },
        Case { Kind::aiff, 16, 44100.0, 1.0f / (1 << 13), 44100.0, "pcm_s16be" },
        Case { Kind::flac, 24, 96000.0, 1.0f / (1 << 22), 96000.0, "flac" },
        Case { Kind::flac, 16, 44100.0, 1.0f / (1 << 13), 44100.0, "flac" },
        Case { Kind::alac, 24, 44100.0, 1.0f / (1 << 22), 44100.0, "alac" },
        Case { Kind::alac, 16, 48000.0, 1.0f / (1 << 13), 48000.0, "alac" },
        Case { Kind::aac, 0, 44100.0, -1.0f, 44100.0, "aac" }, Case { Kind::aac, 0, 192000.0, -1.0f, 96000.0, "aac" },
        Case { Kind::mp3, 0, 44100.0, -1.0f, 44100.0, "mp3" }, Case { Kind::mp3, 0, 96000.0, -1.0f, 48000.0, "mp3" },
        Case { Kind::mp3, 0, 176400.0, -1.0f, 44100.0, "mp3" });
    CAPTURE (static_cast<int> (c.kind), c.bits, c.rate);

    TempFolder temp;
    anomp::RecordingFormat format;
    format.kind = c.kind;
    format.bits = c.bits;
    format.bitrateKbps = 192;
    const auto file = temp.folder.getChildFile ("signal." + format.getExtension());
    const auto seconds = 2.0;
    const auto audio = sines (static_cast<int> (seconds * c.rate), c.rate);
    record (audio, c.rate, format, file);

    const auto decoded = decodeFile (file);
    CHECK (decoded.sampleRate == c.decodedRate);
    CHECK (decoded.codec == c.codec);

    if (c.tolerance >= 0.0f)
    {
        REQUIRE (length (decoded.audio) == length (audio));
        CHECK (maxDifference (decoded.audio, 0, audio, 0, length (audio)) <= c.tolerance);
        return;
    }

    // Lossy: the length (priming trimmed) and the level of the sines.
    const auto expected = seconds * c.decodedRate;
    CHECK (std::abs (length (decoded.audio) - expected) <= 2 * 1152);
    for (size_t ch = 0; ch < 2; ++ch)
    {
        double sum = 0.0;
        const auto start = length (decoded.audio) / 4, end = 3 * length (decoded.audio) / 4;
        for (auto i = start; i < end; ++i)
            sum += decoded.audio[ch][static_cast<size_t> (i)] * decoded.audio[ch][static_cast<size_t> (i)];
        const auto rms = std::sqrt (sum / (end - start));
        CHECK (rms == Catch::Approx (0.5 / std::sqrt (2.0)).margin (0.02));
    }
}
