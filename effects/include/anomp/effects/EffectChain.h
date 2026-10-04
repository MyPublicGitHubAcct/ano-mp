#pragma once

#include <array>
#include <atomic>
#include <cstddef>
#include <memory>

/** ano-mp's real-time effects (PLAN.md X2): a library of its own, in plain
    C++20 with no dependencies, so it builds, tests and benchmarks alone and
    ports with nothing else. The core (`anomp_core`) runs one EffectChain on
    what it plays; nothing else in the core knows the effects.

    This header is the library's only public surface: the catalogue (what
    each effect is called and which parameters it takes, with their ranges)
    and the chain that runs them. */
namespace anomp::fx
{
/** Every effect. The values are stable (the C API passes them); the order
    they run in is `chainOrder`. */
enum class EffectType
{
    reverb = 0,
    chorus,
    freeze,
    echo,
    flanger,
    phaser,
    tremolo,
    lofi
};

inline constexpr std::size_t effectCount = 8;
inline constexpr std::size_t maxParams = 4;

/** The order effects run in, fixed: the spectral freeze sees the music as
    it is, the colouring and modulation come next, and the echo and reverb
    last, so their tails carry everything before them. */
inline constexpr std::array<EffectType, effectCount> chainOrder { EffectType::freeze,  EffectType::lofi,
                                                                  EffectType::tremolo, EffectType::phaser,
                                                                  EffectType::flanger, EffectType::chorus,
                                                                  EffectType::echo,    EffectType::reverb };

/** What a parameter's number means, for showing it. */
enum class Unit
{
    ratio,        ///< 0 to 1 (or -1 to 1), shown as a percentage.
    hertz,        ///< Cycles a second.
    milliseconds, ///< Thousandths of a second.
    seconds,      ///< Seconds.
    bits          ///< Bits of resolution.
};

struct ParamInfo
{
    const char* id = "";
    Unit unit = Unit::ratio;
    float min = 0.0f, max = 1.0f, defaultValue = 0.0f;
    /** Best moved along a logarithmic scale (rates, times, frequencies). */
    bool logarithmic = false;
};

struct EffectInfo
{
    EffectType type = EffectType::reverb;
    /** Stable, lower camel case ("reverb", "lofi"): the settings' key. */
    const char* id = "";
    /** The wet/dry mix it starts with, 0 (dry) to 1 (wet only). */
    float defaultMix = 0.5f;
    std::size_t numParams = 0;
    std::array<ParamInfo, maxParams> params {};
};

/** The catalogue entry for `type`. */
const EffectInfo& describe (EffectType type) noexcept;

/** The type whose id is `id`; false if there is none. */
bool typeOf (const char* id, EffectType& type) noexcept;

class Effect;

/** The effects, run in `chainOrder` over a stereo signal in place.

    Each effect has a switch, a wet/dry mix and its parameters (see
    `describe`). They are set from any thread without locks: each value is
    an atomic the audio thread reads once per short chunk (`chunkSize`
    samples), and the effects smooth every change, so moving a control
    while music plays never clicks. An effect switched on fades in; one
    switched off stops taking input and fades out after its tail (a
    reverb's or echo's) has died away, then stops costing anything. With
    every effect off, `process` leaves the signal untouched.

    The spectral freeze holds the sound while `setFreezeHeld (true)`;
    `trackChanged` (a new track taking over) lets it go, while other tails
    carry on into the new track. */
class EffectChain
{
public:
    /** Samples processed between reads of the settings. */
    static constexpr int chunkSize = 64;

    /** Seconds over which the mix and the switches glide. */
    static constexpr double glideSeconds = 0.05;

    EffectChain();
    ~EffectChain();

    EffectChain (const EffectChain&) = delete;
    EffectChain& operator= (const EffectChain&) = delete;

    //==============================================================================
    // Any thread, lock-free.

    void setEnabled (EffectType type, bool enabled) noexcept;
    bool isEnabled (EffectType type) const noexcept;

    /** The wet/dry mix, clamped to 0..1. */
    void setMix (EffectType type, float mix) noexcept;
    float getMix (EffectType type) const noexcept;

    /** Parameter `index` of `type` (see `describe`), clamped to its range;
        NaN leaves it as it was. Returns false for an unknown index. */
    bool setParameter (EffectType type, std::size_t index, float value) noexcept;
    float getParameter (EffectType type, std::size_t index) const noexcept;

    /** Holds (or lets go of) the spectral freeze. Holding needs the freeze
        to be on; returns whether it holds afterwards. */
    bool setFreezeHeld (bool held) noexcept;
    bool isFreezeHeld() const noexcept;

    /** Whether `type` is processing: on, or still fading out. As the audio
        thread last saw it. */
    bool isActive (EffectType type) const noexcept;

    //==============================================================================
    // The audio thread (or whoever owns the chain while it isn't running).

    /** Allocates for `sampleRate` and clears every effect, letting go of
        the freeze. Not real-time safe. */
    void prepare (double sampleRate);

    /** Clears every effect's state (delay lines, tails, a held freeze's
        sound, letting go of it), keeping the settings. */
    void reset() noexcept;

    /** Processes `numSamples` stereo samples in place. Real-time safe. */
    void process (float* left, float* right, int numSamples) noexcept;

    /** Another track has taken over: lets go of the freeze. */
    void trackChanged() noexcept;

private:
    struct Slot;

    void processChunk (float* left, float* right, int numSamples) noexcept;

    std::array<std::unique_ptr<Slot>, effectCount> slots;
    // One chunk of the signal as it came in, and of each effect's input.
    std::array<std::array<float, chunkSize>, 2> dry {}, wet {};
    std::atomic<bool> freezeHeld { false };
    double rate = 0.0;
};
} // namespace anomp::fx
