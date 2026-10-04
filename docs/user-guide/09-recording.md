# 9. Recording

ano-mp can record what you hear to a file: a mix you made from the queue
with crossfades, music with effects on, a practice loop at a slower speed.
This chapter covers turning it on, the formats and which to choose, where
the files go, and what is and isn't recorded.

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
