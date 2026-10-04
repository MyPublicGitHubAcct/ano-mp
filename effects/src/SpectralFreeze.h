#pragma once

#include "Dsp.h"
#include "Effect.h"
#include "Fft.h"

#include <array>
#include <vector>

namespace anomp::fx
{
/** Holds the sound of a moment for as long as it is held: the spectrum of
    the last ~0.15 s (a Hann-windowed FFT of 8192 points at 44.1 or
    48 kHz, longer at higher rates) is kept, and resynthesised by
    overlap-add every quarter window with the same magnitudes and new random
    phases, so it sustains as a smooth, shimmering drone at the loudness it
    had. Each bin keeps the phase difference between the channels, so the
    stereo image stays where it was.

    While not held it passes the music through untouched (and costs only a
    copy into its history). Holding and letting go crossfade between the
    music and the frozen sound, at equal power, over `fade` seconds. */
class SpectralFreeze final : public Effect
{
public:
    enum Param : std::size_t
    {
        fade
    };

    /** The window, in seconds; the FFT's size is the next power of two. */
    static constexpr double windowSeconds = 0.15;

    void prepare (double sampleRate) override;
    void reset() noexcept override;
    void setParameter (std::size_t index, float value, bool snap) noexcept override;
    void process (float* left, float* right, int numSamples) noexcept override;
    void setHeld (bool held) noexcept override;

    std::size_t fftSize() const noexcept { return points; }

private:
    /** Takes the magnitudes (and the channels' phase differences) of the
        last `points` samples. */
    void capture() noexcept;
    /** Adds one frame to the overlap-add and queues the next hop of output. */
    void synthesise() noexcept;

    double sampleRate = 44100.0;
    std::size_t points = 0, hop = 0, mask = 0;
    Fft fft;
    std::vector<float> window, re, im;
    std::array<std::vector<float>, 2> history, accumulated, queued;
    std::size_t historyIndex = 0, queueIndex = 0;
    std::vector<float> magnitudeLeft, magnitudeRight, stereoCos, stereoSin;
    std::vector<float> phasorCos, phasorSin; // Unit phasors at random angles.
    dsp::Random random;
    dsp::Smoothed amount; // 0: the music; 1: the frozen sound.
    bool held = false, sounding = false;
};
} // namespace anomp::fx
