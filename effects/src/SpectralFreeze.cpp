#include "SpectralFreeze.h"

#include <algorithm>
#include <cmath>

namespace anomp::fx
{
namespace
{
constexpr std::size_t numPhasors = 4096;

/** Hann windows on analysis and synthesis, overlapping four times, sum (in
    squares) to 1.5; random phases make each frame's resynthesis carry
    3/8 of the windowed input's power a sample. Together, 0.5625 of the
    input's power, which this gain puts back. */
constexpr float synthesisGain = 4.0f / 3.0f;
} // namespace

void SpectralFreeze::prepare (double rate)
{
    sampleRate = rate;
    points = 1024;
    while (static_cast<double> (points) < windowSeconds * rate && points < 65536)
        points *= 2;
    hop = points / 4;
    mask = points - 1;
    fft.prepare (points);

    window.resize (points);
    for (std::size_t n = 0; n < points; ++n)
        window[n] = 0.5f - 0.5f * std::cos (dsp::twoPi * static_cast<float> (n) / static_cast<float> (points));

    re.assign (points, 0.0f);
    im.assign (points, 0.0f);
    for (auto* buffers : { &history, &accumulated })
        for (auto& buffer : *buffers)
            buffer.assign (points, 0.0f);
    for (auto& buffer : queued)
        buffer.assign (hop, 0.0f);

    const auto bins = points / 2 + 1;
    for (auto* values : { &magnitudeLeft, &magnitudeRight, &stereoCos, &stereoSin })
        values->assign (bins, 0.0f);

    phasorCos.resize (numPhasors);
    phasorSin.resize (numPhasors);
    dsp::Random angles (12345u);
    for (std::size_t i = 0; i < numPhasors; ++i)
    {
        const auto angle = dsp::twoPi * static_cast<float> (angles.nextInt() >> 8) / 16777216.0f;
        phasorCos[i] = std::cos (angle);
        phasorSin[i] = std::sin (angle);
    }

    reset();
}

void SpectralFreeze::reset() noexcept
{
    for (auto* buffers : { &history, &accumulated, &queued })
        for (auto& buffer : *buffers)
            std::fill (buffer.begin(), buffer.end(), 0.0f);
    historyIndex = 0;
    queueIndex = hop;
    held = sounding = false;
    amount.snap (0.0f);
}

void SpectralFreeze::setParameter (std::size_t index, float value, bool) noexcept
{
    if (index == fade)
        amount.prepare (sampleRate, value);
}

void SpectralFreeze::setHeld (bool hold) noexcept
{
    if (hold == held)
        return;
    held = hold;
    if (held)
    {
        capture();
        if (! sounding)
        {
            // Three frames already overlapping, so the drone starts at its
            // full level; the fourth comes with the first sample.
            for (auto& buffer : accumulated)
                std::fill (buffer.begin(), buffer.end(), 0.0f);
            for (int i = 0; i < 3; ++i)
                synthesise();
            queueIndex = hop;
            sounding = true;
        }
    }
    amount.setTarget (held ? 1.0f : 0.0f);
}

void SpectralFreeze::capture() noexcept
{
    // The history in time order, oldest first, windowed: the left channel
    // as the real part, the right as the imaginary, in one transform.
    for (std::size_t n = 0; n < points; ++n)
    {
        const auto at = (historyIndex + n) & mask;
        re[n] = history[0][at] * window[n];
        im[n] = history[1][at] * window[n];
    }
    fft.forward (re.data(), im.data());

    // Z = L + iR, so L[k] = (Z[k] + conj Z[N-k]) / 2 and
    // R[k] = (Z[k] - conj Z[N-k]) / 2i.
    for (std::size_t k = 0; k <= points / 2; ++k)
    {
        const auto mirror = (points - k) & mask;
        const auto leftRe = 0.5f * (re[k] + re[mirror]), leftIm = 0.5f * (im[k] - im[mirror]);
        const auto rightRe = 0.5f * (im[k] + im[mirror]), rightIm = -0.5f * (re[k] - re[mirror]);
        magnitudeLeft[k] = std::hypot (leftRe, leftIm);
        magnitudeRight[k] = std::hypot (rightRe, rightIm);
        const auto difference = std::atan2 (rightIm, rightRe) - std::atan2 (leftIm, leftRe);
        stereoCos[k] = std::cos (difference);
        stereoSin[k] = std::sin (difference);
    }
}

void SpectralFreeze::synthesise() noexcept
{
    // The DC and Nyquist bins stay empty.
    re[0] = im[0] = re[points / 2] = im[points / 2] = 0.0f;
    for (std::size_t k = 1; k < points / 2; ++k)
    {
        const auto which = random.nextInt() & (numPhasors - 1);
        const auto c = phasorCos[which], s = phasorSin[which];
        const auto leftRe = magnitudeLeft[k] * c, leftIm = magnitudeLeft[k] * s;
        const auto rightRe = magnitudeRight[k] * (c * stereoCos[k] - s * stereoSin[k]);
        const auto rightIm = magnitudeRight[k] * (c * stereoSin[k] + s * stereoCos[k]);
        // Z[k] = L + iR and Z[N-k] = conj L + i conj R: both channels come
        // back real, in the real and imaginary parts.
        re[k] = leftRe - rightIm;
        im[k] = leftIm + rightRe;
        re[points - k] = leftRe + rightIm;
        im[points - k] = rightRe - leftIm;
    }
    fft.inverse (re.data(), im.data());

    for (std::size_t n = 0; n < points; ++n)
    {
        const auto w = window[n] * synthesisGain;
        accumulated[0][n] += re[n] * w;
        accumulated[1][n] += im[n] * w;
    }
    for (std::size_t ch = 0; ch < 2; ++ch)
    {
        auto& sum = accumulated[ch];
        std::copy (sum.begin(), sum.begin() + static_cast<std::ptrdiff_t> (hop), queued[ch].begin());
        std::copy (sum.begin() + static_cast<std::ptrdiff_t> (hop), sum.end(), sum.begin());
        std::fill (sum.end() - static_cast<std::ptrdiff_t> (hop), sum.end(), 0.0f);
    }
    queueIndex = 0;
}

void SpectralFreeze::process (float* left, float* right, int numSamples) noexcept
{
    for (int i = 0; i < numSamples; ++i)
    {
        history[0][historyIndex] = left[i];
        history[1][historyIndex] = right[i];
        historyIndex = (historyIndex + 1) & mask;

        if (! sounding)
            continue;

        if (queueIndex == hop)
            synthesise();
        const auto frozenLeft = queued[0][queueIndex], frozenRight = queued[1][queueIndex];
        ++queueIndex;

        // Equal power: the music and the drone are unrelated.
        const auto gliding = amount.isGliding();
        const auto a = amount.next();
        const auto music = gliding ? std::cos (a * dsp::pi * 0.5f) : 1.0f - a;
        const auto drone = gliding ? std::sin (a * dsp::pi * 0.5f) : a;
        left[i] = left[i] * music + frozenLeft * drone;
        right[i] = right[i] * music + frozenRight * drone;

        if (! held && ! amount.isGliding())
            sounding = false;
    }
}
} // namespace anomp::fx
