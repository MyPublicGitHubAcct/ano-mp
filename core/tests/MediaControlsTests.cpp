#include <catch2/catch_approx.hpp>
#include <catch2/catch_test_macros.hpp>

#include "anomp/anomp.h"
#include "MediaControls.h"

#include <cmath>
#include <limits>
#include <string>
#include <utility>
#include <vector>

// Most of these tests use a recording backend, so the suite never publishes
// to the machine's real Now Playing (which would take over its media keys).
// The C API tests use the platform backend but publish no track.

namespace
{
using Command = anomp::MediaControls::Command;
using Playback = anomp::MediaControls::Playback;

struct Recorded
{
    bool attached = false, detached = false;
    std::vector<std::pair<anomp::MediaControls::State, int>> updates;
};

/** Records what it is asked to publish; shows any artwork that starts with "img". */
struct RecordingBackend final : anomp::MediaControls::Backend
{
    explicit RecordingBackend (Recorded& r) : recorded (r) {}

    void attach (anomp::MediaControls&) override { recorded.attached = true; }
    void detach() override { recorded.detached = true; }
    void update (const anomp::MediaControls::State& state, int changes) override
    {
        recorded.updates.emplace_back (state, changes);
    }
    bool canShowArtwork (const juce::MemoryBlock& artwork) override
    {
        return artwork.getSize() >= 3 && std::memcmp (artwork.getData(), "img", 3) == 0;
    }

    Recorded& recorded;
};

struct Received
{
    std::vector<std::pair<Command, double>> commands;

    anomp::MediaControls::CommandHandler handler()
    {
        return [this] (Command command, double position)
        {
            commands.emplace_back (command, position);
        };
    }
};

juce::MemoryBlock bytes (const char* text) { return juce::MemoryBlock (text, std::strlen (text)); }

std::string fixturePath (const char* name) { return std::string (ANOMP_TEST_FIXTURES_DIR) + "/" + name; }
} // namespace

TEST_CASE ("MediaControls publishes each change with what changed", "[media]")
{
    Recorded recorded;
    {
        anomp::MediaControls controls ({}, std::make_unique<RecordingBackend> (recorded));
        CHECK (recorded.attached);
        CHECK_FALSE (controls.getState().hasTrack);

        controls.setTrack (juce::String::fromUTF8 ("Café Déjà Vu"), "Artist", juce::String::fromUTF8 ("東京 Sessions"));
        REQUIRE (recorded.updates.size() == 1);
        const auto& [track, trackChanges] = recorded.updates.back();
        CHECK (trackChanges == anomp::MediaControls::trackChanged);
        CHECK (track.hasTrack);
        CHECK (track.title == juce::String::fromUTF8 ("Café Déjà Vu"));
        CHECK (track.album == juce::String::fromUTF8 ("東京 Sessions"));

        CHECK (controls.setPlayback (Playback::playing, 12.5, 200.0));
        CHECK (recorded.updates.back().second == anomp::MediaControls::playbackChanged);
        CHECK (recorded.updates.back().first.playback == Playback::playing);
        CHECK (recorded.updates.back().first.elapsed == 12.5);
        CHECK (recorded.updates.back().first.duration == 200.0);

        CHECK (controls.setArtwork (bytes ("img1")));
        CHECK (recorded.updates.back().second == anomp::MediaControls::artworkChanged);

        controls.setNavigation (true, false);
        CHECK (recorded.updates.back().second == anomp::MediaControls::navigationChanged);
        CHECK (recorded.updates.back().first.hasNext);
        CHECK_FALSE (recorded.updates.back().first.hasPrevious);

        // A new track keeps the artwork, playback and navigation until they are set.
        controls.setTrack ("Next", {}, {});
        const auto& next = recorded.updates.back().first;
        CHECK (next.title == "Next");
        CHECK (next.artist.isEmpty());
        CHECK (next.artwork == bytes ("img1"));
        CHECK (next.elapsed == 12.5);
        CHECK (next.hasNext);

        controls.clear();
        CHECK (recorded.updates.back().second == anomp::MediaControls::allChanged);
        CHECK_FALSE (recorded.updates.back().first.hasTrack);
        CHECK (recorded.updates.back().first.artwork.isEmpty());
        CHECK_FALSE (recorded.updates.back().first.hasNext);
        CHECK_FALSE (recorded.detached);
    }
    CHECK (recorded.detached);
}

TEST_CASE ("MediaControls checks playback values", "[media]")
{
    Recorded recorded;
    anomp::MediaControls controls ({}, std::make_unique<RecordingBackend> (recorded));
    const auto nan = std::numeric_limits<double>::quiet_NaN();
    const auto inf = std::numeric_limits<double>::infinity();

    CHECK_FALSE (controls.setPlayback (Playback::playing, nan, 10.0));
    CHECK_FALSE (controls.setPlayback (Playback::playing, 1.0, inf));
    CHECK_FALSE (controls.setPlayback (Playback::playing, 1.0, -1.0));
    CHECK (recorded.updates.empty());

    CHECK (controls.setPlayback (Playback::paused, 15.0, 10.0));
    CHECK (controls.getState().elapsed == 10.0);
    CHECK (controls.setPlayback (Playback::paused, -3.0, 10.0));
    CHECK (controls.getState().elapsed == 0.0);
    // An unknown duration (0) only bounds the position below.
    CHECK (controls.setPlayback (Playback::playing, 42.0, 0.0));
    CHECK (controls.getState().elapsed == 42.0);
    CHECK (recorded.updates.size() == 3);
}

