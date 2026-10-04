#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "Fft.h"
#include "Signals.h"
#include "SpectralFreeze.h"

#include <cmath>

using anomp::fx::EffectType;
using namespace signals;

namespace
{
/** The frequency of the strongest bin of `samples[from, from + 8192)`. */
double dominantHertz (const std::vector<float>& samples, std::size_t from, double sampleRate)
{
    constexpr std::size_t size = 8192;
    anomp::fx::Fft fft;
    fft.prepare (size);
    std::vector<float> re (samples.begin() + static_cast<std::ptrdiff_t> (from),
                           samples.begin() + static_cast<std::ptrdiff_t> (from + size)),
        im (size);
    fft.forward (re.data(), im.data());
    std::size_t best = 1;
    for (std::size_t k = 1; k < size / 2; ++k)
        if (std::hypot (re[k], im[k]) > std::hypot (re[best], im[best]))
            best = k;
    return static_cast<double> (best) * sampleRate / size;
}
} // namespace

TEST_CASE ("Spectral freeze passes the music through untouched until held", "[effects][freeze]")
{
    anomp::fx::SpectralFreeze freeze;
    freeze.prepare (48000.0);
    defaults (freeze, EffectType::freeze);
    const auto input = noise (48000);
    auto audio = input;
    run (freeze, audio);
    CHECK (audio.left == input.left);
    CHECK (audio.right == input.right);
}

TEST_CASE ("Spectral freeze sizes its window to the rate", "[effects][freeze]")
{
    anomp::fx::SpectralFreeze freeze;
    freeze.prepare (44100.0);
    CHECK (freeze.fftSize() == 8192);
    freeze.prepare (96000.0);
    CHECK (freeze.fftSize() == 16384);
    freeze.prepare (192000.0);
    CHECK (freeze.fftSize() == 32768);
}

TEST_CASE ("Spectral freeze sustains what it held, at its pitch and level", "[effects][freeze]")
{
    const auto rate = GENERATE (44100.0, 96000.0);
    anomp::fx::SpectralFreeze freeze;
    freeze.prepare (rate);
    defaults (freeze, EffectType::freeze);
    freeze.setParameter (anomp::fx::SpectralFreeze::fade, 0.05f, true);

    // A second of 440 Hz, held, then three seconds of silence going in.
    const auto second = static_cast<std::size_t> (rate);
    auto audio = sine (440.0, rate, second * 4);
    std::fill (audio.left.begin() + static_cast<std::ptrdiff_t> (second), audio.left.end(), 0.0f);
    std::fill (audio.right.begin() + static_cast<std::ptrdiff_t> (second), audio.right.end(), 0.0f);
    const auto inputLevel = rms (audio.left, 0, second);

    for (std::size_t done = 0; done < audio.size(); done += 64)
    {
        if (done == second - second % 64)
            freeze.setHeld (true);
        freeze.process (audio.left.data() + done, audio.right.data() + done,
                        static_cast<int> (std::min<std::size_t> (64, audio.size() - done)));
    }

    const auto heldLevel = rms (audio.left, second * 2, second * 4);
    CHECK (std::abs (db (heldLevel / inputLevel)) < 3.0);
    CHECK (dominantHertz (audio.left, second * 2, rate) == Catch::Approx (440.0).margin (rate / 8192.0 * 2.0));
    // Joining the music with no click.
    CHECK (maxStep (audio.left, second - 1000) < 0.15f);
}

TEST_CASE ("Spectral freeze keeps the stereo image and lets go when released", "[effects][freeze]")
{
    constexpr double rate = 48000.0;
    anomp::fx::SpectralFreeze freeze;
    freeze.prepare (rate);
    defaults (freeze, EffectType::freeze);
    freeze.setParameter (anomp::fx::SpectralFreeze::fade, 0.1f, true);

    // Noise on the left only.
    auto audio = noise (48000);
    std::fill (audio.right.begin(), audio.right.end(), 0.0f);
    run (freeze, audio);
    freeze.setHeld (true);
    auto held = silence (48000);
    run (freeze, held);
    CHECK (rms (held.left, 24000, 48000) > 0.1);
    CHECK (rms (held.right, 24000, 48000) < 1e-4);

    // Released: back to the music (here silence) after the fade.
    freeze.setHeld (false);
    auto released = silence (48000);
    run (freeze, released);
    CHECK (rms (released.left, 0, 2400) > 0.01);
    CHECK (rms (released.left, 9600, 48000) == 0.0);
}
