#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>

#include "anomp/anomp.h"

#include <juce_core/juce_core.h>

#if JUCE_MAC
#include <CoreFoundation/CoreFoundation.h>
#endif

#include <functional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace
{
std::string fixturePath (const char* name) { return std::string (ANOMP_TEST_FIXTURES_DIR) + "/" + name; }

/** Runs the main run loop, where the engine delivers its events, until
    `done` returns true; false after a timeout. */
bool runEventsUntil (const std::function<bool()>& done)
{
#if JUCE_MAC
    const auto deadline = juce::Time::getMillisecondCounterHiRes() + 10000.0;
    while (! done() && juce::Time::getMillisecondCounterHiRes() < deadline)
        CFRunLoopRunInMode (kCFRunLoopDefaultMode, 0.01, true);
#endif
    return done();
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

    CHECK (anomp_engine_load (nullptr, "/a.flac", 1.0, buffer, sizeof (buffer)) == 0);
    CHECK (std::string_view (buffer) == "Null engine");
    CHECK (anomp_engine_set_next (nullptr, nullptr, 1.0, buffer, sizeof (buffer)) == 0);
    CHECK (anomp_engine_play (nullptr) == 0);
    anomp_engine_pause (nullptr);
    anomp_engine_stop (nullptr);
    CHECK (anomp_engine_seek (nullptr, 1.0) == 0);
    anomp_engine_set_volume (nullptr, 0.5);
    CHECK (anomp_engine_volume (nullptr) == 0.0);
    CHECK (anomp_engine_state (nullptr) == ANOMP_STATE_EMPTY);
    CHECK (anomp_engine_position (nullptr) == 0.0);
    CHECK (anomp_engine_duration (nullptr) == 0.0);
    CHECK (anomp_engine_advance_count (nullptr) == 0);
    CHECK (anomp_engine_set_track_gain (nullptr, "/a.flac", 0.5) == 0);

    CHECK (anomp_engine_output_device_count (nullptr) == 0);
    CHECK (anomp_engine_output_device_name (nullptr, 0, buffer, sizeof (buffer)) == 0);
    CHECK (anomp_engine_open_device (nullptr, nullptr, 0, buffer, sizeof (buffer)) == 0);
    CHECK (std::string_view (buffer) == "Null engine");
    anomp_device_info info {};
    CHECK (anomp_engine_device_info (nullptr, &info) == 0);
    CHECK (anomp_engine_buffer_sizes (nullptr, nullptr, 0) == 0);
}

TEST_CASE ("C API engine without an open device", "[c-api][engine]")
{
    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);

    CHECK (anomp_engine_device_name (engine, nullptr, 0) == 0);
    CHECK (anomp_engine_play_test_tone (engine, 440.0) == 0);
    anomp_engine_stop_test_tone (engine);

    anomp_device_info info { 1, 2, 3.0, 4.0 };
    CHECK (anomp_engine_device_info (engine, &info) == 0);
    CHECK (info.buffer_size == 1); // Left alone.
    int sizes[4] = {};
    CHECK (anomp_engine_buffer_sizes (engine, sizes, 4) == 0);

    // Listing devices doesn't open one, and an unknown name fails without opening one.
    const auto count = anomp_engine_output_device_count (engine);
    CHECK (count >= 0);
    char name[256] = "unchanged";
    CHECK (anomp_engine_output_device_name (engine, count, name, sizeof (name)) == 0);
    CHECK (std::string_view (name).empty());
    CHECK (anomp_engine_output_device_name (engine, -1, nullptr, 0) == 0);
    char error[256] = "";
    CHECK (anomp_engine_open_device (engine, "No such device \xe2\x99\xaa", 0, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "No output device called \"No such device \xe2\x99\xaa\"");
    CHECK (anomp_engine_device_name (engine, nullptr, 0) == 0);

    anomp_engine_destroy (engine);
}

TEST_CASE ("C API player commands without an open device", "[c-api][engine]")
{
    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);
    char error[256] = "";

    CHECK (anomp_engine_state (engine) == ANOMP_STATE_EMPTY);
    CHECK (anomp_engine_play (engine) == 0);

    CHECK (anomp_engine_load (engine, nullptr, 1.0, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "Null path");
    CHECK (anomp_engine_load (engine, "fixtures/flac-44k.flac", 1.0, error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("Path is not absolute"));
    CHECK (anomp_engine_load (engine, fixturePath ("missing.flac").c_str(), 1.0, error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("File not found"));
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_EMPTY);

    REQUIRE (anomp_engine_load (engine, fixturePath ("flac-44k.flac").c_str(), 1.0, error, sizeof (error)) == 1);
    CHECK (std::string_view (error).empty());
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_STOPPED);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (22371 / 44100.0));
    CHECK (anomp_engine_advance_count (engine) == 0);

    CHECK (anomp_engine_set_next (engine, fixturePath ("wav-s16-44k.wav").c_str(), 0.5, error, sizeof (error)) == 1);
    CHECK (anomp_engine_set_next (engine, nullptr, 1.0, error, sizeof (error)) == 1);

    CHECK (anomp_engine_set_track_gain (engine, fixturePath ("flac-44k.flac").c_str(), 0.5) == 1);
    CHECK (anomp_engine_set_track_gain (engine, fixturePath ("wav-s16-44k.wav").c_str(), 0.5) == 0);
    CHECK (anomp_engine_set_track_gain (engine, "flac-44k.flac", 0.5) == 0);
    CHECK (anomp_engine_set_track_gain (engine, nullptr, 0.5) == 0);

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

TEST_CASE ("C API tracks, loops, tempo and crossfeed", "[c-api][engine]")
{
    char error[256] = "";
    CHECK (anomp_engine_load_track (nullptr, "/a.flac", nullptr, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "Null engine");
    CHECK (anomp_engine_set_loop (nullptr, 0.0, 1.0, error, sizeof (error)) == 0);
    CHECK (anomp_engine_loop (nullptr, nullptr, nullptr) == 0);
    CHECK (anomp_engine_set_tempo (nullptr, 1.0, 0.0) == 0);
    CHECK (anomp_engine_set_crossfeed (nullptr, 1) == 0);
    CHECK (anomp_engine_output_is_headphones (nullptr) == -1);
    CHECK (anomp_engine_signal_path (nullptr, nullptr) == 0);
    CHECK (anomp_engine_set_device_sample_rate (nullptr, 48000.0) == 0);
    CHECK (anomp_engine_set_track_gain_at (nullptr, "/a.flac", 0.0, 1.0) == 0);

    const auto defaults = anomp_track_options_default (0.5);
    CHECK (defaults.gain == 0.5);
    CHECK (defaults.start == 0.0);
    CHECK (defaults.end == 0.0);
    CHECK (defaults.skip_from < 0.0);

    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);
    const auto flac = fixturePath ("flac-44k.flac");

    anomp_signal_path path {};
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.loaded == 0);

    auto options = anomp_track_options_default (1.0);
    options.start = 0.1;
    options.end = 0.3;
    REQUIRE (anomp_engine_load_track (engine, flac.c_str(), &options, error, sizeof (error)) == 1);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (0.2));
    options.start = 0.3;
    options.end = 0.0;
    REQUIRE (anomp_engine_set_next_track (engine, flac.c_str(), &options, error, sizeof (error)) == 1);
    CHECK (anomp_engine_set_track_gain_at (engine, flac.c_str(), 0.3, 0.5) == 1);
    CHECK (anomp_engine_set_next_track (engine, nullptr, nullptr, error, sizeof (error)) == 1);
    CHECK (anomp_engine_load_track (engine, flac.c_str(), nullptr, error, sizeof (error)) == 1);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (22371 / 44100.0));

    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.loaded == 1);
    CHECK (std::string_view (path.codec) == "flac");
    CHECK (path.lossless == 1);
    CHECK (path.bits_per_sample == 16);
    CHECK (path.file_sample_rate == 44100.0);
    CHECK (path.tempo == 1.0);

    double start = 0.0, end = 0.0;
    CHECK (anomp_engine_set_loop (engine, 0.1, 0.2, error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("A loop must"));
    REQUIRE (anomp_engine_set_loop (engine, 0.1, 0.4, error, sizeof (error)) == 1);
    CHECK (anomp_engine_loop (engine, &start, &end) == 1);
    CHECK (start == Catch::Approx (0.1));
    CHECK (end == Catch::Approx (0.4));
    CHECK (anomp_engine_set_loop (engine, -1.0, 0.0, error, sizeof (error)) == 1);
    CHECK (anomp_engine_loop (engine, &start, &end) == 0);

    CHECK (anomp_engine_set_tempo (engine, 0.75, -2.0) == 1);
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.tempo == 0.75);
    CHECK (path.semitones == -2.0);
    CHECK (anomp_engine_set_tempo (engine, 2.0, 0.0) == 0);

    CHECK (anomp_engine_set_crossfeed (engine, ANOMP_CROSSFEED_STRONG) == 1);
    CHECK (anomp_engine_set_crossfeed (engine, 4) == 0);
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.crossfeed == ANOMP_CROSSFEED_STRONG);

    // No device is open.
    CHECK (anomp_engine_output_is_headphones (engine) == -1);
    CHECK (anomp_engine_set_device_sample_rate (engine, 48000.0) == 0);

    anomp_engine_destroy (engine);
}

