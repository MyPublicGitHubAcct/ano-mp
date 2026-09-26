#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>

#include "anomp/anomp.h"

#include <string>
#include <string_view>

namespace
{
std::string fixturePath (const char* name)
{
    return std::string (ANOMP_TEST_FIXTURES_DIR) + "/" + name;
}
} // namespace

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

    CHECK (anomp_engine_load (nullptr, "/a.flac", buffer, sizeof (buffer)) == 0);
    CHECK (std::string_view (buffer) == "Null engine");
    CHECK (anomp_engine_set_next (nullptr, nullptr, buffer, sizeof (buffer)) == 0);
    CHECK (anomp_engine_play (nullptr) == 0);
    anomp_engine_pause (nullptr);
    anomp_engine_stop (nullptr);
    CHECK (anomp_engine_seek (nullptr, 1.0) == 0);
    anomp_engine_set_volume (nullptr, 0.5);
    CHECK (anomp_engine_volume (nullptr) == 0.0);
    CHECK (anomp_engine_state (nullptr) == ANOMP_STATE_EMPTY);
    CHECK (anomp_engine_position (nullptr) == 0.0);
    CHECK (anomp_engine_duration (nullptr) == 0.0);
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

TEST_CASE ("C API player commands without an open device", "[c-api][engine]")
{
    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);
    char error[256] = "";

    CHECK (anomp_engine_state (engine) == ANOMP_STATE_EMPTY);
    CHECK (anomp_engine_play (engine) == 0);

    CHECK (anomp_engine_load (engine, nullptr, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "Null path");
    CHECK (anomp_engine_load (engine, "fixtures/flac-44k.flac", error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("Path is not absolute"));
    CHECK (anomp_engine_load (engine, fixturePath ("missing.flac").c_str(), error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("File not found"));
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_EMPTY);

    REQUIRE (anomp_engine_load (engine, fixturePath ("flac-44k.flac").c_str(), error, sizeof (error)) == 1);
    CHECK (std::string_view (error).empty());
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_STOPPED);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (22371 / 44100.0));

    CHECK (anomp_engine_set_next (engine, fixturePath ("wav-s16-44k.wav").c_str(), error, sizeof (error)) == 1);
    CHECK (anomp_engine_set_next (engine, nullptr, error, sizeof (error)) == 1);

    CHECK (anomp_engine_seek (engine, 0.25) == 1);
    CHECK (anomp_engine_position (engine) == Catch::Approx (0.25));

    CHECK (anomp_engine_play (engine) == 1);
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_PLAYING);
    anomp_engine_pause (engine);
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_PAUSED);
    anomp_engine_stop (engine);
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_STOPPED);
    CHECK (anomp_engine_position (engine) == 0.0);

    anomp_engine_set_volume (engine, 0.5);
    CHECK (anomp_engine_volume (engine) == 0.5);
    anomp_engine_set_volume (engine, 3.0);
    CHECK (anomp_engine_volume (engine) == 1.0);

    anomp_engine_destroy (engine);
}
