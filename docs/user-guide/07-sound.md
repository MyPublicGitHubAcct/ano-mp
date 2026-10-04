# 7. Sound

ano-mp plays your music as it is unless you ask otherwise. This chapter
covers the tools that change what you hear (the equaliser, headphone
crossfeed, effects and practice mode), what each does and when to use it,
and the **Signal path**, which shows exactly what happens to the sound on
its way to your speakers.

Volume levelling and crossfades are in [chapter 3](03-playing-music.md).

## The order things happen in

From the file to your speakers, the sound goes through:

1. the file, decoded;
2. the gain: ReplayGain and your gain offsets
   ([chapter 3](03-playing-music.md#volume-levelling));
3. practice mode's speed and pitch, when it's on;
4. resampling, if the file's sample rate differs from the output's;
5. the effects;
6. the equaliser;
7. headphone crossfeed;
8. a crossfade into the next track;
9. a recording, if one is running ([chapter 9](09-recording.md));
10. the volume;
11. the output device.

With all of these off, the music reaches the device exactly as decoded.

## The equaliser

Open **Settings › Equaliser**.

<!-- Screenshot: Settings › Equaliser with a preset chosen. -->

1. Turn on **Equaliser**.
2. Choose a **Preset**: **Flat** (no change), **Bass boost**, **Less
   bass**, **Treble boost**, **Less treble**, **Vocal**, **Loudness (quiet
   listening)**, **Rock**, **Classical** or **Headphones**.
3. Or drag the ten **Bands**, from 31 Hz (deep bass) to 16 kHz (air), each
   up to 12 dB up or down. The preset becomes **Custom**.
4. **Preamp** lowers everything before the bands. Presets set it so their
   boosts don't distort; if you boost bands yourself, lower it by as much
   as your biggest boost.

Changes are heard at once. **Reset the equaliser** goes back to flat.

**A profile for headphones**: turn it on to keep a second set of bands,
used while headphones are plugged in; choose which you're editing with
**Profile** (**Speakers** or **Headphones**). macOS can tell when wired
headphones are plugged into the Mac; Bluetooth headphones may count as
speakers.

## Headphone crossfeed

On speakers, each ear hears both speakers. On headphones, each ear hears
only one channel, which makes old recordings with instruments panned hard
to one side tiring to listen to. **Headphone crossfeed** blends a little of
each channel into the other, as speakers do.

In **Settings › Features**, set **Headphone crossfeed** to **Light**,
**Medium** or **Strong** (**Off** at first). With **Only with headphones**
on (at first), it works only while macOS says headphones are plugged in;
turn that off for Bluetooth headphones, which macOS can't tell apart from
speakers.

## Effects

Effects change the music in real time, for fun or atmosphere. They are off
until you turn them on.

1. Turn on **Effects** in **Settings › Features**, or in **Settings ›
   Effects** (the same switch).
2. In **Settings › Effects**, turn on the effects you want, or choose a
   **Preset**: **Small room**, **Concert hall**, **Dreamy**, **Slapback
   echo**, **Dub delay**, **Jet flanger**, **Psychedelic**, **Old radio**,
   **Auto-pan**, **Frozen hall**, or **All off**. A preset replaces every
   effect's settings.
3. Each effect has a **Mix** (how much of the effect you hear against the
   music) and its own controls. Changes are heard as you move a control.

<!-- Screenshot: Settings › Effects with reverb on. -->

| Effect | What it does | Its controls |
|---|---|---|
| **Reverb** | A room around the music: its size sets how long it rings, the damping how soon the highs fade. | **Size**, **Damping**, **Width**, **Pre-delay** |
| **Echo** | Repeats that fade and darken, bouncing between the speakers as the spread rises. | **Time**, **Feedback**, **Tone**, **Spread** |
| **Chorus** | Copies drifting slightly in pitch and time against the music, for a fuller, shimmering sound. | **Rate**, **Depth** |
| **Flanger** | A sweeping, jet-like whoosh. | **Rate**, **Depth**, **Feedback** |
| **Phaser** | Soft notches drifting up and down the sound. | **Rate**, **Depth**, **Feedback** |
| **Tremolo and auto-pan** | The level rising and falling, or, with **Stereo** up, the sound moving from speaker to speaker. | **Rate**, **Depth**, **Stereo** |
| **Lo-fi** | Fewer bits and a lower sample rate: gritty, like an early sampler. | **Bits**, **Sample rate** |
| **Spectral freeze** | Holds a moment of the music and sustains it as a drone, under the music or instead of it. | **Fade** |

**Spectral freeze**: with it on, click **Hold** in Settings, or the
snowflake on the player bar, to capture the sound playing now; click **Let
go** (or the snowflake again) to release it. The next track lets go by
itself.

**Reset the effects** puts every effect back as it started. With
**Effects** off, the music plays untouched whatever the effects are set to.
The settings reference lists each control's range
([settings reference](settings-reference.md#effects)).

## Practice mode

Practice mode is for learning a part: loop a passage, slow it down without
changing its pitch, or change the pitch alone.

1. Turn on **Practice mode** in **Settings › Features**.
2. Click **Practice** on the player bar.
3. To loop a passage: click **Set A** where it starts, play on, and click
   **Set B** where it ends. It plays **Looping** between them until you
   click **Loop** to stop looping, or **Clear** to forget A and B.
4. **Speed** plays slower or faster (50% to 150%) at the same pitch.
5. **Pitch** moves the pitch up or down, up to 12 semitones, at the same
   speed.
6. **Normal speed and pitch** puts both back.

## Match the device's sample rate

Music files come at different sample rates: 44.1 kHz from CDs, 48, 96 or
192 kHz from studio downloads. Your output device runs at one rate, and
ano-mp converts (resamples) any track that differs, with high quality.

**Match the device’s sample rate** (off at first, in **Settings ›
Features**) switches the device to each track's rate instead, when the
device offers it, so nothing is resampled. The sound pauses for a moment at
each switch, and other apps playing through the same device change rate
too.

## The signal path

Click the format on the player bar (for example "FLAC 24/96"; needs
**Signal path**, on at first) to see every step between the file and the
speakers as it is right now:

- **File**: the codec, bit depth, sample rate and bit rate, marked
  **(lossy)** for MP3, AAC and the like;
- **Gain**: ReplayGain and your offsets;
- **Practice**: speed and pitch, when in use;
- **Rate**: **Not resampled**, or **Resampled** from one rate to another
  (with a hint that **Match the device’s sample rate** avoids it);
- **Effects**, **Equaliser** and **Crossfeed**: on or off, and how;
- **Crossfade**: how far into the next track it starts;
- **Recording**, while one runs;
- **Volume**;
- **Device**: which device, at what rate, with **(headphones)** when macOS
  says so.

Use it to check that a hi-res file really plays at its rate, or to see why
something sounds different.

## Output device and buffer

In **Settings › Playback**, under **Output**:

- **Device**: where the sound goes. **System default** follows the device
  chosen in macOS's Sound settings. If you choose a device and it's
  disconnected, the default plays until it's back.
- **Buffer size**: smaller buffers respond sooner (pause, seek, the
  visualizer) but may crackle on a busy computer; larger ones are safer.
  **Device default** suits most Macs.

Settings shows what is playing now: "Playing through MacBook Pro Speakers
at 48 kHz, 512 samples a block, about 13 ms to the speakers."