TEST_CASE ("C API crossfade and equaliser", "[c-api][engine]")
{
    char error[256] = "";
    const double flat[ANOMP_EQ_BANDS] = {};
    CHECK (anomp_engine_set_equaliser (nullptr, flat, 0.0) == 0);
    CHECK (anomp_track_options_default (1.0).crossfade == 0.0);

    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);
    const auto flac = fixturePath ("flac-44k.flac");
    const auto wav = fixturePath ("wav-s16-44k.wav");

    REQUIRE (anomp_engine_load_track (engine, flac.c_str(), nullptr, error, sizeof (error)) == 1);
    auto options = anomp_track_options_default (1.0);
    options.crossfade = 0.1;
    REQUIRE (anomp_engine_set_next_track (engine, wav.c_str(), &options, error, sizeof (error)) == 1);
    anomp_signal_path path {};
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.crossfade == Catch::Approx (0.1));
    CHECK (path.equaliser == 0);

    double gains[ANOMP_EQ_BANDS] = { 3.0, 0.0, 0.0, 0.0, 0.0, -3.0, 0.0, 0.0, 0.0, 12.0 };
    CHECK (anomp_engine_set_equaliser (engine, gains, -6.0) == 1);
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.equaliser == 1);
    gains[0] = 12.5;
    CHECK (anomp_engine_set_equaliser (engine, gains, 0.0) == 0);
    CHECK (anomp_engine_set_equaliser (engine, flat, -13.0) == 0);
    CHECK (anomp_engine_set_equaliser (engine, nullptr, 0.0) == 1);
    REQUIRE (anomp_engine_signal_path (engine, &path) == 1);
    CHECK (path.equaliser == 0);

    anomp_engine_destroy (engine);
}

