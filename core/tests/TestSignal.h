#pragma once

// The deterministic signal encoded in every file in fixtures/. Must match
// scripts/make-test-fixtures.py.

#include <array>
#include <cmath>
#include <cstdint>

namespace anomp::test
{
inline constexpr double signalAmplitude = 0.5;
inline constexpr std::array<std::array<double, 2>, 2> signalChirps { { { 200.0, 5000.0 }, { 300.0, 7000.0 } } };

/** Plus an odd remainder, so no codec frame size divides it. */
inline std::int64_t signalLength (int sampleRate, double seconds)
{
    return static_cast<std::int64_t> (sampleRate * seconds) + 321;
}

/** A linear chirp per channel; non-periodic, so misalignment is unambiguous. */
inline double signalSample (int channel, std::int64_t n, int sampleRate, double seconds)
{
    const auto [f0, f1] = signalChirps[static_cast<size_t> (channel)];
    const auto t = static_cast<double> (n) / sampleRate;
    const auto duration = static_cast<double> (signalLength (sampleRate, seconds)) / sampleRate;
    const auto phase = 2.0 * 3.14159265358979323846 * (f0 * t + (f1 - f0) * t * t / (2.0 * duration));
    return signalAmplitude * std::sin (phase);
}

/** The signal as stored in the 16-bit source, then decoded to float. */
inline float signalSample16 (int channel, std::int64_t n, int sampleRate, double seconds)
{
    const auto quantised = std::floor (signalSample (channel, n, sampleRate, seconds) * 32767.0 + 0.5);
    return static_cast<float> (quantised / 32768.0);
}
} // namespace anomp::test
