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

**… can’t be reached. Tracks already downloaded still play.**
The music server doesn't answer: it may be off, or you may be away from
its network. Its tracks are kept; see
[When the server can't be reached](15-music-servers.md#when-the-server-cant-be-reached).

**… no longer accepts this sign-in: sign in again in Settings › Servers**
The server refused ano-mp's sign-in, for example after your password
changed. Sign in again in **Settings › Servers**.

**This kind of server is turned off in Settings › Features**
The track is on a music server, and **Jellyfin servers** or **Plex
servers** (whichever it is) is off. Turn it on to play it.

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

**This playlist comes from …: change it there**
A music server's playlist changes only on the server. Change it in
Jellyfin; the next sync brings the change. Or copy its tracks into a
playlist of your own.

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

**Play the file in the effects workbench before recording it.**
**Record One Take** records the workbench's file while it's the one
playing. Click the play button on the page, wait for the file to start,
then record.

**Nothing is playing: play a track to use it in the effects workbench.**
**Use the Playing Track** takes the track that's playing. Play one, then
click it again, or choose a file instead.

## Music servers

These are about Jellyfin and Plex servers
([chapter 15](15-music-servers.md)).

**… answered with an error (HTTP …)**
The server had a problem with the request. Try again later; if it keeps
happening, check the server's own log.

**… can’t be reached**
The server doesn't answer: check it's on, and that you're on its network
or its address works from where you are.

**…’s https certificate isn’t one the system trusts. Self-signed certificates aren’t supported: use a certificate from a certificate authority, or the server’s http address on your network.**
The server's https certificate isn't one macOS trusts, often because it
is self-signed. Give the server a certificate from a certificate
authority (such as Let's Encrypt), or use its http:// address while on
your network.

**… no longer accepts this sign-in: sign in again in Settings › Servers**
The server refused ano-mp's sign-in, for example after your password
changed or the session was removed in Jellyfin. Sign in again.

**A server outside your network needs an https:// address, so your password and music don’t cross the internet unprotected**
Use the server's https:// address, or its http:// address while you're on
its network.

**Couldn’t look for servers on the network**
ano-mp couldn't ask the local network for servers, perhaps because macOS
doesn't allow it to use the local network (System Settings › Privacy &
Security › Local Network). Type the server's address instead.

**Nothing at that address answers as a Jellyfin server**
Check the address, including its port (Jellyfin's is usually 8096 for
http), and that the server is on.

**Quick Connect is turned off on this server: sign in with your password, or ask its administrator to turn it on**
Sign in with your user name and password, or turn on Quick Connect in
Jellyfin's dashboard.

**That isn’t a server address, such as http://192.168.1.10:8096**
Type the address as the example shows: http:// or https://, the server's
name or IP address, and its port if it has one.

**The file is on the server: play it to see more about it**
Get Info reads a server's track from its downloaded copy. Play the track,
then open Get Info again.

**The server didn’t accept that user name and password**
Check them, and try again. They're the ones you use in Jellyfin's own
apps.

**Nothing at that address answers as this Plex server**
Another device, or another Plex server, answers where this server used
to. ano-mp tries the server's other addresses; if none answers, check the
server is on, then sync again.

**The Plex sign-in took too long. Start again.**
The code from **Sign In with Plex** wasn't approved in time. Click **Sign
In with Plex** again and approve it in the browser.

**plex.tv no longer accepts this sign-in: sign in with Plex again in Settings › Servers**
Your Plex account's sign-in was ended, for example after you changed your
Plex password or removed ano-mp from your account's devices. Click **Sign
In with Plex** again.

**Your Plex account no longer reaches that server**
The server was removed from your account, or its owner stopped sharing it
with you, while you were adding its libraries.

**… can’t be reached directly. To play through Plex’s relay, turn on “Use Plex’s relay” in Settings › Servers.**
None of the server's own addresses answers, and Plex's relay is off. Turn
on **Use Plex's relay** to play through it (slower, and at a lower bit
rate), or go back to the server's network.

**… won’t stream this away from its network. Plex may need a Plex Pass for that; it still plays on the server’s own network.**
The Plex server refused to send the track while you're away from its
network, which Plex may keep for Plex Pass subscribers. Play it on the
server's network, keep it available offline before you leave, or see
Plex's own pages about Plex Pass.

**The server is already being synced**
Wait for the sync to finish (the sidebar shows its progress).

**This server runs Jellyfin …; ano-mp needs … or later**
Update the server to the version ano-mp names, then sign in again.

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
