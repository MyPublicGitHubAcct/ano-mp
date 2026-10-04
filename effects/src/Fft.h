#pragma once

#include <cstddef>
#include <vector>

namespace anomp::fx
{
/** An in-place complex FFT of a power-of-two size (iterative radix 2), on
    separate real and imaginary arrays. Tables are made by prepare(); the
    transforms allocate nothing. */
class Fft
{
public:
    /** For transforms of `size` points, a power of two of at least 2. */
    void prepare (std::size_t size);

    std::size_t size() const noexcept { return points; }

    /** X[k] = sum x[n] e^(-2 pi i k n / N). */
    void forward (float* re, float* im) const noexcept;

    /** x[n] = 1/N sum X[k] e^(2 pi i k n / N). */
    void inverse (float* re, float* im) const noexcept;

private:
    void transform (float* re, float* im, float direction) const noexcept;

    std::size_t points = 0;
    std::vector<std::size_t> reversed;
    std::vector<float> cosines, sines; // e^(-2 pi i k / N) for k < N / 2.
};
} // namespace anomp::fx
