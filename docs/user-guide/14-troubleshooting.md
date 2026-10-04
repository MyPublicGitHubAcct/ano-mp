# 14. Troubleshooting and FAQ

When something goes wrong, find the symptom below and try its steps in
order. If none helps, report it (see the end of this chapter) with what it
asks for. [Appendix D](appendix-d-errors.md) explains each error message.

## No sound

1. Check the volume on the player bar isn't at zero or muted, and the
   Mac's own volume is up.
2. Look at the player bar: if the track's time isn't moving, it isn't
   playing. Press Space.
3. Open **Settings › Playback**. Under **Output**, the line under the
   device says what is playing through, or **No output device is open.**
   Choose another **Device**, or **Use the default device**.
4. If you chose a device that is now disconnected, ano-mp plays through
   the default device and says so.
5. If the sound crackles or drops out, choose a larger **Buffer size**.
6. Click the format on the player bar for the **Signal path**: check that
   an effect or the equaliser isn't making it silent (a preamp turned far
   down, or the spectral freeze holding silence).

Send: the **Copy diagnostics** text, and which device you expect to hear.

## A folder won't scan, or tracks are missing

1. Look at the folder in the sidebar or **Settings › Library**. If it says
   it can't be read, follow
   [Folders that can't be found](12-library-health.md#folders-that-cant-be-found):
   connect the drive, or **Locate…** it.
2. If it says **Not scanned yet**, or the scan never finishes, click
   **Rescan** beside it. A first scan of a large folder can take several
   minutes; the sidebar shows its progress.
3. Only files ano-mp can play are listed ([Appendix
   B](appendix-b-formats.md)). Other files in the folder (pictures, PDFs,
   unsupported formats) are passed over.
4. Files in iCloud that aren't downloaded show with their file name as
   their title until downloaded
   ([Files in iCloud](12-library-health.md#files-in-icloud)).
5. If a folder adds many files that fail to read, **Settings › Library**
   says so (**12 more files could not be read**).

Send: the diagnostics, and the folder's state as Settings shows it.

## A file won't play

1. ano-mp passes over a track it can't open and plays the next, with a
   message saying why (**Skipped “…”**: and the reason).
2. If the message says the folder isn't available, see above.
3. If it says **The file took more than 20 s to open**, the file is on a
   slow or sleeping drive or share; try again once it is awake.
4. With **Library health report** and the **Loudness analysis** on,
   look in **Library health** under **Don’t decode** and **Cut short**: the
   file may be damaged. Try playing it in another app.
5. Check its format in **Get Info**: some formats aren't supported
   ([Appendix B](appendix-b-formats.md)).

Send: the format line from **Get Info** (Codec, Sample rate, Bits per
sample), the message shown, and, if you can share it, the file.

## Wrong tags, titles or albums

ano-mp shows what your files' tags say. It doesn't edit them.

1. Open **Get Info** and look at **Every tag**: that's what ano-mp read.
2. Fix the tags with a tag editor, such as MusicBrainz Picard or Mp3tag.
3. ano-mp picks up the change by itself (or click **Rescan** on the
   folder).
4. An album split in two, or albums merged, usually has tracks whose
   **Album** or **Album artist** tags differ: **Library health** lists them
   under **Albums whose tracks disagree**.

## Wrong cover, details or biography

1. For a cover, click it on the album's page and choose another
   ([Covers](05-track-and-album-details.md#covers)).
2. For an album's details, or a cover from the Cover Art Archive, the
   album may be matched to the wrong release: use **Find Details…**
   ([Fixing a wrong match](06-online-information.md#fixing-a-wrong-match)).
3. For a biography about the wrong person, use **Wrong artist?** on the
   artist's page.
4. Nothing online at all? Check **Use online services** and each source in
   **Settings › Online sources**, and whether the sidebar's **Online
   sources** says a service **can’t be reached**.

## ano-mp uses a lot of processor time

1. Right after adding a large folder, the scan and the online lookups run
   for a while. They finish by themselves.
2. The **Loudness analysis** uses a lot of processor time, once, for hours
   on a large library. Settings shows its progress. Turn it off to pause it.
3. The visualizer draws 60 times a second. Set **Frame rate** to 30 in
   **Settings › Visualizer**, or close the visualizer.
4. Effects, practice mode and crossfeed use some processor time while
   they're on.

Send: the diagnostics, and what was happening (scanning, playing, the
visualizer open).

## ano-mp doesn't open after an update

1. If macOS says it can't check the app, or that it is damaged, download
   it again from the release page (**Settings › About › Download from
   GitHub** links there) and drag it to Applications, replacing the old
   one.
2. If it opens with **The library needs repair**, see
   [chapter 12](12-library-health.md#if-the-library-needs-repair).
3. If you went back to an older version after using a newer one, it may
   refuse a library the newer one upgraded. Use the newer version.

Send: the macOS version, the ano-mp version you installed and the one
before, and the log folder's latest file.

## Frequently asked questions

**Does ano-mp change my music files?**
No. It only reads them. Ratings, hearts, playlists, covers you choose and
playback preferences are kept in ano-mp's own library, not in your files.

**Can it use my Music app (iTunes) library?**
Add the folder your music files are in (often **Music › Music › Media**).
Playlists can be exported from the Music app as M3U and imported into
ano-mp. Ratings and play counts from the Music app aren't read.

**Why are some tracks greyed out?**
Their folder can't be read right now: see
[chapter 12](12-library-health.md#folders-that-cant-be-found).

**Why is there a gap (or no gap) between tracks?**
Albums play gapless. Between other tracks, a crossfade plays if you've set
one; otherwise the next track starts straight after. A track with silence
at its end has that silence; **Skip long silences** can skip it.

**Why did shuffle play two tracks in a row from one album?**
They run into each other (**Keep segues together in shuffle**), or they're
movements of one work (**Classical works**), or the album is set to
**Never shuffle it**.

**Where does ano-mp keep its data?**
In its own folder on your Mac, inside its app container in your Library
folder. **Export Library Data…** makes a copy you can keep or move
([chapter 12](12-library-health.md#moving-your-library-to-another-mac)).

**How do I start over?**
Remove every folder in **Settings › Library**, then add your folders
again. Removing a folder removes its tracks, with their plays, ratings and
playlist entries, so export your data first if you want to keep them, and
import it once the folders are back.

## Reporting a problem

1. Open **Settings › About**.
2. Click **Copy diagnostics**, and paste the text into your report. It
   holds no file paths, titles or artists
   ([chapter 13](13-privacy.md#logs-and-diagnostics)).
3. Say what you did, what you expected and what happened instead, and
   whether it happens every time.
4. If asked for the log, **Show logs** opens its folder; attach the latest
   file. It doesn't contain your keys, and at its normal level not your
   titles or file names either.
5. If the log doesn't show enough, you may be asked to turn on
   **Detailed logging** in **Settings › About**, make the problem happen
   again, and attach `ano-mp-detailed.log` from the same folder. That
   file names your files and music
   ([chapter 13](13-privacy.md#detailed-logging)): read it first, and turn
   the switch off afterwards, which deletes it.
