# 4. Playlists and favourites

This chapter covers making playlists by hand, smart playlists that fill
themselves from rules, moving playlists in and out of ano-mp, and marking
what you love with hearts and stars.

## Playlists

A playlist is a list of tracks in the order you choose. The same track can
be in many playlists, or in one several times. Deleting a playlist never
deletes its tracks from the library.

### Make a playlist

- **File › New Playlist** (⌘N), or the **+** beside **Playlists** in the
  sidebar, then **New Playlist**. The new playlist opens with its name
  ready to type over.
- Or select tracks, open their menu and choose **Add to Playlist › New
  Playlist…**: the playlist starts with those tracks.
- Or, in the queue, click **Save the queue as a playlist**.

### Add tracks

- Select tracks anywhere, open their menu and choose **Add to Playlist**,
  then the playlist.
- Or drag tracks onto the playlist's name in the sidebar.
- Or, with the playlist open, drag tracks into it, where you want them.

Albums and artists have **Add to Playlist** in their menus too.

### Change a playlist

Open it from **Playlists** in the sidebar (click the heading to show the
list).

<!-- Screenshot: a playlist open, with its name and tracks. -->

- Click its name to **rename** it.
- **Play** and **Shuffle** play it.
- Drag a track by its handle to move it. Select several to move them
  together.
- Select tracks and press Delete, or choose **Remove from Playlist** in
  their menu, to take them out.

The **⋯** beside a playlist in the sidebar (or right-click) has **Play**,
**Shuffle**, **Play Next**, **Add to Queue**, **Rename**, **Export as
M3U8…** and **Delete Playlist**.

## Smart playlists

A smart playlist lists whatever in your library matches its rules, and
keeps itself up to date: a smart playlist of "rated 4 stars or more" gains
a track as soon as you rate one.

### Make a smart playlist

1. Choose **File › New Smart Playlist…** (⌥⌘N), or **+** › **New Smart
   Playlist…** in the sidebar.
2. Give it a **Name**.
3. Under **Match**, choose **all of these conditions** or **any of these
   conditions**.
4. Add conditions with **Add a condition**. Each is one of:

   | Condition | Matches tracks… |
   |---|---|
   | **Genre** | with this genre (a name you type) |
   | **Artist** | by this artist (a name you type) |
   | **Year** | released **from** one year **to** another |
   | **Format** | of this format: FLAC, MP3… |
   | **Added in the last** | added to the library in the last so many **days** |
   | **Favourite** | hearted (**yes**) or not (**no**) |
   | **Rating** | rated **at least** so many stars |
   | **Plays** | played **at least** and, if you like, **at most** so many times (**No upper limit** at first) |
   | **Not played in the last** | not played in the last so many **days** |

   Remove one with **Remove this condition**.
5. Choose an **Order**: **Shuffled**, **Most recently added**, **Most
   played**, **Most recently played**, **Highest rated**, **Album** or
   **Title**.
6. To keep it short, set **At most** to a number of tracks.
7. As you change the rules, the dialog says how many tracks match now.
   Click **Save**.

To change the rules later, choose **Edit Rules…** in the playlist's menu.

A smart playlist's tracks follow its rules, so you can't add, move or
remove them by hand. Rules about plays need **Listening history** on.

## Import and export playlists

ano-mp reads and writes **M3U** playlists (.m3u and .m3u8 files), which
most music players understand.

- **Import**: **File › Import Playlist…**, or **+** › **Import Playlist…**
  in the sidebar, then choose the file. You can also open an .m3u or .m3u8
  file from the Finder with ano-mp. The playlist is made from the tracks in
  your library that the file names; ano-mp says how many it found, and how
  many weren't in the library (**3 not in the library**).
- **Export**: open the playlist, then **File › Export Playlist…**, or
  choose **Export as M3U8…** in its menu, and choose where to save it.

## Favourites

Click the **heart** on a track, an album or an artist to make it a
favourite; click it again to take it out. You'll find hearts on every row
of a list, on album and artist pages, and on the player bar. Several
selected tracks can be hearted at once from their menu (**Add to
favourites**, **Remove from favourites**).

- **Favourites** in the sidebar (⌘3) lists everything hearted: **Favourite
  albums**, **Favourite tracks** and artists. **Play favourite tracks**
  plays the tracks.
- The **Favourites only** button in a library view keeps only what is
  hearted, or on a hearted album, or by a hearted artist.
- Smart playlists can use **Favourite** as a condition.
- Home shows favourites you haven't played for a year.

## Ratings

Give a track one to five stars:

- choose **Rating** in its menu, then the stars (or **No Rating**); this
  works for several selected tracks at once;
- or add the **Rating** column to track lists in **Settings › Display**
  and click the stars in it.

Ratings are kept in ano-mp, not written into your files. Smart playlists
can use them as a condition and an order.
