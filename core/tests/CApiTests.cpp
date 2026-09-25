#include <catch2/catch_test_macros.hpp>

#include "anomp/anomp.h"

#include <string_view>

TEST_CASE ("C API reports a version", "[c-api]")
{
    REQUIRE (anomp_version() != nullptr);
    CHECK (std::string_view (anomp_version()) == "0.1.0");
}

TEST_CASE ("C API format query", "[c-api]")
{
    CHECK (anomp_can_decode_extension ("flac") == 1);
    CHECK (anomp_can_decode_extension (".mp3") == 1);
    CHECK (anomp_can_decode_extension ("docx") == 0);
    CHECK (anomp_can_decode_extension (nullptr) == 0);
}
