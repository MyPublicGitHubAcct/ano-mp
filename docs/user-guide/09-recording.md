# 9. Recording

ano-mp can record what you hear to a file: a mix you made from the queue
with crossfades, music with effects on, a practice loop at a slower speed.
This chapter covers turning it on, the formats and which to choose, where
the files go, what is and isn't recorded, and the **Effects workbench**,
which plays one file through the effects and records it.

Recording is off until you turn it on, because the files can be large.

## Turn recording on

1. In **Settings › Features**, turn on **Recording**. A **Recording**
   section appears in Settings, and a **Record** button on the player bar.
2. In **Settings › Recording**, click **Choose Folder…** and choose where
   recordings go. ano-mp remembers the folder and may write only there.
3. Choose a **Format** (below).

## Record

1. Play something.
2. Click **Record** on the player bar, or choose **Controls › Record**
   (⌥⌘R). The button shows the time recorded so far.
3. Click it again (or choose the menu item again) to stop. ano-mp says
   **Recording saved** with the file's name.

Each recording is named by when it began, for example "ano-mp 2026-10-04
21.15.03.wav", never by what was playing.

## What is recorded

- **What you hear**: the effects, equaliser and crossfeed are recorded;
  so are practice mode's speed and pitch, crossfades and volume levelling.
- **Not the volume**: turning the volume down, or muting, doesn't make the
  recording quieter.
- **Across track changes**: one recording runs on through the queue,
  gapless albums and crossfades included.
- **Nothing while paused**: pausing leaves no silence in the file.
- **A change of sample rate starts a new file**: if the output's rate
  changes (a track at another rate with **Match the device’s sample rate**
  on, or a different device), the recording carries on in a second file,
  named after the first with a number ("… 2.wav").

With **Cue sheet** on (at first), ano-mp writes a .cue file beside each
recording, naming the tracks it holds and where each one begins. Add the
recordings' folder to your library with **Cue sheets and chapters as
tracks** on, and each recording shows as its tracks.

## Formats

| Format | Kind | Choose it when |
|---|---|---|
| **WAV** | lossless, uncompressed | you'll edit the recording, or want it exactly as heard. The largest files. |
| **AIFF** | lossless, uncompressed | as WAV, for apps that prefer AIFF. |
| **FLAC** | lossless, compressed | you want it exactly as heard, in about half the space. |
| **Apple Lossless (.m4a)** | lossless, compressed | as FLAC, for Apple's apps and devices. |
| **AAC (.m4a)** | lossy | you want small files for a phone. |
| **MP3** | lossy | you want small files that play anywhere. |

For the lossless formats, **Samples** sets the bit depth: **16-bit** (CD
quality), **24-bit**, or **32-bit float** (WAV only, and the default: it
can't clip, whatever the effects do). For AAC and MP3, **Bitrate** sets the
quality, 96 to 320 kbps (256 at first). Settings shows about how many
megabytes a minute the choice takes. MP3 records at up to 48 kHz and AAC at
up to 96 kHz; faster rates are converted.

## The effects workbench

The **Effects workbench** is for hearing one music file through the
effects and saving the result: a song with reverb, a loop with a flanger,
a drone from the spectral freeze. The file doesn't need to be in your
library.

<!-- Screenshot: the Effects workbench with a file open, reverb on, and a take recording. -->

1. Click **Effects workbench** in the sidebar. (It's there while
   **Effects workbench** is on in **Settings › Features**, as it is at
   first.)
2. Click **Choose a File…** and pick a music file, or drop one on the
   window from the Finder. It starts playing, next in the queue, as a file
   opened from the Finder does: the rest of your queue is kept, and the
   file isn't added to the library. Or click **Use the Playing Track** to
   work on the track playing now, where it is in the queue: nothing is
   added to the queue.
3. The page shows the file's title, format and length, a play button and
   the seek bar. If something else starts playing, **Play** goes back to
   the file; if the file has left the queue, **Play** opens it again.
4. Below are the same **Effects** controls as in **Settings › Effects**
   ([chapter 7](07-sound.md#effects)), with their own switch: the
   workbench never turns the effects on for you. Changes are heard as you
   move a control.
5. Under **Recording**, turn on **Recording** if it's off (it stays on for
   the **Record** button on the player bar too), and choose the folder and
   format as in **Settings › Recording**.
6. Click **Record One Take**. The file (or track) starts again from the
   beginning, is recorded exactly as you hear it, and recording stops by
   itself when it ends; playback stops there too. The page shows the time
   recorded, then the name of the saved file.

To record only part of the file, turn on **Practice mode** in **Settings ›
Features**, and set an A–B loop on the page with **Set A** and **Set B**
([chapter 7](07-sound.md#practice-mode)). **Record One Take** then records
once round the loop, from A to B, and pauses. A loop belongs to the file
it was set on: choosing or dropping another file clears it, so set A and
B again on the new one.

**Stop Recording** stops a take early; what was recorded is kept. Playing
something else, or stopping the recording from the player bar, ends the
take too. A take is accurate to the sample at the file's end, and to
about a hundredth of a second at a loop's end.

## When something goes wrong

- If the disk fills up or the file can't be written, the recording stops,
  and what was recorded until then is kept.
- If the disk was too slow to keep up for a moment, ano-mp says so when the
  recording stops (**The disk fell behind once while recording, and a
  moment was lost.**). Record to a faster disk, or choose a compressed
  format.
- If the recordings' folder was moved, deleted or is on a drive that isn't
  connected, choose it again in **Settings › Recording**.

[Appendix D](appendix-d-errors.md) lists every message.
