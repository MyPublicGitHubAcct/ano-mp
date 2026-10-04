# 3. Playing music

This chapter covers the player bar, the queue, shuffle and repeat, library
radio, gapless albums and crossfades, the sleep timer, your own rules for a
track or album, and evening out loudness.

## The player bar

<!-- Screenshot: the player bar while a track plays, with the waveform seek bar. -->

Along the bottom of the window, from left to right:

- **The track playing**: its cover, title and artist, and a heart to make
  it a favourite. Click the title to open the **Now Playing** view, where
  the artist's name opens their page.
- **Previous**, **Play**/**Pause** and **Next**. **Previous** goes back to
  the start of the track if it has played for more than three seconds, and
  to the track before otherwise.
- **The seek bar**, with the time played and the track's length. Click or
  drag to move through the track. With **Waveform seek bar** on, the bar
  draws the track's waveform, so you can see quiet intros, loud drops and
  hidden tracks; the part played is in the accent colour.
- **Shuffle**, and **Repeat**, which goes from **Repeat off** to **Repeat
  all** to **Repeat one** each time you click it.
- **Sleep timer** (below).
- The track's format (for example "FLAC 16/44.1"): click it for the
  **Signal path** ([chapter 7](07-sound.md#the-signal-path)).
- **Practice**, while **Practice mode** is on
  ([chapter 7](07-sound.md#practice-mode)).
- A snowflake while the spectral freeze is on
  ([chapter 7](07-sound.md#effects)), and **Record** while **Recording** is
  on ([chapter 9](09-recording.md)).
- **Volume**, with **Mute**.
- **Visualizer** and the queue button.

While a file takes a moment to open (a drive waking up, a network share),
the bar says **Opening…**; for a file being downloaded from iCloud it says
**Downloading from iCloud…**.

### From the keyboard

| Action | Keys |
|---|---|
| Play or pause | Space |
| Back or forward 5 seconds | ← or → |
| Next or previous track | ⌘→ or ⌘← |
| Volume up or down | ⌘↑ or ⌘↓ |
| Stop after this track | ⌥⌘. |
| Go to the album playing | ⌘L |

The menu shortcuts work wherever you are, even while typing in the search
box. All of them are in [Appendix A](appendix-a-shortcuts.md), and in
**Help › Keyboard Shortcuts** (⌘/).

## Starting music

- **Double-click a track** (or select it and press Return) to play its
  album, or the list it's in, from that track.
- **Play** on a list, an album or a group plays all of it; **Shuffle**
  plays it shuffled.
- **Play Next** in a menu puts the tracks right after the one playing;
  **Add to Queue** puts them at the end.
- Drag tracks onto **Queue** in the sidebar, or onto the queue panel.
- Drag audio files from the Finder onto the window to play them, or open
  them from the Finder ([chapter 10](10-other-controls.md#opening-files-from-the-finder)).
  Files from outside your library are marked **not in the library** in the
  queue.

## The queue

The queue is the list of what plays, in order. Show it beside the main area
with the queue button on the player bar (⌥⌘Q), or on its own with **Queue**
in the sidebar (⌘5).

<!-- Screenshot: the queue panel with the current track highlighted. -->

- The track playing is highlighted. Click another to play it.
- **Reorder** by dragging a track's handle. Select several (⌘-click,
  ⇧-click) and drag one of their handles to move them together.
- **Remove** a track with its **✕**, or select tracks and press Delete.
- **Add** tracks by dragging them from any list onto the queue.
- Right-click a track for **Play**, **Play Next**, **Stop After This Track**
  and **Remove from Queue**.
- **Clear** empties the queue.
- **Save the queue as a playlist** makes a playlist of it.

Tracks you've asked to skip in album play (see
[Playback preferences](#playback-preferences)) show as **skipped in album
play**.

The queue is kept when you quit. When ano-mp opens again, the queue is as
you left it, paused at the same place in the same track: press Space to
carry on.

### Long tracks remember where you were

A track over 20 minutes long, such as an audiobook chapter or a DJ mix,
starts where you left it the next time you play it (unless you were in its
first or last 30 seconds).

## Shuffle and repeat

**Shuffle** (the player bar, or **Controls › Shuffle**) plays the queue in
a random order. Turning it off goes back to the queue's own order, carrying
on from the track playing.

Shuffle keeps together what belongs together:

- **Keep segues together in shuffle** (on at first): tracks that run
  into each other, as on live albums, DJ mixes and concept albums, play as
  one, in order. ano-mp finds them by checking whether music is still
  sounding at the end of one track and the start of the next on an album,
  so it needs the [loudness analysis](#the-loudness-analysis).
- With **Classical works** on, a work's movements play together, in order.
- An album you've marked **Never shuffle it** plays whole, in order.

**Repeat** (**Controls › Repeat**) is **Off**, **All** (the queue starts
again when it ends) or **One** (the same track again).

## Library radio

**Start Radio** in a track's menu (with **Library radio** on) plays tracks
from your library like that one: the same genres, era, label and related
artists. The queue shows **Radio**, and each track it adds says why it was
picked. It keeps adding tracks as it plays; click **Radio** at the top of
the queue to stop it.

**Keep playing when the queue ends** (off at first) carries any queue on
as radio from its last track, so the music never stops by itself.

Radio leaves out tracks whose folder can't be read at the moment.

## Gapless albums and crossfades

Albums play **gapless**: one track runs straight into the next with no
silence, as on the CD. That matters for live albums, classical music and
albums mixed as one piece.

A **crossfade** fades one track out as the next fades in. Set it in
**Settings › Playback › Crossfade**: **Between tracks**, **Off** or up to
12 seconds. ano-mp never crossfades:

- between tracks of the same album, or of the same work;
- between tracks that run into each other;
- between tracks with different sample rates.

So albums stay gapless, and a shuffled playlist of singles crossfades.

## Stop after a track, and the sleep timer

- **Stop After This Track** (⌥⌘., or **Controls › Stop After This
  Track**) stops when the track playing ends. To stop after a later track,
  right-click it in the queue and choose **Stop After This Track**. The
  queue marks it **Playback stops after this track**.
- **Sleep timer**: click the moon on the player bar, or use **Controls ›
  Sleep Timer**. Choose a number of minutes (15, 30, 45, 60 or 90 on the
  bar; the menu offers **In 15 Minutes** to **In 90 Minutes**), **End of
  track** or **End of album**. A timed stop fades the music out over its
  last ten seconds. The button shows the time left; **Turn off** (or
  **Controls › Sleep Timer › Off**) cancels it.

## Playback preferences

With **Playback preferences** on (it is at first), **Playback
Preferences…** in a track's or an album's menu sets your own rules for it.
They are kept in ano-mp's library, never written into your files.

- **Skip in album and shuffle play**: the track is passed over when its
  album or a shuffle plays. It still plays when you choose it yourself.
  Useful for intros, skits and bonus tracks.
- For an album: **Skip its tracks in album and shuffle play**, and **Never
  shuffle it: shuffle plays it whole, in order**.
- **Gain offset (dB)**: makes the track or album louder or quieter than
  the rest, on top of any volume levelling.
- **Trim the start (seconds)** and **Trim the end (seconds)**: skip a
  silence, a count-in or applause.

A track's own setting overrides its album's; **As the album** leaves it to
the album. **Clear all** removes the track's or album's preferences. A
change applies from the next time the track starts. Tracks and albums with
preferences show a small mark in lists (**Has playback preferences**).

## Skip long silences

**Skip long silences** (off at first, in **Settings › Features**) jumps
over a long silence inside a track, such as the gap before a hidden track,
and ends a track that finishes in silence, once **Skip after** seconds of
it have played (5 at first). It needs the
[loudness analysis](#the-loudness-analysis).

## Volume levelling

Some albums are mastered much louder than others. ano-mp can even them
out, so you don't reach for the volume between albums.

### ReplayGain

Many music files carry **ReplayGain** tags: a measurement of their
loudness, written by a program such as foobar2000, beets or loudgain. ano-mp
reads them when it scans, and uses them as **Settings › Playback ›
ReplayGain** says:

- **Off** (at first): every track plays as it was mastered.
- **By track**: every track at about the same loudness. Best for shuffle.
- **By album**: every album at about the same loudness, keeping the quiet
  and loud tracks within it. Best for albums.

Beside it:

- **Preamp**: added to every tagged track's level. ReplayGain aims a
  little quieter than most modern releases; raise this if everything
  sounds quiet.
- **Untagged tracks**: the level for tracks without ReplayGain tags, so
  they aren't much louder than tagged ones.
- **Prevent clipping** (on at first): never turns a track up so far that
  its loudest moment would distort.

### The loudness analysis

**Loudness analysis** in **Settings › Features** (off at first) measures
every track's loudness in the background, so ReplayGain evens out files
without tags too. It doesn't change your files. The same analysis finds
tracks that run into each other, long silences, and the problems in the
library health report. On a large library it takes hours of processor time,
once; Settings shows how far it has got (**1,200 of 9,000 analysed.**).
Files in iCloud that aren't downloaded are left until they are.
