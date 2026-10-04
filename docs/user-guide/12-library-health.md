# 12. Keeping the library healthy

ano-mp keeps your library in step with your folders by itself. This chapter
explains what it does when a folder goes missing or files live in iCloud,
how rescans work, how to find problems in your files, how to move your
library to another Mac, and what to do if the library itself is damaged.

## Rescans and keeping up to date

A **rescan** looks through a folder again: it reads files that are new or
changed since the last scan, and drops tracks whose files are gone.

- ano-mp rescans every folder in the background when it opens (**Check the
  folders when ano-mp opens**), and watches them while it runs (**Watch the
  folders for changes**), so you rarely need to do it yourself.
- To rescan by hand: the rescan button beside **Folders** in the sidebar
  (all folders), the one beside a folder (that folder), or **Rescan** and
  **Rescan all** in **Settings › Library**.
- Only one scan runs at a time. While one runs, the sidebar shows its
  progress.

### Moving or renaming files

A file you move or rename **within your library's folders** keeps its
plays, rating, heart and playlist places: ano-mp recognises it at the next
scan. If you edit a file's tags, the next scan picks up the change.

If you move or rename a **whole library folder**, or the drive it is on,
ano-mp may no longer find it; use **Locate…** (below).

## Folders that can't be found

If a folder can't be read (an external drive is unplugged, a network share
isn't mounted, the folder was moved or deleted), ano-mp **keeps its
tracks**. Your plays, ratings, favourites and playlists stay as they were,
and everything comes back when the folder does.

<!-- Screenshot: the message listing a folder that can't be found. -->

While a folder can't be read:

- A message at the top of the main area says **1 folder can’t be found**,
  and lists each folder with the reason:
  - **not found**: its drive isn't connected, or it was deleted;
  - **empty: not mounted?**: it is there but empty; its drive or share may
    not be mounted;
  - **in the Trash**: it was moved to the Trash;
  - **no access**: ano-mp may no longer read it, and needs you to locate
    it again;
  - **tracks missing**: most of its tracks weren't found.
- Its tracks are **dimmed** in lists. Playing one says the folder isn't
  available. Radio, smart playlists' play and Home's suggestions leave them
  out.
- The sidebar marks the folder.
- Everything else plays as usual.

What you can do, from the message or **Settings › Library**:

- **Locate…**: show ano-mp where the folder is now (or choose it again to
  give access). Its tracks keep their plays and playlists.
- **Remove…**: take the folder out of the library, with its tracks.
- **Remove missing tracks…**: for a folder that reads as empty or lost most
  of its tracks, remove the tracks that weren't found. Their plays,
  ratings, favourites and playlist entries go with them.
- **Keep**: keep its tracks and hide the message until the next launch.

If **none** of your folders can be read (a library on a drive that isn't
plugged in), the window says **Your music can’t be found**. Connect the
drive and the library comes back by itself.

Why so careful? A rescan of an unplugged drive would otherwise see no files
and delete every track, with years of plays and playlists. ano-mp only
removes tracks when it can see the folder and most of its files, or when
you ask.

## Files in iCloud

With **Optimize Mac Storage** on (in macOS's iCloud Drive settings),
macOS may keep some files only in iCloud, downloading them when something
opens them. ano-mp never downloads a file in the background:

- A scan lists such a file without reading it, with its file name as its
  title, and **Settings › Library** says how many of a folder's files are
  **in iCloud, not downloaded**. Each scan checks them again, and reads a
  file's tags once it has been downloaded.
- Playing one downloads it; the player bar says **Downloading from
  iCloud…**.
- The loudness analysis and covers leave them alone until they are
  downloaded.

To have ano-mp read them all, download the folder in the Finder (select it,
then **Download Now**), or keep it downloaded.

## The library health report

**Library health** in the sidebar (with **Library health report** on, as it
is at first) lists problems ano-mp can find in your files. It never changes
anything: **Show** (or **Show in Finder**) reveals each file so you can fix
it.

<!-- Screenshot: the Library health view with a few problems. -->

- **Don’t decode**: files that fail to play.
- **Cut short**: files that end sooner than their headers say, often from
  an interrupted copy or download.
- **Suspected transcodes**: lossless files (FLAC, Apple Lossless…) whose
  sound stops at 15–19.5 kHz for the whole track, as MP3 or AAC encoders
  leave it: probably made from a lossy file. A guess: some recordings are
  like that anyway.
- **Albums whose tracks disagree**: tracks of one album with different
  years, disc counts, missing or repeated track numbers, or different album
  artists in one folder (**No album artist**).
- **Likely duplicates**: the same recording more than once.

The first three need the loudness analysis: the report says how many
tracks it covers so far (**The decoding and transcode checks cover the 400
of 9,000 analysed so far**).

## Moving your library to another Mac

**Export Library Data…** (in the **File** menu, or **Export…** under
**Your data** in **Settings › Library**) saves one file with your
playlists, favourites, ratings, listening history, playback preferences,
cover and match choices, the queue and your settings. It doesn't contain
the music.

To move to another Mac, or restore after a problem:

1. Copy your music to the new Mac, and add its folders to ano-mp there.
2. Choose **File › Import Library Data…** (or **Import…** under **Your
   data**) and choose the file.
3. ano-mp asks whether to **Also replace your settings, sort rules and
   online source settings with the file's**: **Import with Settings** or
   **Keep My Settings**. Playlists, favourites, ratings and history are
   added either way.
4. It finds your tracks by their paths within your folders, and says how
   many it found (**Imported: found 9,000 of 9,012 tracks; 25 playlists, 300
   favourites and 41,000 plays added**).

A file from a newer version of ano-mp can't be imported by an older one.

## If the library needs repair

ano-mp keeps your library (everything except the music files themselves)
in a database on your Mac, and checks it at each launch. If it finds
damage, it shows **The library needs repair** before anything else. Your
music files are not affected.

<!-- Screenshot: the repair dialog. -->

- **Restore copy**: go back to the copy ano-mp made before its last
  upgrade. Changes since then (new playlists, plays, ratings) are lost.
- **Rebuild**: save your playlists, favourites, ratings, history and
  settings, start a new library, scan every folder again and put them back.
  If they can't be saved from the damaged library, ano-mp asks before
  rebuilding without them (**Rebuild anyway**).
- **Not now**: carry on for now; you'll be asked again at the next launch.

Either way the damaged library is kept beside the new one, and ano-mp
restarts.