TEST_CASE ("C API Dock menu reports choices of enabled items", "[c-api][dock]")
{
    CHECK (anomp_dock_menu_perform (nullptr, 1) == 0);
    anomp_dock_menu_set_items (nullptr, nullptr, 0);
    anomp_dock_menu_destroy (nullptr);

    struct Chosen
    {
        std::vector<int> ids;
    } chosen;
    auto* menu =
        anomp_dock_menu_create ([] (int id, void* user) { static_cast<Chosen*> (user)->ids.push_back (id); }, &chosen);
    REQUIRE (menu != nullptr);
    CHECK (anomp_dock_menu_create (nullptr, nullptr) == nullptr); // One at a time.

    const anomp_menu_item items[] = {
        { 1, "Now playing: Song", 0, 0 },
        { 0, "", 1, 0 },
        { 2, "Pause", 1, 0 },
        { 3, "Shuffle", 1, 1 },
    };
    anomp_dock_menu_set_items (menu, items, 4);
    CHECK (anomp_dock_menu_perform (menu, 2) == 1);
    CHECK (anomp_dock_menu_perform (menu, 3) == 1);
    CHECK (anomp_dock_menu_perform (menu, 1) == 0); // Disabled.
    CHECK (anomp_dock_menu_perform (menu, 0) == 0); // A separator.
    CHECK (anomp_dock_menu_perform (menu, 9) == 0);
    CHECK (chosen.ids == std::vector<int> { 2, 3 });
    CHECK ((anomp_dock_menu_supported() == 1) == (JUCE_MAC != 0));

    anomp_dock_menu_destroy (menu);
    auto* again = anomp_dock_menu_create (nullptr, nullptr);
    CHECK (again != nullptr);
    anomp_dock_menu_destroy (again);
}

