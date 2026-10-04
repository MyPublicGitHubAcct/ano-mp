#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>
#include <catch2/generators/catch_generators.hpp>

#include "Fft.h"

#include <cmath>
#include <complex>
#include <numbers>
#include <vector>

TEST_CASE ("Fft matches a plain DFT and inverts exactly", "[effects][fft]")
{
    const auto size = static_cast<std::size_t> (GENERATE (2, 16, 1024));
    anomp::fx::Fft fft;
    fft.prepare (size);
    CHECK (fft.size() == size);

    std::vector<float> re (size), im (size);
    for (std::size_t n = 0; n < size; ++n)
    {
        re[n] = static_cast<float> (std::sin (0.37 * static_cast<double> (n * n)));
        im[n] = static_cast<float> (std::cos (1.1 * static_cast<double> (n)));
    }
    const auto originalRe = re, originalIm = im;

    fft.forward (re.data(), im.data());
    for (std::size_t k = 0; k < size; ++k)
    {
        std::complex<double> sum = 0.0;
        for (std::size_t n = 0; n < size; ++n)
            sum +=
                std::complex<double> (originalRe[n], originalIm[n])
                * std::polar (1.0, -2.0 * std::numbers::pi * static_cast<double> (k * n) / static_cast<double> (size));
        CHECK (re[k] == Catch::Approx (sum.real()).margin (1e-3));
        CHECK (im[k] == Catch::Approx (sum.imag()).margin (1e-3));
    }

    fft.inverse (re.data(), im.data());
    for (std::size_t n = 0; n < size; ++n)
    {
        CHECK (re[n] == Catch::Approx (originalRe[n]).margin (1e-5));
        CHECK (im[n] == Catch::Approx (originalIm[n]).margin (1e-5));
    }
}
