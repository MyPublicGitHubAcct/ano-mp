#pragma once

#include <juce_core/juce_core.h>

#include <functional>
#include <memory>

namespace anomp
{
/** The OS's media controls: what is playing (title, artist, album, artwork,
    a progress bar) and the commands sent back (media keys, Control Center,
    the lock screen).

    This class is platform-neutral: it keeps the published state, checks it,
    and gates the commands that come back. The platform part is a Backend,
    one per OS: MediaControls_apple.mm (MPNowPlayingInfoCenter and
    MPRemoteCommandCenter) and MediaControls_none.cpp. The base Backend does
    nothing, so it is the fallback.

    Main thread only: create, use and destroy it there. Commands reach the
    handler on the main thread, never from inside a call to this class.
*/
class MediaControls
{
public:
    enum class Command
    {
        play,
        pause,
        toggle,
        next,
        previous,
        seek ///< To `positionSeconds` (finite, clamped to the track).
    };

    /** Whether the player has a track, and whether it is playing. */
    enum class Playback
    {
        stopped,
        paused,
        playing
    };

    /** Everything published. The system extrapolates the position from
        `elapsed`, as of when it was published, at rate 1 while playing. */
    struct State
    {
        bool hasTrack = false; ///< False: nothing is published.
        juce::String title, artist, album;
        Playback playback = Playback::stopped;
        double elapsed = 0.0;      ///< Seconds into the track.
        double duration = 0.0;     ///< Seconds; 0 if unknown.
        juce::MemoryBlock artwork; ///< Encoded image (JPEG, PNG, ...); empty if none.
        bool hasNext = false, hasPrevious = false;
    };

    /** What changed in an update, so a backend can skip unchanged parts
        (e.g. decoding the artwork again). */
    enum Change
    {
        trackChanged = 1,
        playbackChanged = 2,
        artworkChanged = 4,
        navigationChanged = 8,
        allChanged = 15
    };

    /** The platform part. The base class is the no-op fallback. */
    struct Backend
    {
        virtual ~Backend() = default;

        /** Starts delivering the OS's commands to `owner.handleCommand`. */
        virtual void attach (MediaControls& owner) { juce::ignoreUnused (owner); }

        /** Stops delivering commands and clears what was published. */
        virtual void detach() {}

        /** Publishes `published`; `changes` is a mask of Change values. */
        virtual void update (const State& published, int changes) { juce::ignoreUnused (published, changes); }

        /** Whether the platform can show `artwork` (an encoded image). */
        virtual bool canShowArtwork (const juce::MemoryBlock& artwork) { return ! artwork.isEmpty(); }
    };

    using CommandHandler = std::function<void (Command, double positionSeconds)>;

    /** The platform's backend: the Apple one on macOS and iOS, else the fallback. */
    static std::unique_ptr<Backend> createPlatformBackend();

    /** Whether createPlatformBackend() talks to the OS (false for the fallback). */
    static bool isSupported();

    explicit MediaControls (CommandHandler handler, std::unique_ptr<Backend> backend = createPlatformBackend());

    /** Clears what was published and stops receiving commands. */
    ~MediaControls();

    /** Publishes a new track. The artwork, playback and navigation stay as
        they are: set them too if they changed. */
    void setTrack (const juce::String& title, const juce::String& artist, const juce::String& album);

    /** Publishes the playback state and the position (seconds into the
        track) at this moment. `elapsed` is clamped to [0, duration] (or to
        0 and up when the duration is 0, i.e. unknown). Returns false, and
        changes nothing, if either value is not finite or the duration is
        negative. */
    bool setPlayback (Playback playback, double elapsed, double duration);

    /** Publishes the artwork, an encoded image; empty clears it. Returns
        false if the platform cannot show it, in which case the artwork is
        cleared. */
    bool setArtwork (juce::MemoryBlock artwork);

    /** Enables or disables the next and previous commands. */
    void setNavigation (bool hasNext, bool hasPrevious);

    /** Clears everything published (e.g. the queue was emptied). */
    void clear();

    const State& getState() const noexcept { return state; }

    /** Called by the backend for each command the OS sends. Next and
        previous are dropped while disabled, and a seek to a position that
        is not finite is dropped; a seek is clamped to the track. Returns
        true if the handler was called. */
    bool handleCommand (Command command, double positionSeconds = 0.0);

private:
    void publish (int changes);

    CommandHandler handler;
    std::unique_ptr<Backend> backend;
    State state;

    JUCE_DECLARE_NON_COPYABLE (MediaControls)
};
} // namespace anomp
