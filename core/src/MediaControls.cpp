#include "MediaControls.h"

#include <cmath>

namespace anomp
{
MediaControls::MediaControls (CommandHandler handlerIn, std::unique_ptr<Backend> backendIn)
    : handler (std::move (handlerIn)),
      backend (backendIn != nullptr ? std::move (backendIn) : std::make_unique<Backend>())
{
    backend->attach (*this);
}

MediaControls::~MediaControls() { backend->detach(); }

void MediaControls::setTrack (const juce::String& title, const juce::String& artist, const juce::String& album)
{
    state.hasTrack = true;
    state.title = title;
    state.artist = artist;
    state.album = album;
    publish (trackChanged);
}

bool MediaControls::setPlayback (Playback playback, double elapsed, double duration)
{
    if (! std::isfinite (elapsed) || ! std::isfinite (duration) || duration < 0.0)
        return false;

    state.playback = playback;
    state.duration = duration;
    state.elapsed = duration > 0.0 ? juce::jlimit (0.0, duration, elapsed) : std::max (0.0, elapsed);
    publish (playbackChanged);
    return true;
}

bool MediaControls::setArtwork (juce::MemoryBlock artwork)
{
    const auto shown = artwork.isEmpty() || backend->canShowArtwork (artwork);

    if (! shown)
        artwork.reset();

    state.artwork = std::move (artwork);
    publish (artworkChanged);
    return shown;
}

void MediaControls::setNavigation (bool hasNext, bool hasPrevious)
{
    state.hasNext = hasNext;
    state.hasPrevious = hasPrevious;
    publish (navigationChanged);
}

void MediaControls::clear()
{
    state = {};
    publish (allChanged);
}

bool MediaControls::handleCommand (Command command, double positionSeconds)
{
    if (handler == nullptr)
        return false;

    switch (command)
    {
        case Command::next:
            if (! state.hasNext)
                return false;
            break;
        case Command::previous:
            if (! state.hasPrevious)
                return false;
            break;
        case Command::seek:
            if (! std::isfinite (positionSeconds))
                return false;
            positionSeconds = state.duration > 0.0 ? juce::jlimit (0.0, state.duration, positionSeconds)
                                                   : std::max (0.0, positionSeconds);
            break;
        case Command::play:
        case Command::pause:
        case Command::toggle: break;
    }

    handler (command, command == Command::seek ? positionSeconds : 0.0);
    return true;
}

void MediaControls::publish (int changes) { backend->update (state, changes); }
} // namespace anomp
