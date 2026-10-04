# Appendix D. Error messages

Every error message ano-mp can show, with what it means and what to do.
"…" stands for what the message fills in: a name, a folder, a number.
Messages are grouped by what they are about, and alphabetical within each
group.

A few problems that are rare, or come from macOS itself, are shown in their
own words and aren't listed here. If one puzzles you, report it with
**Copy diagnostics** ([chapter 14](14-troubleshooting.md#reporting-a-problem)).

## Folders and files

**Folder not available: …**
The folder can't be read just now. ano-mp keeps its tracks. See
[Folders that can't be found](12-library-health.md#folders-that-cant-be-found).

**… is empty: its drive or share may not be mounted. Its tracks are kept.**
The folder is there but has nothing in it, which usually means the drive
or network share it lives on isn't connected. Connect it; or, if its music
really is gone, use **Remove missing tracks…**.

**… was moved to the Trash**
Take the folder out of the Trash and use **Locate…**, or remove it from the
library.

**Can’t find …: its drive isn’t connected, or it was deleted**
Connect the drive; or use **Locate…** if you moved the folder; or remove it
from the library.

**Most of the tracks in … weren’t found, so they were kept. Remove them from the folder’s message if they’re gone.**
A rescan found most of the folder's files missing, so ano-mp kept their
tracks in case the files come back. If you deleted them on purpose, use
**Remove missing tracks…** in the message about the folder.

**ano-mp may no longer read …: locate it again**
macOS no longer lets ano-mp read the folder (for example after it was
renamed or moved, or ano-mp was reinstalled). Use **Locate…** and choose the
folder, which gives ano-mp access again.

**That library folder is no longer in the library**
The folder was removed while you were acting on it. Nothing to do.

**The file took more than … s to open**
The file is on a slow drive, a sleeping disk or a network share, or is
downloading from iCloud. ano-mp passed over it; try again once the drive is
awake.

**A library scan is already running**
Wait for the scan to finish (the sidebar shows its progress), then try
again.

**Wait for the library scan to finish**
This needs the library as it will be after the scan. Try again when the
scan has finished.

## Tracks, albums, artists and playlists

**The track is no longer in the library**
**The album is no longer in the library**
**The artist is no longer in the library**
It was removed (by a rescan, or with its folder) while you were looking at
it. Go back and choose again.

**The playlist no longer exists**
The playlist was deleted. Choose another.

**A playlist’s name must be 1 to … characters**
Give the playlist a shorter name, or one that isn't empty.

**… is too large for a playlist**
The playlist file you imported is too big to be a playlist. Check you chose
the right file.

**Not a smart playlist**
Rules can only be edited for a smart playlist.

**A smart playlist’s tracks follow its rules**
You can't add, move or remove a smart playlist's tracks by hand. Change its
rules with **Edit Rules…**, or make an ordinary playlist.

**A genre or artist condition needs a name of 1 to … characters**
Fill in each **Genre** and **Artist** condition of the smart playlist, or
remove it.

**A rating is 1 to 5 stars, not …**
Choose one to five stars, or **No Rating**.

**A sleep timer runs for 1 minute to a day**
Choose a time within that range.

## Recording

**Choose a folder for recordings in Settings › Recording.**
Recording needs a folder: click **Choose Folder…** in **Settings ›
Recording**.

**The recordings’ folder … can’t be opened (…). Choose it again in Settings › Recording.**
The folder was moved, deleted or is on a drive that isn't connected, or
macOS no longer lets ano-mp write there. Choose it (or another) again.

**Can’t start recording: …**
The recording couldn't begin, for the reason given. Check the folder's
disk has space and is writable, and try another format.

**The recording stopped: the disk is full. What was recorded until then is kept.**
Free some space, or choose a folder on another disk.

**The recording stopped: the file couldn’t be written (…). What was recorded until then is kept.**
The disk stopped accepting the file, for the reason given (the drive was
unplugged, for example).

**Already recording.**
A recording is already running; stop it first.

**Not recording.**
There's no recording to stop.

## Settings, features and files you open

**… is turned off in Settings › Features**
What you asked for belongs to a feature that is off. Turn it on in
**Settings › Features**.

**That isn’t an ano-mp theme file**
The file you chose to import as a theme wasn't exported from ano-mp's
**Settings › Appearance**.

**That isn’t an ano-mp data file**
The file you chose to import wasn't made by **Export Library Data…**.

**That data file is from a newer version of ano-mp**
Update ano-mp to the version that exported the file (or a later one), then
import it.

**The third-party notices couldn’t be read**
The app's copy of its licences is missing or damaged. Download ano-mp again
and replace it.

## Repairing the library

**Your playlists, favourites and history couldn’t be saved from the damaged library**
The library is too damaged to save your data from before rebuilding it.
Restore the earlier copy if there is one; otherwise **Rebuild anyway**
starts afresh (the damaged library is kept). See
[chapter 12](12-library-health.md#if-the-library-needs-repair).

**There is no copy of the library to restore**
ano-mp hasn't been upgraded since the library was made, so there is no
earlier copy. Choose **Rebuild** instead.
