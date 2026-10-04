#include "Fft.h"

#include <cmath>
#include <numbers>
#include <utility>

namespace anomp::fx
{
void Fft::prepare (std::size_t size)
{
    points = size;
    std::size_t bits = 0;
    while ((std::size_t { 1 } << bits) < size)
        ++bits;

    reversed.resize (size);
    for (std::size_t i = 0; i < size; ++i)
    {
        std::size_t r = 0;
        for (std::size_t b = 0; b < bits; ++b)
            r |= ((i >> b) & 1u) << (bits - 1 - b);
        reversed[i] = r;
    }

    cosines.resize (size / 2);
    sines.resize (size / 2);
    for (std::size_t k = 0; k < size / 2; ++k)
    {
        const auto angle = -2.0 * std::numbers::pi * static_cast<double> (k) / static_cast<double> (size);
        cosines[k] = static_cast<float> (std::cos (angle));
        sines[k] = static_cast<float> (std::sin (angle));
    }
}

void Fft::forward (float* re, float* im) const noexcept { transform (re, im, 1.0f); }

void Fft::inverse (float* re, float* im) const noexcept
{
    transform (re, im, -1.0f);
    const auto scale = 1.0f / static_cast<float> (points);
    for (std::size_t i = 0; i < points; ++i)
    {
        re[i] *= scale;
        im[i] *= scale;
    }
}

void Fft::transform (float* re, float* im, float direction) const noexcept
{
    for (std::size_t i = 0; i < points; ++i)
    {
        const auto j = reversed[i];
        if (i < j)
        {
            std::swap (re[i], re[j]);
            std::swap (im[i], im[j]);
        }
    }

    for (std::size_t length = 2; length <= points; length *= 2)
    {
        const auto half = length / 2;
        const auto stride = points / length;
        for (std::size_t start = 0; start < points; start += length)
        {
            for (std::size_t k = 0; k < half; ++k)
            {
                const auto wr = cosines[k * stride];
                const auto wi = direction * sines[k * stride];
                const auto a = start + k, b = a + half;
                const auto tr = re[b] * wr - im[b] * wi;
                const auto ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
            }
        }
    }
}
} // namespace anomp::fx