TEST_CASE ("C API volume watcher reports volumes through its callback", "[c-api][volumes]")
{
    CHECK (anomp_volume_watcher_start (nullptr, nullptr) == nullptr);
    anomp_volume_watcher_notify (nullptr, 1, "/Volumes/Music");
    anomp_volume_watcher_stop (nullptr);

    struct Seen
    {
        std::vector<std::pair<int, std::string>> volumes;
    } seen;
    auto* watcher =
        anomp_volume_watcher_start ([] (int mounted, const char* path, void* user)
                                    { static_cast<Seen*> (user)->volumes.emplace_back (mounted, path); }, &seen);
    REQUIRE (watcher != nullptr);
    anomp_volume_watcher_notify (watcher, 1, "/Volumes/Música");
    anomp_volume_watcher_notify (watcher, 0, nullptr);
    CHECK (seen.volumes == std::vector<std::pair<int, std::string>> { { 1, "/Volumes/Música" }, { 0, "" } });
    CHECK ((anomp_volume_watcher_supported() == 1) == (JUCE_MAC != 0));
    anomp_volume_watcher_stop (watcher);
}

TEST_CASE ("C API log reaches its callback, with JUCE's Logger and failed assertions", "[c-api][log]")
{
    struct Seen
    {
        std::vector<std::pair<int, std::string>> messages;
    } seen;
    anomp_log_write (ANOMP_LOG_ERROR, "dropped: no callback yet");
    anomp_set_log_callback ([] (int level, const char* message, void* user)
                            { static_cast<Seen*> (user)->messages.emplace_back (level, message); }, &seen);
    anomp_log_write (ANOMP_LOG_WARN, "a warning");
    anomp_log_write (99, nullptr);
    juce::Logger::writeToLog ("from JUCE");
    juce::logAssertion ("Some/File.cpp", 42);
    anomp_set_log_callback (nullptr, nullptr);
    anomp_log_write (ANOMP_LOG_ERROR, "dropped again");

    CHECK (seen.messages
           == std::vector<std::pair<int, std::string>> {
               { ANOMP_LOG_WARN, "a warning" },
               { ANOMP_LOG_DEBUG, "" },
               { ANOMP_LOG_INFO, "from JUCE" },
               { ANOMP_LOG_ERROR, "JUCE Assertion failure in File.cpp:42" },
           });
}

namespace
{
std::vector<std::string> loggedAtExit;
}

TEST_CASE ("C API log may stay set when the process exits", "[c-api][log]")
{
    // The host never clears its callback: the core's logger is destroyed
    // with the process's statics, while it's still JUCE's current logger
    // unless it steps down first. Each test runs in its own process, so
    // this one fails by aborting at exit (a pure virtual call).
    anomp_set_log_callback ([] (int, const char* message, void*) { loggedAtExit.emplace_back (message); }, nullptr);
    juce::Logger::writeToLog ("still set");
    CHECK (loggedAtExit == std::vector<std::string> { "still set" });
}

TEST_CASE ("C API engine shuts JUCE down after its timer", "[c-api][engine][log]")
{
    // JUCE's runtime must outlive everything else in the engine, its timer
    // included, or the timer's thread outlives the message manager.
    static std::vector<std::string> logged;
    anomp_set_log_callback ([] (int, const char* message, void*) { logged.emplace_back (message); }, nullptr);
    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);
    anomp_engine_destroy (engine);
    anomp_set_log_callback (nullptr, nullptr);
    for (const auto& message : logged)
        CHECK_FALSE (std::string_view (message).starts_with ("JUCE Assertion failure"));
}

