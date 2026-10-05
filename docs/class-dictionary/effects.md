# The effects' types (C++)

Every class, struct and enum of the effects library (`effects/`,
`anomp_effects`), in alphabetical order. Its public types are in
`anomp::fx`, declared in its only public header,
`effects/include/anomp/effects/EffectChain.h`; the rest are private to the
library, the shared building blocks in `anomp::fx::dsp`. Everything here
is plain C++20 with no dependencies. The developer guide's [chapter
5](../developer-guide/05-effects.md) explains the chain, how settings reach
the audio thread, and the bit-identical rule.

Back to the [index](README.md).

### `Chorus`

Class · [`effects/src/Modulation.h`](../../effects/src/Modulation.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Two voices a channel, each a copy delayed by 7 to 25 ms as an LFO moves
it, so the copies drift in pitch against the original. Parameters: rate
and depth.

### `Chorus::Param`

Helper · [`effects/src/Modulation.h`](../../effects/src/Modulation.h)

The chorus's parameter indices, in catalogue order.

### `DelayLine`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

A circular buffer of past samples, read at whole or fractional delays
(four-point Hermite interpolation). The chorus, flanger, echo and reverb
are built on it.

### `Echo`

Class · [`effects/src/Echo.h`](../../effects/src/Echo.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Repeats, each darker than the last (a lowpass in the feedback, as on
tape), from both channels at once or bouncing between them. A new time
crossfades the repeats over 50 ms rather than gliding, which would bend
the pitch. Parameters: time, feedback, tone and spread.

### `Echo::Param`

Helper · [`effects/src/Echo.h`](../../effects/src/Echo.h)

The echo's parameter indices, in catalogue order.

### `Effect`

Class · [`effects/src/Effect.h`](../../effects/src/Effect.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

The interface each effect implements: `prepare` (allocates), `reset`,
`setParameter` (gliding, or snapping as it starts), `process` (turns a
chunk of at most `EffectChain::chunkSize` samples into its wet signal in
place), `tailSeconds` and, for the freeze, `setHeld`. The chain mixes the
wet signal with the dry. Owned by the chain's `EffectChain::Slot`s.

### `EffectChain`

Class · [`effects/include/anomp/effects/EffectChain.h`](../../effects/include/anomp/effects/EffectChain.h) · settings from any thread, processing on the audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Runs the effects in `chainOrder` over a stereo signal in place. Each
effect's switch, mix and parameters are atomics the audio thread reads
once a 64-sample chunk, and every change glides; an effect switched off
fades out after its tail, then costs nothing, and with every effect off
the signal is untouched. Holds the spectral freeze, which a new track lets
go. Owned by the core's `PlayerEngine`.

### `EffectChain::Slot`

Struct · [`effects/src/EffectChain.cpp`](../../effects/src/EffectChain.cpp) · [D2: How settings reach the audio thread](../developer-guide/05-effects.md#how-settings-reach-the-audio-thread)

One effect in the chain: the `Effect`, its catalogue entry, the atomics
any thread sets (switch, mix, parameters), and what the audio thread
last applied, with its fade state.

### `EffectInfo`

Struct · [`effects/include/anomp/effects/EffectChain.h`](../../effects/include/anomp/effects/EffectChain.h) · any thread · [D2: Adding an effect](../developer-guide/05-effects.md#adding-an-effect)

An effect's catalogue entry, returned by `describe`: its type, stable id
(the settings' key), default mix and parameters. `anomp_effect_info` in
the C API.

### `EffectType`

Enum · [`effects/include/anomp/effects/EffectChain.h`](../../effects/include/anomp/effects/EffectChain.h) · [D2: Adding an effect](../developer-guide/05-effects.md#adding-an-effect)

Every effect, with stable values that the C API passes as
`ANOMP_EFFECT_*`: a new one goes at the end. The order they run in is
`chainOrder`, not this.

### `Fft`

Class · [`effects/src/Fft.h`](../../effects/src/Fft.h) · audio thread

An in-place complex FFT of a power-of-two size (iterative radix 2) on
separate real and imaginary arrays; `prepare` makes the tables, and the
transforms allocate nothing. The spectral freeze's.

### `Flanger`

Class · [`effects/src/Modulation.h`](../../effects/src/Modulation.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

A copy delayed by 0.3 to 6 ms as an LFO sweeps it, fed back, so a comb of
notches sweeps through the spectrum. Parameters: rate, depth and
feedback.

### `Flanger::Param`

Helper · [`effects/src/Modulation.h`](../../effects/src/Modulation.h)

The flanger's parameter indices, in catalogue order.

### `Lfo`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread

A low-frequency oscillator: a phase from 0 to 1 at a rate in hertz, read
as a raised sine at an offset, so each channel can run apart.

### `LoFi`

Class · [`effects/src/Echo.h`](../../effects/src/Echo.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Fewer bits and a lower sample rate: each sample rounded to a number of
bits and held until the next tick of a slower clock, aliasing as 1980s
samplers did. Parameters: bits and sample rate.

### `LoFi::Param`

Helper · [`effects/src/Echo.h`](../../effects/src/Echo.h)

The lo-fi effect's parameter indices, in catalogue order.

### `OnePole`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread

A one-pole lowpass: the echo's tone, darkening each repeat.

### `ParamInfo`

Struct · [`effects/include/anomp/effects/EffectChain.h`](../../effects/include/anomp/effects/EffectChain.h) · any thread · [D2: Adding an effect](../developer-guide/05-effects.md#adding-an-effect)

One parameter in the catalogue: its id, `Unit`, range, default and whether
it is best moved on a logarithmic scale. `anomp_effect_param` in the C
API; its id names `effects.param.<id>` in `en.json`.

### `Phaser`

Class · [`effects/src/Modulation.h`](../../effects/src/Modulation.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Six first-order allpass stages a channel whose corner sweeps from 150 Hz
towards 5 kHz, fed back, so notches move through the spectrum. Parameters:
rate, depth and feedback.

### `Phaser::Channel`

Helper · [`effects/src/Modulation.h`](../../effects/src/Modulation.h)

One channel's allpass stages' state and its last output, for the
feedback.

### `Phaser::Param`

Helper · [`effects/src/Modulation.h`](../../effects/src/Modulation.h)

The phaser's parameter indices, in catalogue order.

### `Random`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread

Fast, repeatable noise (xorshift32), as the spectral freeze's new phases
use.

### `Reverb`

Class · [`effects/src/Reverb.h`](../../effects/src/Reverb.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

A room: Jezar's Freeverb (public domain), eight damped feedback combs and
four allpasses a channel, tuned to the sample rate, after a pre-delay.
Parameters: size, damping, width and pre-delay. Its tail decides how long
the chain keeps it running after it is switched off.

### `Reverb::Allpass`

Helper · [`effects/src/Reverb.h`](../../effects/src/Reverb.h)

One of the reverb's allpass filters: a delay line and its length.

### `Reverb::Channel`

Helper · [`effects/src/Reverb.h`](../../effects/src/Reverb.h)

One channel's combs and allpasses.

### `Reverb::Comb`

Helper · [`effects/src/Reverb.h`](../../effects/src/Reverb.h)

One of the reverb's damped feedback combs: a delay line, its length and
its filter's state.

### `Reverb::Param`

Helper · [`effects/src/Reverb.h`](../../effects/src/Reverb.h)

The reverb's parameter indices, in catalogue order.

### `ScopedFlushDenormals`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread

Flushes denormal numbers to zero while it exists, so a feedback path
decaying into silence never slows the audio thread. The chain holds one
around its processing.

### `Smoothed`

Class · [`effects/src/Dsp.h`](../../effects/src/Dsp.h) · audio thread · [D2: How settings reach the audio thread](../developer-guide/05-effects.md#how-settings-reach-the-audio-thread)

A value that moves linearly to its target over a set number of samples:
how every parameter and mix change glides instead of clicking.

### `SpectralFreeze`

Class · [`effects/src/SpectralFreeze.h`](../../effects/src/SpectralFreeze.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

Holds the sound of a moment while held: keeps the spectrum of the last
~0.15 s and resynthesises it by overlap-add with the same magnitudes and
new random phases, keeping the stereo image. Not held, it passes the music
through untouched. Holding and letting go crossfade over `fade` seconds,
its one parameter.

### `SpectralFreeze::Param`

Helper · [`effects/src/SpectralFreeze.h`](../../effects/src/SpectralFreeze.h)

The freeze's parameter index (fade).

### `Tremolo`

Class · [`effects/src/Modulation.h`](../../effects/src/Modulation.h) · audio thread · [D2: The chain](../developer-guide/05-effects.md#the-chain)

The level (tremolo) or the position (auto-pan) moving with an LFO.
Parameters: rate, depth and stereo (0 moves both channels together, 1 in
opposition).

### `Tremolo::Param`

Helper · [`effects/src/Modulation.h`](../../effects/src/Modulation.h)

The tremolo's parameter indices, in catalogue order.

### `Unit`

Enum · [`effects/include/anomp/effects/EffectChain.h`](../../effects/include/anomp/effects/EffectChain.h) · [D2: On the Rust and UI side](../developer-guide/05-effects.md#on-the-rust-and-ui-side)

What a parameter's number means, for showing it: a ratio (as a
percentage), hertz, milliseconds, seconds or bits.
`ANOMP_EFFECT_UNIT_*` in the C API.