TEST_CASE ("MediaControls clears artwork the platform cannot show", "[media]")
{
    Recorded recorded;
    anomp::MediaControls controls ({}, std::make_unique<RecordingBackend> (recorded));

    CHECK (controls.setArtwork (bytes ("img")));
    CHECK (controls.getState().artwork == bytes ("img"));
    CHECK_FALSE (controls.setArtwork (bytes ("not an image")));
    CHECK (controls.getState().artwork.isEmpty());
    CHECK (recorded.updates.back().second == anomp::MediaControls::artworkChanged);
    CHECK (controls.setArtwork ({}));
    CHECK (controls.getState().artwork.isEmpty());
}

TEST_CASE ("MediaControls gates commands", "[media]")
{
    Received received;
    Recorded recorded;
    anomp::MediaControls controls (received.handler(), std::make_unique<RecordingBackend> (recorded));

    CHECK (controls.handleCommand (Command::play));
    CHECK (controls.handleCommand (Command::pause));
    CHECK (controls.handleCommand (Command::toggle));

    // Next and previous start disabled.
    CHECK_FALSE (controls.handleCommand (Command::next));
    CHECK_FALSE (controls.handleCommand (Command::previous));
    controls.setNavigation (true, false);
    CHECK (controls.handleCommand (Command::next));
    CHECK_FALSE (controls.handleCommand (Command::previous));
    controls.setNavigation (false, true);
    CHECK_FALSE (controls.handleCommand (Command::next));
    CHECK (controls.handleCommand (Command::previous));

    // Seeks must be finite and are clamped to the track.
    CHECK_FALSE (controls.handleCommand (Command::seek, std::numeric_limits<double>::quiet_NaN()));
    CHECK (controls.handleCommand (Command::seek, 30.0));
    CHECK (controls.setPlayback (Playback::playing, 0.0, 20.0));
    CHECK (controls.handleCommand (Command::seek, 30.0));
    CHECK (controls.handleCommand (Command::seek, -1.0));

    const std::vector<std::pair<Command, double>> expected {
        { Command::play, 0.0 },     { Command::pause, 0.0 }, { Command::toggle, 0.0 }, { Command::next, 0.0 },
        { Command::previous, 0.0 }, { Command::seek, 30.0 }, { Command::seek, 20.0 },  { Command::seek, 0.0 },
    };
    CHECK (received.commands == expected);

    // Only a seek carries a position.
    CHECK (controls.handleCommand (Command::play, 5.0));
    CHECK (received.commands.back() == std::pair (Command::play, 0.0));

    anomp::MediaControls silent ({}, std::make_unique<RecordingBackend> (recorded));
    CHECK_FALSE (silent.handleCommand (Command::play));
}

TEST_CASE ("MediaControls fallback accepts everything and still routes commands", "[media]")
{
    Received received;
    anomp::MediaControls controls (received.handler(), std::make_unique<anomp::MediaControls::Backend>());

    controls.setTrack ("Title", "Artist", "Album");
    CHECK (controls.setPlayback (Playback::playing, 1.0, 2.0));
    CHECK (controls.setArtwork (bytes ("any bytes")));
    controls.setNavigation (true, true);
    CHECK (controls.handleCommand (Command::next));
    controls.clear();
    CHECK (received.commands.size() == 1);

    // A null backend means the fallback.
    anomp::MediaControls defaulted (received.handler(), nullptr);
    CHECK (defaulted.handleCommand (Command::toggle));
}

//==============================================================================
namespace
{
struct CommandLog
{
    std::vector<anomp_media_command> commands;

    static void callback (const anomp_media_command* command, void* userData)
    {
        static_cast<CommandLog*> (userData)->commands.push_back (*command);
    }
};

struct ControlsDeleter
{
    void operator() (anomp_media_controls* controls) const { anomp_media_controls_destroy (controls); }
};
using ControlsPtr = std::unique_ptr<anomp_media_controls, ControlsDeleter>;
} // namespace

TEST_CASE ("C API media controls accept a null handle", "[c-api][media]")
{
    const anomp_media_track track { "Title", nullptr, nullptr };
    const anomp_media_command play { ANOMP_MEDIA_PLAY, 0.0 };
    const unsigned char image[] = { 1, 2, 3 };

    anomp_media_controls_destroy (nullptr);
    CHECK (anomp_media_controls_set_track (nullptr, &track) == 0);
    CHECK (anomp_media_controls_set_playback (nullptr, ANOMP_STATE_PLAYING, 0.0, 1.0) == 0);
    CHECK (anomp_media_controls_set_artwork (nullptr, image, sizeof (image)) == 0);
    anomp_media_controls_set_navigation (nullptr, 1, 1);
    anomp_media_controls_clear (nullptr);
    CHECK (anomp_media_controls_perform (nullptr, &play) == 0);
}

