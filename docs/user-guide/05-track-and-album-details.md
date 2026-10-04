# 5. Track and album details

This chapter covers looking closely at a track (Get Info), lyrics, choosing
an album's cover, albums kept as one long file, and albums with many
artists.

## Get Info

Select a track and choose **Get Info** in its menu, or press ⌘I (**File ›
Get Info**). The **Get Info** window shows, without changing anything:

- the title, artist, album and your rating;
- **Cover**: the picture shown for the track;
- **Format**: the **Codec** (with "(lossless)" when it is), **Bits per
  sample**, **Sample rate**, **Channels** (**Mono**, **Stereo**…), **Bit
  rate**, **Length** and **Size**;
- **Part of the file**, for a track that is a part of a longer file (see
  below): where it starts and ends;
- **Added** (when it came into the library) and **Plays**;
- **File**: where the file is, with **Show in Finder**;
- **MusicBrainz**: links to the track's **Recording**, **Release**,
  **Release group**, **Artist**, **Album artist** and **Work** on
  MusicBrainz, when its tags carry them;
- every **Embedded picture** in the file;
- **Tags**: **Every tag** the file has, as written in it.

ano-mp never edits your tags. To fix a tag, use a tag editor (such as Mp3tag
or MusicBrainz Picard), then rescan the folder; see
[chapter 12](12-library-health.md#rescans-and-keeping-up-to-date).

## Lyrics

With **Lyrics** on (it is at first), the **Now Playing** view shows the
lyrics of the track playing, beside its cover. Open the view by clicking
the track on the player bar, or with **View › Now Playing** (⌘4).

ano-mp finds lyrics in two places:

- **in the track's tags** (an "unsynced lyrics" or "lyrics" tag), shown as
  plain text (**From the track’s tags**);
- **in a .lrc file next to the track**, named like it: "Song.lrc" or
  "Song.flac.lrc" beside "Song.flac" (**From a .lrc file next to the
  track**).

A .lrc file usually has **synced** lyrics: each line has a time. Synced
lines light up as the music reaches them, and clicking a line jumps there.

ano-mp doesn't fetch lyrics online.

## Covers

Each album shows one cover. ano-mp looks for pictures in this order, unless
you change it in **Settings › Online sources** (**Album art**):

1. **Embedded in files**: a picture inside the album's music files.
2. **Images in album folders**: a picture file in the album's folder,
   named "cover", "folder", "front", "album" or "albumart" (.jpg or .png).
3. **Cover Art Archive**: the cover of the album's release on MusicBrainz,
   downloaded (needs the album matched on MusicBrainz).

The first picture found is shown.

### Choose a cover

1. On the album's page, click the cover, or click **Choose Cover…**. (Or
   choose **Choose Cover…** in the album's menu.)
2. **Choose a cover for** the album shows the pictures each source has,
   with their sizes. The Cover Art Archive's are downloaded as you look.
3. Click a picture, then **Use this picture**.

Your choice is kept (**Your choice**), and automatic lookups never replace
it. To go back to the first picture found, click **Use automatic**.

If every album art source is turned off, the dialog says so.

## Albums as one file: cue sheets and chapters

Some albums are ripped as one long file with a **cue sheet** (a .cue file
beside it, or one inside the file) that says where each track starts.
Audiobooks and some mixes come as one file with **chapters**. With **Cue
sheets and chapters as tracks** on (it is at first), each cue or chapter is
a track of its own in the library: you can play, queue, rate, heart and add
it to playlists like any other track. Get Info shows which part of the file
it is.

The tracks of one file play gaplessly from one to the next. Turning the
setting off or on reads every file again.

## Compilations and many artists

- A track whose artist tag names several artists ("A; B", or several
  artist tags) is listed under each of them on the **Artists** page; the
  first is its main artist. The track keeps the credit as the tag wrote it.
- An album is filed under its **album artist** tag. Without one, it is filed
  under **Various Artists** when the files say it is a compilation, or when
  three or more artists share an album title in one folder; otherwise under
  the track's artist.
- An artist's page lists their own albums, and under **Appears on** the
  other albums they are on.
- With **Classical works** on, the **Composer** view groups by composer and
  work, album pages list **Works**, and a work plays and shuffles as one.
  This needs the files' composer and work tags (as MusicBrainz Picard
  writes them).

If an album is split in two, or two albums are merged, the files' album or
album artist tags disagree. The library health report lists such albums;
see [chapter 12](12-library-health.md#the-library-health-report).
