# 1. Getting started

In this chapter you install nothing extra, add the folders your music is in,
and learn your way around the window.

## What ano-mp is

ano-mp plays the music files on your Mac or on a drive connected to it. It
reads the folders you give it and builds a **library** from them: every
track, album and artist, with their covers. Your files are never changed,
moved or renamed; ano-mp only reads them.

It plays MP3, FLAC, AAC and Apple Lossless (in .m4a files), Ogg Vorbis,
Opus, WAV, AIFF and WMA. [Appendix B](appendix-b-formats.md) has the full
list. Albums play without gaps between their tracks.

Beyond your files' own tags, ano-mp can look albums and artists up on online
services (MusicBrainz, the Cover Art Archive and Wikipedia) for covers,
release details and biographies. You can turn that off; see
[chapter 6](06-online-information.md).

## The first run

The first time ano-mp opens, the library is empty and the window shows
**Welcome to ano-mp**.

<!-- Screenshot: the Welcome view, light theme, before a folder is added. -->

1. Click **Add your Music folder** to use the Music folder in your home
   folder, or **Choose another folder…** to pick any other. You can also
   drag a folder from the Finder onto the window.
2. ano-mp starts reading the folder at once. While it does, the window
   says **Reading your music…**, and albums appear as they are found.
3. When it has finished, the library opens. Add more folders at any time
   (below).

Under **Online sources**, the Welcome view lists each online service that
is on and exactly what it is sent ("album titles, artist names and track
lengths, to find each album and artist", for MusicBrainz). Your files and
their paths are never sent. To change what is on before anything is looked
up, click **Change the online sources…**.

## Add your music

You can add as many folders as you like: your Music folder, a folder on an
external drive, a network share.

- **File › Add Folder to Library…** (⇧⌘O), or
- the **+** beside **Folders** in the sidebar, or
- **Add folder…** in **Settings › Library**, or
- drag a folder from the Finder onto the window. ano-mp asks **Add a
  folder**, and adds it when you click **Add to Library**.

### Why macOS asks you to choose folders

macOS keeps apps to the files you allow them. ano-mp reads only the folders
you choose in its folder picker (or drop on its window), and remembers that
you allowed each one, so you don't have to choose it again at the next
launch. If you move a folder or rename the drive it is on, ano-mp may lose
track of it; [chapter 12](12-library-health.md) shows how to point it at the
new place with **Locate…**.

### What the scan does

Adding a folder starts a **scan**: ano-mp looks at every file in it and its
subfolders, and for each one it can play, reads the tags (title, artist,
album, track number, genre and so on), the length, the format and any cover
picture. While it scans, the sidebar says **Scanning: read 120 of 900 new or
changed files**.

- The first scan of a large folder can take several minutes. You can play
  music while it runs.
- Later scans read only files that are new or changed, so they are quick.
- Files kept in iCloud and not downloaded to this Mac ("Optimize Mac
  Storage") are listed but not downloaded during a scan; see
  [chapter 12](12-library-health.md#files-in-icloud).
- ano-mp keeps the library up to date by itself: it checks your folders
  when it opens, and notices music added, changed or removed while it runs.
  Both can be turned off in **Settings › Library**.

## A tour of the window

<!-- Screenshot: the main window with a demo library, an album open, the queue panel shown. -->

The window has four parts.

**The sidebar**, on the left, is how you move around:

- **Home**, **Favourites**, **Artists**, **History** and **Library health**:
  views of your library ([chapter 2](02-finding-music.md)). Some appear only
  while their feature is on.
- **Effects workbench**: one file played through the effects and recorded
  ([chapter 9](09-recording.md#the-effects-workbench)).
- **Now Playing** (if you choose to show it in **Settings › Display**),
  **Visualizer** and **Queue**: the music that's playing.
- **Library**: your library sorted different ways, such as **Album
  artist**, **Genre**, **Year**, **Folder** and **Composer**. Click the
  heading to open or close the list.
- **Playlists**: your playlists, with **+** to make a new one.
- **Folders**: the folders in your library, with buttons to add a folder,
  rescan, and remove one.
- **Online sources**, which shows what the online lookups are doing, and
  **Settings** (⌘,).

On a narrow window the sidebar hides; the button at the top left shows it.

**The main area**, in the middle, shows the view you chose. Along its top
are **Back** (Esc also goes back), where you are (for example "Album
artist › Miles Davis › Kind of Blue", each part clickable), and the
**Search** box (⌘F).

**The player bar**, along the bottom, shows the track playing, the controls
to play, pause, skip and seek, the volume, shuffle and repeat, and buttons
for the queue, the visualizer and other panels
([chapter 3](03-playing-music.md)). Click the track's title to open the
**Now Playing** view: the cover as large as the window allows, with the
lyrics if the track has any.

**The queue panel**, on the right, lists what will play next. Show or hide
it with the queue button at the right end of the player bar, or **View ›
Show or Hide the Queue Panel** (⌥⌘Q). **View › Queue** (⌘5) shows the queue
in the main area instead.

## Playing your first track

Open a view under **Library** in the sidebar, click your way to an album,
and double-click a track. The album plays from that track. To play a whole
album, click the play button on its row. See
[chapter 3](03-playing-music.md) for everything else.