TEST_CASE ("C API loads asynchronously, reporting each request once", "[c-api][engine][async]")
{
#if ! JUCE_MAC
    SKIP ("The test runs the macOS main run loop");
#endif
    char error[256] = "unchanged";
    CHECK (anomp_engine_load_track_async (nullptr, "/a.flac", nullptr, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "Null engine");
    CHECK (anomp_engine_set_next_track_async (nullptr, "/a.flac", nullptr, error, sizeof (error)) == 0);
    anomp_engine_cancel_load (nullptr, 1);

    auto* engine = anomp_engine_create();
    REQUIRE (engine != nullptr);

    struct Finished
    {
        long long request;
        int result;
        std::string error;
    };
    std::vector<Finished> finished;
    anomp_engine_set_event_callback (
        engine,
        [] (const anomp_event* event, void* userData)
        {
            REQUIRE (event->error != nullptr); // Never null, whatever the event.
            if (event->type == ANOMP_EVENT_LOAD_FINISHED)
                static_cast<std::vector<Finished>*> (userData)->push_back (
                    { event->request, event->result, event->error });
        },
        &finished);

    CHECK (anomp_engine_load_track_async (engine, nullptr, nullptr, error, sizeof (error)) == 0);
    CHECK (std::string_view (error) == "Null path");
    CHECK (anomp_engine_load_track_async (engine, "flac-44k.flac", nullptr, error, sizeof (error)) == 0);
    CHECK (std::string_view (error).starts_with ("Path is not absolute"));

    const auto load =
        anomp_engine_load_track_async (engine, fixturePath ("flac-44k.flac").c_str(), nullptr, error, sizeof (error));
    CHECK (load > 0);
    CHECK (std::string_view (error).empty());
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_EMPTY); // Not handed over yet.
    REQUIRE (runEventsUntil ([&] { return ! finished.empty(); }));
    REQUIRE (finished.size() == 1);
    CHECK (finished[0].request == load);
    CHECK (finished[0].result == ANOMP_LOAD_LOADED);
    CHECK (finished[0].error.empty());
    CHECK (anomp_engine_state (engine) == ANOMP_STATE_STOPPED);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (22371 / 44100.0));

    // A next superseded by a load, which fails; then a load cancelled.
    finished.clear();
    const auto next = anomp_engine_set_next_track_async (engine, fixturePath ("wav-s16-44k.wav").c_str(), nullptr,
                                                         error, sizeof (error));
    const auto missing =
        anomp_engine_load_track_async (engine, fixturePath ("missing.flac").c_str(), nullptr, error, sizeof (error));
    const auto cancelled =
        anomp_engine_load_track_async (engine, fixturePath ("wav-s16-44k.wav").c_str(), nullptr, error, sizeof (error));
    anomp_engine_cancel_load (engine, cancelled);
    REQUIRE (runEventsUntil ([&] { return finished.size() == 3; }));
    CHECK (finished[0].request == next);
    CHECK (finished[0].result == ANOMP_LOAD_CANCELLED);
    CHECK (finished[1].request == missing);
    CHECK (finished[1].result == ANOMP_LOAD_CANCELLED); // Superseded by the one cancelled after it.
    CHECK (finished[2].request == cancelled);
    CHECK (finished[2].result == ANOMP_LOAD_CANCELLED);
    CHECK (anomp_engine_duration (engine) == Catch::Approx (22371 / 44100.0)); // Unchanged.

    finished.clear();
    const auto failing =
        anomp_engine_load_track_async (engine, fixturePath ("missing.flac").c_str(), nullptr, error, sizeof (error));
    REQUIRE (runEventsUntil ([&] { return finished.size() == 1; }));
    CHECK (finished[0].request == failing);
    CHECK (finished[0].result == ANOMP_LOAD_FAILED);
    CHECK (std::string_view (finished[0].error).starts_with ("File not found"));

    anomp_engine_destroy (engine);
}
