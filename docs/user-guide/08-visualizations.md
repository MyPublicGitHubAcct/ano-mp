# 8. Visualizations

The visualizer draws pictures that move with the music. This chapter shows
how to open it, describes each visualization, and covers its settings and
calm mode.

## Open the visualizer

Click **Visualizer** in the sidebar, the visualizer button on the player
bar, or **View › Visualizer** (⌘6). It fills the main area; **Full screen**
(or F) fills the screen.

<!-- Screenshot: the visualizer showing the spectrum, with its controls. -->

Along its top are the visualization picker, **Calm** and **Full screen**,
and the button to close it; along its bottom, the track playing. The
controls fade away while the pointer rests, and come back when it moves.

| Keys | Action |
|---|---|
| V | Next visualization |
| ⇧V | Previous visualization |
| F | Full screen |
| Esc | Leave full screen, or close the visualizer |
| Space | Play or pause |

### Before it starts: flashing light

The first time the visualizer opens, a note comes first, **Before the
visualizer starts**, and nothing moves until you've read it. Some
visualizations pulse and flash in time with the music. ano-mp limits
flashes to three a second, and dims the picture if it would flicker faster.
If flashing light affects you, choose **Start in calm mode**: no pulses on
beats, and slower movement. Otherwise click **Continue**.

**Calm mode** can be turned on or off at any time with **Calm** in the
visualizer or **Calm mode** in **Settings › Visualizer**. It is always on
while your Mac is set to reduce motion (**System Settings › Accessibility
› Display › Reduce motion**).

## The visualizations

<!-- Screenshots: one picture of each visualization, playing the same demo track. -->

| Visualization | What it shows |
|---|---|
| **Spectrum** | Bars from bass to treble, with falling peaks. |
| **Oscilloscope** | The waveform as a glowing trace; each channel faintly behind their mix. |
| **VU meters** | Analog meters with VU ballistics and peak lamps. |
| **Cover wall** | The covers of albums from the same year, or by the same artist, pulsing with the music. **Same year** and **Same artist** switch between them. |
| **Ridgelines** | The last few seconds of the spectrum as stacked ridges, like the cover of Unknown Pleasures. |
| **Circle of fifths** | The notes sounding, around the circle of fifths, and the key the music seems to be in. |
| **Vectorscope** | The stereo image: mono draws a vertical line, wide stereo a cloud; with the channels' correlation. |
| **Kaleidoscope** | The current cover, mirrored into a turning kaleidoscope that blooms with the music. |
| **Tonnetz** | The harmonic lattice: fifths along each row and thirds up it, so every triangle is a chord, lit as it sounds. |
| **Recurrence plot** | The track compared with itself as it plays: parts that come back, like choruses, light lines beside the diagonal. |
| **Cymatics** | Sand on a vibrating plate, gathering into the figure of the notes sounding: finer figures for higher notes. |
| **Phase portrait** | The waveform plotted against itself a moment later, turning in three dimensions: a pure tone draws a ring, a rich sound a knot, noise a cloud. |
| **Pitch spiral** | Seven octaves of notes on a spiral, one octave a turn, so octaves share a spoke and harmonics make patterns. |
| **Harmonograph** | A pendulum drawing tuned to the interval between the two loudest notes: a fifth draws the 3:2 figure, a major third the 5:4. |
| **Rhythm rings** | Each bar of music a ring, with the beats marked where they fall, so a repeating rhythm lines up from ring to ring. |
| **Stereo stage** | Where each sound sits between the speakers, bass at the bottom and treble at the top. |
| **Resonance** | Cymatics with the phase portrait drawn over it. |
| **Harmony** | The pitch spiral beside the Tonnetz: the notes sounding, and the chords they make. |

## Visualizer settings

In **Settings › Visualizer**:

- **Visualization**: the one the visualizer opens with (V changes it
  there).
- **Change by itself**: **Never**, or move on to the next visualization
  every so many seconds or minutes while the visualizer shows.
- **Cover wall shows**: **Albums from the same year** or **Albums by the
  same artist**.
- **Frame rate**: how often the sound is analysed: 60 a second, or 30 to
  use less power.
- **Sensitivity**: scales the spectrum. Raise it for quiet music; lower it
  if the bars sit at the top.
- **Colours from the album cover**: otherwise every album gets the same
  colours.
- **Calm mode**: above.

If the visualizer can't follow the audio, it says so (**The visualizer
can't follow the audio**) and the music plays on.
