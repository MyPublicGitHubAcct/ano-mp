#include "anomp/effects/EffectChain.h"

#include "Dsp.h"
#include "Echo.h"
#include "Effect.h"
#include "Modulation.h"
#include "Reverb.h"
#include "SpectralFreeze.h"

#include <algorithm>
#include <cmath>
#include <cstring>

namespace anomp::fx
{
namespace
{
constexpr ParamInfo ratio (const char* id, float defaultValue, float min = 0.0f, float max = 1.0f)
{
    return { id, Unit::ratio, min, max, defaultValue, false };
}

constexpr ParamInfo rateParam (float defaultValue, float min = 0.05f, float max = 5.0f)
{
    return { "rate", Unit::hertz, min, max, defaultValue, true };
}

// Indexed by EffectType; parameters in each effect's Param order.
constexpr std::array<EffectInfo, effectCount> catalogue {
    EffectInfo { EffectType::reverb,
                 "reverb",
                 0.25f,
                 4,
                 { ratio ("size", 0.6f), ratio ("damping", 0.4f), ratio ("width", 1.0f),
                   ParamInfo { "preDelay", Unit::milliseconds, 0.0f, 250.0f, 20.0f, false } } },
    EffectInfo { EffectType::chorus, "chorus", 0.5f, 2, { rateParam (0.8f), ratio ("depth", 0.5f) } },
    EffectInfo { EffectType::freeze,
                 "freeze",
                 1.0f,
                 1,
                 { ParamInfo { "fade", Unit::seconds, 0.05f, 2.0f, 0.3f, true } } },
    EffectInfo { EffectType::echo,
                 "echo",
                 0.3f,
                 4,
                 { ParamInfo { "time", Unit::milliseconds, 20.0f, 2000.0f, 375.0f, true },
                   ratio ("feedback", 0.35f, 0.0f, 0.95f), ratio ("tone", 0.6f), ratio ("spread", 0.5f) } },
    EffectInfo { EffectType::flanger,
                 "flanger",
                 0.5f,
                 3,
                 { rateParam (0.25f), ratio ("depth", 0.7f), ratio ("feedback", 0.5f, -0.95f, 0.95f) } },
    EffectInfo { EffectType::phaser,
                 "phaser",
                 0.5f,
                 3,
                 { rateParam (0.4f), ratio ("depth", 0.8f), ratio ("feedback", 0.4f, 0.0f, 0.9f) } },
    EffectInfo { EffectType::tremolo,
                 "tremolo",
                 1.0f,
                 3,
                 { rateParam (5.0f, 0.1f, 20.0f), ratio ("depth", 0.5f), ratio ("stereo", 0.0f) } },
    EffectInfo { EffectType::lofi,
                 "lofi",
                 1.0f,
                 2,
                 { ParamInfo { "bits", Unit::bits, 2.0f, 16.0f, 8.0f, false },
                   ParamInfo { "sampleRate", Unit::hertz, 1000.0f, 44100.0f, 11025.0f, true } } },
};

constexpr std::size_t indexOf (EffectType type) { return static_cast<std::size_t> (type); }

std::unique_ptr<Effect> create (EffectType type)
{
    switch (type)
    {
        case EffectType::reverb:  return std::make_unique<Reverb>();
        case EffectType::chorus:  return std::make_unique<Chorus>();
        case EffectType::freeze:  return std::make_unique<SpectralFreeze>();
        case EffectType::echo:    return std::make_unique<Echo>();
        case EffectType::flanger: return std::make_unique<Flanger>();
        case EffectType::phaser:  return std::make_unique<Phaser>();
        case EffectType::tremolo: return std::make_unique<Tremolo>();
        case EffectType::lofi:    return std::make_unique<LoFi>();
    }
    return nullptr;
}
} // namespace

const EffectInfo& describe (EffectType type) noexcept { return catalogue[indexOf (type)]; }

bool typeOf (const char* id, EffectType& type) noexcept
{
    if (id == nullptr)
        return false;
    for (const auto& info : catalogue)
    {
        if (std::strcmp (info.id, id) == 0)
        {
            type = info.type;
            return true;
        }
    }
    return false;
}

//==============================================================================
struct EffectChain::Slot
{
    explicit Slot (EffectType type) : effect (create (type)), info (describe (type)), mix (info.defaultMix)
    {
        for (std::size_t i = 0; i < maxParams; ++i)
            params[i].store (info.params[i].defaultValue);
    }

    std::unique_ptr<Effect> effect;
    const EffectInfo& info;

    // Set from any thread.
    std::atomic<bool> enabled { false };
    std::atomic<float> mix;
    std::array<std::atomic<float>, maxParams> params {};

