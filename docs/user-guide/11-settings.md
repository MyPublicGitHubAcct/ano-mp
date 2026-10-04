# 11. Settings

Open Settings with **ano-mp › Settings…** (⌘,), or **Settings** at the
bottom of the sidebar. The sections are listed down the side (along the top
on a narrow window); ↑ and ↓ move between them, and Esc closes Settings.
Every change is saved as you make it.

This chapter goes through each section in order: what each option changes.
The [settings reference](settings-reference.md) lists every setting with
the value it starts with and the values it can take. Most sections have a
**Reset to defaults** button at the bottom, which puts that section back as
it started.

<!-- Screenshot: Settings open on the General section. -->

## General

The menu bar, the mini player, notifications and the keyboard.

- **Controls in the menu bar**: an icon in the macOS menu bar with the
  track playing, play and pause, next and previous
  ([chapter 10](10-other-controls.md#controls-in-the-menu-bar)). Off at
  first.
- **Keep the mini player above other windows**: the mini player stays in
  front of other apps' windows. **Open the mini player** opens it. Off at
  first.
- **Show each new track**: a notification with the cover when the track
  changes, while ano-mp isn't the app in front. macOS asks first. Off at
  first.
- **Keyboard shortcuts…** opens the list of shortcuts
  ([Appendix A](appendix-a-shortcuts.md)).

## Library

The folders your music is in, keeping up with them, and your data.

- **The folders**: each folder in the library, with its number of tracks
  and when it was last scanned, or **Not scanned yet**. Beside each:
  **Rescan**, and **Remove** (which takes it out of the library and leaves
  its files where they are). A folder that can't be read says why, with
  **Locate…** to point ano-mp at where it is now
  ([chapter 12](12-library-health.md)). Folders with files in iCloud that
  aren't downloaded say how many.
- **Add folder…** adds a folder; **Rescan all** rescans every folder.
- **Check the folders when ano-mp opens**: a rescan in the background at
  each launch, at a low priority, so changes made while ano-mp was closed
  show up. On at first.
- **Watch the folders for changes**: music added, changed or removed while
  ano-mp is open shows up within seconds. On at first.
- **Your data**: **Export…** and **Import…** save and restore your
  playlists, favourites, ratings, history and settings
  ([chapter 12](12-library-health.md#moving-your-library-to-another-mac)).

## Sorting

How the library is grouped and ordered in the sidebar's **Library**
views.

- **Views**: the views listed under **Library** in the sidebar (at first
  **Album artist**, **Genre**, **Year**, **Folder** and **Composer**).
  Click one to change it (**Edit**), or click **Add view** for a new one.
  For each view:
  - **Name**: as the sidebar shows it.
  - **Group by**: the **Levels**, from the top: **Album artist**,
    **Artist**, **Album**, **Genre**, **Year**, **Composer** or **Work**.
    **Add a level…** adds one; each can be moved up or down or removed.
    **Folder (on its own)** groups by your folders as they are on disk, and
    can't be combined with other levels. No level at all (**Nothing: one
    list of every track**) lists every track in one list.
  - **Albums sorted by**: **Title**, **Year, oldest first** or **Date
    added, newest first**, where the view lists albums.
  - **Tracks sorted by**: the **Track order** within a group, by **Album
    artist**, **Artist**, **Album**, **Year**, **Disc number**, **Track
    number**, **Title**, **File path**, **Date added, newest first** or
    **Movement number**, in order of precedence. **Add a sort key…** adds
    one.
  - **Save** keeps your changes; **Cancel** or **Revert** drops them;
    **Remove view** deletes the view.
- **Skipped words** (**Words skipped when sorting**): leading words that
  sorting skips in names and titles, so "The Beatles" sorts under B. At
  first "The A". Separate them with spaces; leave it empty to sort by every
  word.

**Reset to defaults** restores the built-in views and skipped words, and
removes views you added.

## Display

What track lists and album pages show.

- **Track list columns**: what lists show beside each track's title, in
  order: **Track number**, **Artist**, **Album**, **Album artist**,
  **Year**, **Genre**, **Length**, **Format**, **Bit rate**, **Sample
  rate**, **Plays**, **Last played**, **Date added**, **Composer**,
  **Rating**. **Add a column…** adds one; each can be moved or removed.
  Columns sit beside the title when the window is wide enough, and under
  it when it isn't. The track number always leads the row and the length
  ends it. **Preview** shows a sample row.
- **Album facts** (**Release facts**): the line under an album's title:
  **Release date**, **Label and catalogue number**, **Country**,
  **Format** and **Type**, in the order you choose, from the online
  source the album is matched on.
- **Show album descriptions and artist biographies**: the text from
  Wikipedia on album and artist pages. On at first.
- **Show Now Playing in the sidebar**: a **Now Playing** item in the
  sidebar. The player bar opens the same view either way. Off at first.

## Appearance

The app's colours, font, text size, density and corners. This section
shows while **Themes** is on in **Features** (it is at first); with it off,
ano-mp uses its standard look.

- **Themes**: pick one of the built-in themes (**Standard**, **Light**,
  **Dark**, **High contrast**, **Paper**, **Midnight**, **Forest**,
  **Ocean**, **Rose**, **Graphite**, **Sunset**, **Meadow**) or one you
  saved. **Use the standard theme** goes back to the standard look.
- **Colours**:
  - **Light or dark**: **As the system is** (follows macOS's appearance),
    **Always light** or **Always dark**.
  - Each theme has light and dark colours; **Colours being edited**
    chooses **Light colours** or **Dark colours**. Change any of
    **Background**, **Panels**, **Raised panels**, **Text**, **Secondary
    text**, **Icons and lines**, **Borders**, **Accent**, **Text on
    accent**, **Warnings**, **Hearts** and **Stars** with its colour well
    or as a hex colour. The **Preview** shows them together.
  - ano-mp checks that text stays readable against its background (the
    WCAG AA contrast standard) and lists any pair that falls short.
  - **Accent from the cover**: the accent colour takes the playing album's
    main colour, made dark or light enough to read. Off at first.
  - **High contrast when the system asks for it**: while **Increase
    contrast** is on in **System Settings › Accessibility › Display**, the
    high-contrast colours replace the theme's. On at first.
- **Text and shape**: **Font** (**System**, **Rounded**, **Serif**,
  **Monospaced**, **Avenir**), **Text size** (12 to 22 px; everything else
  grows or shrinks with it once you let go of the slider), **Density**
  (**Compact**, **Regular**, **Roomy**: how much room list rows and
  controls take) and **Corners** (how rounded).
- **Name**, **Save theme**: your changes are used at once; name the theme
  and save it to keep it in the list (a saved theme with the same name is
  replaced). **Remove** deletes a saved theme. **Export…** writes the theme
  to a file, and **Import…** reads one, to share themes between Macs.

## Playback

Where the sound goes, evening out loudness, and crossfades.

- **Output**: **Device** and **Buffer size**
  ([chapter 7](07-sound.md#output-device-and-buffer)), with what is playing
  now. **Use the default device** goes back to the system's.
- **ReplayGain**: **Off**, **By track** or **By album**, with **Preamp**,
  **Untagged tracks** and **Prevent clipping**
  ([chapter 3](03-playing-music.md#volume-levelling)).
- **Crossfade**: **Between tracks**, **Off** or 1 to 12 seconds
  ([chapter 3](03-playing-music.md#gapless-albums-and-crossfades)).

**Reset playback** puts these back as they started.

## Equaliser

Shape the sound, with a profile for headphones if you like: **Equaliser**
on or off, **A profile for headphones**, **Profile**, **Preset**,
**Preamp** and the ten **Bands**. See
[chapter 7](07-sound.md#the-equaliser). **Reset the equaliser** sets it
flat.

## Effects

Real-time effects on what is playing: **Effects** on or off, a **Preset**,
and each effect with its switch, **Mix** and controls. See
[chapter 7](07-sound.md#effects). **Reset the effects** puts them back as
they started.

## Recording

Where recordings go and what they are written as. This section shows while
**Recording** is on in **Features**: **Folder** (**Choose Folder…**),
**Format**, **Samples** or **Bitrate**, and **Cue sheet**. See
[chapter 9](09-recording.md).

## Visualizer

What the visualizer shows, and how: **Visualization**, **Change by
itself**, **Cover wall shows**, **Frame rate**, **Sensitivity**, **Colours
from the album cover** and **Calm mode**. See
[chapter 8](08-visualizations.md#visualizer-settings).

## Online sources

Where album details, covers and biographies come from, beyond your files'
tags: **Use online services**, **Look up albums and artists
automatically**, each source's switch and key, and the **Order** sources
are tried in. See [chapter 6](06-online-information.md#turn-services-on-or-off).

## Features

Features beyond the usual. Each can be turned off; online ones, and ones
that listen on the network, are off until you turn them on. They are in
four groups.

**Sound and playback**

| Feature | What it does | At first |
|---|---|---|
| **Loudness analysis** | Measures every track's loudness in the background, so volume levelling works without ReplayGain tags. Hours of processor time on a large library, once. Shows its progress. ([chapter 3](03-playing-music.md#the-loudness-analysis)) | Off |
| **Waveform seek bar** | Draws the track's waveform in the seek bar. | On |
| **Keep segues together in shuffle** | Tracks that run into each other shuffle as one, in order. Needs the analysis. | On |
| **Skip long silences** | Jumps over long silences inside a track, after **Skip after** seconds of one. Needs the analysis. | Off |
| **Signal path** | Clicking the format on the player bar shows every step to the speakers. ([chapter 7](07-sound.md#the-signal-path)) | On |
| **Match the device’s sample rate** | Switches the device to each track's rate. ([chapter 7](07-sound.md#match-the-devices-sample-rate)) | Off |
| **Headphone crossfeed** | **Off**, **Light**, **Medium** or **Strong**, with **Only with headphones**. ([chapter 7](07-sound.md#headphone-crossfeed)) | Off |
| **Practice mode** | A–B loops, speed and pitch, from **Practice** on the player bar. ([chapter 7](07-sound.md#practice-mode)) | Off |
| **Effects** | Reverb, echo, chorus, a spectral freeze and more. ([chapter 7](07-sound.md#effects)) | Off |
| **Recording** | The **Record** button. ([chapter 9](09-recording.md)) | Off |

**Library**

| Feature | What it does | At first |
|---|---|---|
| **Library health report** | Finds broken, cut-short and suspect files, albums whose tags disagree, and duplicates. **Open the health report**. ([chapter 12](12-library-health.md#the-library-health-report)) | On |
| **Cue sheets and chapters as tracks** | A cue sheet's or a file's chapters become tracks. Changing it reads every file again. ([chapter 5](05-track-and-album-details.md#albums-as-one-file-cue-sheets-and-chapters)) | On |
| **Classical works** | Works and movements on album pages, **Composer** and **Work** in browsing, and a work shuffles as one. | On |
| **Playback preferences** | Your own rules per track or album. ([chapter 3](03-playing-music.md#playback-preferences)) | On |
| **Lyrics** | Lyrics from tags and .lrc files in the Now Playing view. ([chapter 5](05-track-and-album-details.md#lyrics)) | On |

**Listening and discovery**

| Feature | What it does | At first |
|---|---|---|
| **Listening history** | Remembers what you play, on this Mac only. **Open history**; **Clear history…** forgets every play. | On |
| **Send listens to ListenBrainz** | Sends each play to your ListenBrainz account. Goes online. ([chapter 6](06-online-information.md#send-listens-to-listenbrainz)) | Off |
| **Recently played** | On Home and History. Needs the history. | On |
| **Top 20 of a year or month** | On the History page. Needs the history. | On |
| **Library radio** | **Start Radio** in a track's menu. ([chapter 3](03-playing-music.md#library-radio)) | On |
| **Keep playing when the queue ends** | Any queue carries on as radio. | Off |
| **Recently added** | The newest albums, on Home. | On |
| **Released on this day** | Albums released on today's date, on Home. | On |
| **More in this genre** | Five random albums sharing a genre, on album pages. | On |
| **Recommendations** | **More Like This**, and **You might like** on Home, from your library alone. | On |
| **Recommendations from outside the library** | Artists you don't have, from ListenBrainz. Goes online, and shows what it sends. ([chapter 6](06-online-information.md#recommendations-from-outside-the-library)) | Off |
| **Similar artists** | A **Similar artists** row on artist pages. | On |

**Look and feel**

| Feature | What it does | At first |
|---|---|---|
| **Themes** | The **Appearance** section. Off, the standard look. | On |

**Remote control**: **Control playback from a phone**, the **Port**,
pairing and paired phones. Off at first. See
[chapter 10](10-other-controls.md#control-it-from-a-phone).

**Reset features** turns every feature back to how it started.

A feature that is off hides its buttons and menu items. If something asks
for one that is off, ano-mp says which (**… is turned off in Settings ›
Features**).

## About

Versions, logs and diagnostics for reporting a problem.

- **Versions**: ano-mp's version and the versions of what it is built
  with.
- **Third-party notices**: the licences of the open-source software ano-mp
  uses.
- **Updates**: **Check for updates**, **Check for updates automatically**
  (off at first) and **Download from GitHub**
  ([chapter 6](06-online-information.md#checking-for-updates)).
- **Reporting a problem**: **Show logs** opens the folder of ano-mp's log
  files in the Finder; **Copy diagnostics** copies a summary to paste into
  a bug report. Neither holds your file names, titles, artists or keys
  ([chapter 13](13-privacy.md#logs-and-diagnostics)).
- **Detailed logging** (off at first): while it's on, ano-mp also writes a
  second log file, `ano-mp-detailed.log`, that names the files, folders,
  titles, artists and web addresses it works with, to trace a problem the
  normal log can't explain. It stays on until you turn it off or quit. The
  file is deleted when you turn it off, or the next time ano-mp opens
  ([chapter 13](13-privacy.md#detailed-logging)).