TEST_CASE ("C API media controls report platform support", "[c-api][media]")
{
#if JUCE_MAC || JUCE_IOS
    CHECK (anomp_media_controls_supported() == 1);
#else
    CHECK (anomp_media_controls_supported() == 0);
#endif
}

TEST_CASE ("C API media commands reach the callback", "[c-api][media]")
{
    CommandLog log;
    ControlsPtr controls (anomp_media_controls_create (&CommandLog::callback, &log));
    REQUIRE (controls != nullptr);

    const auto perform = [&] (int type, double position = 0.0)
    {
        const anomp_media_command command { type, position };
        return anomp_media_controls_perform (controls.get(), &command);
    };

    CHECK (perform (ANOMP_MEDIA_PLAY) == 1);
    CHECK (perform (ANOMP_MEDIA_PAUSE) == 1);
    CHECK (perform (ANOMP_MEDIA_TOGGLE) == 1);
    CHECK (perform (ANOMP_MEDIA_NEXT) == 0);
    CHECK (perform (ANOMP_MEDIA_PREVIOUS) == 0);
    anomp_media_controls_set_navigation (controls.get(), 1, 7);
    CHECK (perform (ANOMP_MEDIA_NEXT) == 1);
    CHECK (perform (ANOMP_MEDIA_PREVIOUS) == 1);
    CHECK (perform (ANOMP_MEDIA_SEEK, 12.5) == 1);
    CHECK (perform (ANOMP_MEDIA_SEEK, -4.0) == 1);
    CHECK (perform (ANOMP_MEDIA_SEEK, std::numeric_limits<double>::infinity()) == 0);
    CHECK (perform (0) == 0);
    CHECK (perform (99) == 0);
    CHECK (anomp_media_controls_perform (controls.get(), nullptr) == 0);

    const std::vector<std::pair<int, double>> expected {
        { ANOMP_MEDIA_PLAY, 0.0 }, { ANOMP_MEDIA_PAUSE, 0.0 },    { ANOMP_MEDIA_TOGGLE, 0.0 },
        { ANOMP_MEDIA_NEXT, 0.0 }, { ANOMP_MEDIA_PREVIOUS, 0.0 }, { ANOMP_MEDIA_SEEK, 12.5 },
        { ANOMP_MEDIA_SEEK, 0.0 },
    };
    std::vector<std::pair<int, double>> received;
    for (const auto& command : log.commands)
        received.emplace_back (command.type, command.position);
    CHECK (received == expected);

    ControlsPtr silent (anomp_media_controls_create (nullptr, nullptr));
    REQUIRE (silent != nullptr);
    const anomp_media_command play { ANOMP_MEDIA_PLAY, 0.0 };
    CHECK (anomp_media_controls_perform (silent.get(), &play) == 0);
}

TEST_CASE ("C API media controls check their arguments", "[c-api][media]")
{
    // No track is published here: that would show on this machine's real
    // Now Playing. These calls fail, or change only the artwork, first.
    ControlsPtr controls (anomp_media_controls_create (nullptr, nullptr));
    REQUIRE (controls != nullptr);

    CHECK (anomp_media_controls_set_track (controls.get(), nullptr) == 0);
    CHECK (anomp_media_controls_set_playback (controls.get(), -1, 0.0, 1.0) == 0);
    CHECK (anomp_media_controls_set_playback (controls.get(), 4, 0.0, 1.0) == 0);
    CHECK (anomp_media_controls_set_playback (controls.get(), ANOMP_STATE_PLAYING, std::nan (""), 1.0) == 0);
    CHECK (anomp_media_controls_set_playback (controls.get(), ANOMP_STATE_PAUSED, 0.0, -1.0) == 0);
    CHECK (anomp_media_controls_set_artwork (controls.get(), nullptr, 4) == 0);
    CHECK (anomp_media_controls_set_artwork (controls.get(), nullptr, 0) == 1);
}

TEST_CASE ("C API media controls decode artwork", "[c-api][media]")
{
    // The tagged fixture's embedded cover is a PNG.
    char error[256] = "";
    auto* tags =
        anomp_read_tags (fixturePath ("tagged-vorbis.flac").c_str(), ANOMP_TAGS_PICTURE, error, sizeof (error));
    REQUIRE (tags != nullptr);
    REQUIRE (tags->picture != nullptr);

    ControlsPtr controls (anomp_media_controls_create (nullptr, nullptr));
    REQUIRE (controls != nullptr);
    CHECK (anomp_media_controls_set_artwork (controls.get(), tags->picture, tags->picture_size) == 1);

    if (anomp_media_controls_supported() == 1)
    {
        const unsigned char notAnImage[] = { 'n', 'o', 'p', 'e' };
        CHECK (anomp_media_controls_set_artwork (controls.get(), notAnImage, sizeof (notAnImage)) == 0);
    }

    anomp_tags_free (tags);
}