    // The audio thread's.
    std::array<float, maxParams> applied {};
    dsp::Smoothed dryGain, wetGain, sendGain;
    bool running = false, stopping = false;
    double tailLeft = 0.0; // Samples the tail still sounds for, once stopping.
    std::atomic<bool> active { false };
};

static_assert (std::atomic<float>::is_always_lock_free && std::atomic<bool>::is_always_lock_free);

EffectChain::EffectChain()
{
    for (std::size_t i = 0; i < effectCount; ++i)
        slots[i] = std::make_unique<Slot> (static_cast<EffectType> (i));
}

EffectChain::~EffectChain() = default;

void EffectChain::setEnabled (EffectType type, bool enabled) noexcept
{
    slots[indexOf (type)]->enabled.store (enabled);
    if (type == EffectType::freeze && ! enabled)
        freezeHeld.store (false);
}

bool EffectChain::isEnabled (EffectType type) const noexcept { return slots[indexOf (type)]->enabled.load(); }

void EffectChain::setMix (EffectType type, float mix) noexcept
{
    if (! std::isnan (mix))
        slots[indexOf (type)]->mix.store (std::clamp (mix, 0.0f, 1.0f));
}

float EffectChain::getMix (EffectType type) const noexcept { return slots[indexOf (type)]->mix.load(); }

bool EffectChain::setParameter (EffectType type, std::size_t index, float value) noexcept
{
    const auto& info = describe (type);
    if (index >= info.numParams)
        return false;
    if (! std::isnan (value))
    {
        const auto& param = info.params[index];
        slots[indexOf (type)]->params[index].store (std::clamp (value, param.min, param.max));
    }
    return true;
}

float EffectChain::getParameter (EffectType type, std::size_t index) const noexcept
{
    return index < maxParams ? slots[indexOf (type)]->params[index].load() : 0.0f;
}

bool EffectChain::setFreezeHeld (bool held) noexcept
{
    const auto holds = held && isEnabled (EffectType::freeze);
    freezeHeld.store (holds);
    return holds;
}

bool EffectChain::isFreezeHeld() const noexcept { return freezeHeld.load(); }

bool EffectChain::isActive (EffectType type) const noexcept { return slots[indexOf (type)]->active.load(); }

void EffectChain::trackChanged() noexcept { freezeHeld.store (false); }

//==============================================================================
void EffectChain::prepare (double sampleRate)
{
    rate = sampleRate > 0.0 ? sampleRate : 44100.0;
    // The freeze's history is gone: there is nothing left to hold.
    freezeHeld.store (false);
    for (auto& slot : slots)
    {
        slot->effect->prepare (rate);
        for (auto* gain : { &slot->dryGain, &slot->wetGain, &slot->sendGain })
            gain->prepare (rate, glideSeconds);
        slot->running = slot->stopping = false;
        slot->active.store (false);
    }
}

void EffectChain::reset() noexcept
{
    freezeHeld.store (false);
    for (auto& slot : slots)
    {
        slot->effect->reset();
        slot->running = slot->stopping = false;
        slot->active.store (false);
    }
}

void EffectChain::process (float* left, float* right, int numSamples) noexcept
{
    if (rate <= 0.0)
        return;
    const dsp::ScopedFlushDenormals flush;
    for (int done = 0; done < numSamples; done += chunkSize)
        processChunk (left + done, right + done, std::min (chunkSize, numSamples - done));
}

void EffectChain::processChunk (float* left, float* right, int numSamples) noexcept
{
    const auto count = static_cast<std::size_t> (numSamples);
    for (const auto type : chainOrder)
    {
        auto& slot = *slots[indexOf (type)];
        const auto on = slot.enabled.load (std::memory_order_relaxed);
        if (! slot.running)
        {
            if (! on)
                continue;
            // Starting: from silence, with its settings as they are, fading in.
            slot.running = true;
            slot.stopping = false;
            slot.effect->reset();
            for (std::size_t i = 0; i < slot.info.numParams; ++i)
            {
                slot.applied[i] = slot.params[i].load (std::memory_order_relaxed);
                slot.effect->setParameter (i, slot.applied[i], true);
            }
            slot.dryGain.snap (1.0f);
            slot.wetGain.snap (0.0f);
            slot.sendGain.snap (1.0f);
            slot.active.store (true);
        }
        else
        {
            for (std::size_t i = 0; i < slot.info.numParams; ++i)
            {
                const auto value = slot.params[i].load (std::memory_order_relaxed);
                if (! dsp::same (value, slot.applied[i]))
                {
                    slot.applied[i] = value;
                    slot.effect->setParameter (i, value, false);
                }
            }
        }

        if (type == EffectType::freeze)
            slot.effect->setHeld (on && freezeHeld.load (std::memory_order_relaxed));

        if (on)
        {
            const auto mix = slot.mix.load (std::memory_order_relaxed);
            slot.stopping = false;
            slot.sendGain.setTarget (1.0f);
            slot.dryGain.setTarget (1.0f - mix);
            slot.wetGain.setTarget (mix);
        }
        else
        {
            // Stopping: no more input, the dry signal back at full, and the
            // wet one kept until the tail has died away.
            if (! slot.stopping)
            {
                slot.stopping = true;
                slot.tailLeft = slot.effect->tailSeconds() * rate;
                slot.sendGain.setTarget (0.0f);
                slot.dryGain.setTarget (1.0f);
            }
            else
            {
                slot.tailLeft -= numSamples;
            }
            if (slot.tailLeft <= 0.0)
                slot.wetGain.setTarget (0.0f);
        }

        for (std::size_t i = 0; i < count; ++i)
        {
            dry[0][i] = left[i];
            dry[1][i] = right[i];
            const auto send = slot.sendGain.next();
            wet[0][i] = left[i] * send;
            wet[1][i] = right[i] * send;
        }
        slot.effect->process (wet[0].data(), wet[1].data(), numSamples);
        for (std::size_t i = 0; i < count; ++i)
        {
            const auto dryGain = slot.dryGain.next(), wetGain = slot.wetGain.next();
            left[i] = dry[0][i] * dryGain + wet[0][i] * wetGain;
            right[i] = dry[1][i] * dryGain + wet[1][i] * wetGain;
        }

        if (slot.stopping && slot.tailLeft <= 0.0 && ! slot.wetGain.isGliding() && ! slot.dryGain.isGliding())
        {
            slot.running = slot.stopping = false;
            slot.effect->reset();
            slot.active.store (false);
        }
    }
}
} // namespace anomp::fx
