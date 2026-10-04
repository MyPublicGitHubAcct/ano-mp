#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <numbers>
#include <vector>

#if defined(__SSE2__) || defined(_M_X64)
#include <xmmintrin.h>
#endif

/** The building blocks the effects share: smoothing, delay lines, filters,
    oscillators and noise. Header-only and allocation-free once prepared. */
namespace anomp::fx::dsp
{
inline constexpr float pi = std::numbers::pi_v<float>;
inline constexpr float twoPi = 2.0f * std::numbers::pi_v<float>;

/** a == b for floats, without -Wfloat-equal's complaint: for values that are
    copied, never computed, so exact comparison is what is meant. */
inline bool same (float a, float b) noexcept { return ! (a < b) && ! (b < a); }

/** Samples in `seconds` at `sampleRate`, at least 1. */
inline int samplesIn (double seconds, double sampleRate) noexcept
{
    return std::max (1, static_cast<int> (std::lround (seconds * sampleRate)));
}

/** Moves linearly to its target over a set number of samples. */
class Smoothed
{
public:
    void prepare (double sampleRate, double rampSeconds) noexcept { rampLength = samplesIn (rampSeconds, sampleRate); }

    /** Glides to `value` from wherever it is now. */
    void setTarget (float value) noexcept
    {
        if (same (value, target))
            return;
        target = value;
        remaining = rampLength;
        step = (target - current) / static_cast<float> (rampLength);
    }

    /** Jumps to `value`. */
    void snap (float value) noexcept
    {
        current = target = value;
        remaining = 0;
    }

    float next() noexcept
    {
        if (remaining > 0)
            current = --remaining == 0 ? target : current + step;
        return current;
    }

    /** Moves on `numSamples` at once; returns the value there. */
    float skip (int numSamples) noexcept
    {
        if (remaining <= numSamples)
        {
            remaining = 0;
            current = target;
        }
        else
        {
            remaining -= numSamples;
            current += step * static_cast<float> (numSamples);
        }
        return current;
    }

    float getCurrent() const noexcept { return current; }
    float getTarget() const noexcept { return target; }
    bool isGliding() const noexcept { return remaining > 0; }

private:
    float current = 0.0f, target = 0.0f, step = 0.0f;
    int remaining = 0, rampLength = 1;
};

/** A circular buffer of past samples, read at whole or fractional delays.
    A delay of 0 is the sample pushed last. */
class DelayLine
{
public:
    /** Room for delays up to `maxDelay` samples. */
    void prepare (int maxDelay)
    {
        std::size_t size = 4;
        while (size < static_cast<std::size_t> (maxDelay) + 4)
            size *= 2;
        buffer.assign (size, 0.0f);
        mask = size - 1;
        writeIndex = 0;
    }

    void clear() noexcept { std::fill (buffer.begin(), buffer.end(), 0.0f); }

    void push (float sample) noexcept
    {
        buffer[writeIndex] = sample;
        writeIndex = (writeIndex + 1) & mask;
    }

    float at (std::size_t delay) const noexcept { return buffer[(writeIndex - 1 - delay) & mask]; }

    /** The signal `delay` samples ago (at least 1), by four-point Hermite
        interpolation, which keeps the highs that a linear one dulls as the
        delay moves. */
    float read (float delay) const noexcept
    {
        delay = std::clamp (delay, 1.0f, static_cast<float> (mask - 3));
        const auto whole = static_cast<std::size_t> (delay);
        const auto t = delay - static_cast<float> (whole);
        const auto ym1 = at (whole - 1), y0 = at (whole), y1 = at (whole + 1), y2 = at (whole + 2);
        const auto c1 = 0.5f * (y1 - ym1);
        const auto c2 = ym1 - 2.5f * y0 + 2.0f * y1 - 0.5f * y2;
        const auto c3 = 0.5f * (y2 - ym1) + 1.5f * (y0 - y1);
        return ((c3 * t + c2) * t + c1) * t + y0;
    }

private:
    std::vector<float> buffer { 0.0f, 0.0f, 0.0f, 0.0f };
    std::size_t mask = 3, writeIndex = 0;
};

/** A one-pole lowpass. */
class OnePole
{
public:
    void setCutoff (float hertz, double sampleRate) noexcept
    {
        const auto nyquistSafe = std::min (static_cast<double> (hertz), sampleRate * 0.45);
        coefficient = static_cast<float> (1.0 - std::exp (-2.0 * std::numbers::pi * nyquistSafe / sampleRate));
    }

    float process (float input) noexcept { return state += coefficient * (input - state); }
    void reset() noexcept { state = 0.0f; }

private:
    float coefficient = 1.0f, state = 0.0f;
};

/** A low-frequency oscillator: a phase from 0 to 1 at a rate in hertz. */
class Lfo
{
public:
    void setRate (float hertz, double sampleRate) noexcept
    {
        increment = static_cast<float> (static_cast<double> (hertz) / sampleRate);
    }

    /** Moves on one sample; returns the phase before the step. */
    float advance() noexcept
    {
        const auto now = phase;
        phase += increment;
        if (phase >= 1.0f)
            phase -= 1.0f;
        return now;
    }

    void reset() noexcept { phase = 0.0f; }

    /** 0 to 1 and back, as a raised sine, `offset` cycles along. */
    static float unipolar (float phase, float offset = 0.0f) noexcept
    {
        return 0.5f - 0.5f * std::cos (twoPi * (phase + offset));
    }

private:
    float phase = 0.0f, increment = 0.0f;
};

/** Fast, repeatable noise (xorshift32). */
class Random
{
public:
    explicit Random (std::uint32_t seed = 0x9e3779b9u) noexcept : state (seed != 0 ? seed : 1u) {}

    std::uint32_t nextInt() noexcept
    {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        return state;
    }

private:
    std::uint32_t state;
};

/** Flushes denormal numbers to zero while it exists, so a feedback path
    decaying into silence never slows the audio thread. */
class ScopedFlushDenormals
{
public:
    ScopedFlushDenormals() noexcept
    {
#if defined(__aarch64__)
        __asm__ __volatile__ ("mrs %0, fpcr" : "=r"(saved));
        const std::uint64_t flushed = saved | (std::uint64_t { 1 } << 24); // FZ
        __asm__ __volatile__ ("msr fpcr, %0" : : "r"(flushed));
#elif defined(__SSE2__) || defined(_M_X64)
        saved = _mm_getcsr();
        _mm_setcsr (saved | 0x8040u); // FTZ and DAZ
#endif
    }

    ~ScopedFlushDenormals()
    {
#if defined(__aarch64__)
        __asm__ __volatile__ ("msr fpcr, %0" : : "r"(saved));
#elif defined(__SSE2__) || defined(_M_X64)
        _mm_setcsr (saved);
#endif
    }

    ScopedFlushDenormals (const ScopedFlushDenormals&) = delete;
    ScopedFlushDenormals& operator= (const ScopedFlushDenormals&) = delete;

private:
#if defined(__aarch64__)
    std::uint64_t saved = 0;
#else
    unsigned int saved = 0;
#endif
};
} // namespace anomp::fx::dsp
