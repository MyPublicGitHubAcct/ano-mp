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

TEST_CASE ("C API engine functions accept a null engine", "[c-api][engine]")
{
    char buffer[64] = "unchanged";

    anomp_engine_destroy (nullptr);
    anomp_engine_set_event_callback (nullptr, nullptr, nullptr);
    CHECK (anomp_engine_open_default_device (nullptr, buffer, sizeof (buffer)) == 0);
    CHECK (std::string_view (buffer) == "Null engine");
    CHECK (anomp_engine_device_name (nullptr, buffer, sizeof (buffer)) == 0);
    CHECK (std::string_view (buffer).empty());
    CHECK (anomp_engine_play_test_tone (nullptr, 440.0) == 0);
    anomp_engine_stop_test_tone (nullptr);
}

TEST_CASE ("C API engine without an open device", "[c-api][engine]")
{
    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);

    CHECK (anomp_engine_device_name (engine, nullptr, 0) == 0);
    CHECK (anomp_engine_play_test_tone (engine, 440.0) == 0);
    anomp_engine_stop_test_tone (engine);

    anomp_engine_destroy (engine);
}
